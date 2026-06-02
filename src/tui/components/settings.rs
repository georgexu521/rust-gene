//! 设置界面组件
//!
//! 交互式 CLI 配置管理界面

use crate::services::config::AppConfig;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Tabs},
    Frame,
};

/// 设置页面
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsPage {
    General,
    Api,
    Features,
    Storage,
    Keybindings,
}

impl SettingsPage {
    pub fn title(&self) -> &'static str {
        match self {
            SettingsPage::General => "General",
            SettingsPage::Api => "API",
            SettingsPage::Features => "Features",
            SettingsPage::Storage => "Storage",
            SettingsPage::Keybindings => "Keybindings",
        }
    }

    pub fn all() -> Vec<SettingsPage> {
        vec![
            SettingsPage::General,
            SettingsPage::Api,
            SettingsPage::Features,
            SettingsPage::Storage,
            SettingsPage::Keybindings,
        ]
    }
}

/// 设置项类型
#[derive(Debug, Clone)]
pub enum SettingValue {
    String(String),
    Bool(bool),
    Number(f64),
    OptionString(Option<String>),
}

/// 设置项
#[derive(Debug, Clone)]
pub struct SettingItem {
    pub key: String,
    pub label: String,
    pub description: String,
    pub value: SettingValue,
    pub editable: bool,
    pub sensitive: bool, // 是否敏感（如 API key）
}

/// 设置状态
#[derive(Debug)]
pub struct SettingsState {
    pub config: AppConfig,
    pub current_page: SettingsPage,
    pub items: Vec<SettingItem>,
    pub selected_index: usize,
    pub edit_mode: bool,
    pub edit_buffer: String,
    pub show_saved: bool,
    pub message: Option<String>,
    pub message_time: Option<std::time::Instant>,
    pub pending_restart: bool,
    pub keybindings: crate::tui::keybindings::Keybindings,
}

impl SettingsState {
    pub fn new(config: AppConfig, keybindings: crate::tui::keybindings::Keybindings) -> Self {
        let mut state = Self {
            config,
            current_page: SettingsPage::General,
            items: Vec::new(),
            selected_index: 0,
            edit_mode: false,
            edit_buffer: String::new(),
            show_saved: false,
            message: None,
            message_time: None,
            pending_restart: false,
            keybindings,
        };
        state.refresh_items();
        state
    }

    /// 刷新当前页面的设置项
    pub fn refresh_items(&mut self) {
        self.items = match self.current_page {
            SettingsPage::General => self.general_settings(),
            SettingsPage::Api => self.api_settings(),
            SettingsPage::Features => self.feature_settings(),
            SettingsPage::Storage => self.storage_settings(),
            SettingsPage::Keybindings => self.keybindings_settings(),
        };
        self.selected_index = self.selected_index.min(self.items.len().saturating_sub(1));
    }

    fn general_settings(&self) -> Vec<SettingItem> {
        vec![
            SettingItem {
                key: "ui.theme".to_string(),
                label: "Theme".to_string(),
                description: "UI color theme (graphite / porcelain / midnight / ember / aurora / nord / dracula / catppuccin-mocha)".to_string(),
                value: SettingValue::String(self.config.ui.theme.clone()),
                editable: true,
                sensitive: false,
            },
            SettingItem {
                key: "ui.show_token_usage".to_string(),
                label: "Show Token Usage".to_string(),
                description: "Display token usage in status bar".to_string(),
                value: SettingValue::Bool(self.config.ui.show_token_usage),
                editable: true,
                sensitive: false,
            },
            SettingItem {
                key: "ui.compact_mode".to_string(),
                label: "Compact Mode".to_string(),
                description: "Use compact UI layout".to_string(),
                value: SettingValue::Bool(self.config.ui.compact_mode),
                editable: true,
                sensitive: false,
            },
        ]
    }

    fn api_settings(&self) -> Vec<SettingItem> {
        vec![
            SettingItem {
                key: "api.model".to_string(),
                label: "Model".to_string(),
                description: "Default AI model (e.g., kimi-k2.5, gpt-4o)".to_string(),
                value: SettingValue::String(self.config.api.model.clone()),
                editable: true,
                sensitive: false,
            },
            SettingItem {
                key: "api.base_url".to_string(),
                label: "Base URL".to_string(),
                description: "API base URL (optional)".to_string(),
                value: SettingValue::String(self.config.api.base_url.clone()),
                editable: true,
                sensitive: false,
            },
            SettingItem {
                key: "api.api_key".to_string(),
                label: "API Key".to_string(),
                description: "API authentication key".to_string(),
                value: SettingValue::OptionString(self.config.api.api_key.clone()),
                editable: true,
                sensitive: true,
            },
            SettingItem {
                key: "api.temperature".to_string(),
                label: "Temperature".to_string(),
                description: "Sampling temperature (0.0 - 2.0)".to_string(),
                value: SettingValue::Number(self.config.api.temperature as f64),
                editable: true,
                sensitive: false,
            },
            SettingItem {
                key: "api.max_tokens".to_string(),
                label: "Max Tokens".to_string(),
                description: "Maximum tokens per response".to_string(),
                value: SettingValue::OptionString(
                    self.config.api.max_tokens.map(|t| t.to_string()),
                ),
                editable: true,
                sensitive: false,
            },
        ]
    }

    fn feature_settings(&self) -> Vec<SettingItem> {
        vec![
            SettingItem {
                key: "features.tui_enabled".to_string(),
                label: "Interactive CLI".to_string(),
                description: "Enable interactive terminal CLI".to_string(),
                value: SettingValue::Bool(self.config.features.tui_enabled),
                editable: true,
                sensitive: false,
            },
            SettingItem {
                key: "features.agent_enabled".to_string(),
                label: "Agent System".to_string(),
                description: "Enable sub-agent functionality".to_string(),
                value: SettingValue::Bool(self.config.features.agent_enabled),
                editable: true,
                sensitive: false,
            },
            SettingItem {
                key: "features.mcp_enabled".to_string(),
                label: "MCP Support".to_string(),
                description: "Enable Model Context Protocol".to_string(),
                value: SettingValue::Bool(self.config.features.mcp_enabled),
                editable: true,
                sensitive: false,
            },
            SettingItem {
                key: "features.skills_enabled".to_string(),
                label: "Skills".to_string(),
                description: "Enable skill system".to_string(),
                value: SettingValue::Bool(self.config.features.skills_enabled),
                editable: true,
                sensitive: false,
            },
            SettingItem {
                key: "features.web_search".to_string(),
                label: "Web Search".to_string(),
                description: "Enable web search functionality".to_string(),
                value: SettingValue::Bool(self.config.features.web_search),
                editable: true,
                sensitive: false,
            },
        ]
    }

    fn storage_settings(&self) -> Vec<SettingItem> {
        vec![
            SettingItem {
                key: "storage.persistence_enabled".to_string(),
                label: "Persistence".to_string(),
                description: "Enable session persistence".to_string(),
                value: SettingValue::Bool(self.config.storage.persistence_enabled),
                editable: true,
                sensitive: false,
            },
            SettingItem {
                key: "storage.auto_save_interval_secs".to_string(),
                label: "Auto Save Interval".to_string(),
                description: "Auto-save interval in seconds".to_string(),
                value: SettingValue::Number(self.config.storage.auto_save_interval_secs as f64),
                editable: true,
                sensitive: false,
            },
        ]
    }

    fn keybindings_settings(&self) -> Vec<SettingItem> {
        let kb = &self.keybindings;
        vec![
            Self::kb_item("global_quit", "Quit", "Exit application", &kb.global_quit),
            Self::kb_item("chat_submit", "Submit", "Send message", &kb.chat_submit),
            Self::kb_item(
                "chat_newline",
                "Newline",
                "Insert newline in input",
                &kb.chat_newline,
            ),
            Self::kb_item(
                "toggle_vim_mode",
                "Toggle Vim",
                "Toggle Vim normal mode",
                &kb.toggle_vim_mode,
            ),
            Self::kb_item(
                "vim_scroll_up",
                "Scroll Up",
                "Scroll messages up (Vim)",
                &kb.vim_scroll_up,
            ),
            Self::kb_item(
                "vim_scroll_down",
                "Scroll Down",
                "Scroll messages down (Vim)",
                &kb.vim_scroll_down,
            ),
            Self::kb_item(
                "plan_approve",
                "Approve Plan",
                "Approve pending plan",
                &kb.plan_approve,
            ),
            Self::kb_item(
                "plan_reject",
                "Reject Plan",
                "Reject pending plan",
                &kb.plan_reject,
            ),
            Self::kb_item(
                "settings_save",
                "Save Settings",
                "Save settings to file",
                &kb.settings_save,
            ),
        ]
    }

    fn kb_item(
        key: &str,
        label: &str,
        desc: &str,
        binding: &crate::tui::keybindings::KeyBinding,
    ) -> SettingItem {
        SettingItem {
            key: format!("keybindings.{}", key),
            label: label.to_string(),
            description: desc.to_string(),
            value: SettingValue::String(binding.to_string()),
            editable: false,
            sensitive: false,
        }
    }

    /// 切换到下一页
    pub fn next_page(&mut self) {
        let pages = SettingsPage::all();
        let current_idx = pages
            .iter()
            .position(|p| *p == self.current_page)
            .unwrap_or(0);
        self.current_page = pages[(current_idx + 1) % pages.len()];
        self.selected_index = 0;
        self.refresh_items();
    }

    /// 切换到上一页
    pub fn prev_page(&mut self) {
        let pages = SettingsPage::all();
        let current_idx = pages
            .iter()
            .position(|p| *p == self.current_page)
            .unwrap_or(0);
        let new_idx = if current_idx == 0 {
            pages.len() - 1
        } else {
            current_idx - 1
        };
        self.current_page = pages[new_idx];
        self.selected_index = 0;
        self.refresh_items();
    }

    /// 选择下一项
    pub fn next_item(&mut self) {
        if !self.items.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.items.len();
        }
    }

    /// 选择上一项
    pub fn prev_item(&mut self) {
        if !self.items.is_empty() {
            self.selected_index = if self.selected_index == 0 {
                self.items.len() - 1
            } else {
                self.selected_index - 1
            };
        }
    }

    /// 开始编辑当前项
    pub fn start_edit(&mut self) {
        if let Some(item) = self.items.get(self.selected_index) {
            if item.editable {
                self.edit_mode = true;
                self.edit_buffer = match &item.value {
                    SettingValue::String(s) => s.clone(),
                    SettingValue::Bool(b) => b.to_string(),
                    SettingValue::Number(n) => n.to_string(),
                    SettingValue::OptionString(opt) => opt.clone().unwrap_or_default(),
                };
            }
        }
    }

    /// 取消编辑
    pub fn cancel_edit(&mut self) {
        self.edit_mode = false;
        self.edit_buffer.clear();
    }

    /// 保存当前编辑
    pub fn save_edit(&mut self) {
        if let Some(item) = self.items.get(self.selected_index).cloned() {
            let new_value = self.edit_buffer.clone();

            // 更新配置
            match item.key.as_str() {
                "ui.theme" => self.config.ui.theme = new_value,
                "ui.show_token_usage" => {
                    self.config.ui.show_token_usage = new_value.parse().unwrap_or(true)
                }
                "ui.compact_mode" => {
                    self.config.ui.compact_mode = new_value.parse().unwrap_or(false)
                }
                "api.model" => self.config.api.model = new_value,
                "api.base_url" => self.config.api.base_url = new_value.clone(),
                "api.api_key" => {
                    self.config.api.api_key = if new_value.is_empty() {
                        None
                    } else {
                        Some(new_value)
                    }
                }
                "api.temperature" => {
                    self.config.api.temperature = new_value.parse().unwrap_or(0.6) as f32
                }
                "api.max_tokens" => {
                    self.config.api.max_tokens = new_value.parse().ok();
                }
                "features.tui_enabled" => {
                    self.config.features.tui_enabled = new_value.parse().unwrap_or(true);
                    self.pending_restart = true;
                }
                "features.agent_enabled" => {
                    self.config.features.agent_enabled = new_value.parse().unwrap_or(true)
                }
                "features.mcp_enabled" => {
                    self.config.features.mcp_enabled = new_value.parse().unwrap_or(false)
                }
                "features.skills_enabled" => {
                    self.config.features.skills_enabled = new_value.parse().unwrap_or(true)
                }
                "features.web_search" => {
                    self.config.features.web_search = new_value.parse().unwrap_or(true)
                }
                "storage.persistence_enabled" => {
                    self.config.storage.persistence_enabled = new_value.parse().unwrap_or(true)
                }
                "storage.auto_save_interval_secs" => {
                    self.config.storage.auto_save_interval_secs =
                        new_value.parse().unwrap_or(300) as u64
                }
                _ => {}
            }

            self.edit_mode = false;
            self.edit_buffer.clear();
            self.refresh_items();
            self.show_message("Setting saved".to_string());
        }
    }

    /// 保存配置到文件
    pub fn save_config(&mut self) -> anyhow::Result<()> {
        self.config.save()?;
        self.show_saved = true;
        self.show_message("Configuration saved to file".to_string());
        Ok(())
    }

    /// 显示消息
    pub fn show_message(&mut self, msg: String) {
        self.message = Some(msg);
        self.message_time = Some(std::time::Instant::now());
    }

    /// 检查消息是否过期
    pub fn check_message_timeout(&mut self) {
        if let Some(time) = self.message_time {
            if time.elapsed().as_secs() > 3 {
                self.message = None;
                self.message_time = None;
            }
        }
    }

    /// 切换布尔值
    pub fn toggle_bool(&mut self) {
        if let Some(item) = self.items.get(self.selected_index) {
            if let SettingValue::Bool(current) = item.value {
                self.edit_buffer = (!current).to_string();
                self.save_edit();
            }
        }
    }

    /// 获取当前选中项
    pub fn selected_item(&self) -> Option<&SettingItem> {
        self.items.get(self.selected_index)
    }
}

/// 渲染设置界面
pub fn render_settings(
    f: &mut Frame,
    state: &SettingsState,
    area: Rect,
    theme: &crate::tui::theme::Theme,
) {
    // 清除背景
    f.render_widget(Clear, area);

    // 主块
    let block = Block::default()
        .title(" Settings ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_active));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // 分割布局
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(inner);

    // 渲染标签页
    render_tabs(f, state, chunks[0], theme);

    // 渲染设置列表
    render_settings_list(f, state, chunks[1], theme);

    // 渲染底部帮助
    render_help_bar(f, state, chunks[2], theme);
}

fn render_tabs(f: &mut Frame, state: &SettingsState, area: Rect, theme: &crate::tui::theme::Theme) {
    let titles: Vec<_> = SettingsPage::all()
        .iter()
        .map(|p| Line::from(p.title()))
        .collect();

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::BOTTOM))
        .select(
            SettingsPage::all()
                .iter()
                .position(|p| *p == state.current_page)
                .unwrap_or(0),
        )
        .highlight_style(
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        )
        .divider(" | ");

    f.render_widget(tabs, area);
}

fn render_settings_list(
    f: &mut Frame,
    state: &SettingsState,
    area: Rect,
    theme: &crate::tui::theme::Theme,
) {
    let items: Vec<_> = state
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_selected = i == state.selected_index;
            let is_editing = is_selected && state.edit_mode;

            // 值显示
            let value_str = if is_editing {
                format!("> {}", state.edit_buffer)
            } else {
                match &item.value {
                    SettingValue::String(s) => {
                        if item.sensitive && !s.is_empty() {
                            "***".to_string()
                        } else {
                            s.clone()
                        }
                    }
                    SettingValue::Bool(b) => {
                        if *b {
                            "✓ Yes".to_string()
                        } else {
                            "✗ No".to_string()
                        }
                    }
                    SettingValue::Number(n) => format!("{:.2}", n),
                    SettingValue::OptionString(opt) => {
                        if item.sensitive && opt.is_some() {
                            "***".to_string()
                        } else {
                            opt.clone().unwrap_or_else(|| "(none)".to_string())
                        }
                    }
                }
            };

            let style = if is_selected {
                Style::default()
                    .fg(theme.warning)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let prefix = if is_selected { "❯ " } else { "  " };

            Line::from(vec![
                Span::raw(prefix),
                Span::styled(format!("{:<20}", item.label), style),
                Span::raw(" "),
                Span::styled(
                    value_str,
                    if is_editing {
                        Style::default()
                            .fg(theme.success)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.info)
                    },
                ),
            ])
        })
        .collect();

    let paragraph = Paragraph::new(Text::from(items))
        .block(Block::default())
        .wrap(ratatui::widgets::Wrap { trim: false });

    f.render_widget(paragraph, area);

    // 显示描述
    if let Some(item) = state.selected_item() {
        if !state.edit_mode {
            let desc = Paragraph::new(item.description.as_str())
                .style(Style::default().fg(theme.text_dim))
                .alignment(Alignment::Left);

            let desc_area = Rect {
                x: area.x,
                y: area.y + area.height.saturating_sub(1),
                width: area.width,
                height: 1,
            };
            f.render_widget(desc, desc_area);
        }
    }

    // 显示消息
    if let Some(ref msg) = state.message {
        let msg_widget = Paragraph::new(msg.as_str())
            .style(
                Style::default()
                    .fg(theme.success)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center);

        let msg_area = Rect {
            x: area.x,
            y: area.y + area.height.saturating_sub(2),
            width: area.width,
            height: 1,
        };
        f.render_widget(msg_widget, msg_area);
    }
}

fn render_help_bar(
    f: &mut Frame,
    state: &SettingsState,
    area: Rect,
    theme: &crate::tui::theme::Theme,
) {
    let help_text = if state.edit_mode {
        "Enter: Save | Esc: Cancel"
    } else {
        "←/→: Tab | ↑/↓: Select | Enter: Edit | Space: Toggle | s: Save | q: Quit"
    };

    let help = Paragraph::new(help_text)
        .style(Style::default().fg(theme.text_dim))
        .alignment(Alignment::Center);

    f.render_widget(help, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_state_navigation() {
        let config = AppConfig::default();
        let kb = crate::tui::keybindings::Keybindings::default();
        let mut state = SettingsState::new(config, kb);

        assert_eq!(state.current_page, SettingsPage::General);
        assert_eq!(state.selected_index, 0);

        state.next_item();
        assert_eq!(state.selected_index, 1);

        state.prev_item();
        assert_eq!(state.selected_index, 0);

        state.next_page();
        assert_eq!(state.current_page, SettingsPage::Api);
    }

    #[test]
    fn test_settings_edit() {
        let config = AppConfig::default();
        let kb = crate::tui::keybindings::Keybindings::default();
        let mut state = SettingsState::new(config, kb);

        // 切换到 API 页面
        state.next_page();
        assert_eq!(state.current_page, SettingsPage::Api);

        // 开始编辑
        state.start_edit();
        assert!(state.edit_mode);

        // 修改值
        state.edit_buffer = "kimi-k2.5".to_string();
        state.save_edit();

        assert!(!state.edit_mode);
        assert_eq!(state.config.api.model, "kimi-k2.5");
    }

    #[test]
    fn test_toggle_bool() {
        let config = AppConfig::default();
        let kb = crate::tui::keybindings::Keybindings::default();
        let mut state = SettingsState::new(config, kb);

        // 默认是 true
        assert!(state.config.ui.show_token_usage);

        // 选择第二个项目 (show_token_usage)
        state.next_item();
        assert_eq!(state.selected_index, 1);

        // 切换
        state.toggle_bool();
        assert!(!state.config.ui.show_token_usage);

        // 再切换回来
        state.toggle_bool();
        assert!(state.config.ui.show_token_usage);
    }
}
