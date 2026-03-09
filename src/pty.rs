use nix::libc;
use nix::pty::openpty;
use nix::sys::signal::{self, SaFlags, SigAction, SigHandler, SigSet, Signal};
use nix::sys::termios::{self, LocalFlags, SetArg, Termios};
use nix::unistd::{dup2, execvp, fork, setsid, ForkResult, Pid};
use std::ffi::CString;
use std::os::fd::{AsRawFd, IntoRawFd, OwnedFd, RawFd};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

/// Global storage for terminal restore on signal/crash.
static ORIG_TERMIOS: std::sync::Mutex<Option<Termios>> = std::sync::Mutex::new(None);
static RAW_MODE_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Self-pipe for signal delivery to the event loop.
static SIGNAL_PIPE_W: AtomicI32 = AtomicI32::new(-1);

/// Result of spawning the child in a PTY.
pub struct PtyChild {
    pub master_fd: OwnedFd,
    pub child_pid: Pid,
}

/// Spawn a child process in a new PTY session.
pub fn spawn_pty(cmd: &[String]) -> nix::Result<PtyChild> {
    let pty = openpty(None, None)?;

    match unsafe { fork()? } {
        ForkResult::Child => {
            // Close master in child
            drop(pty.master);

            // New session
            setsid()?;

            // Set slave as controlling terminal
            unsafe {
                libc::ioctl(pty.slave.as_raw_fd(), libc::TIOCSCTTY as _, 0);
            }

            // Redirect stdio to slave
            dup2(pty.slave.as_raw_fd(), libc::STDIN_FILENO)?;
            dup2(pty.slave.as_raw_fd(), libc::STDOUT_FILENO)?;
            dup2(pty.slave.as_raw_fd(), libc::STDERR_FILENO)?;

            // Close original slave fd if it's not stdin/stdout/stderr
            if pty.slave.as_raw_fd() > 2 {
                drop(pty.slave);
            }

            // Exec the command
            let c_cmd: Vec<CString> = cmd
                .iter()
                .map(|s| CString::new(s.as_str()).expect("invalid command string"))
                .collect();
            execvp(&c_cmd[0], &c_cmd)?;
            unreachable!();
        }
        ForkResult::Parent { child } => {
            // Close slave in parent
            drop(pty.slave);

            // Propagate current terminal size to PTY
            propagate_winsize(libc::STDIN_FILENO, pty.master.as_raw_fd());

            Ok(PtyChild {
                master_fd: pty.master,
                child_pid: child,
            })
        }
    }
}

/// Enter raw mode on stdin. Saves original termios for later restore.
pub fn enter_raw_mode() -> nix::Result<()> {
    let orig = termios::tcgetattr(std::io::stdin())?;
    {
        let mut guard = ORIG_TERMIOS.lock().unwrap();
        *guard = Some(orig.clone());
    }

    let mut raw = orig;
    termios::cfmakeraw(&mut raw);
    // Keep ISIG so we receive signals, but disable ECHO and canonical mode
    raw.local_flags.insert(LocalFlags::ISIG);
    termios::tcsetattr(std::io::stdin(), SetArg::TCSANOW, &raw)?;
    RAW_MODE_ACTIVE.store(true, Ordering::SeqCst);
    Ok(())
}

/// Restore original terminal mode.
pub fn restore_terminal() {
    if !RAW_MODE_ACTIVE.load(Ordering::SeqCst) {
        return;
    }
    let guard = ORIG_TERMIOS.lock().unwrap();
    if let Some(ref orig) = *guard {
        let _ = termios::tcsetattr(std::io::stdin(), SetArg::TCSANOW, orig);
    }
    RAW_MODE_ACTIVE.store(false, Ordering::SeqCst);
}

/// Propagate terminal window size from `from_fd` to `to_fd`.
pub fn propagate_winsize(from_fd: RawFd, to_fd: RawFd) {
    unsafe {
        let mut ws: libc::winsize = std::mem::zeroed();
        if libc::ioctl(from_fd, libc::TIOCGWINSZ, &mut ws) == 0 {
            libc::ioctl(to_fd, libc::TIOCSWINSZ, &ws);
        }
    }
}

/// Read terminal size from `fd`.
pub fn terminal_size(fd: RawFd) -> Option<(u16, u16)> {
    unsafe {
        let mut ws: libc::winsize = std::mem::zeroed();
        if libc::ioctl(fd, libc::TIOCGWINSZ, &mut ws) == 0 && ws.ws_row > 0 && ws.ws_col > 0 {
            Some((ws.ws_row, ws.ws_col))
        } else {
            None
        }
    }
}

/// Create the self-pipe used for signal delivery.
/// Returns (read_fd, write_fd) as raw fds. Caller owns both.
pub fn create_signal_pipe() -> nix::Result<(RawFd, RawFd)> {
    let (r_owned, w_owned) = nix::unistd::pipe()?;
    // Convert to raw fds — we manage lifetime manually
    let r = r_owned.into_raw_fd();
    let w = w_owned.into_raw_fd();

    // Set both ends non-blocking
    set_nonblocking(r)?;
    set_nonblocking(w)?;

    SIGNAL_PIPE_W.store(w, Ordering::SeqCst);
    Ok((r, w))
}

fn set_nonblocking(fd: RawFd) -> nix::Result<()> {
    let flags = nix::fcntl::fcntl(fd, nix::fcntl::FcntlArg::F_GETFL)?;
    let mut oflags = nix::fcntl::OFlag::from_bits_truncate(flags);
    oflags.insert(nix::fcntl::OFlag::O_NONBLOCK);
    nix::fcntl::fcntl(fd, nix::fcntl::FcntlArg::F_SETFL(oflags))?;
    Ok(())
}

/// Signal handler that writes signal number to the self-pipe.
extern "C" fn signal_handler(sig: libc::c_int) {
    let fd = SIGNAL_PIPE_W.load(Ordering::SeqCst);
    if fd >= 0 {
        let buf = [sig as u8];
        unsafe {
            libc::write(fd, buf.as_ptr() as *const libc::c_void, 1);
        }
    }
}

/// Install signal handlers for SIGWINCH, SIGINT, SIGTERM, SIGCHLD.
pub fn install_signal_handlers() -> nix::Result<()> {
    let sa = SigAction::new(
        SigHandler::Handler(signal_handler),
        SaFlags::SA_RESTART,
        SigSet::empty(),
    );
    unsafe {
        signal::sigaction(Signal::SIGWINCH, &sa)?;
        signal::sigaction(Signal::SIGCHLD, &sa)?;
        // For SIGINT/SIGTERM we still want to forward them, but also handle cleanup
        signal::sigaction(Signal::SIGINT, &sa)?;
        signal::sigaction(Signal::SIGTERM, &sa)?;
    }
    Ok(())
}

/// Close a raw fd (for the signal pipe ends on cleanup).
pub fn close_fd(fd: RawFd) {
    unsafe {
        libc::close(fd);
    }
}

/// Forward a signal to the child's process group.
pub fn forward_signal_to_child(child_pid: Pid, sig: Signal) {
    // Kill the entire process group (negative pid)
    let _ = signal::kill(Pid::from_raw(-child_pid.as_raw()), sig);
}
