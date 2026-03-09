/// ANSI escape sequence stripper.
///
/// Processes a byte stream and emits only visible text characters,
/// stripping CSI, OSC, and other escape sequences plus control chars.
/// The state machine is stateful across calls so partial sequences
/// split across chunk boundaries are handled correctly.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Normal,
    EscSeen,
    CsiParam,
    OscString,
    OscStEnd,
}

pub struct AnsiStripper {
    state: State,
    /// Accumulates visible bytes (valid UTF-8 not guaranteed per chunk).
    out: Vec<u8>,
}

impl AnsiStripper {
    pub fn new() -> Self {
        Self {
            state: State::Normal,
            out: Vec::with_capacity(4096),
        }
    }

    /// Feed raw bytes from the child process, return visible text.
    /// The returned slice borrows from an internal buffer that is
    /// only valid until the next call to `strip`.
    pub fn strip(&mut self, data: &[u8]) -> String {
        self.out.clear();
        for &b in data {
            self.feed_byte(b);
        }
        String::from_utf8_lossy(&self.out).into_owned()
    }

    fn feed_byte(&mut self, b: u8) {
        match self.state {
            State::Normal => {
                if b == 0x1b {
                    self.state = State::EscSeen;
                } else if b == b'\n' || b == b'\r' || b == b'\t' {
                    self.out.push(b);
                } else if b < 0x20 {
                    // Control chars: discard (BEL, BS, etc.)
                } else {
                    // Printable byte
                    self.out.push(b);
                }
            }
            State::EscSeen => {
                match b {
                    b'[' => self.state = State::CsiParam,
                    b']' => self.state = State::OscString,
                    // Two-char sequences (e.g., ESC ( B, ESC =, ESC >)
                    _ => self.state = State::Normal,
                }
            }
            State::CsiParam => {
                // CSI parameters: 0x30-0x3F, intermediates: 0x20-0x2F
                // Final byte: 0x40-0x7E
                if (0x40..=0x7E).contains(&b) {
                    self.state = State::Normal;
                }
                // else: stay in CsiParam (parameter/intermediate bytes)
            }
            State::OscString => {
                if b == 0x07 {
                    // BEL terminates OSC
                    self.state = State::Normal;
                } else if b == 0x1b {
                    self.state = State::OscStEnd;
                }
                // else: consume OSC content
            }
            State::OscStEnd => {
                if b == b'\\' {
                    // ST (String Terminator) = ESC backslash
                    self.state = State::Normal;
                } else {
                    // Not a valid ST; treat ESC as start of new sequence
                    self.state = State::EscSeen;
                    self.feed_byte(b);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_passthrough() {
        let mut s = AnsiStripper::new();
        assert_eq!(s.strip(b"hello world"), "hello world");
    }

    #[test]
    fn strip_csi_color() {
        let mut s = AnsiStripper::new();
        assert_eq!(s.strip(b"\x1b[31mred\x1b[0m"), "red");
    }

    #[test]
    fn strip_csi_clear_screen() {
        let mut s = AnsiStripper::new();
        assert_eq!(s.strip(b"\x1b[2J\x1b[H"), "");
    }

    #[test]
    fn strip_osc_title_bel() {
        let mut s = AnsiStripper::new();
        assert_eq!(s.strip(b"\x1b]0;my title\x07rest"), "rest");
    }

    #[test]
    fn strip_osc_title_st() {
        let mut s = AnsiStripper::new();
        assert_eq!(s.strip(b"\x1b]0;my title\x1b\\rest"), "rest");
    }

    #[test]
    fn mixed_ansi_and_text() {
        let mut s = AnsiStripper::new();
        assert_eq!(
            s.strip(b"\x1b[1mBold\x1b[0m text"),
            "Bold text"
        );
    }

    #[test]
    fn control_chars_removed() {
        let mut s = AnsiStripper::new();
        // BEL(0x07), BS(0x08) should be stripped; \n \r \t preserved
        assert_eq!(s.strip(b"a\x07b\x08c\nd\re\tf"), "abc\nd\re\tf");
    }

    #[test]
    fn partial_sequence_across_calls() {
        let mut s = AnsiStripper::new();
        // Split ESC [ 3 1 m across two calls
        assert_eq!(s.strip(b"before\x1b"), "before");
        assert_eq!(s.strip(b"[31mafter"), "after");
    }

    #[test]
    fn newlines_and_cr_preserved() {
        let mut s = AnsiStripper::new();
        assert_eq!(s.strip(b"line1\nline2\rline3"), "line1\nline2\rline3");
    }

    #[test]
    fn empty_input() {
        let mut s = AnsiStripper::new();
        assert_eq!(s.strip(b""), "");
    }

    #[test]
    fn only_escape_sequences() {
        let mut s = AnsiStripper::new();
        assert_eq!(s.strip(b"\x1b[1m\x1b[31m\x1b[0m"), "");
    }

    #[test]
    fn csi_with_multiple_params() {
        let mut s = AnsiStripper::new();
        // CSI 38;5;196m (256-color)
        assert_eq!(s.strip(b"\x1b[38;5;196mcolored\x1b[0m"), "colored");
    }
}
