use once_cell::sync::Lazy;
use std::collections::HashMap;

static ENV_LOCK: Lazy<tokio::sync::Mutex<()>> = Lazy::new(|| tokio::sync::Mutex::new(()));

/// Test-only process env guard.
///
/// Ensures env var mutation is serialized and automatically restored on drop.
pub struct EnvVarGuard {
    _lock: tokio::sync::MutexGuard<'static, ()>,
    saved: HashMap<String, Option<String>>,
}

impl EnvVarGuard {
    pub async fn acquire() -> Self {
        let lock = ENV_LOCK.lock().await;
        Self {
            _lock: lock,
            saved: HashMap::new(),
        }
    }

    pub fn acquire_blocking() -> Self {
        let lock = ENV_LOCK.blocking_lock();
        Self {
            _lock: lock,
            saved: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.capture_if_needed(key);
        // SAFETY: guarded by process-wide ENV_LOCK for tests.
        unsafe { std::env::set_var(key, value) };
    }

    pub fn remove(&mut self, key: &str) {
        self.capture_if_needed(key);
        // SAFETY: guarded by process-wide ENV_LOCK for tests.
        unsafe { std::env::remove_var(key) };
    }

    fn capture_if_needed(&mut self, key: &str) {
        if self.saved.contains_key(key) {
            return;
        }
        self.saved.insert(key.to_string(), std::env::var(key).ok());
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        for (key, old_value) in self.saved.drain() {
            match old_value {
                Some(v) => {
                    // SAFETY: guarded by process-wide ENV_LOCK for tests.
                    unsafe { std::env::set_var(key, v) };
                }
                None => {
                    // SAFETY: guarded by process-wide ENV_LOCK for tests.
                    unsafe { std::env::remove_var(key) };
                }
            }
        }
    }
}
