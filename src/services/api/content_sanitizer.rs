#[derive(Debug, Clone, Copy)]
pub struct HiddenBlockSpec {
    pub open_prefix: &'static str,
    pub close_tag: &'static str,
}

const HIDDEN_BLOCKS: &[HiddenBlockSpec] = &[
    HiddenBlockSpec {
        open_prefix: "<thinking",
        close_tag: "</thinking>",
    },
    HiddenBlockSpec {
        open_prefix: "<think",
        close_tag: "</think>",
    },
    HiddenBlockSpec {
        open_prefix: "<invoke",
        close_tag: "</invoke>",
    },
    HiddenBlockSpec {
        open_prefix: "<tool_call",
        close_tag: "</tool_call>",
    },
    HiddenBlockSpec {
        open_prefix: "<minimax:tool_call",
        close_tag: "</minimax:tool_call>",
    },
];

pub(crate) fn hidden_blocks() -> &'static [HiddenBlockSpec] {
    HIDDEN_BLOCKS
}

pub(crate) fn max_hidden_open_prefix_len() -> usize {
    hidden_blocks()
        .iter()
        .map(|block| block.open_prefix.len())
        .max()
        .unwrap_or(0)
}

pub(crate) fn find_hidden_block_open(buffer: &str) -> Option<(usize, usize, HiddenBlockSpec)> {
    let lower = buffer.to_ascii_lowercase();
    hidden_blocks()
        .iter()
        .filter_map(|block| {
            let start = lower.find(block.open_prefix)?;
            let open_end_rel = lower[start..].find('>')?;
            Some((start, start + open_end_rel + 1, *block))
        })
        .min_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| right.2.open_prefix.len().cmp(&left.2.open_prefix.len()))
        })
}

pub(crate) fn strip_hidden_blocks(content: impl AsRef<str>) -> String {
    let mut output = String::with_capacity(content.as_ref().len());
    let mut rest = content.as_ref();

    loop {
        let Some((open_start, open_end, block)) = find_hidden_block_open(rest) else {
            output.push_str(rest);
            break;
        };
        output.push_str(&rest[..open_start]);

        let lower = rest.to_ascii_lowercase();
        let Some(close_start_rel) = lower[open_end..].find(block.close_tag) else {
            break;
        };
        let close_end = open_end + close_start_rel + block.close_tag.len();
        rest = &rest[close_end..];
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_hidden_and_tool_blocks() {
        let input = "Before <tool_call>{}</tool_call> middle <invoke name=\"x\">{}</invoke> after";
        assert_eq!(strip_hidden_blocks(input), "Before  middle  after");
    }

    #[test]
    fn prefers_longer_prefix_for_overlapping_think_tags() {
        let input = "A <thinking>hidden</thinking> B <think>also hidden</think> C";
        assert_eq!(strip_hidden_blocks(input), "A  B  C");
    }
}
