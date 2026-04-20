//! 新用户引导系统
//!
//! 检测首次启动，在 TUI 中显示交互式引导流程。
//! 完成后写入标志文件 `~/.priority-agent/.onboarded` 避免重复显示。

use std::path::PathBuf;

/// 引导步骤
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingStep {
    Welcome,
    ApiKey,
    Commands,
    Permissions,
    Done,
}

impl OnboardingStep {
    pub fn next(self) -> Option<Self> {
        match self {
            Self::Welcome => Some(Self::ApiKey),
            Self::ApiKey => Some(Self::Commands),
            Self::Commands => Some(Self::Permissions),
            Self::Permissions => Some(Self::Done),
            Self::Done => None,
        }
    }

    pub fn prev(self) -> Option<Self> {
        match self {
            Self::Welcome => None,
            Self::ApiKey => Some(Self::Welcome),
            Self::Commands => Some(Self::ApiKey),
            Self::Permissions => Some(Self::Commands),
            Self::Done => Some(Self::Permissions),
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Welcome => "Welcome",
            Self::ApiKey => "API Key Setup",
            Self::Commands => "Commands",
            Self::Permissions => "Permissions",
            Self::Done => "All Set!",
        }
    }

    pub fn index(&self) -> usize {
        match self {
            Self::Welcome => 0,
            Self::ApiKey => 1,
            Self::Commands => 2,
            Self::Permissions => 3,
            Self::Done => 4,
        }
    }

    pub fn total_steps() -> usize {
        5
    }

    pub fn content(&self) -> &'static str {
        match self {
            Self::Welcome => {
                "Welcome to Priority Agent!\n\n\
                 This is a Claude Code-inspired AI assistant with:\n\
                 - File read/write/edit tools\n\
                 - Bash command execution\n\
                 - Web search and fetch\n\
                 - Git integration\n\
                 - Sub-agent delegation\n\
                 - And much more...\n\n\
                 Let's get you set up in a few quick steps."
            }
            Self::ApiKey => {
                "To use the AI features, you need an API key.\n\n\
                 Supported providers:\n\
                 - Kimi/Moonshot: Set MOONSHOT_API_KEY\n\
                 - OpenAI: Set OPENAI_API_KEY\n\n\
                 You can set it in your shell profile:\n\
                 export MOONSHOT_API_KEY=\"your-key-here\"\n\n\
                 Or create ~/.config/priority-agent/config.toml:\n\
                 [api]\n\
                 api_key = \"your-key\"\n\
                 base_url = \"https://api.moonshot.cn/v1\"\n\
                 model = \"kimi-k2.5\"\n\n\
                 The app will still work without a key in legacy mode (--legacy)."
            }
            Self::Commands => {
                "Useful slash commands in the TUI:\n\n\
                 /help        - Show all available commands\n\
                 /settings    - Open settings panel\n\
                 /permissions - View/update permission rules\n\
                 /vim         - Toggle vim keybindings\n\
                 /share       - Export session to markdown\n\
                 /doctor      - Diagnose environment issues\n\
                 /voice       - Check voice TTS/STT status\n\
                 /telemetry   - View performance tracking status\n\
                 /rewind      - Rollback recent file edits\n\n\
                 Just type your message and press Enter to chat!"
            }
            Self::Permissions => {
                "Permission modes control what tools can run automatically:\n\n\
                 - default: Ask for confirmation on risky operations\n\
                 - auto_low_risk: Auto-approve safe tools (file_read, grep, etc.)\n\
                 - auto_all: Auto-approve everything (use with caution)\n\
                 - read_only: Only allow read operations\n\n\
                 Use /permissions to switch modes at any time.\n\
                 You can also set per-tool rules for finer control."
            }
            Self::Done => {
                "You're all set!\n\n\
                 Quick tips:\n\
                 - Press Enter to send a message\n\
                 - Shift+Enter for multiline input\n\
                 - Press Ctrl+C to exit\n\
                 - Type /help anytime for commands\n\n\
                 Enjoy using Priority Agent!"
            }
        }
    }
}

/// 引导管理器
#[derive(Debug, Clone)]
pub struct OnboardingManager {
    flag_path: PathBuf,
}

impl OnboardingManager {
    pub fn new() -> Self {
        let flag_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".priority-agent")
            .join(".onboarded");
        Self { flag_path }
    }

    /// 检测是否需要显示引导（首次启动）
    pub fn is_first_run(&self) -> bool {
        !self.flag_path.exists()
    }

    /// 标记引导已完成
    pub fn mark_complete(&self) -> std::io::Result<()> {
        if let Some(parent) = self.flag_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.flag_path, b"1")?;
        Ok(())
    }

    /// 重置引导状态（用于测试或重新引导）
    pub fn reset(&self) -> std::io::Result<()> {
        if self.flag_path.exists() {
            std::fs::remove_file(&self.flag_path)?;
        }
        Ok(())
    }

    /// 获取标志文件路径
    pub fn flag_path(&self) -> &PathBuf {
        &self.flag_path
    }
}

impl Default for OnboardingManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 引导状态
#[derive(Debug, Clone)]
pub struct OnboardingState {
    pub step: OnboardingStep,
    pub manager: OnboardingManager,
}

impl OnboardingState {
    pub fn new() -> Self {
        Self {
            step: OnboardingStep::Welcome,
            manager: OnboardingManager::new(),
        }
    }

    pub fn next_step(&mut self) -> bool {
        if let Some(next) = self.step.next() {
            self.step = next;
            true
        } else {
            false
        }
    }

    pub fn prev_step(&mut self) -> bool {
        if let Some(prev) = self.step.prev() {
            self.step = prev;
            true
        } else {
            false
        }
    }

    pub fn is_done(&self) -> bool {
        self.step == OnboardingStep::Done
    }

    pub fn complete(&self) -> std::io::Result<()> {
        self.manager.mark_complete()
    }
}

impl Default for OnboardingState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_onboarding_step_flow() {
        let mut step = OnboardingStep::Welcome;
        step = step.next().unwrap();
        assert!(matches!(step, OnboardingStep::ApiKey));
        step = step.next().unwrap();
        assert!(matches!(step, OnboardingStep::Commands));
        step = step.next().unwrap();
        assert!(matches!(step, OnboardingStep::Permissions));
        step = step.next().unwrap();
        assert!(matches!(step, OnboardingStep::Done));
        assert!(step.next().is_none());
    }

    #[test]
    fn test_onboarding_manager_first_run() {
        let manager = OnboardingManager::new();
        // 清理任何已有的标志
        let _ = manager.reset();
        assert!(manager.is_first_run());

        manager.mark_complete().unwrap();
        assert!(!manager.is_first_run());

        // 清理
        let _ = manager.reset();
    }

    #[test]
    fn test_onboarding_state_navigation() {
        let mut state = OnboardingState::new();
        assert!(matches!(state.step, OnboardingStep::Welcome));
        assert!(!state.is_done());

        assert!(state.next_step()); // Welcome -> ApiKey
        assert!(state.next_step()); // ApiKey -> Commands
        assert!(state.next_step()); // Commands -> Permissions
        assert!(state.next_step()); // Permissions -> Done
        assert!(!state.next_step()); // Already at Done

        assert!(state.is_done());
    }
}
