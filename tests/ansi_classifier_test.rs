use nudge_me::ansi::AnsiStripper;
use nudge_me::classifier::is_meaningful;

#[test]
fn strip_then_classify_meaningful() {
    let mut s = AnsiStripper::new();
    let visible = s.strip(b"\x1b[1mBuilding\x1b[0m project...");
    assert!(is_meaningful(&visible));
}

#[test]
fn strip_then_classify_noise() {
    let mut s = AnsiStripper::new();
    let visible = s.strip(b"\x1b[33m...\x1b[0m");
    assert!(!is_meaningful(&visible));
}

#[test]
fn strip_then_classify_spinner_line() {
    let mut s = AnsiStripper::new();
    // Typical spinner: cursor move + single char
    let visible = s.strip(b"\x1b[2K\r\xe2\xa0\x8b"); // ⠋
    assert!(!is_meaningful(&visible));
}

#[test]
fn strip_complex_output_meaningful() {
    let mut s = AnsiStripper::new();
    let visible = s.strip(
        b"\x1b[2J\x1b[H\x1b[1;34mCompiling\x1b[0m nudge-me v0.1.0\n",
    );
    // After stripping: "Compiling nudge-me v0.1.0\n"
    // Contains meaningful words
    let line = visible.trim();
    assert!(is_meaningful(line));
}

#[test]
fn strip_progress_bar_noise() {
    let mut s = AnsiStripper::new();
    let visible = s.strip(b"\x1b[2K\r[========>           ] ");
    // After stripping: "\r[========>           ] "
    // The brackets and equals/spaces — let's check
    let line = visible.replace('\r', "").trim().to_string();
    // "[========>           ]" — no alnum word >= 2
    assert!(!is_meaningful(&line));
}

#[test]
fn sequential_chunks_maintain_state() {
    let mut s = AnsiStripper::new();

    // Chunk 1: start of CSI sequence
    let v1 = s.strip(b"hello\x1b[");
    assert_eq!(v1, "hello");

    // Chunk 2: rest of CSI sequence + text
    let v2 = s.strip(b"31mworld");
    assert_eq!(v2, "world");

    // Combined result is meaningful
    let combined = format!("{}{}", v1, v2);
    assert!(is_meaningful(&combined));
}
