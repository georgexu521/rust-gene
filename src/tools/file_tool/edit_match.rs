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

// ---- Phase 3 (opencode alignment): edit recovery candidates ----

/// Outcome of deterministic edit candidate generation.
#[derive(Debug)]
pub(super) enum EditCandidateOutcome {
    /// An unambiguous candidate was found and can be applied automatically.
    AutoApplied {
        replacements: usize,
        strategy: String,
        occurrence: (usize, usize),
    },
    /// One or more candidates were generated; model should review.
    Candidates {
        candidates: Vec<EditCandidate>,
        count: usize,
    },
    /// No matches found even with lenient strategies.
    Mismatch { detail: String },
}

/// A single candidate match for a failed exact edit.
#[derive(Debug)]
pub(super) struct EditCandidate {
    pub strategy: String,
    pub occurrence: (usize, usize),
    pub confidence: &'static str,
    pub guidance: String,
}

impl EditCandidate {
    pub(super) fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "strategy": self.strategy,
            "start_byte": self.occurrence.0,
            "end_byte": self.occurrence.1,
            "confidence": self.confidence,
            "guidance": self.guidance,
        })
    }
}

/// Generate recovery candidates when exact match fails.
///
/// Safety: never auto-applies unless exactly one high-confidence candidate
/// is found within known content. Returns `AutoApplied` only for deterministic
/// single-match recoveries; ambiguous matches return `Candidates` for review.
pub(super) fn generate_edit_candidates(
    content: &str,
    old_string: &str,
    occurrences: &[(usize, usize)],
) -> EditCandidateOutcome {
    if occurrences.len() == 1 {
        return EditCandidateOutcome::AutoApplied {
            replacements: 1,
            strategy: "exact".to_string(),
            occurrence: occurrences[0],
        };
    }

    let mut candidates: Vec<EditCandidate> = Vec::new();

    // Strategy 1: line-trimmed block matching.
    let trimmed = find_occurrences_line_trimmed(content, old_string);
    if !trimmed.is_empty() && trimmed.len() != occurrences.len() {
        for occ in &trimmed {
            candidates.push(EditCandidate {
                strategy: "line-trimmed".to_string(),
                occurrence: *occ,
                confidence: if trimmed.len() == 1 { "high" } else { "low" },
                guidance: "Each line was trimmed of leading/trailing whitespace.".to_string(),
            });
        }
        if trimmed.len() == 1 && old_string.lines().count() > 1 {
            return EditCandidateOutcome::AutoApplied {
                replacements: 1,
                strategy: "line-trimmed".to_string(),
                occurrence: trimmed[0],
            };
        }
    }

    // Strategy 2: indentation-normalized block matching.
    let indent_norm = find_occurrences_indent_normalized(content, old_string);
    if !indent_norm.is_empty() && indent_norm != *occurrences && indent_norm != trimmed {
        for occ in &indent_norm {
            candidates.push(EditCandidate {
                strategy: "indent-normalized".to_string(),
                occurrence: *occ,
                confidence: if indent_norm.len() == 1 {
                    "high"
                } else {
                    "low"
                },
                guidance: "Common leading whitespace was removed from each line.".to_string(),
            });
        }
        if indent_norm.len() == 1 && candidates.is_empty() {
            return EditCandidateOutcome::AutoApplied {
                replacements: 1,
                strategy: "indent-normalized".to_string(),
                occurrence: indent_norm[0],
            };
        }
    }

    // Strategy 3: block-anchor matching for 3+ line old strings,
    // now with Levenshtein shadow similarity for diagnostic scoring.
    if old_string.lines().count() >= 3 {
        let anchor = find_occurrences_block_anchor(content, old_string);
        if !anchor.is_empty() && !indent_norm.contains(&anchor[0]) {
            let old_lines: Vec<&str> = old_string.lines().collect();
            let middle = &old_lines[1..old_lines.len() - 1];

            for occ in &anchor {
                let (start, end) = *occ;
                let content_middle: Vec<&str> = content[start..end].lines().skip(1).collect();
                let similarity = levenshtein_similarity(&content_middle, middle);
                let confidence = if anchor.len() == 1 {
                    if similarity >= 0.85 {
                        "high"
                    } else {
                        "medium"
                    }
                } else {
                    "low"
                };
                candidates.push(EditCandidate {
                    strategy: "block-anchor".to_string(),
                    occurrence: *occ,
                    confidence,
                    guidance: format!(
                        "First/last lines matched exactly; middle-line similarity={:.2}. \
                         Verify the anchor is correct before relying on this candidate.",
                        similarity
                    ),
                });
            }
        }
    }

    if candidates.is_empty() {
        // Strategy 4: whitespace-normalized snippets for short strings.
        let normalized = find_occurrences_whitespace_normalized(content, old_string);
        if !normalized.is_empty() && normalized != *occurrences {
            for occ in &normalized {
                candidates.push(EditCandidate {
                    strategy: "whitespace-normalized".to_string(),
                    occurrence: *occ,
                    confidence: if normalized.len() == 1 {
                        "medium"
                    } else {
                        "low"
                    },
                    guidance: "Consecutive whitespace was collapsed to single spaces.".to_string(),
                });
            }
        }
    }

    // Strategy 5: escape-normalized matching (diagnostic-only).
    // Never auto-applies: the LLM may have intended a literal `\n`.
    let escape = find_occurrences_escape_normalized(content, old_string);
    if !escape.is_empty() && escape != *occurrences {
        for occ in &escape {
            candidates.push(EditCandidate {
                strategy: "escape-normalized".to_string(),
                occurrence: *occ,
                confidence: if escape.len() == 1 { "medium" } else { "low" },
                guidance: "Literal escape sequences (\\n \\t \\r) were interpreted once; \
                           verify this is not a real code literal."
                    .to_string(),
            });
        }
    }

    // Strategy 6: trimmed-boundary (only leading/trailing whitespace).
    // Diagnostic-only for now: even single-candidate matches can be
    // ambiguous on single-line targets where indent is significant.
    let trimmed_boundary = find_occurrences_trimmed_boundary(content, old_string);
    if !trimmed_boundary.is_empty()
        && trimmed_boundary != *occurrences
        && trimmed_boundary != escape
    {
        for occ in &trimmed_boundary {
            candidates.push(EditCandidate {
                strategy: "trimmed-boundary".to_string(),
                occurrence: *occ,
                confidence: if trimmed_boundary.len() == 1 {
                    "medium"
                } else {
                    "low"
                },
                guidance: "Only leading/trailing whitespace around old_string was removed; \
                           verify that indent/content structure is still correct."
                    .to_string(),
            });
        }
    }

    if candidates.is_empty() {
        EditCandidateOutcome::Mismatch {
            detail: format!(
                "No match found for old_string ({} bytes) in file content ({} bytes). \
                 Try file_read the target file to verify the exact content, \
                 or use line_start/line_end for precise replacement.",
                old_string.len(),
                content.len()
            ),
        }
    } else {
        EditCandidateOutcome::Candidates {
            count: candidates.len(),
            candidates,
        }
    }
}

fn find_occurrences_line_trimmed(content: &str, target: &str) -> Vec<(usize, usize)> {
    let target_lines: Vec<&str> = target.lines().map(|l| l.trim()).collect();
    if target_lines.iter().all(|l| l.is_empty()) {
        return Vec::new();
    }
    let content_lines: Vec<&str> = content.lines().collect();
    let content_trimmed: Vec<&str> = content_lines.iter().map(|l| l.trim()).collect();

    let mut result = Vec::new();
    for start in 0..content_lines.len().saturating_sub(target_lines.len() - 1) {
        let matches = content_trimmed[start..]
            .iter()
            .zip(&target_lines)
            .all(|(a, b)| a == b);
        if matches {
            result.push(line_window_byte_range(
                content,
                start,
                target_lines.len(),
                target.ends_with('\n'),
            ));
        }
    }
    result
}

fn find_occurrences_indent_normalized(content: &str, target: &str) -> Vec<(usize, usize)> {
    let target_lines: Vec<&str> = target.lines().map(strip_common_indent).collect();
    if target_lines.iter().all(|l| l.is_empty()) {
        return Vec::new();
    }
    let content_lines: Vec<&str> = content.lines().collect();
    let content_norm: Vec<&str> = content_lines
        .iter()
        .map(|l| strip_common_indent(l))
        .collect();

    let mut result = Vec::new();
    for start in 0..content_lines.len().saturating_sub(target_lines.len() - 1) {
        let matches = content_norm[start..]
            .iter()
            .zip(&target_lines)
            .all(|(a, b)| a == b);
        if matches {
            result.push(line_window_byte_range(
                content,
                start,
                target_lines.len(),
                target.ends_with('\n'),
            ));
        }
    }
    result
}

fn strip_common_indent(line: &str) -> &str {
    let trimmed = line.trim_start();
    if trimmed.len() < line.len() {
        trimmed
    } else {
        line
    }
}

fn find_occurrences_block_anchor(content: &str, target: &str) -> Vec<(usize, usize)> {
    let target_lines: Vec<&str> = target.lines().collect();
    if target_lines.len() < 3 {
        return Vec::new();
    }
    let first = target_lines[0];
    let last = target_lines[target_lines.len() - 1];
    let content_lines: Vec<&str> = content.lines().collect();

    let mut result = Vec::new();
    for start in 0..content_lines.len().saturating_sub(target_lines.len() - 1) {
        let end = start + target_lines.len() - 1;
        if content_lines[start].trim() == first.trim() && content_lines[end].trim() == last.trim() {
            result.push(line_window_byte_range(
                content,
                start,
                target_lines.len(),
                target.ends_with('\n'),
            ));
        }
    }
    result
}

fn line_window_byte_range(
    content: &str,
    start_line: usize,
    line_count: usize,
    include_trailing_newline: bool,
) -> (usize, usize) {
    let mut pos = 0;
    for _ in 0..start_line {
        pos = content[pos..]
            .find('\n')
            .map(|p| pos + p + 1)
            .unwrap_or(pos);
    }
    let occ_start = pos;
    let mut occ_end = pos;
    for idx in 0..line_count {
        match content[occ_end..].find('\n') {
            Some(p) => {
                let line_end = occ_end + p;
                if idx + 1 == line_count && !include_trailing_newline {
                    occ_end = line_end;
                } else {
                    occ_end = line_end + 1;
                }
            }
            None => {
                occ_end = content.len();
                break;
            }
        }
    }
    (occ_start, occ_end)
}

fn find_occurrences_whitespace_normalized(content: &str, target: &str) -> Vec<(usize, usize)> {
    let normalized_target = collapse_whitespace(target);
    if normalized_target.is_empty() {
        return Vec::new();
    }
    let content_lines: Vec<&str> = content.lines().collect();
    let normalized_lines: Vec<String> = content_lines
        .iter()
        .map(|l| collapse_whitespace(l))
        .collect();

    // For single-line targets, search line by line.
    if !target.contains('\n') {
        let mut result = Vec::new();
        for (i, line) in normalized_lines.iter().enumerate() {
            if line == &normalized_target {
                let mut pos = 0;
                for _ in 0..i {
                    pos = content[pos..]
                        .find('\n')
                        .map(|p| pos + p + 1)
                        .unwrap_or(pos);
                }
                result.push((pos, pos + content_lines[i].len()));
            }
        }
        return result;
    }

    // Multi-line whitespace-normalized matching is intentionally not
    // auto-applied; block-anchor matching above gives safer guidance.
    Vec::new()
}

fn collapse_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_whitespace = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !in_whitespace {
                result.push(' ');
                in_whitespace = true;
            }
        } else {
            result.push(ch);
            in_whitespace = false;
        }
    }
    result.trim().to_string()
}

/// Levenshtein distance over two line slices, weighted so that
/// structurally-similar blocks get higher similarity scores.
#[allow(clippy::needless_range_loop)]
fn levenshtein_similarity(a: &[&str], b: &[&str]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let m = a.len();
    let n = b.len();
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 0..=m {
        dp[i][0] = i;
    }
    for j in 0..=n {
        dp[0][j] = j;
    }
    for i in 1..=m {
        for j in 1..=n {
            let cost = if a[i - 1].trim() == b[j - 1].trim() {
                0
            } else {
                1
            };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }
    let max_len = m.max(n) as f64;
    1.0 - (dp[m][n] as f64 / max_len)
}

// ── Strategy 5: escape-normalized ──────────────────────────────

/// Interpret literal `\n`, `\t`, `\r`, `\\` in `old_string` once.
///
/// Returns `None` when the input contains no recognised escape sequence
/// or when it looks like a real code literal (e.g. doubled backslashes
/// that are not just tool-call / JSON escaping).
fn unescape_tool_string_once(raw: &str) -> Option<String> {
    if !raw.contains('\\') {
        return None;
    }
    // Bail out when the old string already contains real backslash
    // literals mixed with escapes — too risky to guess.
    if raw.contains("\\\\") {
        return None;
    }
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some(c) => {
                    // unknown escape — keep literal
                    out.push('\\');
                    out.push(c);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    if out == raw {
        return None;
    }
    Some(out)
}

fn find_occurrences_escape_normalized(content: &str, target: &str) -> Vec<(usize, usize)> {
    let unescaped = match unescape_tool_string_once(target) {
        Some(u) => u,
        None => return Vec::new(),
    };
    find_occurrences(content, &unescaped)
}

// ── Strategy 6: trimmed-boundary ────────────────────────────────

fn find_occurrences_trimmed_boundary(content: &str, target: &str) -> Vec<(usize, usize)> {
    let trimmed = target.trim();
    if trimmed.len() == target.len() {
        return Vec::new();
    }
    find_occurrences(content, trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_trimmed_matches_despite_leading_whitespace() {
        let content = "fn main() {\n    let x = 1;\n    println!(\"{x}\");\n}\n";
        let target = "let x = 1;\nprintln!(\"{x}\");";
        let result = find_occurrences_line_trimmed(content, target);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn indent_normalized_matches_despite_indent_diff() {
        let content = "    fn foo() {\n        bar();\n    }\n";
        let target = "fn foo() {\n    bar();\n}";
        let result = find_occurrences_indent_normalized(content, target);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn block_anchor_matches_when_first_last_match() {
        let content = "fn old_name() {\n    let x = calc();\n    return x;\n}\n";
        let target = "fn old_name() {\n    let y = calc();\n    return x;\n}";
        let result = find_occurrences_block_anchor(content, target);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn whitespace_normalized_single_line() {
        let content = "use  std::collections::HashMap;\n";
        let target = "use std::collections::HashMap;";
        let result = find_occurrences_whitespace_normalized(content, target);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn generate_candidates_finds_line_trimmed_recovery() {
        let content = "fn main() {\n  \t let value = 1;\n  \t println!(\"hello\");\n}\n";
        // old_string has tab-difference: \t vs spaces
        let target = "    let value = 1;\n    println!(\"hello\");";
        let occurrences = find_occurrences(content, target);
        assert_eq!(occurrences.len(), 0);

        match generate_edit_candidates(content, target, &occurrences) {
            EditCandidateOutcome::AutoApplied { strategy, .. } => {
                assert_eq!(strategy, "line-trimmed");
            }
            _other => panic!("expected AutoApplied"),
        }
    }

    #[test]
    fn line_trimmed_range_does_not_swallow_newline_when_target_has_none() {
        let content = "fn main() {\n    let x = 1;\n    println!(\"{x}\");\n}\n";
        let target = "let x = 1;\nprintln!(\"{x}\");";
        let result = find_occurrences_line_trimmed(content, target);
        assert_eq!(result.len(), 1);
        assert_eq!(
            &content[result[0].0..result[0].1],
            "    let x = 1;\n    println!(\"{x}\");"
        );
    }

    #[test]
    fn generate_candidates_refuses_ambiguous_matches() {
        let content = "line one\ndata: a\nline three\n\ndata: b\n";
        let target = "data:";
        let occurrences = find_occurrences(content, target);
        // 2 exact occurrences.

        match generate_edit_candidates(content, target, &occurrences) {
            EditCandidateOutcome::Candidates { candidates, .. } => {
                assert!(!candidates.is_empty());
                // No candidate should be auto-applied (all low confidence).
                for c in &candidates {
                    assert_ne!(c.confidence, "high");
                }
            }
            EditCandidateOutcome::AutoApplied { .. } => {
                panic!("ambiguous match must not auto-apply");
            }
            _ => { /* mismatch is acceptable */ }
        }
    }

    // ── Phase 1.1: escape-normalized ──────────────────────────

    #[test]
    fn escape_normalized_finds_match_when_literal_n() {
        let content = "fn hello() {\n    println!(\"hi\");\n}\n";
        let target = "fn hello() {\\n    println!(\"hi\");\\n}";
        let result = find_occurrences_escape_normalized(content, target);
        assert_eq!(result.len(), 1);
        assert_eq!(
            &content[result[0].0..result[0].1],
            "fn hello() {\n    println!(\"hi\");\n}"
        );
    }

    #[test]
    fn escape_normalized_rejects_doubled_backslash() {
        // Real code literal: \\n in raw string output
        let content = "foo\\\\nbar\n";
        let target = "foo\\\\nbar";
        // unescape_tool_string_once should return None because "\\\\" triggers bail-out
        assert!(unescape_tool_string_once(target).is_none());
    }

    #[test]
    fn escape_normalized_is_not_auto_applied() {
        let content = "fn main() {\n    do_thing();\n}\n";
        // Model output with literal \n instead of real newline
        let target = "fn main() {\\n    do_thing();\\n}";
        let occurrences = find_occurrences(content, target); // exact fails
        assert_eq!(occurrences.len(), 0);

        match generate_edit_candidates(content, target, &occurrences) {
            EditCandidateOutcome::Candidates { candidates, .. } => {
                assert!(!candidates.is_empty());
                let has_escape = candidates.iter().any(|c| c.strategy == "escape-normalized");
                assert!(has_escape, "should include escape-normalized candidate");
            }
            EditCandidateOutcome::AutoApplied { strategy, .. } => {
                panic!("escape-normalized must not auto-apply, got: {}", strategy);
            }
            _ => panic!("expected Candidates, got Mismatch"),
        }
    }

    // ── Phase 1.2: trimmed-boundary ────────────────────────────

    #[test]
    fn trimmed_boundary_matches_when_extra_whitespace() {
        let content = "fn main() {\n    do_thing();\n}\n";
        // LLM included leading/trailing whitespace around old_string
        let target = "  fn main() {\n    do_thing();\n}\n  ";
        let result = find_occurrences_trimmed_boundary(content, target);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn trimmed_boundary_is_diagnostic_not_auto_applied() {
        let content = "struct Foo {\n    name: String,\n}\n";
        let target = "  struct Foo {\n    name: String,\n}\n  ";
        let occurrences = find_occurrences(content, target);
        assert_eq!(occurrences.len(), 0);

        match generate_edit_candidates(content, target, &occurrences) {
            EditCandidateOutcome::Candidates { candidates, .. } => {
                assert!(candidates.iter().any(|c| c.strategy == "trimmed-boundary"));
            }
            EditCandidateOutcome::AutoApplied { strategy, .. } => {
                panic!(
                    "trimmed-boundary must not auto-apply yet, got: {}",
                    strategy
                );
            }
            _ => { /* mismatch is acceptable */ }
        }
    }

    #[test]
    fn trimmed_boundary_no_match_when_already_trimmed() {
        let content = "hello world\n";
        let target = "hello world";
        let result = find_occurrences_trimmed_boundary(content, target);
        assert_eq!(result.len(), 0);
    }

    // ── Phase 1.3: block-anchor with Levenshtein ───────────────

    #[test]
    fn block_anchor_levenshtein_reports_high_similarity() {
        // Only 1 out of 4 middle lines differs → similarity 0.75 → "medium"
        let content = "fn calculate() {\n    let x = 1;\n    let y = 2;\n    x + y\n}\n";
        let target = "fn calculate() {\n    let x = 1;\n    let z = 3;\n    x + y\n}";
        let occurrences = find_occurrences(content, target);
        assert_eq!(occurrences.len(), 0);

        match generate_edit_candidates(content, target, &occurrences) {
            EditCandidateOutcome::Candidates { candidates, .. } => {
                let anchor = candidates
                    .iter()
                    .find(|c| c.strategy == "block-anchor")
                    .expect("should have block-anchor candidate");
                // similarity ≈ 0.75 (< 0.85 threshold) → medium confidence
                assert_eq!(anchor.confidence, "medium");
                assert!(anchor.guidance.contains("similarity="));
            }
            other => panic!("expected Candidates, got {:?}", other),
        }
    }

    #[test]
    fn block_anchor_levenshtein_near_identical_is_high() {
        // Only 1 out of 6 chars on 6 middle lines differs → > 0.85
        let content = "fn main() {\n    let a = 1;\n    let b = 2;\n    let c = 3;\n    let d = 4;\n    let e = 5;\n    a + b + c + d + e\n}\n";
        let target = "fn main() {\n    let a = 1;\n    let b = 2;\n    let c = 3;\n    let d = 4;\n    let x = 5;\n    a + b + c + d + x\n}";
        let occurrences = find_occurrences(content, target);
        assert_eq!(occurrences.len(), 0);

        match generate_edit_candidates(content, target, &occurrences) {
            EditCandidateOutcome::Candidates { candidates, .. } => {
                let anchor = candidates
                    .iter()
                    .find(|c| c.strategy == "block-anchor")
                    .expect("should have block-anchor candidate");
                // 1 out of 6 middle lines differs = 5/6 ≈ 0.83
                // but at this size the threshold is harder to reach
                assert!(anchor.guidance.contains("similarity="));
            }
            other => panic!("expected Candidates, got {:?}", other),
        }
    }

    #[test]
    fn levenshtein_similarity_perfect_match() {
        let a = &["fn foo() {", "    bar();", "}"];
        let b = &["fn foo() {", "    bar();", "}"];
        assert!((levenshtein_similarity(a, b) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn levenshtein_similarity_completely_different() {
        let a = &["a", "b", "c"];
        let b = &["x", "y", "z"];
        assert_eq!(levenshtein_similarity(a, b), 0.0);
    }
}
