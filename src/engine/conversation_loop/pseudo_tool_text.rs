//! Conversation-loop controller module.
//!
//! Owns one focused stage of turn execution so permissions, validation, repair, and closeout stay explicit in the runtime.

use std::collections::HashSet;

pub(super) fn contains_unexecuted_tool_command(
    content: &str,
    exposed_tool_names: &HashSet<String>,
) -> bool {
    exposed_tool_names.contains("bash") && contains_pseudo_bash_command(content)
}

pub(super) fn contains_false_bash_unavailable_claim(
    content: &str,
    exposed_tool_names: &HashSet<String>,
) -> bool {
    exposed_tool_names.contains("bash") && contains_bash_unavailable_claim(content)
}

pub(super) fn contains_local_filesystem_claim_without_tool(
    content: &str,
    exposed_tool_names: &HashSet<String>,
) -> bool {
    (exposed_tool_names.contains("file_read") || exposed_tool_names.contains("glob"))
        && contains_local_filesystem_claim(content)
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

fn contains_bash_unavailable_claim(content: &str) -> bool {
    let lower = content.to_ascii_lowercase();
    let direct_bash_claim = contains_any(
        &lower,
        &[
            "bash tool is not available",
            "bash is not available",
            "shell tool is not available",
            "terminal is not available",
            "cannot execute commands",
            "can't execute commands",
            "cannot run commands",
            "can't run commands",
        ],
    ) || contains_any(
        content,
        &[
            "bash 工具在当前会话中不可用",
            "bash工具在当前会话中不可用",
            "bash 工具不可用",
            "bash工具不可用",
            "shell 工具不可用",
            "终端工具不可用",
            "无法帮你直接执行命令",
            "无法直接执行命令",
            "不能直接执行命令",
            "无法直接运行命令",
            "不能直接运行命令",
            "请打开你的终端",
            "请在终端手动运行",
            "请手动运行以下命令",
        ],
    );
    if !direct_bash_claim {
        return false;
    }

    contains_any(
        &lower,
        &[
            "bash", "shell", "terminal", "command", "python", "python3", "pip", "pip3", "npm",
            "cargo",
        ],
    ) || contains_any(content, &["bash", "命令", "终端", "python", "安装", "运行"])
}

fn contains_local_filesystem_claim(content: &str) -> bool {
    let lower = content.to_ascii_lowercase();
    let mentions_local_path = contains_any(
        &lower,
        &[
            "desktop",
            "~/",
            "/users/",
            "folder",
            "directory",
            "file",
            ".txt",
            ".md",
            ".py",
        ],
    ) || contains_any(
        content,
        &["桌面", "文件夹", "目录", "文件", "路径", "里面", "gex"],
    );
    let asserts_state = contains_any(
        &lower,
        &[
            "exists",
            "does not exist",
            "not exist",
            "there is",
            "there are",
            "contains",
            "empty",
            "no ",
            "path:",
            "size:",
            "created",
        ],
    ) || contains_any(
        content,
        &[
            "存在",
            "不存在",
            "没有",
            "有",
            "里面有",
            "只有",
            "为空",
            "是空的",
            "路径：",
            "大小：",
            "创建时间",
            "内容数",
        ],
    );

    mentions_local_path && asserts_state && !content.trim_end().ends_with('?')
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
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
    fn detects_false_bash_unavailable_claim_when_bash_is_exposed() {
        let exposed = HashSet::from(["bash".to_string()]);

        assert!(contains_false_bash_unavailable_claim(
            "根据会话配置，bash 工具在当前会话中不可用，无法帮你直接执行命令。\n请手动运行以下命令：pip3 install pygame",
            &exposed
        ));
    }

    #[test]
    fn ignores_bash_unavailable_claim_when_bash_is_not_exposed() {
        let exposed = HashSet::from(["file_read".to_string()]);

        assert!(!contains_false_bash_unavailable_claim(
            "bash 工具在当前会话中不可用，请手动运行 pip3 install pygame",
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

    #[test]
    fn detects_local_filesystem_claim_without_tool_when_read_tools_are_exposed() {
        let exposed = HashSet::from(["file_read".to_string(), "glob".to_string()]);

        assert!(contains_local_filesystem_claim_without_tool(
            "是的，桌面上有 gex 文件夹。\n路径：~/Desktop/gex\n大小：96 字节",
            &exposed
        ));
    }

    #[test]
    fn ignores_local_filesystem_claim_when_read_tools_are_not_exposed() {
        let exposed = HashSet::from(["ask_user".to_string()]);

        assert!(!contains_local_filesystem_claim_without_tool(
            "桌面上没有 gex 文件夹。",
            &exposed
        ));
    }

    #[test]
    fn does_not_flag_clarifying_filesystem_question() {
        let exposed = HashSet::from(["file_read".to_string(), "glob".to_string()]);

        assert!(!contains_local_filesystem_claim_without_tool(
            "你说的这个文件夹是 ~/Desktop/gex 吗？",
            &exposed
        ));
    }
}
