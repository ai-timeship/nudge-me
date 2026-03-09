use clap::Parser;
use std::os::fd::AsRawFd;
use std::process;
use std::time::Duration;

mod ansi;
mod classifier;
mod overlay;
mod pty;
mod relay;
mod stall;
mod ui;

#[derive(Parser)]
#[command(
    name = "nudge-me",
    about = "PTY wrapper with stall detection — logs stop/move events when child output stalls"
)]
struct Cli {
    /// Seconds of no meaningful output before logging "stop"
    #[arg(long, short = 't', default_value_t = 30)]
    threshold: u64,

    /// Path to the notification event log
    #[arg(long, default_value = "nudge.log")]
    notify_log: String,

    /// Idle overlay style (`card` or `zzz`)
    #[arg(long, default_value = "card", value_enum)]
    idle_overlay: overlay::OverlayKind,

    /// Command to run (everything after --)
    #[arg(last = true, required = true)]
    command: Vec<String>,
}

fn main() {
    let cli = Cli::parse();

    if cli.command.is_empty() {
        eprintln!("nudge-me: no command specified");
        process::exit(1);
    }

    let threshold = Duration::from_secs(cli.threshold);
    let (rows, cols) = pty::terminal_size(std::io::stdin().as_raw_fd()).unwrap_or((24, 80));

    // Create stream processor (ANSI stripper + classifier + stall detector)
    let mut processor = match stall::StreamProcessor::new(threshold, &cli.notify_log) {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "nudge-me: failed to open notification log '{}': {}",
                cli.notify_log, e
            );
            process::exit(1);
        }
    };
    let mut ui = ui::TerminalUi::new(rows, cols, cli.idle_overlay, threshold);

    // Set up self-pipe for signals
    let (sig_pipe_r, sig_pipe_w) = match pty::create_signal_pipe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("nudge-me: failed to create signal pipe: {}", e);
            process::exit(1);
        }
    };

    // Install signal handlers
    if let Err(e) = pty::install_signal_handlers() {
        eprintln!("nudge-me: failed to install signal handlers: {}", e);
        process::exit(1);
    }

    // Spawn child in PTY
    let pty_child = match pty::spawn_pty(&cli.command) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("nudge-me: failed to spawn '{}': {}", cli.command[0], e);
            process::exit(1);
        }
    };

    // Enter raw mode on parent terminal
    if let Err(e) = pty::enter_raw_mode() {
        eprintln!("nudge-me: failed to enter raw mode: {}", e);
        process::exit(1);
    }

    // Run the event loop
    let master_raw_fd = pty_child.master_fd.as_raw_fd();
    let exit_code = relay::run_relay(
        master_raw_fd,
        pty_child.child_pid,
        sig_pipe_r,
        &mut processor,
        &mut ui,
    );

    // Cleanup
    pty::restore_terminal();
    pty::close_fd(sig_pipe_r);
    pty::close_fd(sig_pipe_w);

    process::exit(exit_code);
}
