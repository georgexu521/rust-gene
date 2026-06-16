use super::*;

impl TuiApp {
    pub fn open_command_palette(&mut self) {
        self.command_palette_query.clear();
        self.command_palette_selected = 0;
        self.push_mode(AppMode::CommandPalette);
    }

    pub fn close_command_palette(&mut self) {
        self.command_palette_query.clear();
        self.command_palette_selected = 0;
        self.pop_mode();
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
        if !self.messages.is_empty() {
            commands.push("/jump".to_string());
            commands.push("/search".to_string());
            commands.push("/session".to_string());
            commands.push("/export".to_string());
        }
        if !self.history.is_empty() {
            commands.push("/prompt-history".to_string());
        }
        if !self.composer.text.value().trim().is_empty() || self.prompt_stash.is_some() {
            commands.push("/prompt-stash".to_string());
        }
        if self.pasted_block_count() > 0 {
            commands.push("/paste".to_string());
        }
        if self.composer_attachment_count() > 0 {
            commands.push("/attach".to_string());
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
                    self.composer.text.set_value(format!("{} ", cmd.name));
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
        self.push_mode(AppMode::ShortcutHelp);
    }

    pub fn close_shortcut_help(&mut self) {
        self.shortcut_help_filter.clear();
        self.filtering_shortcut_help = false;
        self.pop_mode();
    }

    pub fn open_connect_wizard(&mut self) {
        self.connect_wizard_state =
            Some(crate::tui::app::connect_wizard::ConnectWizardState::new());
        self.push_mode(AppMode::ConnectWizard);
    }

    pub fn open_connect_wizard_with_provider(&mut self, provider_id: &str) {
        let mut wizard = crate::tui::app::connect_wizard::ConnectWizardState::new();
        let choices = wizard.provider_choices();
        if let Some(pos) = choices.iter().position(|entry| entry.id == provider_id) {
            wizard.selected = pos;
            wizard.confirm_provider();
        }
        self.connect_wizard_state = Some(wizard);
        self.push_mode(AppMode::ConnectWizard);
    }

    pub fn close_connect_wizard(&mut self) {
        self.connect_wizard_state = None;
        self.pop_mode();
    }

    pub fn open_model_select(&mut self) {
        self.model_select_query.clear();
        self.model_select_selected = self
            .model_choices()
            .iter()
            .position(|choice| choice.active)
            .unwrap_or(0);
        self.push_mode(AppMode::ModelSelect);
    }

    pub async fn refresh_discovered_models(&mut self) {
        let provider_label = self.current_provider_label();
        let provider_id =
            crate::services::api::provider_catalog::provider_id_for_label(&provider_label)
                .unwrap_or_default();
        if provider_id.is_empty() {
            self.discovered_models = Vec::new();
            self.discovering_models = false;
            return;
        }
        let manifest =
            crate::services::api::provider_manifest::ProviderManifestLoader::load_merged();
        let Some(entry) = manifest.provider.iter().find(|e| e.id == provider_id) else {
            self.discovered_models = Vec::new();
            self.discovering_models = false;
            return;
        };
        let api_key = entry.resolve_api_key();
        self.discovering_models = true;
        self.discovered_models = self
            .model_discovery
            .list(&provider_id, entry, api_key.as_deref())
            .await;
        self.discovering_models = false;
    }

    pub fn close_model_select(&mut self) {
        self.pop_mode();
    }

    pub fn model_choices(&self) -> Vec<ModelChoice> {
        let provider_label = self.current_provider_label();
        let current = self.current_model_label();

        let mut model_names: Vec<String> = self
            .discovered_models
            .iter()
            .map(|m| m.id.clone())
            .collect();

        // Fallback to catalog static list if discovery is empty.
        if model_names.is_empty() {
            let catalog_id =
                crate::services::api::provider_catalog::provider_id_for_label(&provider_label);
            model_names = catalog_id
                .map(|id| crate::services::api::provider_catalog::supported_models(&id))
                .unwrap_or_else(|| vec![current.clone()]);
        }

        let mut models: Vec<&str> = model_names.iter().map(|s| s.as_str()).collect();
        if !models.iter().any(|m| *m == current) {
            models.insert(0, current.as_str());
        }
        models
            .into_iter()
            .map(|model| ModelChoice {
                provider: provider_label.clone(),
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
            if config.save().is_ok() {
                crate::services::config::init_runtime_config(config);
            }
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
        self.push_mode(AppMode::ProviderSelect);
    }

    pub fn close_provider_select(&mut self) {
        self.pop_mode();
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

        for entry in crate::services::api::provider_catalog::builtin_catalog() {
            if choices.iter().any(|choice| choice.name == entry.id) {
                continue;
            }
            choices.push(ProviderChoice {
                name: entry.id,
                provider_type: format!("{:?}", entry.provider_type),
                model: entry.default_model,
                base_url: String::new(),
                configured: false,
                active: false,
                note: format!("missing {}", entry.key_env_vars.join(" or ")),
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

    pub async fn accept_provider_selection(&mut self) -> String {
        let Some(choice) = self
            .provider_choices()
            .get(self.provider_select_selected)
            .cloned()
        else {
            self.close_provider_select();
            return "No provider selected.".to_string();
        };
        let result = self.switch_provider_by_name(&choice.name);
        self.refresh_discovered_models().await;
        self.close_provider_select();
        result
    }

    pub fn switch_provider_by_name(&mut self, name: &str) -> String {
        let registry = crate::services::api::provider::ProviderRegistry::from_env();
        let result = registry.switch_provider(self.streaming_engine.as_deref(), name);
        // switch_provider is async; palette runs in a synchronous context, so
        // block on the trivial persistence future.
        let result = futures::executor::block_on(result);
        self.provider_notice = Some(format!(
            "Provider switched to {}",
            result.split('\n').next().unwrap_or(&result)
        ));
        self.discovered_models.clear();
        result
    }
}
