//! Output hygiene for the plain CLI: text from the network (ICY titles,
//! directory names, server messages) may contain terminal control
//! sequences; printing them raw would let a stream retitle the window or
//! write the clipboard (OSC 52). The TUI is not affected (ratatui drops
//! control characters).

/// `s` with every control character (C0, DEL, C1 – ESC, BEL, CSI …)
/// replaced by `?`; tabs become a space.
pub fn clean(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\t' => ' ',
            c if c.is_control() => '?',
            c => c,
        })
        .collect()
}

/// Print one line of external text, cleaned.
pub fn say(line: &str) {
    println!("{}", clean(line));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_sequences_are_neutralised() {
        assert_eq!(
            clean("Evil \u{1b}]52;c;SGk=\u{7} - Song"),
            "Evil ?]52;c;SGk=? - Song"
        );
        assert_eq!(clean("a\u{9b}2Jb\u{7f}"), "a?2Jb?", "C1 CSI and DEL");
        assert_eq!(clean("tab\there"), "tab here");
        assert_eq!(
            clean("Björk – Jóga ♪"),
            "Björk – Jóga ♪",
            "printable text kept"
        );
    }
}
