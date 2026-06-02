//! TUI 主题系统
//!
//! Token-based 分层设计，参考 Reasonix 设计语言。
//! 每个主题包含 FG（文字）、SURFACE（表面）、TONE（语义色）、CARD（卡片）、PILL（标签）五层 token。
//! 保留旧扁平字段作为兼容层，新代码应通过 `tokens` 访问颜色。

use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

// ── Token 层定义 ──

/// 文字颜色 token
#[derive(Debug, Clone)]
pub struct FgTokens {
    pub strong: Color,
    pub body: Color,
    pub sub: Color,
    pub meta: Color,
    pub faint: Color,
}

/// 表面颜色 token
#[derive(Debug, Clone)]
pub struct SurfaceTokens {
    pub bg: Color,
    pub bg_input: Color,
    pub bg_code: Color,
    pub bg_elev: Color,
}

/// 语义色 token
#[derive(Debug, Clone)]
pub struct ToneTokens {
    pub brand: Color,
    pub accent: Color,
    pub violet: Color,
    pub ok: Color,
    pub warn: Color,
    pub err: Color,
    pub info: Color,
}

/// 卡片 glyph + 颜色 token
#[derive(Debug, Clone)]
pub struct CardToken {
    pub color: Color,
    pub glyph: &'static str,
}

/// 卡片 token 集
#[derive(Debug, Clone)]
pub struct CardTokens {
    pub user: CardToken,
    pub reasoning: CardToken,
    pub streaming: CardToken,
    pub task: CardToken,
    pub tool: CardToken,
    pub plan: CardToken,
    pub diff: CardToken,
    pub error: CardToken,
    pub warn: CardToken,
    pub usage: CardToken,
    pub subagent: CardToken,
    pub approval: CardToken,
    pub search: CardToken,
    pub memory: CardToken,
    pub ctx: CardToken,
    pub doctor: CardToken,
    pub branch: CardToken,
}

/// Pill section 颜色
#[derive(Debug, Clone)]
pub struct PillSectionToken {
    pub bg: Color,
    pub fg: Color,
}

/// Pill 颜色 token
#[derive(Debug, Clone)]
pub struct PillTokens {
    pub bg: Color,
    pub section_reason: PillSectionToken,
    pub section_output: PillSectionToken,
    pub section_tool: PillSectionToken,
    pub section_shell: PillSectionToken,
    pub section_task: PillSectionToken,
    pub section_task_done: PillSectionToken,
    pub section_task_failed: PillSectionToken,
    pub section_plan: PillSectionToken,
    pub section_user: PillSectionToken,
    pub section_empty: PillSectionToken,
    pub path: PillSectionToken,
    pub model_flash: PillSectionToken,
    pub model_pro: PillSectionToken,
    pub model_r1: PillSectionToken,
    pub model_unknown: PillSectionToken,
}

/// 消息背景 token
#[derive(Debug, Clone)]
pub struct MessageBgTokens {
    pub user: Color,
    pub bash: Color,
    pub selected: Color,
}

/// 完整主题 token 集
#[derive(Debug, Clone)]
pub struct ThemeTokens {
    pub fg: FgTokens,
    pub surface: SurfaceTokens,
    pub tone: ToneTokens,
    pub tone_active: ToneTokens,
    pub card: CardTokens,
    pub pill: PillTokens,
    pub message_bg: MessageBgTokens,
}

// ── 主题预设名称 ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemePreset {
    /// Reasonix graphite（深色，默认）
    Graphite,
    /// Reasonix porcelain（浅色）
    Porcelain,
    /// 传统 dark
    Dark,
    /// 传统 light
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
            ThemePreset::Graphite => write!(f, "graphite"),
            ThemePreset::Porcelain => write!(f, "porcelain"),
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
            "graphite" => Ok(ThemePreset::Graphite),
            "porcelain" => Ok(ThemePreset::Porcelain),
            "dark" => Ok(ThemePreset::Dark),
            "light" => Ok(ThemePreset::Light),
            "high-contrast" | "high_contrast" | "highcontrast" => Ok(ThemePreset::HighContrast),
            "nord" => Ok(ThemePreset::Nord),
            "dracula" => Ok(ThemePreset::Dracula),
            "gruvbox-dark" | "gruvbox_dark" | "gruvbox" => Ok(ThemePreset::GruvboxDark),
            "catppuccin-mocha" | "catppuccin_mocha" | "catppuccin" => {
                Ok(ThemePreset::CatppuccinMocha)
            }
            _ => Err(format!("Unknown theme preset: {}", s)),
        }
    }
}

/// 完整主题（保留旧扁平字段 + 新 token 层）
#[derive(Debug, Clone)]
pub struct Theme {
    // ── Token 层（新代码通过这里访问） ──
    pub tokens: ThemeTokens,

    // ── 旧扁平字段（兼容，逐步迁移） ──
    pub bg: Color,
    pub bg_popup: Color,
    pub bg_selected: Color,
    pub text: Color,
    pub text_dim: Color,
    pub text_highlight: Color,
    pub border: Color,
    pub border_active: Color,
    pub user_message: Color,
    pub user_message_bg: Color,
    pub assistant_message: Color,
    pub system_message: Color,
    pub tool_message: Color,
    pub success: Color,
    pub error: Color,
    pub warning: Color,
    pub info: Color,
    pub diff_add: Color,
    pub diff_remove: Color,
    pub diff_header: Color,
    pub diff_line_number: Color,
    pub status_ready: Color,
    pub status_thinking: Color,
    pub status_vim: Color,
    pub status_worktree: Color,
}

// ── Token 构建辅助 ──

fn card_tokens(tone: &ToneTokens) -> CardTokens {
    CardTokens {
        user: CardToken { color: tone.brand, glyph: "◇" },
        reasoning: CardToken { color: tone.accent, glyph: "◆" },
        streaming: CardToken { color: tone.brand, glyph: "◈" },
        task: CardToken { color: tone.warn, glyph: "▶" },
        tool: CardToken { color: tone.info, glyph: "▣" },
        plan: CardToken { color: tone.accent, glyph: "⊞" },
        diff: CardToken { color: tone.ok, glyph: "±" },
        error: CardToken { color: tone.err, glyph: "✖" },
        warn: CardToken { color: tone.warn, glyph: "⚠" },
        usage: CardToken { color: tone.info, glyph: "Σ" }, // will be overridden
        subagent: CardToken { color: tone.violet, glyph: "⌬" },
        approval: CardToken { color: tone.warn, glyph: "?" },
        search: CardToken { color: tone.info, glyph: "⊙" },
        memory: CardToken { color: tone.info, glyph: "⌑" }, // will be overridden
        ctx: CardToken { color: tone.brand, glyph: "◔" },
        doctor: CardToken { color: tone.info, glyph: "⚕" }, // will be overridden
        branch: CardToken { color: tone.violet, glyph: "⎇" },
    }
}

fn pill_tokens(surface: &SurfaceTokens, tone: &ToneTokens, _fg: &FgTokens) -> PillTokens {
    let bg = surface.bg_elev;
    PillTokens {
        bg,
        section_reason: PillSectionToken { bg, fg: tone.violet },
        section_output: PillSectionToken { bg, fg: tone.info },
        section_tool: PillSectionToken { bg, fg: tone.info },
        section_shell: PillSectionToken { bg, fg: tone.info },
        section_task: PillSectionToken { bg, fg: tone.info },
        section_task_done: PillSectionToken { bg, fg: tone.ok },
        section_task_failed: PillSectionToken { bg, fg: tone.err },
        section_plan: PillSectionToken { bg, fg: tone.violet },
        section_user: PillSectionToken { bg, fg: tone.brand },
        section_empty: PillSectionToken { bg, fg: Color::Gray },
        path: PillSectionToken { bg, fg: Color::Gray },
        model_flash: PillSectionToken { bg, fg: tone.info },
        model_pro: PillSectionToken { bg, fg: tone.violet },
        model_r1: PillSectionToken { bg, fg: tone.accent },
        model_unknown: PillSectionToken { bg, fg: Color::Gray },
    }
}

fn flat_from_tokens(tokens: &ThemeTokens) -> (Color, Color, Color, Color, Color, Color, Color, Color, Color, Color, Color, Color, Color, Color, Color, Color, Color, Color, Color, Color, Color, Color, Color, Color, Color) {
    (
        tokens.surface.bg,
        tokens.surface.bg_elev,
        tokens.surface.bg_input,
        tokens.fg.body,
        tokens.fg.faint,
        tokens.fg.strong,
        tokens.fg.meta,        // border
        tokens.tone.brand,     // border_active
        tokens.tone.brand,     // user_message
        tokens.message_bg.user,
        tokens.tone.ok,        // assistant_message
        tokens.tone.warn,      // system_message
        tokens.tone.info,      // tool_message
        tokens.tone.ok,        // success
        tokens.tone.err,       // error
        tokens.tone.warn,      // warning
        tokens.tone.info,      // info
        tokens.tone.ok,        // diff_add
        tokens.tone.err,       // diff_remove
        tokens.tone.warn,      // diff_header
        tokens.fg.meta,        // diff_line_number
        tokens.tone.ok,        // status_ready
        tokens.tone.warn,      // status_thinking
        tokens.tone.violet,    // status_vim
        tokens.tone.brand,     // status_worktree
    )
}

impl Theme {
    /// 从 tokens 构建 Theme（填充旧扁平字段作为兼容层）
    fn from_tokens(tokens: ThemeTokens) -> Self {
        let (bg, bg_popup, bg_selected, text, text_dim, text_highlight, border, border_active,
             user_message, user_message_bg, assistant_message, system_message, tool_message,
             success, error, warning, info, diff_add, diff_remove, diff_header, diff_line_number,
             status_ready, status_thinking, status_vim, status_worktree) = flat_from_tokens(&tokens);
        Self {
            tokens,
            bg, bg_popup, bg_selected,
            text, text_dim, text_highlight,
            border, border_active,
            user_message, user_message_bg,
            assistant_message, system_message, tool_message,
            success, error, warning, info,
            diff_add, diff_remove, diff_header, diff_line_number,
            status_ready, status_thinking, status_vim, status_worktree,
        }
    }

    /// 根据预设名称获取主题
    pub fn from_preset(preset: ThemePreset) -> Self {
        match preset {
            ThemePreset::Graphite => Self::graphite(),
            ThemePreset::Porcelain => Self::porcelain(),
            ThemePreset::Dark => Self::dark(),
            ThemePreset::Light => Self::light(),
            ThemePreset::HighContrast => Self::high_contrast(),
            ThemePreset::Nord => Self::nord(),
            ThemePreset::Dracula => Self::dracula(),
            ThemePreset::GruvboxDark => Self::gruvbox_dark(),
            ThemePreset::CatppuccinMocha => Self::catppuccin_mocha(),
        }
    }

    // ── 新主题（Reasonix 色值） ──

    /// Graphite — Reasonix 默认深色主题
    pub fn graphite() -> Self {
        let tokens = ThemeTokens {
            fg: FgTokens {
                strong: Color::Rgb(0xf4, 0xf7, 0xfb),
                body:   Color::Rgb(0xd8, 0xde, 0xe9),
                sub:    Color::Rgb(0xa7, 0xb1, 0xc2),
                meta:   Color::Rgb(0x9a, 0xa5, 0xb5),
                faint:  Color::Rgb(0x87, 0x91, 0xa3),
            },
            surface: SurfaceTokens {
                bg:       Color::Rgb(0x0b, 0x10, 0x20),
                bg_input: Color::Rgb(0x0f, 0x17, 0x2a),
                bg_code:  Color::Rgb(0x08, 0x0c, 0x16),
                bg_elev:  Color::Rgb(0x1c, 0x28, 0x44),
            },
            tone: ToneTokens {
                brand:  Color::Rgb(0x7d, 0xd3, 0xfc),
                accent: Color::Rgb(0xc0, 0x84, 0xfc),
                violet: Color::Rgb(0xa7, 0x8b, 0xfa),
                ok:     Color::Rgb(0x86, 0xef, 0xac),
                warn:   Color::Rgb(0xfb, 0xbf, 0x24),
                err:    Color::Rgb(0xf8, 0x71, 0x71),
                info:   Color::Rgb(0x60, 0xa5, 0xfa),
            },
            tone_active: ToneTokens {
                brand:  Color::Rgb(0xba, 0xe6, 0xfd),
                accent: Color::Rgb(0xe9, 0xd5, 0xff),
                violet: Color::Rgb(0xdd, 0xd6, 0xfe),
                ok:     Color::Rgb(0xbb, 0xf7, 0xd0),
                warn:   Color::Rgb(0xfd, 0xe6, 0x8a),
                err:    Color::Rgb(0xfe, 0xca, 0xca),
                info:   Color::Rgb(0xbf, 0xdb, 0xfe),
            },
            card: card_tokens(&ToneTokens {
                brand: Color::Rgb(0x7d, 0xd3, 0xfc), accent: Color::Rgb(0xc0, 0x84, 0xfc),
                violet: Color::Rgb(0xa7, 0x8b, 0xfa), ok: Color::Rgb(0x86, 0xef, 0xac),
                warn: Color::Rgb(0xfb, 0xbf, 0x24), err: Color::Rgb(0xf8, 0x71, 0x71),
                info: Color::Rgb(0x60, 0xa5, 0xfa),
            }),
            pill: pill_tokens(
                &SurfaceTokens { bg: Color::Rgb(0x0b, 0x10, 0x20), bg_input: Color::Rgb(0x0f, 0x17, 0x2a), bg_code: Color::Rgb(0x08, 0x0c, 0x16), bg_elev: Color::Rgb(0x1c, 0x28, 0x44) },
                &ToneTokens {
                    brand: Color::Rgb(0x7d, 0xd3, 0xfc), accent: Color::Rgb(0xc0, 0x84, 0xfc),
                    violet: Color::Rgb(0xa7, 0x8b, 0xfa), ok: Color::Rgb(0x86, 0xef, 0xac),
                    warn: Color::Rgb(0xfb, 0xbf, 0x24), err: Color::Rgb(0xf8, 0x71, 0x71),
                    info: Color::Rgb(0x60, 0xa5, 0xfa),
                },
                &FgTokens { strong: Color::Rgb(0xf4, 0xf7, 0xfb), body: Color::Rgb(0xd8, 0xde, 0xe9), sub: Color::Rgb(0xa7, 0xb1, 0xc2), meta: Color::Rgb(0x9a, 0xa5, 0xb5), faint: Color::Rgb(0x87, 0x91, 0xa3) },
            ),
            message_bg: MessageBgTokens {
                user: Color::Rgb(0x37, 0x37, 0x37),
                bash: Color::Rgb(0x41, 0x3c, 0x41),
                selected: Color::Rgb(0x2c, 0x32, 0x3e),
            },
        };
        Self::from_tokens(tokens)
    }

    /// Porcelain — Reasonix 浅色主题
    pub fn porcelain() -> Self {
        let tokens = ThemeTokens {
            fg: FgTokens {
                strong: Color::Rgb(0x11, 0x18, 0x27),
                body:   Color::Rgb(0x1f, 0x29, 0x37),
                sub:    Color::Rgb(0x4b, 0x55, 0x63),
                meta:   Color::Rgb(0x5c, 0x63, 0x71),
                faint:  Color::Rgb(0x66, 0x6d, 0x7b),
            },
            surface: SurfaceTokens {
                bg:       Color::Rgb(0xff, 0xff, 0xff),
                bg_input: Color::Rgb(0xf1, 0xf5, 0xf9),
                bg_code:  Color::Rgb(0xf3, 0xf4, 0xf6),
                bg_elev:  Color::Rgb(0xee, 0xf2, 0xf7),
            },
            tone: ToneTokens {
                brand:  Color::Rgb(0x25, 0x63, 0xeb),
                accent: Color::Rgb(0x7c, 0x3a, 0xed),
                violet: Color::Rgb(0x6d, 0x28, 0xd9),
                ok:     Color::Rgb(0x15, 0x80, 0x3d),
                warn:   Color::Rgb(0xb4, 0x53, 0x09),
                err:    Color::Rgb(0xdc, 0x26, 0x26),
                info:   Color::Rgb(0x03, 0x69, 0xa1),
            },
            tone_active: ToneTokens {
                brand:  Color::Rgb(0x1d, 0x4e, 0xd8),
                accent: Color::Rgb(0x6d, 0x28, 0xd9),
                violet: Color::Rgb(0x5b, 0x21, 0xb6),
                ok:     Color::Rgb(0x16, 0x65, 0x34),
                warn:   Color::Rgb(0x92, 0x40, 0x0e),
                err:    Color::Rgb(0xb9, 0x1c, 0x1c),
                info:   Color::Rgb(0x07, 0x59, 0x85),
            },
            card: card_tokens(&ToneTokens {
                brand: Color::Rgb(0x25, 0x63, 0xeb), accent: Color::Rgb(0x7c, 0x3a, 0xed),
                violet: Color::Rgb(0x6d, 0x28, 0xd9), ok: Color::Rgb(0x15, 0x80, 0x3d),
                warn: Color::Rgb(0xb4, 0x53, 0x09), err: Color::Rgb(0xdc, 0x26, 0x26),
                info: Color::Rgb(0x03, 0x69, 0xa1),
            }),
            pill: pill_tokens(
                &SurfaceTokens { bg: Color::Rgb(0xff, 0xff, 0xff), bg_input: Color::Rgb(0xf1, 0xf5, 0xf9), bg_code: Color::Rgb(0xf3, 0xf4, 0xf6), bg_elev: Color::Rgb(0xee, 0xf2, 0xf7) },
                &ToneTokens {
                    brand: Color::Rgb(0x25, 0x63, 0xeb), accent: Color::Rgb(0x7c, 0x3a, 0xed),
                    violet: Color::Rgb(0x6d, 0x28, 0xd9), ok: Color::Rgb(0x15, 0x80, 0x3d),
                    warn: Color::Rgb(0xb4, 0x53, 0x09), err: Color::Rgb(0xdc, 0x26, 0x26),
                    info: Color::Rgb(0x03, 0x69, 0xa1),
                },
                &FgTokens { strong: Color::Rgb(0x11, 0x18, 0x27), body: Color::Rgb(0x1f, 0x29, 0x37), sub: Color::Rgb(0x4b, 0x55, 0x63), meta: Color::Rgb(0x5c, 0x63, 0x71), faint: Color::Rgb(0x66, 0x6d, 0x7b) },
            ),
            message_bg: MessageBgTokens {
                user: Color::Rgb(0xe5, 0xe7, 0xeb),
                bash: Color::Rgb(0xf5, 0xe0, 0xe9),
                selected: Color::Rgb(0xdd, 0xe6, 0xf5),
            },
        };
        Self::from_tokens(tokens)
    }

    // ── 传统主题（内部使用 token 构建） ──

    fn legacy_dark_tokens() -> ThemeTokens {
        ThemeTokens {
            fg: FgTokens {
                strong: Color::White, body: Color::White,
                sub: Color::Gray, meta: Color::DarkGray, faint: Color::Gray,
            },
            surface: SurfaceTokens {
                bg: Color::Reset, bg_input: Color::Black, bg_code: Color::Black, bg_elev: Color::DarkGray,
            },
            tone: ToneTokens {
                brand: Color::Cyan, accent: Color::Cyan, violet: Color::Magenta,
                ok: Color::Green, warn: Color::Yellow, err: Color::Red, info: Color::Cyan,
            },
            tone_active: ToneTokens {
                brand: Color::Cyan, accent: Color::Cyan, violet: Color::Magenta,
                ok: Color::Green, warn: Color::Yellow, err: Color::Red, info: Color::Cyan,
            },
            card: card_tokens(&ToneTokens {
                brand: Color::Cyan, accent: Color::Cyan, violet: Color::Magenta,
                ok: Color::Green, warn: Color::Yellow, err: Color::Red, info: Color::Cyan,
            }),
            pill: pill_tokens(&SurfaceTokens { bg: Color::Reset, bg_input: Color::Black, bg_code: Color::Black, bg_elev: Color::DarkGray }, &ToneTokens { brand: Color::Cyan, accent: Color::Cyan, violet: Color::Magenta, ok: Color::Green, warn: Color::Yellow, err: Color::Red, info: Color::Cyan }, &FgTokens { strong: Color::White, body: Color::White, sub: Color::Gray, meta: Color::DarkGray, faint: Color::Gray }),
            message_bg: MessageBgTokens { user: Color::Rgb(55,55,55), bash: Color::Rgb(55,55,55), selected: Color::DarkGray },
        }
    }

    /// Dark 主题
    pub fn dark() -> Self { Self::from_tokens(Self::legacy_dark_tokens()) }
    /// Light 主题
    pub fn light() -> Self {
        let mut t = Self::legacy_dark_tokens();
        t.surface = SurfaceTokens { bg: Color::White, bg_input: Color::Rgb(245,245,245), bg_code: Color::Rgb(245,245,245), bg_elev: Color::Rgb(220,220,220) };
        t.fg = FgTokens { strong: Color::Black, body: Color::Black, sub: Color::Rgb(95,95,95), meta: Color::Rgb(120,120,120), faint: Color::Rgb(150,150,150) };
        t.message_bg = MessageBgTokens { user: Color::Rgb(246,246,246), bash: Color::Rgb(246,246,246), selected: Color::Rgb(220,220,220) };
        Self::from_tokens(t)
    }
    /// High-Contrast 主题
    pub fn high_contrast() -> Self {
        let mut t = Self::legacy_dark_tokens();
        t.fg.strong = Color::White; t.fg.body = Color::White; t.fg.faint = Color::Rgb(200,200,200);
        t.surface.bg = Color::Black; t.surface.bg_elev = Color::White;
        t.tone.brand = Color::Yellow;
        Self::from_tokens(t)
    }
    /// Nord 主题
    pub fn nord() -> Self {
        let tokens = ThemeTokens {
            fg: FgTokens { strong: Color::Rgb(216,222,233), body: Color::Rgb(216,222,233), sub: Color::Rgb(143,188,187), meta: Color::Rgb(76,86,106), faint: Color::Rgb(76,86,106) },
            surface: SurfaceTokens { bg: Color::Rgb(46,52,64), bg_input: Color::Rgb(59,66,82), bg_code: Color::Rgb(46,52,64), bg_elev: Color::Rgb(67,76,94) },
            tone: ToneTokens { brand: Color::Rgb(136,192,208), accent: Color::Rgb(180,142,173), violet: Color::Rgb(180,142,173), ok: Color::Rgb(163,190,140), warn: Color::Rgb(235,203,139), err: Color::Rgb(191,97,106), info: Color::Rgb(136,192,208) },
            tone_active: ToneTokens { brand: Color::Rgb(136,192,208), accent: Color::Rgb(180,142,173), violet: Color::Rgb(180,142,173), ok: Color::Rgb(163,190,140), warn: Color::Rgb(235,203,139), err: Color::Rgb(191,97,106), info: Color::Rgb(136,192,208) },
            card: card_tokens(&ToneTokens { brand: Color::Rgb(136,192,208), accent: Color::Rgb(180,142,173), violet: Color::Rgb(180,142,173), ok: Color::Rgb(163,190,140), warn: Color::Rgb(235,203,139), err: Color::Rgb(191,97,106), info: Color::Rgb(136,192,208) }),
            pill: pill_tokens(&SurfaceTokens { bg: Color::Rgb(46,52,64), bg_input: Color::Rgb(59,66,82), bg_code: Color::Rgb(46,52,64), bg_elev: Color::Rgb(67,76,94) }, &ToneTokens { brand: Color::Rgb(136,192,208), accent: Color::Rgb(180,142,173), violet: Color::Rgb(180,142,173), ok: Color::Rgb(163,190,140), warn: Color::Rgb(235,203,139), err: Color::Rgb(191,97,106), info: Color::Rgb(136,192,208) }, &FgTokens { strong: Color::Rgb(216,222,233), body: Color::Rgb(216,222,233), sub: Color::Rgb(143,188,187), meta: Color::Rgb(76,86,106), faint: Color::Rgb(76,86,106) }),
            message_bg: MessageBgTokens { user: Color::Rgb(59,66,82), bash: Color::Rgb(59,66,82), selected: Color::Rgb(67,76,94) },
        };
        Self::from_tokens(tokens)
    }
    /// Dracula 主题
    pub fn dracula() -> Self {
        let tokens = ThemeTokens {
            fg: FgTokens { strong: Color::Rgb(248,248,242), body: Color::Rgb(248,248,242), sub: Color::Rgb(98,114,164), meta: Color::Rgb(98,114,164), faint: Color::Rgb(98,114,164) },
            surface: SurfaceTokens { bg: Color::Rgb(40,42,54), bg_input: Color::Rgb(68,71,90), bg_code: Color::Rgb(40,42,54), bg_elev: Color::Rgb(98,114,164) },
            tone: ToneTokens { brand: Color::Rgb(139,233,253), accent: Color::Rgb(189,147,249), violet: Color::Rgb(189,147,249), ok: Color::Rgb(80,250,123), warn: Color::Rgb(241,250,140), err: Color::Rgb(255,85,85), info: Color::Rgb(139,233,253) },
            tone_active: ToneTokens { brand: Color::Rgb(139,233,253), accent: Color::Rgb(189,147,249), violet: Color::Rgb(189,147,249), ok: Color::Rgb(80,250,123), warn: Color::Rgb(241,250,140), err: Color::Rgb(255,85,85), info: Color::Rgb(139,233,253) },
            card: card_tokens(&ToneTokens { brand: Color::Rgb(139,233,253), accent: Color::Rgb(189,147,249), violet: Color::Rgb(189,147,249), ok: Color::Rgb(80,250,123), warn: Color::Rgb(241,250,140), err: Color::Rgb(255,85,85), info: Color::Rgb(139,233,253) }),
            pill: pill_tokens(&SurfaceTokens { bg: Color::Rgb(40,42,54), bg_input: Color::Rgb(68,71,90), bg_code: Color::Rgb(40,42,54), bg_elev: Color::Rgb(98,114,164) }, &ToneTokens { brand: Color::Rgb(139,233,253), accent: Color::Rgb(189,147,249), violet: Color::Rgb(189,147,249), ok: Color::Rgb(80,250,123), warn: Color::Rgb(241,250,140), err: Color::Rgb(255,85,85), info: Color::Rgb(139,233,253) }, &FgTokens { strong: Color::Rgb(248,248,242), body: Color::Rgb(248,248,242), sub: Color::Rgb(98,114,164), meta: Color::Rgb(98,114,164), faint: Color::Rgb(98,114,164) }),
            message_bg: MessageBgTokens { user: Color::Rgb(50,50,65), bash: Color::Rgb(50,50,65), selected: Color::Rgb(98,114,164) },
        };
        Self::from_tokens(tokens)
    }
    /// Gruvbox Dark
    pub fn gruvbox_dark() -> Self {
        let theme = Self {
            bg: Color::Rgb(40, 40, 40), bg_popup: Color::Rgb(60, 56, 54), bg_selected: Color::Rgb(80, 73, 69),
            text: Color::Rgb(235, 219, 178), text_dim: Color::Rgb(146, 131, 116), text_highlight: Color::Rgb(131, 165, 152),
            border: Color::Rgb(146, 131, 116), border_active: Color::Rgb(254, 128, 25),
            user_message: Color::Rgb(131, 165, 152), user_message_bg: Color::Rgb(60, 56, 54),
            assistant_message: Color::Rgb(184, 187, 38), system_message: Color::Rgb(250, 189, 47),
            tool_message: Color::Rgb(211, 134, 155),
            success: Color::Rgb(184, 187, 38), error: Color::Rgb(251, 73, 52),
            warning: Color::Rgb(250, 189, 47), info: Color::Rgb(131, 165, 152),
            diff_add: Color::Rgb(184, 187, 38), diff_remove: Color::Rgb(251, 73, 52),
            diff_header: Color::Rgb(250, 189, 47), diff_line_number: Color::Rgb(146, 131, 116),
            status_ready: Color::Rgb(184, 187, 38), status_thinking: Color::Rgb(250, 189, 47),
            status_vim: Color::Rgb(211, 134, 155), status_worktree: Color::Rgb(131, 165, 152),
            tokens: ThemeTokens {
                fg: FgTokens { strong: Color::Rgb(235,219,178), body: Color::Rgb(235,219,178), sub: Color::Rgb(146,131,116), meta: Color::Rgb(146,131,116), faint: Color::Rgb(146,131,116) },
                surface: SurfaceTokens { bg: Color::Rgb(40,40,40), bg_input: Color::Rgb(60,56,54), bg_code: Color::Rgb(40,40,40), bg_elev: Color::Rgb(80,73,69) },
                tone: ToneTokens { brand: Color::Rgb(131,165,152), accent: Color::Rgb(254,128,25), violet: Color::Rgb(211,134,155), ok: Color::Rgb(184,187,38), warn: Color::Rgb(250,189,47), err: Color::Rgb(251,73,52), info: Color::Rgb(131,165,152) },
                tone_active: ToneTokens { brand: Color::Rgb(131,165,152), accent: Color::Rgb(254,128,25), violet: Color::Rgb(211,134,155), ok: Color::Rgb(184,187,38), warn: Color::Rgb(250,189,47), err: Color::Rgb(251,73,52), info: Color::Rgb(131,165,152) },
                card: card_tokens(&ToneTokens { brand: Color::Rgb(131,165,152), accent: Color::Rgb(254,128,25), violet: Color::Rgb(211,134,155), ok: Color::Rgb(184,187,38), warn: Color::Rgb(250,189,47), err: Color::Rgb(251,73,52), info: Color::Rgb(131,165,152) }),
                pill: pill_tokens(&SurfaceTokens { bg: Color::Rgb(40,40,40), bg_input: Color::Rgb(60,56,54), bg_code: Color::Rgb(40,40,40), bg_elev: Color::Rgb(80,73,69) }, &ToneTokens { brand: Color::Rgb(131,165,152), accent: Color::Rgb(254,128,25), violet: Color::Rgb(211,134,155), ok: Color::Rgb(184,187,38), warn: Color::Rgb(250,189,47), err: Color::Rgb(251,73,52), info: Color::Rgb(131,165,152) }, &FgTokens { strong: Color::Rgb(235,219,178), body: Color::Rgb(235,219,178), sub: Color::Rgb(146,131,116), meta: Color::Rgb(146,131,116), faint: Color::Rgb(146,131,116) }),
                message_bg: MessageBgTokens { user: Color::Rgb(60,56,54), bash: Color::Rgb(60,56,54), selected: Color::Rgb(80,73,69) },
            },
        };
        theme
    }
    /// Catppuccin Mocha
    pub fn catppuccin_mocha() -> Self {
        let theme = Self {
            bg: Color::Rgb(30, 30, 46), bg_popup: Color::Rgb(49, 50, 68), bg_selected: Color::Rgb(69, 71, 90),
            text: Color::Rgb(205, 214, 244), text_dim: Color::Rgb(108, 112, 134), text_highlight: Color::Rgb(137, 180, 250),
            border: Color::Rgb(108, 112, 134), border_active: Color::Rgb(137, 180, 250),
            user_message: Color::Rgb(137, 180, 250), user_message_bg: Color::Rgb(49, 50, 68),
            assistant_message: Color::Rgb(166, 227, 161), system_message: Color::Rgb(249, 226, 175),
            tool_message: Color::Rgb(245, 194, 231),
            success: Color::Rgb(166, 227, 161), error: Color::Rgb(243, 139, 168),
            warning: Color::Rgb(249, 226, 175), info: Color::Rgb(137, 180, 250),
            diff_add: Color::Rgb(166, 227, 161), diff_remove: Color::Rgb(243, 139, 168),
            diff_header: Color::Rgb(249, 226, 175), diff_line_number: Color::Rgb(108, 112, 134),
            status_ready: Color::Rgb(166, 227, 161), status_thinking: Color::Rgb(249, 226, 175),
            status_vim: Color::Rgb(245, 194, 231), status_worktree: Color::Rgb(137, 180, 250),
            tokens: ThemeTokens {
                fg: FgTokens { strong: Color::Rgb(205,214,244), body: Color::Rgb(205,214,244), sub: Color::Rgb(108,112,134), meta: Color::Rgb(108,112,134), faint: Color::Rgb(108,112,134) },
                surface: SurfaceTokens { bg: Color::Rgb(30,30,46), bg_input: Color::Rgb(49,50,68), bg_code: Color::Rgb(30,30,46), bg_elev: Color::Rgb(69,71,90) },
                tone: ToneTokens { brand: Color::Rgb(137,180,250), accent: Color::Rgb(245,194,231), violet: Color::Rgb(245,194,231), ok: Color::Rgb(166,227,161), warn: Color::Rgb(249,226,175), err: Color::Rgb(243,139,168), info: Color::Rgb(137,180,250) },
                tone_active: ToneTokens { brand: Color::Rgb(137,180,250), accent: Color::Rgb(245,194,231), violet: Color::Rgb(245,194,231), ok: Color::Rgb(166,227,161), warn: Color::Rgb(249,226,175), err: Color::Rgb(243,139,168), info: Color::Rgb(137,180,250) },
                card: card_tokens(&ToneTokens { brand: Color::Rgb(137,180,250), accent: Color::Rgb(245,194,231), violet: Color::Rgb(245,194,231), ok: Color::Rgb(166,227,161), warn: Color::Rgb(249,226,175), err: Color::Rgb(243,139,168), info: Color::Rgb(137,180,250) }),
                pill: pill_tokens(&SurfaceTokens { bg: Color::Rgb(30,30,46), bg_input: Color::Rgb(49,50,68), bg_code: Color::Rgb(30,30,46), bg_elev: Color::Rgb(69,71,90) }, &ToneTokens { brand: Color::Rgb(137,180,250), accent: Color::Rgb(245,194,231), violet: Color::Rgb(245,194,231), ok: Color::Rgb(166,227,161), warn: Color::Rgb(249,226,175), err: Color::Rgb(243,139,168), info: Color::Rgb(137,180,250) }, &FgTokens { strong: Color::Rgb(205,214,244), body: Color::Rgb(205,214,244), sub: Color::Rgb(108,112,134), meta: Color::Rgb(108,112,134), faint: Color::Rgb(108,112,134) }),
                message_bg: MessageBgTokens { user: Color::Rgb(49,50,68), bash: Color::Rgb(49,50,68), selected: Color::Rgb(69,71,90) },
            },
        };
        theme
    }

    /// 从字符串解析
    pub fn from_name(name: &str) -> Self {
        match ThemePreset::from_str(name) {
            Ok(preset) => Self::from_preset(preset),
            Err(_) => Self::graphite(),
        }
    }

    /// 判断是否为暗色主题
    pub fn is_dark(&self) -> bool {
        !matches!(
            self.bg,
            Color::White
                | Color::Rgb(245, 245, 245)
                | Color::Rgb(255, 255, 255)
                | Color::Rgb(250, 250, 250)
        )
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::graphite()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_presets() {
        let g = Theme::graphite();
        assert_eq!(g.tokens.surface.bg, Color::Rgb(0x0b, 0x10, 0x20));
        let p = Theme::porcelain();
        assert_eq!(p.tokens.surface.bg, Color::Rgb(0xff, 0xff, 0xff));
        let hc = Theme::high_contrast();
        assert_eq!(hc.tokens.fg.body, Color::White);
    }

    #[test]
    fn test_theme_from_name() {
        let g = Theme::from_name("graphite");
        assert_eq!(g.tokens.fg.body, Color::Rgb(0xd8, 0xde, 0xe9));
        assert_eq!(Theme::from_name("unknown").tokens.fg.body, Color::Rgb(0xd8, 0xde, 0xe9)); // fallback to graphite
    }

    #[test]
    fn test_flat_compat() {
        for theme in all_themes() {
            assert_eq!(theme.bg, theme.tokens.surface.bg, "bg mismatch");
            assert_eq!(theme.text, theme.tokens.fg.body, "text mismatch");
            assert_eq!(theme.text_dim, theme.tokens.fg.faint, "text_dim mismatch");
            assert_eq!(theme.success, theme.tokens.tone.ok, "success mismatch");
            assert_eq!(theme.error, theme.tokens.tone.err, "error mismatch");
            assert_eq!(theme.warning, theme.tokens.tone.warn, "warning mismatch");
            assert_eq!(theme.info, theme.tokens.tone.info, "info mismatch");
            assert_eq!(theme.user_message_bg, theme.tokens.message_bg.user, "user_message_bg mismatch");
        }
    }

    fn all_themes() -> Vec<Theme> {
        vec![
            Theme::graphite(),
            Theme::porcelain(),
            Theme::dark(),
            Theme::light(),
            Theme::high_contrast(),
            Theme::nord(),
            Theme::dracula(),
            Theme::gruvbox_dark(),
            Theme::catppuccin_mocha(),
        ]
    }

    #[test]
    fn test_theme_preset_parse() {
        assert_eq!("graphite".parse::<ThemePreset>().unwrap(), ThemePreset::Graphite);
        assert_eq!("porcelain".parse::<ThemePreset>().unwrap(), ThemePreset::Porcelain);
        assert_eq!("nord".parse::<ThemePreset>().unwrap(), ThemePreset::Nord);
        assert_eq!("dark".parse::<ThemePreset>().unwrap(), ThemePreset::Dark);
        assert!("unknown".parse::<ThemePreset>().is_err());
    }

    #[test]
    fn test_card_tokens_graphite() {
        let g = Theme::graphite();
        assert_eq!(g.tokens.card.user.glyph, "◇");
        assert_eq!(g.tokens.card.tool.glyph, "▣");
        assert_eq!(g.tokens.card.streaming.glyph, "◈");
        assert_eq!(g.tokens.card.error.glyph, "✖");
        assert_eq!(g.tokens.card.search.glyph, "⊙");
    }
}