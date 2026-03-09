use chrono::Local;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::time::{Duration, Instant};

use crate::ansi::AnsiStripper;
use crate::classifier::is_meaningful;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StallEvent {
    Started,
    Resumed,
}

pub struct StallDetector {
    threshold: Duration,
    last_meaningful: Instant,
    stalled: bool,
    log_file: File,
}

impl StallDetector {
    pub fn new(threshold: Duration, log_path: &str) -> std::io::Result<Self> {
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;
        Ok(Self {
            threshold,
            last_meaningful: Instant::now(),
            stalled: false,
            log_file,
        })
    }

    pub fn on_meaningful(&mut self) -> Option<StallEvent> {
        self.last_meaningful = Instant::now();
        if self.stalled {
            self.stalled = false;
            self.write_log("move");
            return Some(StallEvent::Resumed);
        }
        None
    }

    /// Check elapsed time; call periodically from the event loop.
    pub fn tick(&mut self, now: Instant) -> Option<StallEvent> {
        if !self.stalled && now.duration_since(self.last_meaningful) > self.threshold {
            self.stalled = true;
            self.write_log("stop");
            return Some(StallEvent::Started);
        }
        None
    }

    fn write_log(&mut self, event: &str) {
        let ts = Local::now().format("%H:%M:%S");
        let _ = writeln!(self.log_file, "{} {}", ts, event);
        let _ = self.log_file.flush();
    }
}

/// Combines ANSI stripping, classification, and stall detection.
pub struct StreamProcessor {
    stripper: AnsiStripper,
    line_buf: String,
    pub stall: StallDetector,
}

impl StreamProcessor {
    pub fn new(threshold: Duration, log_path: &str) -> std::io::Result<Self> {
        Ok(Self {
            stripper: AnsiStripper::new(),
            line_buf: String::with_capacity(1024),
            stall: StallDetector::new(threshold, log_path)?,
        })
    }

    /// Feed raw bytes from the child process. Strips ANSI, classifies,
    /// and updates stall state.
    pub fn feed(&mut self, raw: &[u8]) -> Option<StallEvent> {
        let visible = self.stripper.strip(raw);
        let mut transition = None;
        for ch in visible.chars() {
            match ch {
                '\n' => {
                    if is_meaningful(&self.line_buf) {
                        transition = transition.or_else(|| self.stall.on_meaningful());
                    }
                    self.line_buf.clear();
                }
                '\r' => {
                    if is_meaningful(&self.line_buf) {
                        transition = transition.or_else(|| self.stall.on_meaningful());
                    }
                    self.line_buf.clear();
                }
                _ => {
                    self.line_buf.push(ch);
                    // Periodic check for long lines without newlines
                    if self.line_buf.len() % 64 == 0 && is_meaningful(&self.line_buf) {
                        transition = transition.or_else(|| self.stall.on_meaningful());
                    }
                }
            }
        }
        transition
    }

    /// Tick the stall detector (call from event loop on every poll return).
    pub fn tick(&mut self, now: Instant) -> Option<StallEvent> {
        self.stall.tick(now)
    }
}
