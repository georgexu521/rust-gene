const MAX_MATCH_CONTEXT_OCCURRENCES: usize = 12;

/// 查找所有精确匹配位置
pub(super) fn find_occurrences(content: &str, target: &str) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    let mut start = 0;
    while let Some(pos) = content[start..].find(target) {
        let match_start = start + pos;
        let match_end = match_start + target.len();
        result.push((match_start, match_end));
        start = match_end;
    }
    result
}

/// 查找所有模糊匹配位置（去除首尾空白后匹配）
pub(super) fn fuzzy_find_occurrences(content: &str, target: &str) -> Vec<(usize, usize)> {
    let trimmed_target = target.trim();
    if trimmed_target.is_empty() {
        return Vec::new();
    }
    let mut result = Vec::new();
    for (line_idx, line) in content.lines().enumerate() {
        let trimmed_line = line.trim();
        if trimmed_line == trimmed_target {
            // 计算在原始内容中的实际起始位置
            let mut pos = 0;
            for _ in 0..line_idx {
                pos = content[pos..]
                    .find('\n')
                    .map(|p| pos + p + 1)
                    .unwrap_or(pos);
            }
            let line_start = pos;
            let line_end = line_start + line.len();
            result.push((line_start, line_end));
        }
    }
    result
}

/// 归一化空白后查找所有精确匹配位置（限制在同一行内扩展）
pub(super) fn find_occurrences_normalized(content: &str, target: &str) -> Vec<(usize, usize)> {
    let trimmed_target = target.trim();
    if trimmed_target.is_empty() {
        return find_occurrences(content, target);
    }
    let mut result = Vec::new();
    let mut start = 0;
    while start < content.len() {
        if let Some(pos) = content[start..].find(trimmed_target) {
            let match_start = start + pos;
            let match_end = match_start + trimmed_target.len();

            // 向前扩展：限制在当前行内（不跨越 \n）
            let line_start = content[..match_start]
                .rfind('\n')
                .map(|i| i + 1)
                .unwrap_or(0);
            let actual_start = content[line_start..match_start]
                .find(|c: char| !c.is_whitespace())
                .map(|i| line_start + i)
                .unwrap_or(match_start);

            // 向后扩展：限制在当前行内
            let line_end = content[match_end..]
                .find('\n')
                .map(|i| match_end + i)
                .unwrap_or(content.len());
            let actual_end = content[match_end..line_end]
                .rfind(|c: char| !c.is_whitespace())
                .map(|i| match_end + i + 1)
                .unwrap_or(line_end);

            result.push((actual_start, actual_end));
            start = line_end.max(match_end);
        } else {
            break;
        }
    }
    result
}

/// 构建匹配位置的上下文提示
pub(super) fn build_match_context(
    content: &str,
    occurrences: &[(usize, usize)],
    context_lines: usize,
) -> String {
    let lines: Vec<&str> = content.lines().collect();

    let mut parts = vec![format!("Found {} occurrence(s):", occurrences.len())];
    for (occ_idx, (start, _end)) in occurrences
        .iter()
        .take(MAX_MATCH_CONTEXT_OCCURRENCES)
        .enumerate()
    {
        let start_line = content[..*start].matches('\n').count();
        let ctx_start = start_line.saturating_sub(context_lines);
        let ctx_end = (start_line + 1 + context_lines).min(lines.len());
        parts.push(format!(
            "\n  Match #{} at line {}:",
            occ_idx + 1,
            start_line + 1
        ));
        for (li, line) in lines
            .iter()
            .enumerate()
            .skip(ctx_start)
            .take(ctx_end - ctx_start)
        {
            parts.push(format!("    {:4} | {}", li + 1, line));
        }
    }
    if occurrences.len() > MAX_MATCH_CONTEXT_OCCURRENCES {
        parts.push(format!(
            "\n  ... showing first {} of {} matches. The old_string is too broad; use a unique old_string copied from the target lines or a precise line_start/line_end replacement.",
            MAX_MATCH_CONTEXT_OCCURRENCES,
            occurrences.len()
        ));
    }
    parts.join("\n")
}

pub(super) fn contains_file_read_line_prefix(text: &str) -> bool {
    text.lines().any(|line| {
        let trimmed = line.trim_start();
        let Some((digits, rest)) = trimmed.split_once('|') else {
            return false;
        };
        !digits.trim().is_empty()
            && digits.trim().chars().all(|ch| ch.is_ascii_digit())
            && rest.starts_with(' ')
    })
}

pub(super) fn file_read_line_prefix_guidance(field: &str) -> String {
    format!(
        "{field} appears to include file_read display line prefixes like `12 |`. \
         Those prefixes are not part of the file content. Retry with text copied after the pipe, \
         or use line_start/line_end when the line numbers are the evidence you trust."
    )
}

pub(super) fn occurrence_line_numbers(content: &str, occurrences: &[(usize, usize)]) -> Vec<usize> {
    occurrences
        .iter()
        .take(MAX_MATCH_CONTEXT_OCCURRENCES)
        .map(|(start, _)| content[..*start].matches('\n').count() + 1)
        .collect()
}
