//! E2E scenario tests using MockProvider.
//!
//! These tests verify complete flows: user prompt → tool call → observation → closeout.

use super::mock_provider::MockProvider;
use priority_agent::services::api::LlmProvider;

#[test]
fn e2e_smoke_mock_provider_compiles() {
    let provider = MockProvider::new("mock-model");
    assert_eq!(provider.base_url(), "http://mock");
    assert_eq!(provider.default_model(), "mock-model");
}
