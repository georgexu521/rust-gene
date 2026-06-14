//! 简易模糊匹配组件
//!
//! 给候选字符串与查询字符串打分；仅用于局部文件/路径补全。

/// 如果 `query` 是 `candidate` 的模糊子序列，返回非负分数；否则返回 None。
///
/// 分数越高表示匹配越好。当前规则：
/// - 每个匹配的字符 +10
/// - 相邻匹配（与上一次匹配位置连续）额外 +15
/// - 候选字符串越短惩罚越小（用候选长度做 mild 负分）
///
/// 匹配不区分大小写。
pub fn fuzzy_score(candidate: &str, query: &str) -> Option<u32> {
    if query.is_empty() {
        return Some(0);
    }

    let candidate_lower = candidate.to_ascii_lowercase();
    let query_lower = query.to_ascii_lowercase();

    let mut score: u32 = 0;
    let mut prev_match: Option<usize> = None;
    let mut query_chars = query_lower.chars();
    let mut current_query = query_chars.next()?;

    for (idx, c) in candidate_lower.char_indices() {
        if c == current_query {
            score += 10;
            if let Some(prev) = prev_match {
                if idx == prev + 1 {
                    score += 15;
                }
            }
            prev_match = Some(idx);

            match query_chars.next() {
                Some(next) => current_query = next,
                None => {
                    // 所有查询字符都已匹配
                    let length_penalty = candidate.len() as u32 / 4;
                    return Some(score.saturating_sub(length_penalty));
                }
            }
        }
    }

    None
}

/// 判断 `query` 是否模糊匹配 `candidate`。
pub fn fuzzy_matches(candidate: &str, query: &str) -> bool {
    fuzzy_score(candidate, query).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_matches_everything() {
        assert_eq!(fuzzy_score("foo", ""), Some(0));
    }

    #[test]
    fn exact_match_scores_high() {
        let exact = fuzzy_score("Cargo.toml", "cargo.toml").unwrap();
        let partial = fuzzy_score("cargo-lock", "cargo.toml");
        assert!(partial.is_none());
        assert!(exact > 0);
    }

    #[test]
    fn fuzzy_subsequence_matches_out_of_order_chars() {
        let score = fuzzy_score("src/main.rs", "smr").unwrap();
        assert!(score > 0);
    }

    #[test]
    fn consecutive_bonus_beats_sparse_match() {
        let consecutive = fuzzy_score("foobar", "oob").unwrap();
        let sparse = fuzzy_score("f_o_o_b_a_r", "oob").unwrap();
        assert!(consecutive > sparse);
    }

    #[test]
    fn shorter_candidate_scores_higher_for_same_query() {
        let short = fuzzy_score("main.rs", "mr").unwrap();
        let long = fuzzy_score("src/main_rendering_service.rs", "mr").unwrap();
        assert!(short > long);
    }
}
