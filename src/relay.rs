use nix::libc;
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use nix::sys::signal::Signal;
use nix::sys::wait::{waitpid, WaitPidFlag};
use nix::unistd::Pid;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, BorrowedFd, RawFd};
use std::time::Instant;

use crate::pty;
use crate::stall::{StallEvent, StreamProcessor};
use crate::ui::TerminalUi;

const BUF_SIZE: usize = 8192;
const POLL_TIMEOUT_MS: u16 = 500;

/// Read from a raw fd using libc. Returns number of bytes read, or -1 on error.
fn raw_read(fd: RawFd, buf: &mut [u8]) -> isize {
    unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) }
}

/// Write all bytes to a raw fd using libc.
fn raw_write_all(fd: RawFd, mut data: &[u8]) {
    while !data.is_empty() {
        let n = unsafe { libc::write(fd, data.as_ptr() as *const libc::c_void, data.len()) };
        if n <= 0 {
            break;
        }
        data = &data[n as usize..];
    }
}

/// Run the main event loop: relay bytes between stdin/PTY master,
/// monitor child output for stall detection.
pub fn run_relay(
    master_fd: RawFd,
    child_pid: Pid,
    signal_pipe_r: RawFd,
    processor: &mut StreamProcessor,
    ui: &mut TerminalUi,
) -> i32 {
    let stdin_fd = io::stdin().as_raw_fd();
    let mut buf = [0u8; BUF_SIZE];
    let mut child_exited = false;
    let mut exit_code: i32 = 0;

    loop {
        let master_bfd = unsafe { BorrowedFd::borrow_raw(master_fd) };
        let stdin_bfd = unsafe { BorrowedFd::borrow_raw(stdin_fd) };
        let sigpipe_bfd = unsafe { BorrowedFd::borrow_raw(signal_pipe_r) };

        let mut fds = [
            PollFd::new(stdin_bfd, PollFlags::POLLIN),
            PollFd::new(master_bfd, PollFlags::POLLIN),
            PollFd::new(sigpipe_bfd, PollFlags::POLLIN),
        ];

        match poll(&mut fds, PollTimeout::from(POLL_TIMEOUT_MS)) {
            Ok(_) => {}
            Err(nix::errno::Errno::EINTR) => {
                processor.tick(Instant::now());
                continue;
            }
            Err(e) => {
                eprintln!("nudge-me: poll error: {}", e);
                break;
            }
        }

        // Handle signals from the self-pipe
        if let Some(revents) = fds[2].revents() {
            if revents.contains(PollFlags::POLLIN) {
                let mut sig_buf = [0u8; 32];
                let n = raw_read(signal_pipe_r, &mut sig_buf);
                for i in 0..n.max(0) as usize {
                    match sig_buf[i] as i32 {
                        libc::SIGWINCH => {
                            pty::propagate_winsize(stdin_fd, master_fd);
                            if let Some((rows, cols)) = pty::terminal_size(stdin_fd) {
                                write_stdout(&ui.on_resize(rows, cols));
                            }
                        }
                        libc::SIGCHLD => match waitpid(child_pid, Some(WaitPidFlag::WNOHANG)) {
                            Ok(nix::sys::wait::WaitStatus::Exited(_, code)) => {
                                exit_code = code;
                                child_exited = true;
                            }
                            Ok(nix::sys::wait::WaitStatus::Signaled(_, sig, _)) => {
                                exit_code = 128 + sig as i32;
                                child_exited = true;
                            }
                            _ => {}
                        },
                        libc::SIGINT => {
                            pty::forward_signal_to_child(child_pid, Signal::SIGINT);
                        }
                        libc::SIGTERM => {
                            pty::forward_signal_to_child(child_pid, Signal::SIGTERM);
                        }
                        _ => {}
                    }
                }
            }
        }

        // Read from stdin → write to PTY master
        if let Some(revents) = fds[0].revents() {
            if revents.contains(PollFlags::POLLIN) {
                let n = raw_read(stdin_fd, &mut buf);
                if n > 0 {
                    write_stdout(&ui.on_user_input(Instant::now()));
                    raw_write_all(master_fd, &buf[..n as usize]);
                }
                // n == 0: stdin EOF, n < 0: error — both ignored (keep running)
            }
        }

        // Read from PTY master → write to stdout + feed processor
        if let Some(revents) = fds[1].revents() {
            if revents.contains(PollFlags::POLLIN) {
                let n = raw_read(master_fd, &mut buf);
                if n > 0 {
                    let data = &buf[..n as usize];
                    if let Some(event) = processor.feed(data) {
                        handle_stall_event(ui, event, Instant::now());
                    }
                    write_stdout(&ui.on_child_output(data));
                } else if n == 0 {
                    child_exited = true;
                } else {
                    // EIO on PTY master typically means child exited
                    let errno = io::Error::last_os_error().raw_os_error().unwrap_or(0);
                    if errno == libc::EIO {
                        child_exited = true;
                    }
                }
            }
            if revents.contains(PollFlags::POLLHUP) {
                child_exited = true;
            }
        }

        // Tick stall detector
        let now = Instant::now();
        if let Some(event) = processor.tick(now) {
            handle_stall_event(ui, event, now);
        } else {
            write_stdout(&ui.on_tick(now));
        }

        if child_exited {
            drain_master(master_fd, processor, ui);
            break;
        }
    }

    let _ = waitpid(child_pid, None);
    exit_code
}

/// Drain remaining bytes from PTY master after child exits.
fn drain_master(master_fd: RawFd, processor: &mut StreamProcessor, ui: &mut TerminalUi) {
    let mut buf = [0u8; BUF_SIZE];
    loop {
        let n = raw_read(master_fd, &mut buf);
        if n <= 0 {
            break;
        }
        let data = &buf[..n as usize];
        if let Some(event) = processor.feed(data) {
            handle_stall_event(ui, event, Instant::now());
        }
        write_stdout(&ui.on_child_output(data));
    }
}

fn handle_stall_event(ui: &mut TerminalUi, event: StallEvent, now: Instant) {
    write_stdout(&ui.on_stall_event(event, now));
}

fn write_stdout(data: &[u8]) {
    if data.is_empty() {
        return;
    }
    let _ = io::stdout().write_all(data);
    let _ = io::stdout().flush();
}
