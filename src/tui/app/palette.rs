use super::*;

impl TuiApp {
    pub fn open_command_palette(&mut self) {
        self.command_palette_query.clear();
        self.command_palette_selected = 0;
        self.mode = AppMode::CommandPalette;
    }

    pub fn close_command_palette(&mut self) {
        self.command_palette_query.clear();
        self.command_palette_selected = 0;
        self.mode = if self.vim_mode {
            AppMode::VimNormal
        } else {
            AppMode::Chat
        };
    }

    pub fn command_palette_items(&self) -> Vec<&crate::tui::commands::CommandDef> {
        let boosted_commands = self.command_palette_boosted_commands();
        let mut items = self.command_registry.palette_items(
            &self.command_palette_query,
            18,
            boosted_commands.as_slice(),
        );
        let contextual = self.contextual_palette_commands();
        if self.command_palette_query.is_empty() && !contextual.is_empty() {
            items.sort_by_key(|cmd| {
                contextual
                    .iter()
                    .position(|name| name == cmd.name)
                    .unwrap_or(usize::MAX)
            });
        }
        items
    }

    pub fn contextual_palette_commands(&self) -> Vec<String> {
        let mut commands = Vec::new();
        if self.pending_permission_request.is_some() {
            commands.push("/reject".to_string());
            commands.push("/permissions".to_string());
            commands.push("/quick".to_string());
        }
        if self.pending_plan.is_some() || self.pending_question.is_some() {
            commands.push("/quick".to_string());
            commands.push("/reject".to_string());
        }
        if self.messages.len() > 1 {
            commands.push("/search".to_string());
            commands.push("/session".to_string());
            commands.push("/export".to_string());
        }
        dedupe_palette_commands(commands)
    }

    pub fn is_contextual_palette_command(&self, name: &str) -> bool {
        self.contextual_palette_commands()
            .iter()
            .any(|command| command == name)
    }

    fn command_palette_boosted_commands(&self) -> Vec<String> {
        let mut commands = self
            .recent_palette_commands
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        commands.extend(self.contextual_palette_commands().into_iter().rev());
        dedupe_palette_commands(commands)
    }

    pub fn command_palette_next(&mut self) {
        let len = self.command_palette_items().len();
        if len > 0 {
            self.command_palette_selected = (self.command_palette_selected + 1).min(len - 1);
        }
    }

    pub fn command_palette_prev(&mut self) {
        self.command_palette_selected = self.command_palette_selected.saturating_sub(1);
    }

    pub fn command_palette_push(&mut self, c: char) {
        self.command_palette_query.push(c);
        self.command_palette_selected = 0;
    }

    pub fn command_palette_backspace(&mut self) {
        self.command_palette_query.pop();
        self.command_palette_selected = 0;
    }

    pub async fn accept_command_palette_selection(&mut self) {
        let selected = self
            .command_palette_items()
            .get(self.command_palette_selected)
            .map(|cmd| {
                (
                    (*cmd).clone(),
                    crate::tui::commands::command_accept_behavior(cmd),
                )
            });
        if let Some((cmd, behavior)) = selected {
            self.record_palette_command(cmd.name);
            match behavior {
                crate::tui::commands::CommandAcceptBehavior::Execute => {
                    self.close_command_palette();
                    self.handle_slash_command(cmd.name).await;
                    return;
                }
                crate::tui::commands::CommandAcceptBehavior::Insert => {
                    self.input.set_value(format!("{} ", cmd.name));
                }
            }
        }
        self.close_command_palette();
    }

    fn record_palette_command(&mut self, name: &str) {
        self.recent_palette_commands.retain(|cmd| cmd != name);
        self.recent_palette_commands.push_back(name.to_string());
        while self.recent_palette_commands.len() > 8 {
            self.recent_palette_commands.pop_front();
        }
    }

    pub fn open_shortcut_help(&mut self) {
        self.shortcut_help_filter.clear();
        self.filtering_shortcut_help = false;
        self.mode = AppMode::ShortcutHelp;
    }

    pub fn close_shortcut_help(&mut self) {
        self.shortcut_help_filter.clear();
        self.filtering_shortcut_help = false;
        self.mode = if self.vim_mode {
            AppMode::VimNormal
        } else {
            AppMode::Chat
        };
    }

    pub fn open_model_select(&mut self) {
        self.model_select_query.clear();
        self.model_select_selected = self
            .model_choices()
            .iter()
            .position(|choice| choice.active)
            .unwrap_or(0);
        self.mode = AppMode::ModelSelect;
    }

    pub fn close_model_select(&mut self) {
        self.mode = if self.vim_mode {
            AppMode::VimNormal
        } else {
            AppMode::Chat
        };
    }

    pub fn model_choices(&self) -> Vec<ModelChoice> {
        let provider = self.current_provider_label();
        let current = self.current_model_label();
        let mut models = match provider.as_str() {
            "MiniMax" => vec![
                "MiniMax-M3",
                "MiniMax-M2.7",
                "MiniMax-M2.7-highspeed",
                "MiniMax-M2.5",
                "MiniMax-M2",
            ],
            "Kimi Code" => vec!["kimi-for-coding"],
            "DeepSeek" => vec!["deepseek-v4-pro", "deepseek-v4-flash", "deepseek-chat"],
            "GLM" => vec!["glm-5.1", "glm-4.7", "glm-4.6"],
            "OpenAI" => vec!["gpt-4o", "gpt-4o-mini"],
            "Kimi" => vec!["kimi-k2.5", "kimi-k2.5-thinking"],
            _ => vec![current.as_str()],
        };
        if !models.iter().any(|m| *m == current) {
            models.insert(0, current.as_str());
        }
        models
            .into_iter()
            .map(|model| ModelChoice {
                provider: provider.clone(),
                model: model.to_string(),
                note: if model == current {
                    "current".to_string()
                } else {
                    "same provider, takes effect next request".to_string()
                },
                active: model == current,
            })
            .filter(|choice| {
                self.model_select_query.is_empty()
                    || choice
                        .model
                        .to_ascii_lowercase()
                        .contains(&self.model_select_query.to_ascii_lowercase())
                    || choice
                        .provider
                        .to_ascii_lowercase()
                        .contains(&self.model_select_query.to_ascii_lowercase())
            })
            .collect()
    }

    pub fn model_select_next(&mut self) {
        let len = self.model_choices().len();
        if len > 0 {
            self.model_select_selected = (self.model_select_selected + 1).min(len - 1);
        }
    }

    pub fn model_select_prev(&mut self) {
        self.model_select_selected = self.model_select_selected.saturating_sub(1);
    }

    pub fn model_select_push(&mut self, c: char) {
        self.model_select_query.push(c);
        self.model_select_selected = 0;
    }

    pub fn model_select_backspace(&mut self) {
        self.model_select_query.pop();
        self.model_select_selected = 0;
    }

    pub fn accept_model_selection(&mut self) {
        let Some(choice) = self
            .model_choices()
            .get(self.model_select_selected)
            .cloned()
        else {
            self.close_model_select();
            return;
        };
        if let Some(engine) = &self.streaming_engine {
            engine.set_model(choice.model.clone());
        }
        if let Ok(mut config) = crate::services::config::AppConfig::load() {
            config.api.model = choice.model.clone();
            let _ = config.save();
        }
        self.model_notice = Some(format!("Model switched to {}", choice.model));
        self.close_model_select();
    }

    /// Switch theme at runtime (in-memory only).
    ///
    /// Does NOT persist to config — callers that need persistence must
    /// also update `AppConfig::ui.theme` and call `.save()`.  The
    /// `/theme` slash handler and `save_settings()` already do this.
    pub fn set_theme(&mut self, name: &str) {
        self.theme = Arc::new(crate::tui::theme::Theme::from_name(name));
    }

    /// List available theme names
    pub fn theme_names(&self) -> Vec<String> {
        vec![
            "graphite".into(),
            "porcelain".into(),
            "nord".into(),
            "dracula".into(),
            "gruvbox-dark".into(),
            "catppuccin-mocha".into(),
            "dark".into(),
            "light".into(),
            "high-contrast".into(),
        ]
    }

    pub fn open_provider_select(&mut self) {
        self.provider_select_query.clear();
        self.provider_select_selected = self
            .provider_choices()
            .iter()
            .position(|choice| choice.active)
            .unwrap_or(0);
        self.mode = AppMode::ProviderSelect;
    }

    pub fn close_provider_select(&mut self) {
        self.mode = if self.vim_mode {
            AppMode::VimNormal
        } else {
            AppMode::Chat
        };
    }

    pub fn provider_choices(&self) -> Vec<ProviderChoice> {
        let active_base = self.current_provider_base_url();
        let registry = crate::services::api::provider::ProviderRegistry::from_env();
        let mut choices = registry
            .list_configs()
            .into_iter()
            .map(|cfg| {
                let base_url = cfg.base_url.unwrap_or_default();
                let active = !active_base.is_empty() && active_base == base_url;
                ProviderChoice {
                    name: cfg.name,
                    provider_type: format!("{:?}", cfg.provider_type),
                    model: cfg.default_model,
                    base_url,
                    configured: true,
                    active,
                    note: if active {
                        "current".to_string()
                    } else {
                        "configured".to_string()
                    },
                }
            })
            .collect::<Vec<_>>();

        for spec in crate::services::api::provider::DEFAULT_PROVIDER_ENV_SPECS {
            if choices.iter().any(|choice| choice.name == spec.id) {
                continue;
            }
            choices.push(ProviderChoice {
                name: spec.id.to_string(),
                provider_type: format!("{:?}", spec.provider_type),
                model: spec.default_model.to_string(),
                base_url: String::new(),
                configured: false,
                active: false,
                note: format!("missing {}", spec.key_env_hint()),
            });
        }

        let query = self.provider_select_query.to_ascii_lowercase();
        if !query.is_empty() {
            choices.retain(|choice| {
                choice.name.to_ascii_lowercase().contains(&query)
                    || choice.provider_type.to_ascii_lowercase().contains(&query)
                    || choice.model.to_ascii_lowercase().contains(&query)
                    || choice.note.to_ascii_lowercase().contains(&query)
            });
        }
        choices.sort_by_key(|choice| (!choice.active, !choice.configured, choice.name.clone()));
        choices
    }

    pub fn provider_select_next(&mut self) {
        let len = self.provider_choices().len();
        if len > 0 {
            self.provider_select_selected = (self.provider_select_selected + 1).min(len - 1);
        }
    }

    pub fn provider_select_prev(&mut self) {
        self.provider_select_selected = self.provider_select_selected.saturating_sub(1);
    }

    pub fn provider_select_push(&mut self, c: char) {
        self.provider_select_query.push(c);
        self.provider_select_selected = 0;
    }

    pub fn provider_select_backspace(&mut self) {
        self.provider_select_query.pop();
        self.provider_select_selected = 0;
    }

    pub fn accept_provider_selection(&mut self) -> String {
        let Some(choice) = self
            .provider_choices()
            .get(self.provider_select_selected)
            .cloned()
        else {
            self.close_provider_select();
            return "No provider selected.".to_string();
        };
        let result = self.switch_provider_by_name(&choice.name);
        self.close_provider_select();
        result
    }

    pub fn switch_provider_by_name(&mut self, name: &str) -> String {
        let registry = crate::services::api::provider::ProviderRegistry::from_env();
        let Some(provider) = registry.get(name) else {
            return format!("Provider '{}' is not configured. Use /provider list to inspect required environment variables.", name);
        };
        let Some(config) = registry.get_config(name).cloned() else {
            return format!("Provider '{}' has no config.", name);
        };
        if let Some(engine) = &self.streaming_engine {
            engine.set_provider(provider, config.default_model.clone());
        }
        if let Ok(mut app_config) = crate::services::config::AppConfig::load() {
            app_config.api.model = config.default_model.clone();
            app_config.api.base_url = config.base_url.clone().unwrap_or_default();
            let _ = app_config.save();
        }
        self.provider_notice = Some(format!(
            "Provider switched to {} ({})",
            config.name, config.default_model
        ));
        format!(
            "Provider switched to {}\nModel: {}\nBase URL: {}",
            config.name,
            config.default_model,
            config.base_url.unwrap_or_default()
        )
    }
}
