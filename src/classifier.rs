/// Classifies visible text (after ANSI stripping) as meaningful or noise.

/// Characters considered "noise" — spinners, dots, bars, slashes.
const NOISE_CHARS: &[char] = &[
    '.', '…', '·', '•', '●', '○', '-', '—', '─',
    '|', '/', '\\', '⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏',
    '▏', '▎', '▍', '▌', '▋', '▊', '▉', '█',
    ' ', '⣾', '⣽', '⣻', '⢿', '⡿', '⣟', '⣯', '⣷',
];

/// Returns true if the text contains meaningful output (not just spinner noise).
pub fn is_meaningful(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    if is_noise(trimmed) {
        return false;
    }
    if has_alnum_word_ge2(trimmed) {
        return true;
    }
    if trimmed.len() >= 8 && has_any_alnum(trimmed) {
        return true;
    }
    false
}

/// Returns true if the string is entirely composed of noise characters.
fn is_noise(s: &str) -> bool {
    s.chars().all(|c| NOISE_CHARS.contains(&c))
}

/// Returns true if the string contains any alphanumeric "word" of length >= 2.
fn has_alnum_word_ge2(s: &str) -> bool {
    s.split(|c: char| !c.is_alphanumeric())
        .any(|word| word.len() >= 2)
}

/// Returns true if the string contains at least one ASCII alphanumeric char.
fn has_any_alnum(s: &str) -> bool {
    s.chars().any(|c| c.is_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_dots() {
        assert!(!is_meaningful("..."));
        assert!(!is_meaningful(".."));
        assert!(!is_meaningful("."));
        assert!(!is_meaningful("… … …"));
    }

    #[test]
    fn noise_spinners() {
        assert!(!is_meaningful("●●●"));
        assert!(!is_meaningful("⠋"));
        assert!(!is_meaningful("⠙⠹⠸"));
        assert!(!is_meaningful("---"));
        assert!(!is_meaningful("|"));
        assert!(!is_meaningful("/"));
        assert!(!is_meaningful("\\"));
    }

    #[test]
    fn noise_mixed_spinner_chars() {
        assert!(!is_meaningful("... --- ..."));
        assert!(!is_meaningful("●·●·●"));
    }

    #[test]
    fn meaningful_words() {
        assert!(is_meaningful("Hello"));
        assert!(is_meaningful("Building project"));
        assert!(is_meaningful("error: foo"));
        assert!(is_meaningful("ab"));
    }

    #[test]
    fn single_char_not_meaningful() {
        assert!(!is_meaningful("a"));
        assert!(!is_meaningful("x"));
    }

    #[test]
    fn long_punctuation_not_meaningful() {
        assert!(!is_meaningful("--------"));
        assert!(!is_meaningful("............"));
    }

    #[test]
    fn mixed_noise_and_text() {
        assert!(is_meaningful("... loading"));
        assert!(is_meaningful("●● done"));
    }

    #[test]
    fn empty_and_whitespace() {
        assert!(!is_meaningful(""));
        assert!(!is_meaningful("   "));
        assert!(!is_meaningful("\t\t"));
    }

    #[test]
    fn long_with_one_alnum() {
        // length >= 8 with at least one alnum
        assert!(is_meaningful("-------a"));
        // length < 8 with one alnum
        assert!(!is_meaningful("---a"));
    }

    #[test]
    fn unicode_alnum() {
        assert!(is_meaningful("日本語テスト"));
    }
}
