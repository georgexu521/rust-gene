//! .gitignore / .ignore 模式解析器
//!
//! 用于 walk_directory fallback 路径（git ls-files 天然支持 gitignore）

use std::collections::HashSet;
use std::path::Path;

/// 一条 gitignore 规则
#[derive(Debug, Clone)]
struct IgnoreRule {
    /// 匹配模式（支持 * 和 ? 通配符）
    pattern: String,
    /// 是否为否定规则（以 ! 开头）
    negated: bool,
    /// 是否只匹配目录（以 / 结尾）
    dir_only: bool,
}

impl IgnoreRule {
    fn parse(line: &str) -> Option<Self> {
        let line = line.trim();

        // 跳过空行和注释
        if line.is_empty() || line.starts_with('#') {
            return None;
        }

        let mut pattern = line.to_string();
        let mut negated = false;
        let mut dir_only = false;

        // 处理否定规则
        if pattern.starts_with('!') {
            negated = true;
            pattern.remove(0);
        }

        // 处理仅目录
        if pattern.ends_with('/') {
            dir_only = true;
            pattern.pop();
        }

        // 去掉开头的 /
        if pattern.starts_with('/') {
            pattern.remove(0);
        }

        Some(Self {
            pattern,
            negated,
            dir_only,
        })
    }

    /// 检查路径是否匹配此规则
    fn matches(&self, relative_path: &str, is_dir: bool) -> bool {
        if self.dir_only && !is_dir {
            return false;
        }

        let path = relative_path.trim_start_matches('/');
        self.glob_match(&self.pattern, path)
    }

    /// 简单的 glob 匹配实现
    fn glob_match(&self, pattern: &str, text: &str) -> bool {
        if pattern.contains("**") {
            return self.glob_match_double_star(pattern, text);
        }

        let pattern_parts: Vec<&str> = pattern.split('/').collect();
        let text_parts: Vec<&str> = text.split('/').collect();

        // 如果模式不包含 /，则只匹配文件名部分
        if !pattern.contains('/') {
            if let Some(filename) = text_parts.last() {
                return self.simple_glob(pattern, filename);
            }
            return false;
        }

        if pattern_parts.len() > text_parts.len() {
            return false;
        }

        // 从任意位置开始尝试匹配
        for offset in 0..=text_parts.len().saturating_sub(pattern_parts.len()) {
            let ok = pattern_parts
                .iter()
                .enumerate()
                .all(|(i, pp)| self.simple_glob(pp, text_parts[offset + i]));
            if ok {
                return true;
            }
        }

        false
    }

    fn glob_match_double_star(&self, pattern: &str, text: &str) -> bool {
        let parts: Vec<&str> = pattern.split("**").collect();
        // "**" 出现多次的情况不支持，回退到普通匹配
        if parts.len() != 2 {
            return self.simple_glob(pattern, text);
        }

        let prefix = parts[0].trim_end_matches('/');
        let suffix = parts[1].trim_start_matches('/');

        match (prefix.is_empty(), suffix.is_empty()) {
            (true, true) => true, // "**" 匹配一切
            (true, false) => {
                // "**/suffix" — 任意深度
                text.split('/').any(|part| self.simple_glob(suffix, part))
            }
            (false, true) => {
                // "prefix/**" — prefix 下所有
                text.starts_with(prefix)
            }
            (false, false) => {
                // "prefix/**/suffix"
                text.starts_with(prefix)
                    && text[prefix.len()..]
                        .split('/')
                        .any(|part| self.simple_glob(suffix, part))
            }
        }
    }

    fn simple_glob(&self, pattern: &str, text: &str) -> bool {
        let p: Vec<char> = pattern.chars().collect();
        let t: Vec<char> = text.chars().collect();
        self.simple_glob_chars(&p, 0, &t, 0)
    }

    fn simple_glob_chars(&self, p: &[char], pi: usize, t: &[char], ti: usize) -> bool {
        if pi >= p.len() {
            return ti >= t.len();
        }

        match p[pi] {
            '*' => {
                if self.simple_glob_chars(p, pi + 1, t, ti) {
                    return true;
                }
                for i in ti..t.len() {
                    if self.simple_glob_chars(p, pi + 1, t, i + 1) {
                        return true;
                    }
                }
                false
            }
            '?' => {
                if ti >= t.len() {
                    return false;
                }
                self.simple_glob_chars(p, pi + 1, t, ti + 1)
            }
            c => {
                if ti >= t.len() || !c.eq_ignore_ascii_case(&t[ti]) {
                    return false;
                }
                self.simple_glob_chars(p, pi + 1, t, ti + 1)
            }
        }
    }
}

/// 需要在文件级别忽略的项（非目录）
const ALWAYS_IGNORE_FILES: &[&str] = &[".DS_Store", "Thumbs.db", ".env.local"];

/// Gitignore 解析器
pub struct GitIgnore {
    rules: Vec<IgnoreRule>,
    /// 始终跳过的目录（硬编码）
    always_skip_dirs: HashSet<String>,
}

impl GitIgnore {
    /// 创建带有默认规则的解析器
    pub fn new() -> Self {
        let always_skip_dirs: HashSet<String> = [
            ".git",
            "node_modules",
            "target",
            "__pycache__",
            ".venv",
            "venv",
            ".tox",
            ".mypy_cache",
            ".pytest_cache",
            ".ruff_cache",
            "dist",
            "build",
            ".next",
            ".nuxt",
            "vendor",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        Self {
            rules: Vec::new(),
            always_skip_dirs,
        }
    }

    /// 从 .gitignore 文件加载规则
    pub fn load_from_dir(&mut self, dir: &Path) {
        self.load_rules_file(&dir.join(".gitignore"));
        self.load_rules_file(&dir.join(".ignore"));
        self.load_rules_file(&dir.join(".rgignore"));
    }

    fn load_rules_file(&mut self, path: &Path) {
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                if let Some(rule) = IgnoreRule::parse(line) {
                    self.rules.push(rule);
                }
            }
        }
    }

    /// 检查目录是否应该被跳过（last-match-wins 语义）
    pub fn should_skip_dir(&self, name: &str, relative: &str) -> bool {
        // 硬编码跳过
        if self.always_skip_dirs.contains(name) {
            return true;
        }

        // last-match-wins: 遍历所有规则，最后一条命中的决定结果
        let mut ignored = false;
        for rule in &self.rules {
            if rule.matches(relative, true) {
                ignored = !rule.negated;
            }
        }
        ignored
    }

    /// 检查文件是否应该被忽略（last-match-wins 语义）
    pub fn should_ignore(&self, relative: &str) -> bool {
        // 硬编码文件忽略
        if let Some(filename) = relative.rsplit('/').next() {
            if ALWAYS_IGNORE_FILES.contains(&filename) {
                return true;
            }
        }

        let mut ignored = false;
        for rule in &self.rules {
            if rule.matches(relative, false) {
                ignored = !rule.negated;
            }
        }
        ignored
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ignore_rule() {
        let rule = IgnoreRule::parse("target/").unwrap();
        assert!(rule.dir_only);
        assert_eq!(rule.pattern, "target");
    }

    #[test]
    fn test_parse_negated() {
        let rule = IgnoreRule::parse("!important.log").unwrap();
        assert!(rule.negated);
    }

    #[test]
    fn test_parse_comment() {
        assert!(IgnoreRule::parse("# comment").is_none());
    }

    #[test]
    fn test_parse_empty() {
        assert!(IgnoreRule::parse("").is_none());
        assert!(IgnoreRule::parse("  ").is_none());
    }

    #[test]
    fn test_glob_simple() {
        let rule = IgnoreRule::parse("*.log").unwrap();
        assert!(rule.matches("debug.log", false));
        assert!(rule.matches("src/debug.log", false));
        assert!(!rule.matches("debug.rs", false));
    }

    #[test]
    fn test_gitignore_skip_dir() {
        let gi = GitIgnore::new();
        assert!(gi.should_skip_dir("target", "target"));
        assert!(gi.should_skip_dir("node_modules", "node_modules"));
        assert!(!gi.should_skip_dir("src", "src"));
    }

    #[test]
    fn test_gitignore_should_ignore() {
        let gi = GitIgnore::new();
        assert!(!gi.should_ignore("src/main.rs"));
    }

    #[test]
    fn test_ds_store_ignored() {
        let gi = GitIgnore::new();
        assert!(gi.should_ignore(".DS_Store"));
        assert!(gi.should_ignore("src/.DS_Store"));
    }

    #[test]
    fn test_negation_last_match_wins() {
        let mut gi = GitIgnore::new();
        gi.rules.push(IgnoreRule::parse("*.log").unwrap());
        gi.rules.push(IgnoreRule::parse("!important.log").unwrap());

        // "important.log" 先被 *.log 忽略，又被 !important.log 取消忽略
        // last-match-wins: !important.log 胜出
        assert!(!gi.should_ignore("important.log"));
        assert!(gi.should_ignore("debug.log"));
    }
}
