//! 任务感知 Prompt 组装器
//!
//! 基于用户消息推断任务类型，并在基础 system prompt 上附加短指令。

use crate::services::api::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    Coding,
    Debugging,
    Review,
    Architecture,
    General,
}

pub fn infer_task_type(user_message: &str) -> TaskType {
    let text = user_message.to_lowercase();
    let mut score_review = 0i32;
    let mut score_debug = 0i32;
    let mut score_arch = 0i32;
    let mut score_coding = 0i32;

    score_review += score_hits(
        &text,
        &[
            "code review",
            "审查",
            "review",
            "bug risk",
            "回归",
            "finding",
            "line reference",
        ],
    ) * 3;
    score_debug += score_hits(
        &text,
        &[
            "报错",
            "error",
            "panic",
            "failed",
            "失败",
            "fix bug",
            "debug",
            "修复",
            "stack trace",
            "cannot find",
        ],
    ) * 3;
    score_arch += score_hits(
        &text,
        &[
            "设计",
            "architecture",
            "架构",
            "tradeoff",
            "方案",
            "迁移",
            "演进",
        ],
    ) * 2;
    score_coding += score_hits(
        &text,
        &[
            "实现",
            "写代码",
            "implement",
            "add feature",
            "修改代码",
            "refactor",
            "编程",
            "函数",
            "function",
            "class",
            "module",
            ".rs",
            ".ts",
            ".py",
            "cargo",
        ],
    ) * 2;

    if text.contains("review") && text.contains("fix") {
        // 既评审又修复时优先调试/实现，减少纯 review 误判
        score_debug += 2;
        score_coding += 1;
    }

    let ranked = [
        (TaskType::Review, score_review),
        (TaskType::Debugging, score_debug),
        (TaskType::Architecture, score_arch),
        (TaskType::Coding, score_coding),
    ];
    let (best_type, best_score) = ranked
        .into_iter()
        .max_by_key(|(_, score)| *score)
        .unwrap_or((TaskType::General, 0));
    if best_score <= 0 {
        TaskType::General
    } else {
        best_type
    }
}

pub fn compose_task_aware_system_prompt(base: &str, user_message: &str) -> String {
    let task_type = infer_task_type(user_message);
    render_task_focus_prompt(base, task_type)
}

pub fn compose_task_aware_system_prompt_with_history(
    base: &str,
    user_message: &str,
    history: &[Message],
) -> String {
    let task_type = infer_task_type_with_history(user_message, history);
    render_task_focus_prompt(base, task_type)
}

pub fn infer_task_type_with_history(user_message: &str, history: &[Message]) -> TaskType {
    let current = infer_task_type(user_message);
    if !matches!(current, TaskType::General) {
        return current;
    }
    if !is_continuation_message(user_message) {
        return current;
    }

    for msg in history.iter().rev() {
        if let Message::User { content } = msg {
            let inferred = infer_task_type(content);
            if !matches!(inferred, TaskType::General) {
                return inferred;
            }
        }
    }
    TaskType::General
}

fn render_task_focus_prompt(base: &str, task_type: TaskType) -> String {
    if matches!(task_type, TaskType::General) {
        return base.to_string();
    }

    let extra = match task_type {
        TaskType::Coding => {
            "Task Focus: Coding\n- Prefer minimal, compilable changes.\n- Prioritize concrete implementation and tests."
        }
        TaskType::Debugging => {
            "Task Focus: Debugging\n- Reproduce first, then fix root cause.\n- Preserve failing diagnostics and verify with targeted checks."
        }
        TaskType::Review => {
            "Task Focus: Code Review\n- Prioritize correctness risks, regressions, and missing tests.\n- Provide findings with file/line references first."
        }
        TaskType::Architecture => {
            "Task Focus: Architecture\n- State tradeoffs and constraints explicitly.\n- Favor incremental migration over large rewrites."
        }
        TaskType::General => "",
    };

    format!(
        "{base}\n\n<task-focus type=\"{task_type:?}\">\n{extra}\n</task-focus>"
    )
}

fn score_hits(text: &str, keywords: &[&str]) -> i32 {
    keywords.iter().filter(|k| text.contains(**k)).count() as i32
}

fn is_continuation_message(text: &str) -> bool {
    let lower = text.to_lowercase();
    let markers = [
        "继续",
        "接着",
        "继续吧",
        "continue",
        "go on",
        "same as above",
        "as discussed",
        "按刚才",
    ];
    markers.iter().any(|m| lower.contains(m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_task_type_debug() {
        assert_eq!(infer_task_type("请修复这个 error"), TaskType::Debugging);
    }

    #[test]
    fn test_infer_task_type_review() {
        assert_eq!(infer_task_type("帮我做 code review"), TaskType::Review);
    }

    #[test]
    fn test_infer_task_type_debug_beats_review_when_fix_requested() {
        assert_eq!(
            infer_task_type("请 review 一下这个报错并修复"),
            TaskType::Debugging
        );
    }

    #[test]
    fn test_infer_task_type_architecture() {
        assert_eq!(infer_task_type("讨论一下系统架构 tradeoff"), TaskType::Architecture);
    }

    #[test]
    fn test_infer_task_type_with_history_for_continuation() {
        let history = vec![
            Message::user("请修复这个 error[E0425] 并给出根因"),
            Message::assistant("收到，我先定位"),
        ];
        assert_eq!(
            infer_task_type_with_history("继续", &history),
            TaskType::Debugging
        );
    }

    #[test]
    fn test_compose_with_history_uses_previous_focus() {
        let history = vec![Message::user("帮我做 code review，重点找回归风险")];
        let composed =
            compose_task_aware_system_prompt_with_history("base prompt", "继续", &history);
        assert!(composed.contains("Task Focus: Code Review"));
    }

    #[test]
    fn test_compose_task_aware_prompt_appends_focus() {
        let base = "base prompt";
        let composed = compose_task_aware_system_prompt(base, "请实现一个新功能");
        assert!(composed.contains("base prompt"));
        assert!(composed.contains("<task-focus"));
        assert!(composed.contains("Task Focus: Coding"));
    }
}
