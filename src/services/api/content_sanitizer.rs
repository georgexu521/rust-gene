//! API content sanitization helpers.
//!
//! Normalizes runtime text before it crosses service boundaries or is shown in API responses.

use regex::Regex;

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
    let content = strip_dsml_blocks(content.as_ref());
    let mut output = String::with_capacity(content.len());
    let mut rest = content.as_str();

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

/// Strip DSML tool-call markup leaked into visible assistant content.
///
/// DeepSeek-family models sometimes emit tool calls as text using delimiters
/// such as `<|DSML|function_calls>` or the spaced variant
/// `<| | DSML | | function_calls>`. These blocks are not user-facing prose.
fn strip_dsml_blocks(content: &str) -> String {
    let open_re = Regex::new(
        r"〈DSML｜(?:function_calls|tool_calls)[^〉]*〉|<\|(?:\s*\|)?\s*[Dd][Ss][Mm][Ll]\s*\|(?:\s*\|)?\s*(?:function_calls|tool_calls)[^>]*>",
    )
    .expect("valid DSML open regex");
    let close_re = Regex::new(
        r"〈/DSML｜(?:function_calls|tool_calls)[^〉]*〉|</\|(?:\s*\|)?\s*[Dd][Ss][Mm][Ll]\s*\|(?:\s*\|)?\s*(?:function_calls|tool_calls)[^>]*>|<\|(?:\s*\|)?\s*/\s*[Dd][Ss][Mm][Ll]\s*\|(?:\s*\|)?\s*(?:function_calls|tool_calls)[^>]*>",
    )
    .expect("valid DSML close regex");
    let mut result = content.to_string();
    while let Some(open_match) = open_re.find(&result) {
        let after_open = &result[open_match.end()..];
        let Some(close_match) = close_re.find(after_open) else {
            break;
        };
        let close_end = open_match.end() + close_match.end();
        result.replace_range(open_match.start()..close_end, "");
    }
    result
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

    #[test]
    fn strips_spaced_half_width_dsml_blocks() {
        let input = r#"我来检查。
<| | DSML | | tool_calls>
<| | DSML | | invoke name="bash">
<| | DSML | | parameter name="command" string="true">ls</| | DSML | | parameter>
</| | DSML | | invoke>
</| | DSML | | tool_calls>
Done."#;
        assert!(strip_hidden_blocks(input).trim().ends_with("Done."));
    }

    #[test]
    fn strips_compact_half_width_dsml_blocks() {
        let input = r#"Before <|DSML|tool_calls><|DSML|invoke name="bash"><|DSML|parameter name="command">ls</|DSML|parameter></|DSML|invoke></|DSML|tool_calls> After"#;
        assert_eq!(strip_hidden_blocks(input), "Before  After");
    }

    #[test]
    fn strips_full_width_dsml_blocks() {
        let input = "Before\n〈DSML｜tool_calls〉\n〈DSML｜invoke name=\"bash\"〉\n〈DSML｜parameter name=\"command\" string=\"true\"〉pwd〈/DSML｜parameter〉\n〈/DSML｜invoke〉\n〈/DSML｜tool_calls〉\nAfter";
        assert_eq!(strip_hidden_blocks(input), "Before\n\nAfter");
    }
}
