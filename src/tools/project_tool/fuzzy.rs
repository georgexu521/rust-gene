//! 模糊搜索评分引擎
//!
//! 实现类似 fzf/nucleo 的评分算法：
//! - boundary bonus (路径分隔符后匹配)
//! - camel case bonus (大写字母前匹配)
//! - consecutive bonus (连续匹配)
//! - first char bonus (首字符匹配)
//! - gap penalty (间隔惩罚)

const SCORE_MATCH: i32 = 16;
const BONUS_BOUNDARY: i32 = 8;
const BONUS_CAMEL: i32 = 6;
const BONUS_CONSECUTIVE: i32 = 4;
const BONUS_FIRST_CHAR: i32 = 8;
const PENALTY_GAP_START: i32 = 3;
const PENALTY_GAP_EXTENSION: i32 = 1;

/// 搜索结果，包含路径和分数（越高越好）
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub path: String,
    pub score: i32,
}

/// 判断字符是否为路径边界字符 (/, -, _, ., 空格)
fn is_boundary(c: char) -> bool {
    matches!(c, '/' | '-' | '_' | '.' | ' ')
}

/// 在 haystack 中找 needle 的最佳匹配分数
fn best_match_score(needle: &[char], haystack: &[char]) -> Option<i32> {
    if needle.is_empty() {
        return Some(0);
    }
    if haystack.is_empty() || needle.len() > haystack.len() {
        return None;
    }

    // 首先做字符存在性检查（子序列检查）
    let mut ni = 0;
    for &hc in haystack.iter() {
        if ni < needle.len() && hc.eq_ignore_ascii_case(&needle[ni]) {
            ni += 1;
        }
    }
    if ni != needle.len() {
        return None;
    }

    // 预计算 haystack 小写形式（避免 score_from 中重复分配）
    let lower_haystack: Vec<char> = haystack.iter().map(|c| c.to_ascii_lowercase()).collect();
    let is_test_file = {
        let s: String = lower_haystack.iter().collect();
        s.contains("test") || s.contains("spec")
    };

    // 尝试所有可能的起始位置，取最高分
    let mut best: Option<i32> = None;
    for start in 0..haystack.len() {
        if let Some(score) = score_from(needle, haystack, &lower_haystack, start, is_test_file) {
            best = Some(best.map_or(score, |b| b.max(score)));
        }
    }
    best
}

/// 从 haystack 的 start 位置开始尝试匹配 needle
fn score_from(
    needle: &[char],
    haystack: &[char],
    lower_haystack: &[char],
    start: usize,
    is_test_file: bool,
) -> Option<i32> {
    if needle.is_empty() {
        return Some(0);
    }

    let mut score: i32 = 0;
    let mut hi = start;
    let mut prev_matched: Option<usize> = None;
    let mut first_matched: Option<usize> = None;
    let mut last_matched: Option<usize> = None;

    for (ni, &nc) in needle.iter().enumerate() {
        let nc_lower = nc.to_ascii_lowercase();

        // 从 hi 开始找下一个匹配的字符
        let mut found_at = None;
        while hi < haystack.len() {
            if lower_haystack[hi] == nc_lower {
                found_at = Some(hi);
                break;
            }
            hi += 1;
        }

        let pos = found_at?;

        // 基础分
        score += SCORE_MATCH;

        // 首字符 bonus
        if ni == 0 {
            score += BONUS_FIRST_CHAR;
            first_matched = Some(pos);
        }

        // boundary bonus
        if pos == 0 || is_boundary(haystack[pos - 1]) {
            score += BONUS_BOUNDARY;
        }

        // camel case bonus
        if pos > 0 && haystack[pos].is_ascii_uppercase() && haystack[pos - 1].is_ascii_lowercase() {
            score += BONUS_CAMEL;
        }

        // consecutive bonus
        if let Some(prev) = prev_matched {
            if pos == prev + 1 {
                score += BONUS_CONSECUTIVE;
            }
        }

        prev_matched = Some(pos);
        last_matched = Some(pos);
        hi = pos + 1;
    }

    // gap penalty（基于实际匹配位置计算 span）
    if let (Some(first), Some(last)) = (first_matched, last_matched) {
        if needle.len() > 1 {
            let span = last - first;
            let gaps = span.saturating_sub(needle.len() - 1);
            score -= (gaps as i32) * PENALTY_GAP_EXTENSION;
            if gaps > 0 {
                score -= PENALTY_GAP_START;
            }
        }
    }

    // test 文件惩罚
    if is_test_file {
        score = (score as f64 * 0.95) as i32;
    }

    Some(score)
}

/// 在 haystack 中搜索 needle，返回最佳匹配分数
pub fn fuzzy_score(needle: &str, haystack: &str) -> Option<i32> {
    if needle.is_empty() {
        return Some(0);
    }
    let needle_chars: Vec<char> = needle.chars().collect();
    let haystack_chars: Vec<char> = haystack.chars().collect();
    best_match_score(&needle_chars, &haystack_chars)
}

/// 模糊搜索：对文件列表进行评分排序
pub fn fuzzy_search(query: &str, files: &[String], limit: usize) -> Vec<SearchResult> {
    let mut results: Vec<SearchResult> = files
        .iter()
        .filter_map(|f| {
            fuzzy_score(query, f).map(|score| SearchResult {
                path: f.clone(),
                score,
            })
        })
        .collect();

    // 按分数降序排列（分数越高越好）
    results.sort_by(|a, b| b.score.cmp(&a.score));
    results.truncate(limit);
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let score = fuzzy_score("main", "src/main.rs");
        assert!(score.is_some());
        assert!(score.unwrap() > 0);
    }

    #[test]
    fn test_boundary_bonus() {
        let s1 = fuzzy_score("mod", "src/mod.rs").unwrap();
        let s2 = fuzzy_score("mod", "src/amod.rs").unwrap();
        assert!(s1 > s2);
    }

    #[test]
    fn test_consecutive_bonus() {
        let s1 = fuzzy_score("abc", "abc.rs").unwrap();
        let s2 = fuzzy_score("abc", "aXbc.rs").unwrap();
        assert!(s1 > s2, "consecutive ({}) should beat gap ({})", s1, s2);
    }

    #[test]
    fn test_no_match() {
        let score = fuzzy_score("xyz", "src/main.rs");
        assert!(score.is_none());
    }

    #[test]
    fn test_camel_case() {
        let score = fuzzy_score("query", "queryEngine.ts");
        assert!(score.is_some());
    }

    #[test]
    fn test_fuzzy_search_ordering() {
        let files = vec![
            "src/amod.rs".to_string(),
            "src/main.rs".to_string(),
            "src/main_backup.rs".to_string(),
        ];
        let results = fuzzy_search("main", &files, 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].path, "src/main.rs");
    }

    #[test]
    fn test_case_insensitive() {
        let score = fuzzy_score("Main", "src/main.rs");
        assert!(score.is_some());
    }

    #[test]
    fn test_non_consecutive_match() {
        let score = fuzzy_score("abc", "aXbc.rs");
        assert!(score.is_some());
    }

    #[test]
    fn test_gap_penalty_from_offset() {
        // "mod" 匹配 "src/mod.rs"（连续，无 gap）
        let s1 = fuzzy_score("mod", "src/mod.rs").unwrap();
        // "mod" 匹配 "src/modx.rs"（连续但后面多了 'x' 不影响）
        // vs "mod" 匹配 "src/mood.rs"（'m','o','d' 中间跳了第二个 'o'）
        let s2 = fuzzy_score("mod", "src/mood.rs").unwrap();
        assert!(s1 > s2, "no-gap ({}) should beat gap ({})", s1, s2);
    }

    #[test]
    fn test_empty_needle() {
        assert_eq!(fuzzy_score("", "anything"), Some(0));
    }

    #[test]
    fn test_single_char() {
        let score = fuzzy_score("m", "src/main.rs");
        assert!(score.is_some());
        assert!(score.unwrap() > 0);
    }
}
