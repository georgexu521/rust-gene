use super::*;
use priority_agent::services::api::credentials::CredentialSaveOutcome;
use std::process::Command;

pub(crate) trait DesktopCredentialStore {
    fn backend_id(&self) -> &'static str;
    fn backend_label(&self) -> &'static str;
    fn is_available(&self) -> bool;
    fn save_secret(&self, provider_id: &str, key: &str) -> Result<(), String>;
}

struct MacosKeychainCredentialStore;

impl DesktopCredentialStore for MacosKeychainCredentialStore {
    fn backend_id(&self) -> &'static str {
        "system_keychain"
    }

    fn backend_label(&self) -> &'static str {
        "macOS Keychain"
    }

    fn is_available(&self) -> bool {
        cfg!(target_os = "macos") && security_tool_available()
    }

    fn save_secret(&self, provider_id: &str, key: &str) -> Result<(), String> {
        save_to_macos_keychain(provider_id, key)
    }
}

pub(crate) fn desktop_credential_storage_status_value() -> DesktopCredentialStorageStatus {
    let dotenv_path = priority_agent::services::api::credentials::credential_env_path();
    let keychain = MacosKeychainCredentialStore;
    let keychain_available = keychain.is_available();
    DesktopCredentialStorageStatus {
        active_store: if keychain_available {
            keychain.backend_id().to_string()
        } else {
            "dotenv_fallback".to_string()
        },
        preferred_store: if keychain_available {
            keychain.backend_id().to_string()
        } else {
            "dotenv_fallback".to_string()
        },
        system_keychain_available: keychain_available,
        dotenv_fallback_path: dotenv_path.display().to_string(),
        activation_mirror: Some("dotenv_runtime_env".to_string()),
        environment_only_available: true,
        acknowledgement_required: !keychain_available,
        migration_available: keychain_available && dotenv_path.exists(),
        last_updated_source: if keychain_available {
            "system_keychain_available".to_string()
        } else if dotenv_path.exists() {
            "dotenv_file".to_string()
        } else {
            "environment_or_missing".to_string()
        },
        last_save_backend: None,
        detail: if keychain_available {
            "Desktop credential saving uses macOS Keychain when available and mirrors to the Priority Agent dotenv file so the current runtime can activate the provider.".to_string()
        } else {
            "Desktop credential saving currently uses the Priority Agent dotenv fallback; system keychain support is not active on this platform.".to_string()
        },
    }
}

pub(crate) fn save_desktop_provider_credential(
    provider_id: &str,
    key: &str,
) -> Result<String, String> {
    let keychain = MacosKeychainCredentialStore;
    let stores: [&dyn DesktopCredentialStore; 1] = [&keychain];
    let primary_backend = save_preferred_primary_store(provider_id, key, &stores)?;
    let activation = priority_agent::services::api::credentials::save_credential(provider_id, key);
    let provider_id = provider_id.trim();
    match activation {
        CredentialSaveOutcome::Verified => Ok(match primary_backend {
            Some(backend) => format!(
                "Saved and activated key for {provider_id} using {backend}; dotenv mirror updated for runtime activation"
            ),
            None => format!("Saved and activated key for {provider_id} using dotenv fallback"),
        }),
        CredentialSaveOutcome::SavedUnverified => Ok(match primary_backend {
            Some(backend) => format!(
                "Saved key for {provider_id} using {backend}, but provider activation could not be verified; dotenv mirror updated"
            ),
            None => format!(
                "Saved key for {provider_id}, but provider activation could not be verified"
            ),
        }),
        CredentialSaveOutcome::Rejected { reason } => Err(reason),
    }
}

pub(crate) fn save_preferred_primary_store(
    provider_id: &str,
    key: &str,
    stores: &[&dyn DesktopCredentialStore],
) -> Result<Option<&'static str>, String> {
    let provider_id = provider_id.trim();
    let key = key.trim();
    if provider_id.is_empty() || key.is_empty() {
        return Err("provider id and key must not be empty".to_string());
    }
    let Some(store) = stores.iter().find(|store| store.is_available()) else {
        return Ok(None);
    };
    store.save_secret(provider_id, key)?;
    Ok(Some(store.backend_label()))
}

fn security_tool_available() -> bool {
    Command::new("security")
        .arg("-h")
        .output()
        .map(|output| output.status.success() || !output.stderr.is_empty())
        .unwrap_or(false)
}

fn save_to_macos_keychain(provider_id: &str, key: &str) -> Result<(), String> {
    if provider_id.trim().is_empty() || key.trim().is_empty() {
        return Err("provider id and key must not be empty".to_string());
    }
    let status = Command::new("security")
        .args([
            "add-generic-password",
            "-a",
            provider_id.trim(),
            "-s",
            "Priority Agent",
            "-w",
            key.trim(),
            "-U",
        ])
        .status()
        .map_err(|err| format!("cannot invoke macOS Keychain: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "macOS Keychain save failed with status {}",
            status.code().unwrap_or(-1)
        ))
    }
}

#[cfg(test)]
mod credential_store_tests {
    use super::*;
    use std::cell::RefCell;

    struct FakeCredentialStore {
        id: &'static str,
        label: &'static str,
        available: bool,
        result: Result<(), String>,
        saves: RefCell<Vec<(String, String)>>,
    }

    impl DesktopCredentialStore for FakeCredentialStore {
        fn backend_id(&self) -> &'static str {
            self.id
        }

        fn backend_label(&self) -> &'static str {
            self.label
        }

        fn is_available(&self) -> bool {
            self.available
        }

        fn save_secret(&self, provider_id: &str, key: &str) -> Result<(), String> {
            self.saves
                .borrow_mut()
                .push((provider_id.to_string(), key.to_string()));
            self.result.clone()
        }
    }

    fn fake_store(available: bool, result: Result<(), String>) -> FakeCredentialStore {
        FakeCredentialStore {
            id: "fake_keychain",
            label: "Fake Keychain",
            available,
            result,
            saves: RefCell::new(Vec::new()),
        }
    }

    #[test]
    fn preferred_primary_store_skips_unavailable_backends() {
        let store = fake_store(false, Ok(()));

        let backend = save_preferred_primary_store("openai", "sk-test", &[&store]).unwrap();

        assert_eq!(backend, None);
        assert!(store.saves.borrow().is_empty());
    }

    #[test]
    fn preferred_primary_store_saves_to_available_backend() {
        let store = fake_store(true, Ok(()));

        let backend = save_preferred_primary_store(" openai ", " sk-test ", &[&store]).unwrap();

        assert_eq!(backend, Some("Fake Keychain"));
        assert_eq!(
            store.saves.borrow().as_slice(),
            &[("openai".to_string(), "sk-test".to_string())]
        );
    }

    #[test]
    fn preferred_primary_store_surfaces_backend_failure() {
        let store = fake_store(true, Err("backend unavailable".to_string()));

        let err = save_preferred_primary_store("openai", "sk-test", &[&store]).unwrap_err();

        assert_eq!(err, "backend unavailable");
    }
}
