//! Intent-routing heuristic support.
//!
//! Keeps deterministic route hints separate from the model-owned engineering judgment.

use super::RiskLevel;
use std::collections::HashMap;

pub(super) fn repeated_tools(tools: &[String], min_count: usize) -> Vec<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for tool in tools {
        *counts.entry(tool.clone()).or_default() += 1;
    }
    let mut repeated = counts
        .into_iter()
        .filter_map(|(tool, count)| (count >= min_count).then_some(tool))
        .collect::<Vec<_>>();
    repeated.sort();
    repeated
}

pub(super) fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

pub(super) fn is_debug_request(lower: &str, zh: &str) -> bool {
    contains_any(
        lower,
        &[
            "live coding regression task",
            "type: `bug_fix`",
            "type: bug_fix",
            "bug_fix",
            "fix",
            "bug",
            "error",
            "panic",
            "fail",
            "failing",
            "debug",
        ],
    ) || contains_any(zh, &["报错", "错误", "修复", "失败", "调试", "bug"])
        || (zh.contains("问题") && contains_any(zh, &["运行", "启动", "执行", "测试"]))
}

pub(super) fn is_live_coding_code_change_request(lower: &str) -> bool {
    (lower.contains("live coding regression task") && !is_no_diff_eval_intent(lower))
        || (lower.contains("eval intent") && lower.contains("seeded_code_change"))
        || lower.contains("this is a real code-change evaluation")
}

pub(super) fn is_live_coding_audit_request(lower: &str) -> bool {
    lower.contains("live coding regression task") && is_no_diff_eval_intent(lower)
}

pub(super) fn is_no_diff_eval_intent(lower: &str) -> bool {
    lower.contains("eval intent: `audit_or_regression_check`")
        || lower.contains("eval intent: audit_or_regression_check")
        || lower.contains("eval intent: `stale_or_already_satisfied`")
        || lower.contains("eval intent: stale_or_already_satisfied")
}

pub(super) fn live_coding_risk(lower: &str) -> RiskLevel {
    if lower.contains("risk: `high`")
        || lower.contains("risk: high")
        || lower.contains("- risk: `high`")
    {
        RiskLevel::High
    } else {
        RiskLevel::Medium
    }
}

pub(super) fn code_change_tool_recommendations() -> Vec<String> {
    vec![
        "project_list".into(),
        "grep".into(),
        "file_read".into(),
        "file_write".into(),
        "file_edit".into(),
        "bash".into(),
    ]
}

pub(super) fn code_change_tool_recommendations_for(dependency_install_intent: bool) -> Vec<String> {
    let mut tools = code_change_tool_recommendations();
    maybe_recommend_dependency_install_tool(&mut tools, dependency_install_intent);
    tools
}

pub(super) fn debug_tool_recommendations(dependency_install_intent: bool) -> Vec<String> {
    let mut tools = vec!["grep".into(), "file_read".into(), "bash".into()];
    maybe_recommend_dependency_install_tool(&mut tools, dependency_install_intent);
    tools
}

pub(super) fn configuration_tool_recommendations(mcp_auth_intent: bool) -> Vec<String> {
    let mut tools = vec!["config".into(), "mcp".into()];
    if mcp_auth_intent {
        tools.push("mcp_auth".into());
    }
    tools
}

pub(super) fn maybe_recommend_dependency_install_tool(
    tools: &mut Vec<String>,
    dependency_install_intent: bool,
) {
    if dependency_install_intent && !tools.iter().any(|tool| tool == "install_dependencies") {
        tools.push("install_dependencies".into());
    }
}

pub(super) fn is_dependency_install_request(lower: &str, zh: &str) -> bool {
    if contains_any(
        lower,
        &[
            "do not install",
            "don't install",
            "dont install",
            "without installing",
            "only report",
            "just report",
        ],
    ) || contains_any(zh, &["不要安装", "别安装", "不用安装", "只报告", "仅报告"])
    {
        return false;
    }

    if contains_any(
        lower,
        &[
            "pip install",
            "pip3 install",
            "uv pip install",
            "npm install",
            "npm i ",
            "pnpm install",
            "pnpm i ",
            "yarn install",
            "yarn add",
            "cargo add",
            "go get",
            "poetry add",
            "poetry install",
        ],
    ) {
        return true;
    }

    let explicit_dependency_phrase = contains_any(
        lower,
        &[
            "install dependencies",
            "install deps",
            "install packages",
            "install package",
            "add dependency",
            "add dependencies",
            "add package",
            "add packages",
        ],
    ) || contains_any(
        zh,
        &[
            "安装依赖",
            "装依赖",
            "依赖安装",
            "安装包",
            "装包",
            "安装模块",
            "装模块",
            "安装库",
            "装库",
            "补依赖",
            "加依赖",
            "添加依赖",
        ],
    );
    if explicit_dependency_phrase {
        return true;
    }

    let package_context = contains_any(
        lower,
        &[
            "dependency",
            "dependencies",
            "package",
            "module",
            "library",
            "requirements.txt",
            "package.json",
            "cargo.toml",
            "go.mod",
            "pip",
            "pip3",
            "npm",
            "pnpm",
            "yarn",
            "poetry",
            "python",
            "python3",
            "node",
            "rust",
            "go",
        ],
    ) || contains_any(zh, &["依赖", "包", "模块", "库"]);
    let explicit_install_action =
        contains_any(lower, &["install ", "add package", "add dependency"])
            || contains_any(zh, &["帮我安装", "帮我装", "安装一下", "装一下"]);

    package_context && explicit_install_action
}

pub(super) fn is_mcp_auth_request(lower: &str, zh: &str) -> bool {
    let has_mcp_subject = lower.contains("mcp") || zh.contains("MCP") || zh.contains("mcp");
    if !has_mcp_subject {
        return false;
    }
    contains_any(
        lower,
        &[
            "auth",
            "authenticate",
            "authentication",
            "authorize",
            "authorization",
            "oauth",
            "login",
            "log in",
            "token",
            "credential",
        ],
    ) || contains_any(
        zh,
        &["认证", "授权", "登录", "登陆", "令牌", "凭据", "token"],
    )
}

pub(super) fn is_error_explanation_request(lower: &str, zh: &str) -> bool {
    let has_error_subject = contains_any(
        lower,
        &[
            "error",
            "exception",
            "stack trace",
            "traceback",
            "bad_request",
            "status 400",
            "failed to",
        ],
    ) || contains_any(zh, &["报错", "错误", "异常"]);
    let asks_for_explanation = contains_any(
        lower,
        &[
            "what does",
            "what is",
            "what means",
            "what does this mean",
            "explain",
            "why",
            "原因",
        ],
    ) || contains_any(
        zh,
        &[
            "是什么意思",
            "什么意思",
            "解释",
            "原因是什么",
            "为什么",
            "怎么回事",
        ],
    );
    let asks_for_repair_or_action = contains_any(
        lower,
        &[
            "fix", "repair", "solve", "resolve", "debug", "change", "edit",
        ],
    ) || contains_any(zh, &["修复", "解决", "改", "修改", "调试"]);

    has_error_subject && asks_for_explanation && !asks_for_repair_or_action
}

pub(super) fn is_code_change_request(lower: &str, zh: &str) -> bool {
    is_debug_request(lower, zh)
        || is_natural_code_creation_request(lower, zh)
        || contains_any(
            lower,
            &[
                "implement",
                "add ",
                "change",
                "update",
                "edit",
                "build",
                "optimize",
                "refactor",
            ],
        )
        || contains_any(
            zh,
            &["实现", "新增", "修改", "优化", "完善", "开发", "重构"],
        )
}

pub(super) fn is_read_only_request(lower: &str, zh: &str) -> bool {
    contains_any(
        lower,
        &[
            "read-only",
            "readonly",
            "do not modify",
            "don't modify",
            "do not edit",
            "don't edit",
            "no edits",
            "without editing",
            "without modifying",
            "do not write",
            "don't write",
        ],
    ) || contains_any(
        zh,
        &[
            "只读",
            "不要修改",
            "不要改",
            "不要编辑",
            "不要写",
            "不要写入",
            "不修改",
            "不改文件",
            "不写文件",
            "不能修改",
            "无需修改",
        ],
    )
}

pub(super) fn is_natural_code_creation_request(lower: &str, zh: &str) -> bool {
    let has_creation_verb = contains_any(
        lower,
        &[
            "create", "make", "generate", "write a", "write an", "build a",
        ],
    ) || contains_any(
        zh,
        &[
            "做",
            "写",
            "弄一个",
            "做一个",
            "写一个",
            "创建一个",
            "创建一",
            "生成一个",
            "生成一",
        ],
    );
    has_creation_verb && has_code_artifact_signal(lower, zh)
}

pub(super) fn has_code_artifact_signal(lower: &str, zh: &str) -> bool {
    contains_any(
        lower,
        &[
            "python",
            ".py",
            "html",
            "javascript",
            "typescript",
            "rust",
            "shell",
            "bash",
            "script",
            "game",
            "app",
            "program",
            "code",
            "snake",
        ],
    ) || contains_any(
        zh,
        &[
            "脚本",
            "游戏",
            "页面",
            "网页",
            "程序",
            "代码",
            "应用",
            "贪吃蛇",
        ],
    )
}

pub(super) fn is_file_mutation_request(lower: &str, zh: &str) -> bool {
    let has_mutation = contains_any(lower, &["delete", "remove", "rename", "move", "trash"])
        || contains_any(zh, &["删除", "删掉", "删了", "移除", "重命名", "移动"]);
    let has_file_scope = contains_any(
        lower,
        &["file", "folder", "directory", ".txt", ".md", ".json", ".py"],
    ) || contains_any(zh, &["文件", "文件夹", "目录"]);
    has_mutation && has_file_scope
}

pub(super) fn is_terminal_operation_request(lower: &str, zh: &str) -> bool {
    let terminal_subject = contains_any(
        lower,
        &[
            "terminal",
            "shell",
            "bash",
            "command",
            "pip",
            "pip3",
            "python",
            "python3",
            "npm",
            "pnpm",
            "yarn",
            "node",
            "cargo",
            "brew",
            "docker",
            "pytest",
            "venv",
            "virtualenv",
            "background",
            "handle",
            "process",
            "server",
            "watch",
        ],
    ) || contains_any(
        zh,
        &[
            "终端",
            "命令",
            "默认的python",
            "默认 python",
            "默认python",
            "依赖",
            "模块",
            "环境",
            "后台",
            "句柄",
            "进程",
            "服务器",
        ],
    );
    let terminal_action = contains_any(
        lower,
        &[
            "install",
            "uninstall",
            "run",
            "execute",
            "start",
            "launch",
            "check",
            "list",
            "show",
            "status",
            "version",
            "which ",
            "where ",
            "is installed",
            "package",
            "dependency",
            "read output",
            "cancel",
            "stop",
        ],
    ) || contains_any(
        zh,
        &[
            "安装",
            "卸载",
            "运行",
            "执行",
            "启动",
            "检查",
            "查看",
            "看看",
            "有哪些",
            "状态",
            "有没有安装",
            "怎么运行",
            "跑一下",
            "装一下",
            "读取输出",
            "取消",
            "停止",
        ],
    );

    terminal_subject && terminal_action
}

pub(super) fn is_background_shell_followup(lower: &str, zh: &str) -> bool {
    contains_any(
        lower,
        &[
            "background",
            "handle",
            "process",
            "bash_output",
            "bash_cancel",
            "bash_tasks",
        ],
    ) || contains_any(zh, &["后台", "句柄", "进程"])
}

pub(super) fn is_local_inspection_request(lower: &str, zh: &str) -> bool {
    let asks_to_inspect = contains_any(
        lower,
        &[
            "check", "look", "list", "show", "find", "read", "cat ", "open", "exists", "exist",
            "is there", "whether", "inside", "contents",
        ],
    ) || contains_any(
        zh,
        &[
            "看看",
            "查看",
            "检查",
            "列出",
            "找找",
            "找一下",
            "读取",
            "读一下",
            "打开",
            "有没有",
            "是否存在",
            "在不在",
            "有吗",
            "里面",
            "有什么",
            "内容",
        ],
    );
    let local_scope = contains_any(
        lower,
        &[
            "desktop",
            "~/",
            "/users/",
            "workspace",
            "repo",
            "folder",
            "directory",
            "file",
            ".txt",
            ".md",
            ".json",
            ".py",
        ],
    ) || contains_any(
        zh,
        &["桌面", "工作区", "项目", "仓库", "文件夹", "目录", "文件"],
    );
    let anaphora_scope = contains_any(
        lower,
        &["inside", "there", "that folder", "this folder", "it"],
    ) || contains_any(zh, &["里面", "其中", "这个", "那里", "刚才"]);
    asks_to_inspect && (local_scope || anaphora_scope)
}

pub(super) fn is_file_read_request(lower: &str, zh: &str) -> bool {
    if contains_any(lower, &["http://", "https://"]) {
        return false;
    }

    let english_read = lower.starts_with("read ")
        || lower.starts_with("cat ")
        || lower.starts_with("open ")
        || lower.contains(" read ")
        || lower.contains(" cat ");
    let chinese_read = contains_any(zh, &["读取", "读一下", "打开"]);
    if !english_read && !chinese_read {
        return false;
    }

    let likely_path_or_file = contains_any(
        lower,
        &[
            "./", "../", "~/", "/", ".txt", ".md", ".json", ".yaml", ".yml", ".toml", ".rs", ".py",
            ".ts", ".tsx", ".js", ".jsx", ".html", ".css", "readme", "marker",
        ],
    ) || contains_any(zh, &["文件", "这个", "那个", "里面"]);
    if likely_path_or_file {
        return true;
    }

    let words = lower.split_whitespace().collect::<Vec<_>>();
    words.len() <= 4
        && words.iter().any(|word| {
            word.chars().any(|ch| ch == '_' || ch == '-' || ch == '.')
                || matches!(*word, "marker" | "readme" | "todo" | "notes" | "config")
        })
}

pub(super) fn is_calculation_request(lower: &str, zh: &str) -> bool {
    let has_calc_word = contains_any(
        lower,
        &["calculate", "compute", "evaluate", "what is", "what's"],
    );
    let has_math_operator = lower
        .chars()
        .any(|ch| matches!(ch, '+' | '-' | '*' | '/' | '^' | '(' | ')'));
    let digit_count = lower.chars().filter(|ch| ch.is_ascii_digit()).count();
    let chinese_calc_word = contains_any(zh, &["计算", "算一下", "算出"]);
    let chinese_operator_word = contains_any(zh, &["加", "减", "乘", "除", "平方", "开方"]);
    if chinese_calc_word {
        return digit_count >= 1 && (has_math_operator || chinese_operator_word);
    }

    has_calc_word && has_math_operator && digit_count >= 2
}
