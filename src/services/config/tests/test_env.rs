static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(super) struct EnvOverride {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
    _guard: std::sync::MutexGuard<'static, ()>,
}

impl EnvOverride {
    pub(super) fn set(key: &'static str, value: &'static str) -> Self {
        let guard = ENV_LOCK.lock().expect("env lock poisoned");
        let previous = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        Self {
            key,
            previous,
            _guard: guard,
        }
    }
}

impl Drop for EnvOverride {
    fn drop(&mut self) {
        unsafe {
            if let Some(value) = &self.previous {
                std::env::set_var(self.key, value);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }
}
