//! 可配置键位系统
//!
//! 支持从 TOML 文件加载自定义键位，并提供默认键位映射

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};

/// 单键位定义
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyBinding {
    pub modifiers: KeyModifiers,
    pub code: KeyCode,
}

impl KeyBinding {
    pub fn parse(s: &str) -> Result<Self, String> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err("Empty keybinding".to_string());
        }

        let parts: Vec<&str> = trimmed.split('+').collect();
        let mut modifiers = KeyModifiers::empty();
        let key_part = if parts.len() > 1 {
            for part in &parts[..parts.len() - 1] {
                match part.trim().to_ascii_lowercase().as_str() {
                    "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
                    "shift" => modifiers |= KeyModifiers::SHIFT,
                    "alt" => modifiers |= KeyModifiers::ALT,
                    _ => return Err(format!("Unknown modifier: {}", part)),
                }
            }
            parts.last().expect("keybinding has no parts").trim()
        } else {
            parts[0].trim()
        };

        let code = if key_part.len() == 1
            && key_part
                .chars()
                .next()
                .expect("empty key part")
                .is_ascii_graphic()
        {
            KeyCode::Char(key_part.chars().next().expect("empty key part"))
        } else {
            match key_part.to_ascii_lowercase().as_str() {
                "enter" | "return" => KeyCode::Enter,
                "esc" | "escape" => KeyCode::Esc,
                "tab" => KeyCode::Tab,
                "backspace" => KeyCode::Backspace,
                "delete" | "del" => KeyCode::Delete,
                "up" => KeyCode::Up,
                "down" => KeyCode::Down,
                "left" => KeyCode::Left,
                "right" => KeyCode::Right,
                "home" => KeyCode::Home,
                "end" => KeyCode::End,
                "space" => KeyCode::Char(' '),
                "pageup" => KeyCode::PageUp,
                "pagedown" => KeyCode::PageDown,
                "backslash" => KeyCode::Char('\\'),
                "comma" => KeyCode::Char(','),
                "f1" => KeyCode::F(1),
                "f2" => KeyCode::F(2),
                "f3" => KeyCode::F(3),
                "f4" => KeyCode::F(4),
                "f5" => KeyCode::F(5),
                "f6" => KeyCode::F(6),
                "f7" => KeyCode::F(7),
                "f8" => KeyCode::F(8),
                "f9" => KeyCode::F(9),
                "f10" => KeyCode::F(10),
                "f11" => KeyCode::F(11),
                "f12" => KeyCode::F(12),
                _ => return Err(format!("Unknown key: {}", key_part)),
            }
        };

        Ok(KeyBinding { modifiers, code })
    }

    pub fn matches(&self, event: KeyEvent) -> bool {
        self.modifiers == event.modifiers && self.code == event.code
    }
}

impl std::str::FromStr for KeyBinding {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl std::fmt::Display for KeyBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut prefix = String::new();
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            prefix.push_str("ctrl+");
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            prefix.push_str("shift+");
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            prefix.push_str("alt+");
        }
        let key_str = match self.code {
            KeyCode::Enter => "enter",
            KeyCode::Esc => "esc",
            KeyCode::Tab => "tab",
            KeyCode::Backspace => "backspace",
            KeyCode::Delete => "delete",
            KeyCode::Up => "up",
            KeyCode::Down => "down",
            KeyCode::Left => "left",
            KeyCode::Right => "right",
            KeyCode::Home => "home",
            KeyCode::End => "end",
            KeyCode::PageUp => "pageup",
            KeyCode::PageDown => "pagedown",
            KeyCode::Char(' ') => "space",
            KeyCode::Char(c) => return write!(f, "{}{}", prefix, c),
            KeyCode::F(n) => return write!(f, "{}f{}", prefix, n),
            _ => "unknown",
        };
        write!(f, "{}{}", prefix, key_str)
    }
}

/// 应用动作（抽象键位语义）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    None,
    Quit,
    Cancel,
    Submit,
    InsertNewline,
    ToggleVimMode,
    ScrollUp,
    ScrollDown,
    ScrollTop,
    ScrollBottom,
    VimInsert,
    VimCommand,
    PlanApprove,
    PlanReject,
    PlanModify,
    PermissionApprove,
    PermissionReject,
    PermissionViewDiff,
    SettingsSave,
    SettingsNextPage,
    SettingsPrevPage,
    SettingsNextItem,
    SettingsPrevItem,
    SettingsEdit,
    SettingsToggleBool,
    OpenCommandPalette,
    OpenPromptHistory,
    OpenModelSelect,
    OpenProviderSelect,
    OpenShortcutHelp,
    ToggleExpandDetails,
    OpenToolOutput,
    CycleStatusBarDensity,
    ToggleSidebar,
    OpenMessageSearch,
    LeaderPalette,
    LeaderSidebar,
    LeaderToolDiff,
    LeaderSessionCycle,
}

/// TOML 配置结构（字符串形式）
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct KeybindingsFile {
    #[serde(default)]
    pub global_quit: Option<String>,
    #[serde(default)]
    pub global_quit_alt: Option<String>,
    #[serde(default)]
    pub chat_submit: Option<String>,
    #[serde(default)]
    pub chat_newline: Option<String>,
    #[serde(default)]
    pub toggle_vim_mode: Option<String>,
    #[serde(default)]
    pub vim_scroll_up: Option<String>,
    #[serde(default)]
    pub vim_scroll_down: Option<String>,
    #[serde(default)]
    pub vim_scroll_top: Option<String>,
    #[serde(default)]
    pub vim_scroll_bottom: Option<String>,
    #[serde(default)]
    pub vim_insert: Option<String>,
    #[serde(default)]
    pub vim_command: Option<String>,
    #[serde(default)]
    pub plan_approve: Option<String>,
    #[serde(default)]
    pub plan_reject: Option<String>,
    #[serde(default)]
    pub plan_modify: Option<String>,
    #[serde(default)]
    pub permission_approve: Option<String>,
    #[serde(default)]
    pub permission_reject: Option<String>,
    #[serde(default)]
    pub permission_view_diff: Option<String>,
    #[serde(default)]
    pub settings_save: Option<String>,
    #[serde(default)]
    pub settings_next_page: Option<String>,
    #[serde(default)]
    pub settings_prev_page: Option<String>,
    #[serde(default)]
    pub settings_next_item: Option<String>,
    #[serde(default)]
    pub settings_prev_item: Option<String>,
    #[serde(default)]
    pub settings_edit: Option<String>,
    #[serde(default)]
    pub settings_toggle_bool: Option<String>,
    #[serde(default)]
    pub leader: Option<String>,
    #[serde(default)]
    pub leader_timeout_ms: Option<u64>,
    #[serde(default)]
    pub global_command_palette: Option<String>,
    #[serde(default)]
    pub global_prompt_history: Option<String>,
    #[serde(default)]
    pub global_model_select: Option<String>,
    #[serde(default)]
    pub global_provider_select: Option<String>,
    #[serde(default)]
    pub global_shortcut_help: Option<String>,
    #[serde(default)]
    pub global_expand_details: Option<String>,
    #[serde(default)]
    pub global_tool_output: Option<String>,
    #[serde(default)]
    pub global_status_bar_density: Option<String>,
    #[serde(default)]
    pub global_sidebar_toggle: Option<String>,
    #[serde(default)]
    pub global_message_search: Option<String>,
}

/// 键位映射表
#[derive(Debug, Clone)]
pub struct Keybindings {
    pub global_quit: KeyBinding,
    pub global_quit_alt: KeyBinding,
    pub chat_submit: KeyBinding,
    pub chat_newline: KeyBinding,
    pub toggle_vim_mode: KeyBinding,
    pub vim_scroll_up: KeyBinding,
    pub vim_scroll_down: KeyBinding,
    pub vim_scroll_top: KeyBinding,
    pub vim_scroll_bottom: KeyBinding,
    pub vim_insert: KeyBinding,
    pub vim_command: KeyBinding,
    pub plan_approve: KeyBinding,
    pub plan_reject: KeyBinding,
    pub plan_modify: KeyBinding,
    pub permission_approve: KeyBinding,
    pub permission_reject: KeyBinding,
    pub permission_view_diff: KeyBinding,
    pub settings_save: KeyBinding,
    pub settings_next_page: KeyBinding,
    pub settings_prev_page: KeyBinding,
    pub settings_next_item: KeyBinding,
    pub settings_prev_item: KeyBinding,
    pub settings_edit: KeyBinding,
    pub settings_toggle_bool: KeyBinding,
    pub global_command_palette: KeyBinding,
    pub global_prompt_history: KeyBinding,
    pub global_model_select: KeyBinding,
    pub global_provider_select: KeyBinding,
    pub global_shortcut_help: KeyBinding,
    pub global_expand_details: KeyBinding,
    pub global_tool_output: KeyBinding,
    pub global_status_bar_density: KeyBinding,
    pub global_sidebar_toggle: KeyBinding,
    pub global_message_search: KeyBinding,
    pub leader: KeyBinding,
    pub leader_timeout_ms: u64,
}

impl Default for Keybindings {
    fn default() -> Self {
        Self {
            global_quit: KeyBinding::parse("ctrl+c").expect("invalid built-in keybinding: ctrl+c"),
            global_quit_alt: KeyBinding::parse("ctrl+q")
                .expect("invalid built-in keybinding: ctrl+q"),
            chat_submit: KeyBinding::parse("enter").expect("invalid built-in keybinding: enter"),
            chat_newline: KeyBinding::parse("shift+enter")
                .expect("invalid built-in keybinding: shift+enter"),
            toggle_vim_mode: KeyBinding::parse("ctrl+v")
                .expect("invalid built-in keybinding: ctrl+v"),
            vim_scroll_up: KeyBinding::parse("k").expect("invalid built-in keybinding: k"),
            vim_scroll_down: KeyBinding::parse("j").expect("invalid built-in keybinding: j"),
            vim_scroll_top: KeyBinding::parse("g").expect("invalid built-in keybinding: g"),
            vim_scroll_bottom: KeyBinding::parse("G").expect("invalid built-in keybinding: G"),
            vim_insert: KeyBinding::parse("i").expect("invalid built-in keybinding: i"),
            vim_command: KeyBinding::parse(":").expect("invalid built-in keybinding: :"),
            plan_approve: KeyBinding::parse("y").expect("invalid built-in keybinding: y"),
            plan_reject: KeyBinding::parse("n").expect("invalid built-in keybinding: n"),
            plan_modify: KeyBinding::parse("m").expect("invalid built-in keybinding: m"),
            permission_approve: KeyBinding::parse("y").expect("invalid built-in keybinding: y"),
            permission_reject: KeyBinding::parse("n").expect("invalid built-in keybinding: n"),
            permission_view_diff: KeyBinding::parse("d").expect("invalid built-in keybinding: d"),
            settings_save: KeyBinding::parse("s").expect("invalid built-in keybinding: s"),
            settings_next_page: KeyBinding::parse("l").expect("invalid built-in keybinding: l"),
            settings_prev_page: KeyBinding::parse("h").expect("invalid built-in keybinding: h"),
            settings_next_item: KeyBinding::parse("j").expect("invalid built-in keybinding: j"),
            settings_prev_item: KeyBinding::parse("k").expect("invalid built-in keybinding: k"),
            settings_edit: KeyBinding::parse("enter").expect("invalid built-in keybinding: enter"),
            settings_toggle_bool: KeyBinding::parse("space")
                .expect("invalid built-in keybinding: space"),
            global_command_palette: KeyBinding::parse("ctrl+p")
                .expect("invalid built-in keybinding: ctrl+p"),
            global_prompt_history: KeyBinding::parse("ctrl+r")
                .expect("invalid built-in keybinding: ctrl+r"),
            global_model_select: KeyBinding::parse("ctrl+m")
                .expect("invalid built-in keybinding: ctrl+m"),
            global_provider_select: KeyBinding::parse("ctrl+l")
                .expect("invalid built-in keybinding: ctrl+l"),
            global_shortcut_help: KeyBinding::parse("f1").expect("invalid built-in keybinding: f1"),
            global_expand_details: KeyBinding::parse("ctrl+o")
                .expect("invalid built-in keybinding: ctrl+o"),
            global_tool_output: KeyBinding::parse("ctrl+t")
                .expect("invalid built-in keybinding: ctrl+t"),
            global_status_bar_density: KeyBinding::parse("ctrl+shift+s")
                .expect("invalid built-in keybinding: ctrl+shift+s"),
            global_sidebar_toggle: KeyBinding::parse("ctrl+b")
                .expect("invalid built-in keybinding: ctrl+b"),
            global_message_search: KeyBinding::parse("ctrl+f")
                .expect("invalid built-in keybinding: ctrl+f"),
            leader: KeyBinding::parse("backslash").expect("invalid built-in keybinding: backslash"),
            leader_timeout_ms: 500,
        }
    }
}

impl Keybindings {
    pub fn default_bindings() -> Self {
        Self::default()
    }

    pub fn load() -> Self {
        let path = dirs::config_dir()
            .map(|d| d.join("priority-agent").join("keybindings.toml"))
            .unwrap_or_else(|| {
                std::path::PathBuf::from(".priority-agent").join("keybindings.toml")
            });

        if !path.exists() {
            return Self::default();
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => Self::from_toml(&content),
            Err(e) => {
                tracing::warn!("Failed to read keybindings file: {}", e);
                Self::default()
            }
        }
    }

    pub fn from_toml(content: &str) -> Self {
        let file: KeybindingsFile = match toml::from_str(content) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!("Failed to parse keybindings file: {}", e);
                return Self::default();
            }
        };

        let defaults = Self::default();

        macro_rules! override_binding {
            ($field:ident) => {
                match file.$field.as_deref() {
                    Some(s) => match KeyBinding::parse(s) {
                        Ok(kb) => kb,
                        Err(e) => {
                            tracing::warn!("Invalid keybinding for {}: {}", stringify!($field), e);
                            defaults.$field
                        }
                    },
                    None => defaults.$field,
                }
            };
        }

        Self {
            global_quit: override_binding!(global_quit),
            global_quit_alt: override_binding!(global_quit_alt),
            chat_submit: override_binding!(chat_submit),
            chat_newline: override_binding!(chat_newline),
            toggle_vim_mode: override_binding!(toggle_vim_mode),
            vim_scroll_up: override_binding!(vim_scroll_up),
            vim_scroll_down: override_binding!(vim_scroll_down),
            vim_scroll_top: override_binding!(vim_scroll_top),
            vim_scroll_bottom: override_binding!(vim_scroll_bottom),
            vim_insert: override_binding!(vim_insert),
            vim_command: override_binding!(vim_command),
            plan_approve: override_binding!(plan_approve),
            plan_reject: override_binding!(plan_reject),
            plan_modify: override_binding!(plan_modify),
            permission_approve: override_binding!(permission_approve),
            permission_reject: override_binding!(permission_reject),
            permission_view_diff: override_binding!(permission_view_diff),
            settings_save: override_binding!(settings_save),
            settings_next_page: override_binding!(settings_next_page),
            settings_prev_page: override_binding!(settings_prev_page),
            settings_next_item: override_binding!(settings_next_item),
            settings_prev_item: override_binding!(settings_prev_item),
            settings_edit: override_binding!(settings_edit),
            settings_toggle_bool: override_binding!(settings_toggle_bool),
            global_command_palette: override_binding!(global_command_palette),
            global_prompt_history: override_binding!(global_prompt_history),
            global_model_select: override_binding!(global_model_select),
            global_provider_select: override_binding!(global_provider_select),
            global_shortcut_help: override_binding!(global_shortcut_help),
            global_expand_details: override_binding!(global_expand_details),
            global_tool_output: override_binding!(global_tool_output),
            global_status_bar_density: override_binding!(global_status_bar_density),
            global_sidebar_toggle: override_binding!(global_sidebar_toggle),
            global_message_search: override_binding!(global_message_search),
            leader: override_binding!(leader),
            leader_timeout_ms: file.leader_timeout_ms.unwrap_or(500),
        }
    }

    pub fn action_for(&self, key: KeyEvent, mode: crate::tui::app::AppMode) -> AppAction {
        match mode {
            crate::tui::app::AppMode::PlanApproval => {
                if self.plan_approve.matches(key) || self.chat_submit.matches(key) {
                    return AppAction::PlanApprove;
                }
                if self.plan_reject.matches(key) {
                    return AppAction::PlanReject;
                }
                if self.plan_modify.matches(key) {
                    return AppAction::PlanModify;
                }
                if self.global_quit.matches(key) || self.global_quit_alt.matches(key) {
                    return AppAction::Quit;
                }
            }
            crate::tui::app::AppMode::PermissionApproval => {
                if self.permission_approve.matches(key) || self.chat_submit.matches(key) {
                    return AppAction::PermissionApprove;
                }
                if self.permission_reject.matches(key) {
                    return AppAction::PermissionReject;
                }
                if self.permission_view_diff.matches(key) {
                    return AppAction::PermissionViewDiff;
                }
                if self.global_quit.matches(key) || self.global_quit_alt.matches(key) {
                    return AppAction::Quit;
                }
            }
            crate::tui::app::AppMode::Settings => {
                if self.global_quit.matches(key) || self.global_quit_alt.matches(key) {
                    return AppAction::Quit;
                }
                if self.settings_save.matches(key) {
                    return AppAction::SettingsSave;
                }
                if self.settings_next_page.matches(key) {
                    return AppAction::SettingsNextPage;
                }
                if self.settings_prev_page.matches(key) {
                    return AppAction::SettingsPrevPage;
                }
                if self.settings_next_item.matches(key) {
                    return AppAction::SettingsNextItem;
                }
                if self.settings_prev_item.matches(key) {
                    return AppAction::SettingsPrevItem;
                }
                if self.settings_edit.matches(key) || self.chat_submit.matches(key) {
                    return AppAction::SettingsEdit;
                }
                if self.settings_toggle_bool.matches(key)
                    || (KeyBinding {
                        modifiers: KeyModifiers::NONE,
                        code: KeyCode::Char(' '),
                    })
                    .matches(key)
                {
                    return AppAction::SettingsToggleBool;
                }
            }
            crate::tui::app::AppMode::VimNormal => {
                if self.global_quit.matches(key) || self.global_quit_alt.matches(key) {
                    return AppAction::Quit;
                }
                if self.toggle_vim_mode.matches(key) {
                    return AppAction::ToggleVimMode;
                }
                if self.vim_insert.matches(key) {
                    return AppAction::VimInsert;
                }
                if self.vim_command.matches(key) {
                    return AppAction::VimCommand;
                }
                if self.vim_scroll_up.matches(key) {
                    return AppAction::ScrollUp;
                }
                if self.vim_scroll_down.matches(key) {
                    return AppAction::ScrollDown;
                }
                if self.vim_scroll_top.matches(key) {
                    return AppAction::ScrollTop;
                }
                if self.vim_scroll_bottom.matches(key) {
                    return AppAction::ScrollBottom;
                }
            }
            crate::tui::app::AppMode::Chat
            | crate::tui::app::AppMode::DiffViewer
            | crate::tui::app::AppMode::ToolViewer => {
                if self.global_quit.matches(key) || self.global_quit_alt.matches(key) {
                    return AppAction::Quit;
                }
                if key.modifiers.is_empty() && key.code == KeyCode::Esc {
                    return AppAction::Cancel;
                }
                if self.global_command_palette.matches(key) {
                    return AppAction::OpenCommandPalette;
                }
                if self.global_prompt_history.matches(key) {
                    return AppAction::OpenPromptHistory;
                }
                if self.global_model_select.matches(key) {
                    return AppAction::OpenModelSelect;
                }
                if self.global_provider_select.matches(key) {
                    return AppAction::OpenProviderSelect;
                }
                if self.global_shortcut_help.matches(key) {
                    return AppAction::OpenShortcutHelp;
                }
                if self.global_expand_details.matches(key) {
                    return AppAction::ToggleExpandDetails;
                }
                if self.global_tool_output.matches(key) {
                    return AppAction::OpenToolOutput;
                }
                if self.global_status_bar_density.matches(key) {
                    return AppAction::CycleStatusBarDensity;
                }
                if self.global_sidebar_toggle.matches(key) {
                    return AppAction::ToggleSidebar;
                }
                if self.global_message_search.matches(key) {
                    return AppAction::OpenMessageSearch;
                }
                if self.leader.matches(key) {
                    return AppAction::LeaderPalette;
                }
                if self.toggle_vim_mode.matches(key) {
                    return AppAction::ToggleVimMode;
                }
                if self.chat_newline.matches(key) {
                    return AppAction::InsertNewline;
                }
                if self.chat_submit.matches(key) {
                    return AppAction::Submit;
                }
            }
            crate::tui::app::AppMode::AskUser | crate::tui::app::AppMode::Onboarding => {
                // AskUser 和 Onboarding 模式的键盘事件在 handle_key_event 中单独处理
            }
            crate::tui::app::AppMode::MessageSearch => {
                // MessageSearch 模式的键盘事件在 handle_key_event 中单独处理
            }
            crate::tui::app::AppMode::CommandPalette
            | crate::tui::app::AppMode::ShortcutHelp
            | crate::tui::app::AppMode::PromptHistory
            | crate::tui::app::AppMode::ModelSelect
            | crate::tui::app::AppMode::ProviderSelect
            | crate::tui::app::AppMode::FilePicker
            | crate::tui::app::AppMode::WorkspaceSwitcher => {
                // Overlay modes are handled directly in handle_key_event.
            }
        }
        AppAction::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keybinding_parsing() {
        let kb = KeyBinding::parse("ctrl+c").unwrap();
        assert_eq!(kb.modifiers, KeyModifiers::CONTROL);
        assert_eq!(kb.code, KeyCode::Char('c'));

        let kb = KeyBinding::parse("shift+enter").unwrap();
        assert_eq!(kb.modifiers, KeyModifiers::SHIFT);
        assert_eq!(kb.code, KeyCode::Enter);

        let kb = KeyBinding::parse("esc").unwrap();
        assert_eq!(kb.modifiers, KeyModifiers::NONE);
        assert_eq!(kb.code, KeyCode::Esc);

        let kb = KeyBinding::parse("G").unwrap();
        assert_eq!(kb.code, KeyCode::Char('G'));
    }

    #[test]
    fn test_keybinding_display() {
        let kb = KeyBinding::parse("ctrl+shift+j").unwrap();
        assert_eq!(kb.to_string(), "ctrl+shift+j");

        let kb = KeyBinding::parse("enter").unwrap();
        assert_eq!(kb.to_string(), "enter");

        let kb = KeyBinding::parse("space").unwrap();
        assert_eq!(kb.to_string(), "space");
    }

    fn ke(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        }
    }

    #[test]
    fn test_default_keybindings() {
        let kb = Keybindings::default();
        assert!(kb
            .global_quit
            .matches(ke(KeyCode::Char('c'), KeyModifiers::CONTROL)));
        assert!(kb
            .chat_submit
            .matches(ke(KeyCode::Enter, KeyModifiers::NONE)));
    }

    #[test]
    fn test_action_for_chat_mode() {
        let kb = Keybindings::default();
        assert_eq!(
            kb.action_for(
                ke(KeyCode::Enter, KeyModifiers::NONE),
                crate::tui::app::AppMode::Chat
            ),
            AppAction::Submit
        );
        assert_eq!(
            kb.action_for(
                ke(KeyCode::Char('c'), KeyModifiers::CONTROL),
                crate::tui::app::AppMode::Chat
            ),
            AppAction::Quit
        );
        assert_eq!(
            kb.action_for(
                ke(KeyCode::Esc, KeyModifiers::NONE),
                crate::tui::app::AppMode::Chat
            ),
            AppAction::Cancel
        );
    }

    #[test]
    fn test_action_for_permission_approval_mode() {
        let kb = Keybindings::default();
        assert_eq!(
            kb.action_for(
                ke(KeyCode::Char('y'), KeyModifiers::NONE),
                crate::tui::app::AppMode::PermissionApproval
            ),
            AppAction::PermissionApprove
        );
        assert_eq!(
            kb.action_for(
                ke(KeyCode::Char('n'), KeyModifiers::NONE),
                crate::tui::app::AppMode::PermissionApproval
            ),
            AppAction::PermissionReject
        );
        assert_eq!(
            kb.action_for(
                ke(KeyCode::Char('d'), KeyModifiers::NONE),
                crate::tui::app::AppMode::PermissionApproval
            ),
            AppAction::PermissionViewDiff
        );
        assert_eq!(
            kb.action_for(
                ke(KeyCode::Enter, KeyModifiers::NONE),
                crate::tui::app::AppMode::PermissionApproval
            ),
            AppAction::PermissionApprove
        );
    }

    #[test]
    fn test_load_from_toml() {
        let toml = r#"
global_quit = "ctrl+x"
chat_submit = "alt+enter"
"#;
        let kb = Keybindings::from_toml(toml);
        assert!(kb
            .global_quit
            .matches(ke(KeyCode::Char('x'), KeyModifiers::CONTROL)));
        assert!(kb
            .chat_submit
            .matches(ke(KeyCode::Enter, KeyModifiers::ALT)));
        // Unspecified fields keep defaults
        assert!(kb
            .chat_newline
            .matches(ke(KeyCode::Enter, KeyModifiers::SHIFT)));
    }

    #[test]
    fn test_leader_binding_defaults_to_backslash() {
        let kb = Keybindings::default();
        assert!(kb
            .leader
            .matches(ke(KeyCode::Char('\\'), KeyModifiers::NONE)));
        assert_eq!(kb.leader_timeout_ms, 500);
    }

    #[test]
    fn test_leader_binding_override_from_toml() {
        let toml = r#"
leader = "comma"
leader_timeout_ms = 1200
"#;
        let kb = Keybindings::from_toml(toml);
        assert!(kb
            .leader
            .matches(ke(KeyCode::Char(','), KeyModifiers::NONE)));
        assert_eq!(kb.leader_timeout_ms, 1200);
    }

    #[test]
    fn test_backslash_alias_parses() {
        let kb = KeyBinding::parse("backslash").unwrap();
        assert_eq!(kb.code, KeyCode::Char('\\'));
        assert!(kb.modifiers.is_empty());
    }

    #[test]
    fn test_comma_alias_parses() {
        let kb = KeyBinding::parse("comma").unwrap();
        assert_eq!(kb.code, KeyCode::Char(','));
        assert!(kb.modifiers.is_empty());
    }
}
