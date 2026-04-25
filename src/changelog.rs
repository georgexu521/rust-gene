//! Changelog 生成与管理
//!
//! 跟踪版本变更，记录发布说明

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

/// 变更类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeType {
    /// 新增功能
    Added,
    /// 改进功能
    Changed,
    /// Bug 修复
    Fixed,
    /// 废弃功能
    Deprecated,
    /// 移除功能
    Removed,
    /// 安全修复
    Security,
    /// 性能优化
    Performance,
    /// 文档更新
    Docs,
}

impl ChangeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChangeType::Added => "Added",
            ChangeType::Changed => "Changed",
            ChangeType::Fixed => "Fixed",
            ChangeType::Deprecated => "Deprecated",
            ChangeType::Removed => "Removed",
            ChangeType::Security => "Security",
            ChangeType::Performance => "Performance",
            ChangeType::Docs => "Docs",
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            ChangeType::Added => "+",
            ChangeType::Changed => "~",
            ChangeType::Fixed => "*",
            ChangeType::Deprecated => "D",
            ChangeType::Removed => "-",
            ChangeType::Security => "#",
            ChangeType::Performance => "P",
            ChangeType::Docs => "docs",
        }
    }
}

/// 单个变更条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeEntry {
    /// 变更类型
    pub change_type: ChangeType,
    /// 变更描述
    pub description: String,
    /// 关联的模块
    pub module: Option<String>,
    /// 关联的命令（如果有）
    pub command: Option<String>,
    /// 关联的工具（如果有）
    pub tool: Option<String>,
}

/// 发布条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseEntry {
    /// 版本号
    pub version: String,
    /// 发布日期
    pub date: DateTime<Local>,
    /// 通道 (alpha/beta/stable)
    pub channel: String,
    /// 变更列表
    pub changes: Vec<ChangeEntry>,
    /// 是否正式发布
    pub is_stable: bool,
    /// 破坏性变更标记
    pub breaking: bool,
}

impl ReleaseEntry {
    pub fn new(version: &str, channel: &str, is_stable: bool) -> Self {
        Self {
            version: version.to_string(),
            date: Local::now(),
            channel: channel.to_string(),
            changes: vec![],
            is_stable,
            breaking: false,
        }
    }

    pub fn add_change(&mut self, change: ChangeEntry) {
        self.changes.push(change);
    }

    /// 标记为破坏性变更
    pub fn mark_breaking(&mut self) {
        self.breaking = true;
    }

    /// 生成 Markdown 格式的变更日志
    pub fn to_markdown(&self) -> String {
        let mut lines = vec![];

        let stability_badge = if self.is_stable {
            "[stable]".to_string()
        } else {
            format!("[{}]", self.channel)
        };

        lines.push(format!(
            "## {} {} ({}) {}",
            if self.breaking { "!" } else { "" },
            self.version,
            self.date.format("%Y-%m-%d"),
            stability_badge
        ));

        // Group by change type
        let mut grouped_added = vec![];
        let mut grouped_changed = vec![];
        let mut grouped_performance = vec![];
        let mut grouped_fixed = vec![];
        let mut grouped_security = vec![];
        let mut grouped_deprecated = vec![];
        let mut grouped_removed = vec![];
        let mut grouped_docs = vec![];

        for change in &self.changes {
            let scope = if let Some(ref cmd) = change.command {
                format!(" `/{}`", cmd)
            } else if let Some(ref tool) = change.tool {
                format!(" `{}`", tool)
            } else if let Some(ref module) = change.module {
                format!(" `{}`", module)
            } else {
                String::new()
            };

            let entry = format!("- {}{}", change.description, scope);

            match change.change_type {
                ChangeType::Added => grouped_added.push(entry),
                ChangeType::Changed => grouped_changed.push(entry),
                ChangeType::Performance => grouped_performance.push(entry),
                ChangeType::Fixed => grouped_fixed.push(entry),
                ChangeType::Security => grouped_security.push(entry),
                ChangeType::Deprecated => grouped_deprecated.push(entry),
                ChangeType::Removed => grouped_removed.push(entry),
                ChangeType::Docs => grouped_docs.push(entry),
            }
        }

        for (name, entries) in [
            ("Added", &grouped_added),
            ("Changed", &grouped_changed),
            ("Performance", &grouped_performance),
            ("Fixed", &grouped_fixed),
            ("Security", &grouped_security),
            ("Deprecated", &grouped_deprecated),
            ("Removed", &grouped_removed),
            ("Docs", &grouped_docs),
        ] {
            if !entries.is_empty() {
                lines.push(format!("\n### {}", name));
                lines.extend(entries.iter().cloned());
            }
        }

        lines.join("\n")
    }
}

/// Changelog 管理器
#[derive(Debug, Default)]
pub struct Changelog {
    /// 所有发布条目
    releases: Vec<ReleaseEntry>,
}

impl Changelog {
    pub fn new() -> Self {
        Self { releases: vec![] }
    }

    /// 添加发布条目
    pub fn add_release(&mut self, release: ReleaseEntry) {
        self.releases.insert(0, release); // Newest first
    }

    /// 获取所有版本
    pub fn versions(&self) -> Vec<String> {
        self.releases.iter().map(|r| r.version.clone()).collect()
    }

    /// 获取指定版本的变更
    pub fn get_version(&self, version: &str) -> Option<&ReleaseEntry> {
        self.releases.iter().find(|r| r.version == version)
    }

    /// 生成完整的 Markdown changelog
    pub fn to_markdown(&self) -> String {
        let mut lines = vec![
            "# Changelog".to_string(),
            "".to_string(),
            "All notable changes to this project will be documented in this file.".to_string(),
            "".to_string(),
            "The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/)."
                .to_string(),
            "".to_string(),
        ];

        for release in &self.releases {
            lines.push(release.to_markdown());
            lines.push(String::new());
        }

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_entry() {
        let mut release = ReleaseEntry::new("1.0.0", "stable", true);
        release.add_change(ChangeEntry {
            change_type: ChangeType::Added,
            description: "New command /hello".to_string(),
            module: None,
            command: Some("hello".to_string()),
            tool: None,
        });

        let md = release.to_markdown();
        assert!(md.contains("1.0.0"));
        assert!(md.contains("/hello"));
    }

    #[test]
    fn test_changelog() {
        let mut changelog = Changelog::new();
        changelog.add_release(ReleaseEntry::new("1.0.0", "stable", true));

        assert_eq!(changelog.versions(), vec!["1.0.0"]);
    }
}
