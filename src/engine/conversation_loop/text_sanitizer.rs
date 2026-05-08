const THINK_OPEN_TAG: &str = "<think>";
const THINK_CLOSE_TAG: &str = "</think>";

#[derive(Default)]
pub(super) struct VisibleTextSanitizer {
    buffer: String,
    in_think_block: bool,
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
            if self.in_think_block {
                if let Some(end_idx) = self.buffer.find(THINK_CLOSE_TAG) {
                    let drain_len = end_idx + THINK_CLOSE_TAG.len();
                    self.buffer.drain(..drain_len);
                    self.in_think_block = false;
                    continue;
                }

                if flush_all {
                    self.buffer.clear();
                } else {
                    let keep = THINK_CLOSE_TAG.len().saturating_sub(1);
                    if self.buffer.len() > keep {
                        let drain_len = floor_char_boundary(&self.buffer, self.buffer.len() - keep);
                        self.buffer.drain(..drain_len);
                    }
                }
                break;
            }

            if let Some(start_idx) = self.buffer.find(THINK_OPEN_TAG) {
                out.push_str(&self.buffer[..start_idx]);
                let drain_len = start_idx + THINK_OPEN_TAG.len();
                self.buffer.drain(..drain_len);
                self.in_think_block = true;
                continue;
            }

            if flush_all {
                out.push_str(&self.buffer);
                self.buffer.clear();
            } else {
                let keep = THINK_OPEN_TAG.len().saturating_sub(1);
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

pub(super) fn strip_think_blocks(text: &str) -> String {
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
