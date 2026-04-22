//! Terminal notification system
//!
//! Sends terminal-level notifications using OSC sequences.
//! Supports: iTerm2 (OSC 9), Kitty/Ghostty (OSC 99), generic bell (BEL).

/// Send a terminal notification with the given title and body
pub fn send_notification(title: &str, body: &str) {
    // Try OSC 99 (kitty/ghostty) first
    print!("\x1b]99;i=1:d=0:p=body;{}\x1b\\", escape_osc(body));
    // Try OSC 9 (iTerm2) as fallback
    print!("\x1b]9;{}\x1b\\", escape_osc(&format!("{}: {}", title, body)));
    // Always send BEL as universal fallback
    print!("\x07");
    let _ = std::io::Write::flush(&mut std::io::stdout());
}

/// Send a simple bell (BEL) notification
pub fn send_bell() {
    print!("\x07");
    let _ = std::io::Write::flush(&mut std::io::stdout());
}

/// Send taskbar progress indicator (OSC 9;4)
/// `percent`: 0-100, or -1 to clear
pub fn send_progress(percent: i32) {
    if percent < 0 {
        print!("\x1b]9;4;0;0\x1b\\"); // clear
    } else {
        let p = percent.clamp(0, 100);
        print!("\x1b]9;4;1;{}\x1b\\", p);
    }
    let _ = std::io::Write::flush(&mut std::io::stdout());
}

fn escape_osc(s: &str) -> String {
    s.replace('\x07', "")
        .replace('\x1b', "")
        .replace('\n', " ")
        .replace('\r', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_osc() {
        assert_eq!(escape_osc("hello\nworld"), "hello world");
        assert_eq!(escape_osc("bell\x07"), "bell");
        assert_eq!(escape_osc("esc\x1b"), "esc");
    }

    #[test]
    fn test_send_bell_does_not_panic() {
        send_bell(); // just verify it doesn't panic
    }

    #[test]
    fn test_send_notification_does_not_panic() {
        send_notification("Test", "Hello world");
    }

    #[test]
    fn test_send_progress_does_not_panic() {
        send_progress(50);
        send_progress(-1);
        send_progress(200);
    }
}
