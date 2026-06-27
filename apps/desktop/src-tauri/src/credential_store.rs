use super::*;
use priority_agent::services::api::credentials::{CredentialRemoveOutcome, CredentialSaveOutcome};
use std::process::Command;

pub(crate) trait DesktopCredentialStore {
    fn backend_id(&self) -> &'static str;
    fn backend_label(&self) -> &'static str;
    fn is_available(&self) -> bool;
    fn save_secret(&self, provider_id: &str, key: &str) -> Result<(), String>;
    fn load_status(
        &self,
        provider_id: &str,
    ) -> Result<DesktopCredentialProviderBackendStatus, String>;
    fn delete_secret(&self, provider_id: &str) -> Result<(), String>;
    fn migrate_from_dotenv(&self, provider_id: &str, key: &str) -> Result<(), String> {
        self.save_secret(provider_id, key)
    }
    fn backend_health(&self) -> DesktopCredentialBackendHealth {
        let available = self.is_available();
        DesktopCredentialBackendHealth {
            backend_id: self.backend_id().to_string(),
            backend_label: self.backend_label().to_string(),
            available,
            can_save: available,
            can_load_status: available,
            can_delete: available,
            can_migrate_from_dotenv: available,
        }
    }
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

    fn load_status(
        &self,
        provider_id: &str,
    ) -> Result<DesktopCredentialProviderBackendStatus, String> {
        let available = self.is_available();
        let credential_present = available && macos_keychain_credential_exists(provider_id)?;
        Ok(DesktopCredentialProviderBackendStatus {
            backend_id: self.backend_id().to_string(),
            backend_label: self.backend_label().to_string(),
            provider_id: provider_id.trim().to_string(),
            available,
            credential_present,
            detail: if !available {
                "macOS Keychain backend is not available".to_string()
            } else if credential_present {
                "credential exists in macOS Keychain".to_string()
            } else {
                "credential not found in macOS Keychain".to_string()
            },
        })
    }

    fn delete_secret(&self, provider_id: &str) -> Result<(), String> {
        delete_from_macos_keychain(provider_id)
    }
}

pub(crate) fn desktop_credential_storage_status_value() -> DesktopCredentialStorageStatus {
    let dotenv_path = priority_agent::services::api::credentials::credential_env_path();
    let keychain = MacosKeychainCredentialStore;
    let keychain_available = keychain.is_available();
    let backend_health = vec![keychain.backend_health()];
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
        backend_health,
        dotenv_fallback_path: dotenv_path.display().to_string(),
        activation_mirror: Some("dotenv_runtime_env".to_string()),
        environment_only_available: true,
        acknowledgement_required: !keychain_available,
        migration_available: keychain_available && dotenv_path.exists(),
        delete_available: true,
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

pub(crate) fn desktop_provider_credential_backend_status(
    provider_id: &str,
) -> Result<DesktopCredentialProviderBackendStatus, String> {
    let keychain = MacosKeychainCredentialStore;
    keychain.load_status(provider_id)
}

pub(crate) fn delete_desktop_provider_credential(provider_id: &str) -> Result<String, String> {
    let provider_id = provider_id.trim();
    if provider_id.is_empty() {
        return Err("provider id must not be empty".to_string());
    }
    let keychain = MacosKeychainCredentialStore;
    let keychain_deleted = if keychain.is_available() {
        keychain.delete_secret(provider_id)?;
        true
    } else {
        false
    };
    let dotenv_deleted =
        match priority_agent::services::api::credentials::remove_credential(provider_id) {
            CredentialRemoveOutcome::Removed => true,
            CredentialRemoveOutcome::NotFound => false,
            CredentialRemoveOutcome::Rejected { reason } => return Err(reason),
        };
    Ok(match (keychain_deleted, dotenv_deleted) {
        (true, true) => {
            format!("Deleted key for {provider_id} from macOS Keychain and dotenv mirror")
        }
        (true, false) => format!("Deleted key for {provider_id} from macOS Keychain"),
        (false, true) => format!("Deleted key for {provider_id} from dotenv fallback"),
        (false, false) => format!("No stored key found for {provider_id}"),
    })
}

pub(crate) fn migrate_desktop_provider_credential_to_keychain(
    provider_id: &str,
) -> Result<String, String> {
    let provider_id = provider_id.trim();
    if provider_id.is_empty() {
        return Err("provider id must not be empty".to_string());
    }
    let keychain = MacosKeychainCredentialStore;
    if !keychain.is_available() {
        return Err("macOS Keychain backend is not available".to_string());
    }
    let _ = priority_agent::services::api::credentials::load_product_credential_env();
    let status = priority_agent::services::api::credentials::status_for(provider_id)
        .ok_or_else(|| format!("unknown provider '{provider_id}'"))?;
    let env_var = status
        .active_env_var
        .ok_or_else(|| format!("no dotenv credential found for {provider_id}"))?;
    let key = std::env::var(&env_var)
        .map_err(|_| format!("no runtime credential value found for {env_var}"))?;
    keychain.migrate_from_dotenv(provider_id, &key)?;
    Ok(format!(
        "Migrated key for {provider_id} from dotenv mirror to macOS Keychain"
    ))
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

fn macos_keychain_credential_exists(provider_id: &str) -> Result<bool, String> {
    if provider_id.trim().is_empty() {
        return Err("provider id must not be empty".to_string());
    }
    let output = Command::new("security")
        .args([
            "find-generic-password",
            "-a",
            provider_id.trim(),
            "-s",
            "Priority Agent",
        ])
        .output()
        .map_err(|err| format!("cannot query macOS Keychain: {err}"))?;
    Ok(output.status.success())
}

fn delete_from_macos_keychain(provider_id: &str) -> Result<(), String> {
    if provider_id.trim().is_empty() {
        return Err("provider id must not be empty".to_string());
    }
    let output = Command::new("security")
        .args([
            "delete-generic-password",
            "-a",
            provider_id.trim(),
            "-s",
            "Priority Agent",
        ])
        .output()
        .map_err(|err| format!("cannot invoke macOS Keychain: {err}"))?;
    if output.status.success() || output.status.code() == Some(44) {
        Ok(())
    } else {
        Err(format!(
            "macOS Keychain delete failed with status {}",
            output.status.code().unwrap_or(-1)
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
        present: bool,
        deletes: RefCell<Vec<String>>,
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

        fn load_status(
            &self,
            provider_id: &str,
        ) -> Result<DesktopCredentialProviderBackendStatus, String> {
            Ok(DesktopCredentialProviderBackendStatus {
                backend_id: self.backend_id().to_string(),
                backend_label: self.backend_label().to_string(),
                provider_id: provider_id.to_string(),
                available: self.available,
                credential_present: self.available && self.present,
                detail: "fake status".to_string(),
            })
        }

        fn delete_secret(&self, provider_id: &str) -> Result<(), String> {
            self.deletes.borrow_mut().push(provider_id.to_string());
            self.result.clone()
        }
    }

    fn fake_store(available: bool, result: Result<(), String>) -> FakeCredentialStore {
        FakeCredentialStore {
            id: "fake_keychain",
            label: "Fake Keychain",
            available,
            result,
            present: available,
            deletes: RefCell::new(Vec::new()),
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

    #[test]
    fn backend_status_reports_present_credentials() {
        let store = fake_store(true, Ok(()));

        let status = store.load_status("openai").unwrap();

        assert_eq!(status.provider_id, "openai");
        assert!(status.available);
        assert!(status.credential_present);
    }

    #[test]
    fn backend_health_reports_supported_keychain_operations() {
        let store = fake_store(true, Ok(()));

        let health = store.backend_health();

        assert_eq!(health.backend_id, "fake_keychain");
        assert!(health.available);
        assert!(health.can_save);
        assert!(health.can_load_status);
        assert!(health.can_delete);
        assert!(health.can_migrate_from_dotenv);
    }

    #[test]
    fn fake_backend_delete_and_migration_use_store_methods() {
        let store = fake_store(true, Ok(()));

        store.delete_secret("openai").unwrap();
        store.migrate_from_dotenv("deepseek", "sk-test").unwrap();

        assert_eq!(store.deletes.borrow().as_slice(), &["openai".to_string()]);
        assert_eq!(
            store.saves.borrow().as_slice(),
            &[("deepseek".to_string(), "sk-test".to_string())]
        );
    }
}
