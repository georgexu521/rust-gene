//! Bash command arity dictionary.
//!
//! Maps command prefixes to the number of tokens that form a "human-
//! understandable" scope for permission always-allow patterns.
//!
//! For example, `git push origin main` with arity 2 produces the
//! always-allow pattern `git *`, while `npm run dev` with arity 3
//! produces `npm run *`.
//!
//! This dictionary is used to generate **permission rule suggestions**
//! — it never auto-writes `always_allow` rules.  Dangerous, network,
//! external-path, compound, and wrapper commands get exact-command
//! scopes only (no broad prefix).

use once_cell::sync::Lazy;
use std::collections::HashMap;

static ARITY: Lazy<HashMap<&'static str, usize>> = Lazy::new(|| {
    let mut m = HashMap::new();

    // ── Unix basics ────────────────────────────────────────────
    for cmd in [
        "cat", "cp", "mv", "rm", "mkdir", "ls", "touch", "chmod", "chown", "echo", "grep", "kill",
        "ps", "pwd", "tail", "cd", "sleep", "source", "which", "head", "wc", "sort", "uniq", "cut",
        "tr", "sed", "awk", "find", "xargs", "tee", "du", "df", "ln", "diff",
    ] {
        m.insert(cmd, 1);
    }

    // ── Git ─────────────────────────────────────────────────────
    m.insert("git", 2);
    m.insert("git remote", 3);
    m.insert("git stash", 3);
    m.insert("git config", 3);
    m.insert("git branch", 3);
    m.insert("git tag", 3);
    m.insert("git worktree", 3);
    m.insert("git submodule", 3);

    // ── Package managers ────────────────────────────────────────
    m.insert("npm", 2);
    m.insert("npm run", 3);
    m.insert("npm exec", 3);
    m.insert("npm init", 3);
    m.insert("npm install", 3);
    m.insert("npm view", 3);
    m.insert("pnpm", 2);
    m.insert("pnpm run", 3);
    m.insert("yarn", 2);
    m.insert("yarn run", 3);
    m.insert("bun", 2);
    m.insert("bun run", 3);
    m.insert("cargo", 2);
    m.insert("cargo run", 3);
    m.insert("cargo add", 3);
    m.insert("cargo test", 3);
    m.insert("cargo build", 3);
    m.insert("cargo install", 3);
    m.insert("pip", 2);
    m.insert("pip install", 3);
    m.insert("pip3", 2);
    m.insert("pip3 install", 3);
    m.insert("pipx", 2);
    m.insert("pipx install", 3);
    m.insert("brew", 2);
    m.insert("brew install", 3);

    // ── Containers ──────────────────────────────────────────────
    m.insert("docker", 2);
    m.insert("docker compose", 3);
    m.insert("docker container", 3);
    m.insert("docker image", 3);
    m.insert("docker network", 3);
    m.insert("docker volume", 3);
    m.insert("docker buildx", 3);
    m.insert("podman", 2);
    m.insert("podman container", 3);
    m.insert("podman image", 3);

    // ── Kubernetes ──────────────────────────────────────────────
    m.insert("kubectl", 2);
    m.insert("kubectl kustomize", 3);
    m.insert("kubectl rollout", 3);
    m.insert("kind", 2);
    m.insert("kind create", 3);
    m.insert("helm", 2);
    m.insert("helm install", 3);
    m.insert("helm upgrade", 3);

    // ── Cloud / Infra ───────────────────────────────────────────
    m.insert("terraform", 2);
    m.insert("terraform workspace", 3);
    m.insert("terraform init", 3);
    m.insert("terraform plan", 3);
    m.insert("terraform apply", 3);
    m.insert("aws", 3);
    m.insert("az", 3);
    m.insert("gcloud", 3);
    m.insert("cdk", 2);
    m.insert("serverless", 2);
    m.insert("pulumi", 2);
    m.insert("pulumi stack", 3);
    m.insert("ansible", 2);

    // ── Languages / build ───────────────────────────────────────
    m.insert("go", 2);
    m.insert("go run", 3);
    m.insert("go build", 3);
    m.insert("go test", 3);
    m.insert("python", 2);
    m.insert("python3", 2);
    m.insert("rustup", 2);
    m.insert("rustup target", 3);
    m.insert("rustup component", 3);
    m.insert("deno", 2);
    m.insert("deno task", 3);
    m.insert("deno run", 3);
    m.insert("make", 2);
    m.insert("cmake", 2);
    m.insert("bazel", 2);
    m.insert("bazel build", 3);
    m.insert("bazel test", 3);
    m.insert("gradle", 2);
    m.insert("mvn", 2);
    m.insert("nx", 2);
    m.insert("turbo", 2);

    // ── Databases ───────────────────────────────────────────────
    m.insert("psql", 2);
    m.insert("mysql", 2);
    m.insert("mongosh", 2);
    m.insert("redis-cli", 2);
    m.insert("sqlite3", 2);

    // ── GitHub / VCS tools ──────────────────────────────────────
    m.insert("gh", 3);
    m.insert("gh pr", 3);
    m.insert("gh issue", 3);
    m.insert("gh release", 3);

    // ── Other common tools ──────────────────────────────────────
    m.insert("curl", 2);
    m.insert("wget", 2);
    m.insert("systemctl", 2);
    m.insert("journalctl", 2);
    m.insert("ufw", 2);
    m.insert("flyctl", 2);
    m.insert("heroku", 2);
    m.insert("vercel", 2);
    m.insert("hugo", 2);
    m.insert("composer", 2);
    m.insert("ip", 2);
    m.insert("ip addr", 3);
    m.insert("ip link", 3);
    m.insert("ip route", 3);
    m.insert("openssl", 2);
    m.insert("openssl req", 3);
    m.insert("openssl x509", 3);
    m.insert("tmux", 2);
    m.insert("sst", 2);
    m.insert("swift", 2);
    m.insert("rake", 2);
    m.insert("nvm", 2);
    m.insert("volta", 2);
    m.insert("rbenv", 2);
    m.insert("pyenv", 2);
    m.insert("pipenv", 2);
    m.insert("poetry", 2);
    m.insert("poetry add", 3);
    m.insert("poetry run", 3);
    m.insert("npx", 2);

    m
});

/// Control‑word set for commands that only change the shell state
/// and do not need a `bash` permission request.
pub const CWD_CMDS: &[&str] = &["cd", "chdir", "pushd", "popd"];

/// Commands known to create, modify, or delete files.
pub const FILE_OPS: &[&str] = &[
    "rm",
    "cp",
    "mv",
    "mkdir",
    "touch",
    "chmod",
    "chown",
    "cat",
    "tee",
    // PowerShell / Windows aliases
    "get-content",
    "set-content",
    "add-content",
    "copy-item",
    "move-item",
    "remove-item",
    "new-item",
    "rename-item",
    // cmd.exe
    "copy",
    "del",
    "erase",
    "rd",
    "ren",
    "rename",
    "rmdir",
    "type",
];

/// Compute the arity-derived permission scope for a command token list.
///
/// Tries the longest matching prefix first, then progressively shortens.
/// Falls back to just the command name when no dictionary entry is found.
pub fn arity_prefix(tokens: &[String]) -> Vec<String> {
    for len in (1..=tokens.len()).rev() {
        let prefix = tokens[..len].join(" ");
        if let Some(&arity) = ARITY.get(prefix.as_str()) {
            return tokens[..arity].to_vec();
        }
    }
    tokens.first().map(|t| vec![t.clone()]).unwrap_or_default()
}

/// Generate an always-allow wildcard pattern from a token list.
pub fn always_pattern(tokens: &[String]) -> String {
    let prefix = arity_prefix(tokens);
    format!("{} *", prefix.join(" "))
}

/// Whether the command is a file-manipulating operation whose path
/// arguments should trigger external-directory checks.
pub fn is_file_operation(cmd_name: &str) -> bool {
    let lower = cmd_name.to_lowercase();
    FILE_OPS.iter().any(|op| *op == lower)
}

/// Whether the command is a directory‑change (cd-alike).
pub fn is_cwd_change(cmd_name: &str) -> bool {
    let lower = cmd_name.to_lowercase();
    CWD_CMDS.iter().any(|c| *c == lower)
}

/// Whether an arity‑scoped always-allow suggestion is safe to offer.
///
/// Returns `false` for: dangerous commands, network‑enabled commands,
/// commands targeting external paths, compound commands with `&&`/`||`,
/// and shell-wrapper commands (`bash -c`, `sh -c`, `eval`, `source`).
pub fn arity_suggestion_safe(
    executable: &str,
    has_external_paths: bool,
    has_network: bool,
    has_compound: bool,
    is_wrapper: bool,
) -> bool {
    if has_external_paths || has_network || has_compound || is_wrapper {
        return false;
    }
    // Also block known dangerous / privileged commands.
    let lower = executable.to_lowercase();
    !matches!(
        lower.as_str(),
        "sudo"
            | "doas"
            | "pkexec"
            | "su"
            | "chown"
            | "chmod"
            | "rm"
            | "mkfs"
            | "dd"
            | "mount"
            | "umount"
            | "shutdown"
            | "reboot"
            | "init"
            | "systemctl"
            | "kill"
            | "pkill"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(cmd: &str) -> Vec<String> {
        cmd.split_whitespace().map(|s| s.to_string()).collect()
    }

    #[test]
    fn arity_git_returns_git_wildcard() {
        let t = tokens("git push origin main");
        // arity(git)=2 → takes first 2 tokens → "git push *"
        assert_eq!(always_pattern(&t), "git push *");
    }

    #[test]
    fn arity_git_remote_returns_more_specific() {
        let t = tokens("git remote add upstream https://...");
        // arity(git remote)=3 → takes first 3 tokens → "git remote add *"
        assert_eq!(always_pattern(&t), "git remote add *");
    }

    #[test]
    fn arity_npm_run_is_npm_run() {
        let t = tokens("npm run dev");
        // arity(npm run)=3 → takes first 3 tokens → "npm run dev *"
        assert_eq!(always_pattern(&t), "npm run dev *");
    }

    #[test]
    fn arity_cargo_test_is_cargo_test() {
        let t = tokens("cargo test --lib");
        // arity(cargo test)=3 → takes first 3 tokens → "cargo test --lib *"
        assert_eq!(always_pattern(&t), "cargo test --lib *");
    }

    #[test]
    fn arity_unknown_falls_back_to_command_name() {
        let t = tokens("my-custom-tool --flag value");
        assert_eq!(arity_prefix(&t), vec!["my-custom-tool"]);
    }

    #[test]
    fn arity_docker_compose_is_narrow() {
        let t = tokens("docker compose up -d");
        // arity(docker compose)=3 → takes first 3 tokens → "docker compose up *"
        assert_eq!(always_pattern(&t), "docker compose up *");
    }

    #[test]
    fn arity_suggestion_rejected_for_sudo() {
        assert!(!arity_suggestion_safe("sudo", false, false, false, false));
    }

    #[test]
    fn arity_suggestion_rejected_for_rm() {
        assert!(!arity_suggestion_safe("rm", false, false, false, false));
    }

    #[test]
    fn arity_suggestion_rejected_for_compound() {
        assert!(!arity_suggestion_safe("git", false, false, true, false));
    }

    #[test]
    fn arity_suggestion_ok_for_git() {
        assert!(arity_suggestion_safe("git", false, false, false, false));
    }

    #[test]
    fn file_ops_detect_mutation_commands() {
        assert!(is_file_operation("rm"));
        assert!(is_file_operation("cp"));
        assert!(is_file_operation("mv"));
        assert!(!is_file_operation("ls"));
        assert!(!is_file_operation("echo"));
    }

    #[test]
    fn cwd_cmds_detect_directory_changers() {
        assert!(is_cwd_change("cd"));
        assert!(is_cwd_change("pushd"));
        assert!(!is_cwd_change("ls"));
    }
}
