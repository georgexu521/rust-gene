use std::collections::HashSet;

pub(super) fn contains_unexecuted_tool_command(
    content: &str,
    exposed_tool_names: &HashSet<String>,
) -> bool {
    exposed_tool_names.contains("bash") && contains_pseudo_bash_command(content)
}

fn contains_pseudo_bash_command(content: &str) -> bool {
    let mut in_command_fence = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(lang) = fenced_code_language(trimmed) {
            in_command_fence = is_command_fence_language(lang);
            continue;
        }
        if trimmed.starts_with("```") {
            in_command_fence = false;
            continue;
        }
        if contains_prefixed_command(trimmed) {
            return true;
        }
        if in_command_fence && looks_like_shell_command(trimmed) {
            return true;
        }
    }
    false
}

fn fenced_code_language(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("```")?;
    if rest.starts_with('`') {
        return None;
    }
    Some(rest.trim())
}

fn is_command_fence_language(lang: &str) -> bool {
    matches!(
        lang.to_ascii_lowercase().as_str(),
        "" | "code" | "bash" | "shell" | "sh" | "zsh" | "console" | "terminal"
    )
}

fn contains_prefixed_command(line: &str) -> bool {
    let trimmed = line
        .trim()
        .trim_start_matches('`')
        .trim_start_matches('$')
        .trim();
    let lower = trimmed.to_ascii_lowercase();
    let Some(command) = lower
        .strip_prefix("bash:")
        .or_else(|| lower.strip_prefix("shell:"))
        .or_else(|| lower.strip_prefix("sh:"))
        .or_else(|| lower.strip_prefix("command:"))
    else {
        return false;
    };
    looks_like_shell_command(command)
}

fn looks_like_shell_command(line: &str) -> bool {
    let command = line
        .trim()
        .trim_start_matches('$')
        .trim_start_matches('>')
        .trim();
    if command.is_empty() || command.starts_with('#') {
        return false;
    }
    matches!(
        command.split_whitespace().next(),
        Some(
            "ls" | "find"
                | "test"
                | "["
                | "stat"
                | "cat"
                | "grep"
                | "rg"
                | "pwd"
                | "du"
                | "wc"
                | "python"
                | "python3"
                | "pip"
                | "pip3"
                | "npm"
                | "pnpm"
                | "yarn"
                | "node"
                | "cargo"
                | "brew"
                | "docker"
                | "pytest"
                | "uv"
                | "uvx"
                | "conda"
                | "which"
                | "where"
                | "command"
                | "git"
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_unexecuted_bash_text_when_bash_is_exposed() {
        let exposed = HashSet::from(["bash".to_string(), "file_read".to_string()]);

        assert!(contains_unexecuted_tool_command(
            "我来查一下。\n\n```code\nbash: ls -la /Users/georgexu/Desktop | grep -i gex\n```",
            &exposed
        ));
    }

    #[test]
    fn normal_answer_is_not_flagged() {
        let exposed = HashSet::from(["bash".to_string()]);

        assert!(!contains_unexecuted_tool_command(
            "桌面上没有名为 gex 的文件夹。",
            &exposed
        ));
    }

    #[test]
    fn ignores_bash_text_when_bash_is_not_exposed() {
        let exposed = HashSet::from(["file_read".to_string()]);

        assert!(!contains_unexecuted_tool_command(
            "bash: ls -la /Users/georgexu/Desktop",
            &exposed
        ));
    }

    #[test]
    fn detects_fenced_command_blocks_when_bash_is_exposed() {
        let exposed = HashSet::from(["bash".to_string()]);

        assert!(contains_unexecuted_tool_command(
            "我先检查：\n\n```code\npython3 -c \"import pygame; print(pygame.__version__)\"\n```\n\n```code\npip3 show pygame\n```",
            &exposed
        ));
    }

    #[test]
    fn detects_bash_fenced_commands_when_bash_is_exposed() {
        let exposed = HashSet::from(["bash".to_string()]);

        assert!(contains_unexecuted_tool_command(
            "```bash\npip3 install pygame\n```",
            &exposed
        ));
    }

    #[test]
    fn does_not_treat_python_source_code_as_shell_command() {
        let exposed = HashSet::from(["bash".to_string()]);

        assert!(!contains_unexecuted_tool_command(
            "```python\nimport pygame\nprint('ready')\n```",
            &exposed
        ));
    }
}
