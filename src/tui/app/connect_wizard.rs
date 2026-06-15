//! Interactive `/connect` wizard state machine.
//!
//! Guides the user through selecting a provider, entering an API key, and
//! optionally validating it. The wizard is rendered as an overlay on top of
//! the chat screen.

use crate::services::api::provider_catalog;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectStep {
    /// Provider list selection.
    SelectProvider,
    /// Enter API key.
    InputKey,
    /// Validation in progress.
    Validating,
    /// Result / done.
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WizardStatus {
    None,
    Success(String),
    Error(String),
}

#[derive(Debug, Clone)]
pub struct ConnectWizardState {
    pub step: ConnectStep,
    /// Index into the filtered provider list.
    pub selected: usize,
    /// Search/filter input for the provider list.
    pub query: String,
    /// Currently selected provider id, if any.
    pub provider_id: Option<String>,
    /// Buffer holding the user's API key input.
    pub input_buffer: String,
    /// Mask input to avoid shoulder-surfing.
    pub mask_input: bool,
    /// Result status shown on the final step.
    pub status: WizardStatus,
}

impl Default for ConnectWizardState {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectWizardState {
    pub fn new() -> Self {
        Self {
            step: ConnectStep::SelectProvider,
            selected: 0,
            query: String::new(),
            provider_id: None,
            input_buffer: String::new(),
            mask_input: true,
            status: WizardStatus::None,
        }
    }

    pub fn provider_choices(
        &self,
    ) -> Vec<crate::services::api::provider_catalog::ProviderCatalogEntry> {
        let mut entries = provider_catalog::builtin_catalog();
        let query = self.query.to_ascii_lowercase();
        if !query.is_empty() {
            entries.retain(|entry| {
                entry.id.to_ascii_lowercase().contains(&query)
                    || entry.label.to_ascii_lowercase().contains(&query)
                    || entry.default_model.to_ascii_lowercase().contains(&query)
            });
        }
        entries
    }

    pub fn selected_provider(
        &self,
    ) -> Option<crate::services::api::provider_catalog::ProviderCatalogEntry> {
        self.provider_choices().into_iter().nth(self.selected)
    }

    pub fn select_next(&mut self) {
        let len = self.provider_choices().len();
        if len > 0 {
            self.selected = (self.selected + 1).min(len - 1);
        }
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn push_query(&mut self, c: char) {
        self.query.push(c);
        self.selected = 0;
    }

    pub fn backspace_query(&mut self) {
        self.query.pop();
        self.selected = 0;
    }

    pub fn push_key(&mut self, c: char) {
        self.input_buffer.push(c);
    }

    pub fn backspace_key(&mut self) {
        self.input_buffer.pop();
    }

    pub fn selected_key_env_var(&self) -> Option<String> {
        self.selected_provider()
            .and_then(|entry| entry.key_env_vars.into_iter().next())
    }

    pub fn confirm_provider(&mut self) -> bool {
        if let Some(entry) = self.selected_provider() {
            self.provider_id = Some(entry.id);
            self.step = ConnectStep::InputKey;
            self.input_buffer.clear();
            self.status = WizardStatus::None;
            true
        } else {
            false
        }
    }

    pub fn start_validating(&mut self) {
        self.step = ConnectStep::Validating;
    }

    pub fn finish(&mut self, status: WizardStatus) {
        self.step = ConnectStep::Done;
        self.status = status;
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wizard_starts_at_provider_selection() {
        let wizard = ConnectWizardState::new();
        assert_eq!(wizard.step, ConnectStep::SelectProvider);
        assert!(wizard.provider_id.is_none());
    }

    #[test]
    fn confirm_provider_moves_to_input() {
        let mut wizard = ConnectWizardState::new();
        assert!(wizard.confirm_provider());
        assert_eq!(wizard.step, ConnectStep::InputKey);
        assert!(wizard.provider_id.is_some());
    }

    #[test]
    fn query_filters_providers() {
        let mut wizard = ConnectWizardState::new();
        wizard.push_query('o');
        let choices = wizard.provider_choices();
        assert!(choices.iter().any(|e| e.id == "openai"));
    }

    #[test]
    fn key_input_is_masked_by_default() {
        let mut wizard = ConnectWizardState::new();
        wizard.confirm_provider();
        wizard.push_key('a');
        wizard.push_key('b');
        assert_eq!(wizard.input_buffer, "ab");
        assert!(wizard.mask_input);
    }
}
