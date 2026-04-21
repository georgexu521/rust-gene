//! 版本与发布管理
//!
//! 支持 semver 版本号和发布列车（alpha/beta/stable）

use serde::{Deserialize, Serialize};

/// Release channel (发布列车)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReleaseChannel {
    /// 开发版/每日构建
    Alpha,
    /// 测试版
    Beta,
    /// 稳定版
    Stable,
}

impl ReleaseChannel {
    /// 从环境变量获取发布通道
    pub fn from_env() -> Self {
        match std::env::var("PRIORITY_AGENT_RELEASE_CHANNEL")
            .as_deref()
            .unwrap_or("stable")
        {
            "alpha" => ReleaseChannel::Alpha,
            "beta" => ReleaseChannel::Beta,
            _ => ReleaseChannel::Stable,
        }
    }

    /// 获取通道名称
    pub fn as_str(&self) -> &'static str {
        match self {
            ReleaseChannel::Alpha => "alpha",
            ReleaseChannel::Beta => "beta",
            ReleaseChannel::Stable => "stable",
        }
    }
}

/// 版本信息
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
    pub channel: ReleaseChannel,
}

impl Version {
    /// 创建新版本
    pub fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
            channel: ReleaseChannel::from_env(),
        }
    }

    /// 完整版本字符串
    pub fn to_string(&self) -> String {
        match self.channel {
            ReleaseChannel::Stable => format!("{}.{}.{}", self.major, self.minor, self.patch),
            _ => format!(
                "{}.{}.{}-{}",
                self.major, self.minor, self.patch, self.channel.as_str()
            ),
        }
    }

    /// 检查是否为稳定版
    pub fn is_stable(&self) -> bool {
        self.channel == ReleaseChannel::Stable
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_string() {
        let v = Version::new(1, 2, 3);
        assert_eq!(v.to_string(), "1.2.3");
    }

    #[test]
    fn test_version_channel() {
        let v = Version::new(1, 0, 0);
        assert!(v.is_stable());
    }
}
