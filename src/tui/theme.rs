//! TUI 主题系统
//!
//! 支持 Dark / Light / High-Contrast 三种预设主题，所有颜色通过 Theme 统一配置。

use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// 主题预设名称
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemePreset {
    Dark,
    Light,
    HighContrast,
}

impl std::fmt::Display for ThemePreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThemePreset::Dark => write!(f, "dark"),
            ThemePreset::Light => write!(f, "light"),
            ThemePreset::HighContrast => write!(f, "high-contrast"),
        }
    }
}

impl std::str::FromStr for ThemePreset {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "dark" => Ok(ThemePreset::Dark),
            "light" => Ok(ThemePreset::Light),
            "high-contrast" | "high_contrast" | "highcontrast" => Ok(ThemePreset::HighContrast),
            _ => Err(format!("Unknown theme preset: {}", s)),
        }
    }
}

/// 完整主题配色表
#[derive(Debug, Clone)]
pub struct Theme {
    // ── 背景色 ──
    pub bg: Color,
    pub bg_popup: Color,
    pub bg_selected: Color,

    // ── 文本色 ──
    pub text: Color,
    pub text_dim: Color,
    pub text_highlight: Color,

    // ── 边框色 ──
    pub border: Color,
    pub border_active: Color,

    // ── 消息角色色 ──
    pub user_message: Color,
    pub assistant_message: Color,
    pub system_message: Color,
    pub tool_message: Color,

    // ── 语义色 ──
    pub success: Color,
    pub error: Color,
    pub warning: Color,
    pub info: Color,

    // ── Diff 色 ──
    pub diff_add: Color,
    pub diff_remove: Color,
    pub diff_header: Color,
    pub diff_line_number: Color,

    // ── 状态色 ──
    pub status_ready: Color,
    pub status_thinking: Color,
    pub status_vim: Color,
    pub status_worktree: Color,
}

impl Theme {
    /// 根据预设名称获取主题
    pub fn from_preset(preset: ThemePreset) -> Self {
        match preset {
            ThemePreset::Dark => Self::dark(),
            ThemePreset::Light => Self::light(),
            ThemePreset::HighContrast => Self::high_contrast(),
        }
    }

    /// Dark 主题（默认）
    pub fn dark() -> Self {
        Self {
            bg: Color::Reset,
            bg_popup: Color::Black,
            bg_selected: Color::DarkGray,
            text: Color::White,
            text_dim: Color::Gray,
            text_highlight: Color::Blue,
            border: Color::DarkGray,
            border_active: Color::Cyan,
            user_message: Color::Cyan,
            assistant_message: Color::Green,
            system_message: Color::Yellow,
            tool_message: Color::Magenta,
            success: Color::Green,
            error: Color::Red,
            warning: Color::Yellow,
            info: Color::Cyan,
            diff_add: Color::Green,
            diff_remove: Color::Red,
            diff_header: Color::Yellow,
            diff_line_number: Color::Gray,
            status_ready: Color::Green,
            status_thinking: Color::Yellow,
            status_vim: Color::Magenta,
            status_worktree: Color::Cyan,
        }
    }

    /// Light 主题
    pub fn light() -> Self {
        Self {
            bg: Color::White,
            bg_popup: Color::Rgb(245, 245, 245),
            bg_selected: Color::Rgb(220, 220, 220),
            text: Color::Black,
            text_dim: Color::Rgb(100, 100, 100),
            text_highlight: Color::Blue,
            border: Color::Rgb(180, 180, 180),
            border_active: Color::Blue,
            user_message: Color::Rgb(0, 100, 150),
            assistant_message: Color::Rgb(0, 120, 0),
            system_message: Color::Rgb(180, 140, 0),
            tool_message: Color::Rgb(140, 0, 140),
            success: Color::Rgb(0, 140, 0),
            error: Color::Rgb(200, 0, 0),
            warning: Color::Rgb(200, 160, 0),
            info: Color::Rgb(0, 100, 200),
            diff_add: Color::Rgb(0, 160, 0),
            diff_remove: Color::Rgb(200, 0, 0),
            diff_header: Color::Rgb(180, 140, 0),
            diff_line_number: Color::Rgb(120, 120, 120),
            status_ready: Color::Rgb(0, 140, 0),
            status_thinking: Color::Rgb(200, 160, 0),
            status_vim: Color::Rgb(140, 0, 140),
            status_worktree: Color::Rgb(0, 100, 150),
        }
    }

    /// High-Contrast 主题（无障碍）
    pub fn high_contrast() -> Self {
        Self {
            bg: Color::Black,
            bg_popup: Color::Black,
            bg_selected: Color::White,
            text: Color::White,
            text_dim: Color::Rgb(200, 200, 200),
            text_highlight: Color::Yellow,
            border: Color::White,
            border_active: Color::Yellow,
            user_message: Color::Cyan,
            assistant_message: Color::Green,
            system_message: Color::Yellow,
            tool_message: Color::Magenta,
            success: Color::Green,
            error: Color::Red,
            warning: Color::Yellow,
            info: Color::Cyan,
            diff_add: Color::Green,
            diff_remove: Color::Red,
            diff_header: Color::Yellow,
            diff_line_number: Color::White,
            status_ready: Color::Green,
            status_thinking: Color::Yellow,
            status_vim: Color::Magenta,
            status_worktree: Color::Cyan,
        }
    }

    /// 从字符串解析（用于配置反序列化兜底）
    pub fn from_name(name: &str) -> Self {
        match ThemePreset::from_str(name) {
            Ok(preset) => Self::from_preset(preset),
            Err(_) => Self::dark(),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_presets() {
        let dark = Theme::dark();
        assert_eq!(dark.bg, Color::Reset);

        let light = Theme::light();
        assert_eq!(light.bg, Color::White);

        let hc = Theme::high_contrast();
        assert_eq!(hc.border, Color::White);
    }

    #[test]
    fn test_theme_from_name() {
        assert_eq!(Theme::from_name("dark").bg, Color::Reset);
        assert_eq!(Theme::from_name("light").bg, Color::White);
        assert_eq!(Theme::from_name("unknown").bg, Color::Reset); // fallback
    }

    #[test]
    fn test_theme_preset_parse() {
        assert_eq!("dark".parse::<ThemePreset>().unwrap(), ThemePreset::Dark);
        assert_eq!(
            "high-contrast".parse::<ThemePreset>().unwrap(),
            ThemePreset::HighContrast
        );
        assert!("unknown".parse::<ThemePreset>().is_err());
    }
}
