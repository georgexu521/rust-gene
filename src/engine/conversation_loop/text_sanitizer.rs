//! Conversation-loop controller module.
//!
//! Owns one focused stage of turn execution so permissions, validation, repair, and closeout stay explicit in the runtime.

use crate::services::api::content_sanitizer::{find_hidden_block_open, max_hidden_open_prefix_len};

#[derive(Default)]
pub(super) struct VisibleTextSanitizer {
    buffer: String,
    hidden_close_tag: Option<&'static str>,
}

impl VisibleTextSanitizer {
    pub(super) fn push_chunk(&mut self, chunk: &str) -> String {
        self.buffer.push_str(chunk);
        self.drain_visible(false)
    }

    pub(super) fn finish(&mut self) -> String {
        self.drain_visible(true)
    }

    fn drain_visible(&mut self, flush_all: bool) -> String {
        let mut out = String::new();
        loop {
            if let Some(close_tag) = self.hidden_close_tag {
                let lower = self.buffer.to_ascii_lowercase();
                if let Some(end_idx) = lower.find(close_tag) {
                    let drain_len = end_idx + close_tag.len();
                    self.buffer.drain(..drain_len);
                    self.hidden_close_tag = None;
                    continue;
                }

                if flush_all {
                    self.buffer.clear();
                } else {
                    let keep = close_tag.len().saturating_sub(1);
                    if self.buffer.len() > keep {
                        let drain_len = floor_char_boundary(&self.buffer, self.buffer.len() - keep);
                        self.buffer.drain(..drain_len);
                    }
                }
                break;
            }

            if let Some((start_idx, open_end, block)) = find_hidden_block_open(&self.buffer) {
                out.push_str(&self.buffer[..start_idx]);
                self.buffer.drain(..open_end);
                self.hidden_close_tag = Some(block.close_tag);
                continue;
            }

            if flush_all {
                out.push_str(&self.buffer);
                self.buffer.clear();
            } else {
                let keep = max_hidden_open_prefix_len().saturating_sub(1);
                if self.buffer.len() > keep {
                    let emit_len = floor_char_boundary(&self.buffer, self.buffer.len() - keep);
                    out.push_str(&self.buffer[..emit_len]);
                    self.buffer.drain(..emit_len);
                }
            }
            break;
        }

        out
    }
}

pub(super) fn strip_hidden_blocks(text: &str) -> String {
    let mut sanitizer = VisibleTextSanitizer::default();
    let mut visible = sanitizer.push_chunk(text);
    visible.push_str(&sanitizer.finish());
    visible
}

fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_streamed_hidden_and_tool_blocks() {
        let mut sanitizer = VisibleTextSanitizer::default();
        let mut visible = sanitizer.push_chunk("Before <tool_");
        visible.push_str(&sanitizer.push_chunk("call>{}</tool_call> After"));
        visible.push_str(&sanitizer.push_chunk(" <invoke name=\"x\">{}</invoke> Done"));
        visible.push_str(&sanitizer.finish());

        assert_eq!(visible, "Before  After  Done");
    }

    #[test]
    fn strips_thinking_without_losing_following_text() {
        let mut sanitizer = VisibleTextSanitizer::default();
        let mut visible = sanitizer.push_chunk("A <thinking>hidden");
        visible.push_str(&sanitizer.push_chunk("</thinking> B"));
        visible.push_str(&sanitizer.finish());

        assert_eq!(visible, "A  B");
    }
}
