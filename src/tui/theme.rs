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
    Nord,
    Dracula,
    GruvboxDark,
    CatppuccinMocha,
}

impl std::fmt::Display for ThemePreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThemePreset::Dark => write!(f, "dark"),
            ThemePreset::Light => write!(f, "light"),
            ThemePreset::HighContrast => write!(f, "high-contrast"),
            ThemePreset::Nord => write!(f, "nord"),
            ThemePreset::Dracula => write!(f, "dracula"),
            ThemePreset::GruvboxDark => write!(f, "gruvbox-dark"),
            ThemePreset::CatppuccinMocha => write!(f, "catppuccin-mocha"),
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
            "nord" => Ok(ThemePreset::Nord),
            "dracula" => Ok(ThemePreset::Dracula),
            "gruvbox-dark" | "gruvbox_dark" | "gruvbox" => Ok(ThemePreset::GruvboxDark),
            "catppuccin-mocha" | "catppuccin_mocha" | "catppuccin" => Ok(ThemePreset::CatppuccinMocha),
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
            ThemePreset::Nord => Self::nord(),
            ThemePreset::Dracula => Self::dracula(),
            ThemePreset::GruvboxDark => Self::gruvbox_dark(),
            ThemePreset::CatppuccinMocha => Self::catppuccin_mocha(),
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

    /// Nord 主题（Arctic Ice）
    pub fn nord() -> Self {
        Self {
            bg: Color::Rgb(46, 52, 64),
            bg_popup: Color::Rgb(59, 66, 82),
            bg_selected: Color::Rgb(67, 76, 94),
            text: Color::Rgb(216, 222, 233),
            text_dim: Color::Rgb(76, 86, 106),
            text_highlight: Color::Rgb(136, 192, 208),
            border: Color::Rgb(76, 86, 106),
            border_active: Color::Rgb(136, 192, 208),
            user_message: Color::Rgb(136, 192, 208),
            assistant_message: Color::Rgb(163, 190, 140),
            system_message: Color::Rgb(235, 203, 139),
            tool_message: Color::Rgb(180, 142, 173),
            success: Color::Rgb(163, 190, 140),
            error: Color::Rgb(191, 97, 106),
            warning: Color::Rgb(235, 203, 139),
            info: Color::Rgb(136, 192, 208),
            diff_add: Color::Rgb(163, 190, 140),
            diff_remove: Color::Rgb(191, 97, 106),
            diff_header: Color::Rgb(235, 203, 139),
            diff_line_number: Color::Rgb(76, 86, 106),
            status_ready: Color::Rgb(163, 190, 140),
            status_thinking: Color::Rgb(235, 203, 139),
            status_vim: Color::Rgb(180, 142, 173),
            status_worktree: Color::Rgb(136, 192, 208),
        }
    }

    /// Dracula 主题
    pub fn dracula() -> Self {
        Self {
            bg: Color::Rgb(40, 42, 54),
            bg_popup: Color::Rgb(68, 71, 90),
            bg_selected: Color::Rgb(98, 114, 164),
            text: Color::Rgb(248, 248, 242),
            text_dim: Color::Rgb(98, 114, 164),
            text_highlight: Color::Rgb(139, 233, 253),
            border: Color::Rgb(98, 114, 164),
            border_active: Color::Rgb(189, 147, 249),
            user_message: Color::Rgb(139, 233, 253),
            assistant_message: Color::Rgb(80, 250, 123),
            system_message: Color::Rgb(241, 250, 140),
            tool_message: Color::Rgb(255, 121, 198),
            success: Color::Rgb(80, 250, 123),
            error: Color::Rgb(255, 85, 85),
            warning: Color::Rgb(241, 250, 140),
            info: Color::Rgb(139, 233, 253),
            diff_add: Color::Rgb(80, 250, 123),
            diff_remove: Color::Rgb(255, 85, 85),
            diff_header: Color::Rgb(241, 250, 140),
            diff_line_number: Color::Rgb(98, 114, 164),
            status_ready: Color::Rgb(80, 250, 123),
            status_thinking: Color::Rgb(241, 250, 140),
            status_vim: Color::Rgb(255, 121, 198),
            status_worktree: Color::Rgb(139, 233, 253),
        }
    }

    /// Gruvbox Dark 主题（复古暖色）
    pub fn gruvbox_dark() -> Self {
        Self {
            bg: Color::Rgb(40, 40, 40),
            bg_popup: Color::Rgb(60, 56, 54),
            bg_selected: Color::Rgb(80, 73, 69),
            text: Color::Rgb(235, 219, 178),
            text_dim: Color::Rgb(146, 131, 116),
            text_highlight: Color::Rgb(131, 165, 152),
            border: Color::Rgb(146, 131, 116),
            border_active: Color::Rgb(254, 128, 25),
            user_message: Color::Rgb(131, 165, 152),
            assistant_message: Color::Rgb(184, 187, 38),
            system_message: Color::Rgb(250, 189, 47),
            tool_message: Color::Rgb(211, 134, 155),
            success: Color::Rgb(184, 187, 38),
            error: Color::Rgb(251, 73, 52),
            warning: Color::Rgb(250, 189, 47),
            info: Color::Rgb(131, 165, 152),
            diff_add: Color::Rgb(184, 187, 38),
            diff_remove: Color::Rgb(251, 73, 52),
            diff_header: Color::Rgb(250, 189, 47),
            diff_line_number: Color::Rgb(146, 131, 116),
            status_ready: Color::Rgb(184, 187, 38),
            status_thinking: Color::Rgb(250, 189, 47),
            status_vim: Color::Rgb(211, 134, 155),
            status_worktree: Color::Rgb(131, 165, 152),
        }
    }

    /// Catppuccin Mocha 主题（现代柔和暗色）
    pub fn catppuccin_mocha() -> Self {
        Self {
            bg: Color::Rgb(30, 30, 46),
            bg_popup: Color::Rgb(49, 50, 68),
            bg_selected: Color::Rgb(69, 71, 90),
            text: Color::Rgb(205, 214, 244),
            text_dim: Color::Rgb(108, 112, 134),
            text_highlight: Color::Rgb(137, 180, 250),
            border: Color::Rgb(108, 112, 134),
            border_active: Color::Rgb(137, 180, 250),
            user_message: Color::Rgb(137, 180, 250),
            assistant_message: Color::Rgb(166, 227, 161),
            system_message: Color::Rgb(249, 226, 175),
            tool_message: Color::Rgb(245, 194, 231),
            success: Color::Rgb(166, 227, 161),
            error: Color::Rgb(243, 139, 168),
            warning: Color::Rgb(249, 226, 175),
            info: Color::Rgb(137, 180, 250),
            diff_add: Color::Rgb(166, 227, 161),
            diff_remove: Color::Rgb(243, 139, 168),
            diff_header: Color::Rgb(249, 226, 175),
            diff_line_number: Color::Rgb(108, 112, 134),
            status_ready: Color::Rgb(166, 227, 161),
            status_thinking: Color::Rgb(249, 226, 175),
            status_vim: Color::Rgb(245, 194, 231),
            status_worktree: Color::Rgb(137, 180, 250),
        }
    }

    /// 从字符串解析（用于配置反序列化兜底）
    pub fn from_name(name: &str) -> Self {
        match ThemePreset::from_str(name) {
            Ok(preset) => Self::from_preset(preset),
            Err(_) => Self::dark(),
        }
    }

    /// 判断是否为暗色主题（用于代码高亮主题选择）
    pub fn is_dark(&self) -> bool {
        match self.bg {
            Color::White
            | Color::Rgb(245, 245, 245)
            | Color::Rgb(255, 255, 255)
            | Color::Rgb(250, 250, 250) => false,
            _ => true,
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
