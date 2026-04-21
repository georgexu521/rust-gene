//! 显示格式化模块

use priority_core::weight_engine::types::{Project, Task, TaskStatus, Weight};

/// ANSI 颜色代码
pub mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    pub const RED: &str = "\x1b[31m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BLUE: &str = "\x1b[34m";
    pub const MAGENTA: &str = "\x1b[35m";
    pub const CYAN: &str = "\x1b[36m";
    pub const WHITE: &str = "\x1b[37m";
}

/// 打印 banner
pub fn print_banner() {
    use colors::*;
    println!();
    println!(
        "{}  ██████╗ ██████╗ ██╗ ██████╗██████╗ ██╗████████╗██╗   ██╗{}",
        CYAN, RESET
    );
    println!(
        "{}  ██╔══██╗██╔══██╗██║██╔════╝██╔══██╗██║╚══██╔══╝╚██╗ ██╔╝{}",
        CYAN, RESET
    );
    println!(
        "{}  ██████╔╝██████╔╝██║██║     ██████╔╝██║   ██║    ╚████╔╝ {}",
        CYAN, RESET
    );
    println!(
        "{}  ██╔═══╝ ██╔══██╗██║██║     ██╔══██╗██║   ██║     ╚██╔╝  {}",
        CYAN, RESET
    );
    println!(
        "{}  ██║     ██║  ██║██║╚██████╗██║  ██║██║   ██║      ██║   {}",
        CYAN, RESET
    );
    println!(
        "{}  ╚═╝     ╚═╝  ╚═╝╚═╝ ╚═════╝╚═╝  ╚═╝╚═╝   ╚═╝      ╚═╝   {}",
        CYAN, RESET
    );
    println!();
    println!("{}  Priority Agent - 加权优先级桌面 Agent{}", BOLD, RESET);
    println!("{}  让 AI 始终专注于最重要的事项{}", DIM, RESET);
    println!();
}

/// 格式化进度条
pub fn format_progress(progress: f64, width: usize) -> String {
    use colors::*;

    let filled = (progress * width as f64) as usize;
    let empty = width - filled;

    let color = if progress >= 0.8 {
        GREEN
    } else if progress >= 0.5 {
        YELLOW
    } else {
        RED
    };

    let filled_str = "█".repeat(filled);
    let empty_str = "░".repeat(empty);
    let bar = format!(
        "{}[{}]{} {:.1}%",
        color,
        filled_str + &empty_str,
        RESET,
        progress * 100.0
    );

    bar
}

/// 格式化任务状态图标
pub fn format_status_icon(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "◯",
        TaskStatus::InProgress => "◐",
        TaskStatus::Completed => "✓",
        TaskStatus::Blocked => "✗",
    }
}

/// 格式化任务状态（带颜色）
pub fn format_status(status: TaskStatus) -> String {
    use colors::*;

    match status {
        TaskStatus::Pending => format!("{}◯ 待处理{}", DIM, RESET),
        TaskStatus::InProgress => format!("{}◐ 进行中{}", YELLOW, RESET),
        TaskStatus::Completed => format!("{}✓ 已完成{}", GREEN, RESET),
        TaskStatus::Blocked => format!("{}✗ 阻塞中{}", RED, RESET),
    }
}

/// 格式化权重
pub fn format_weight(weight: Weight) -> String {
    use colors::*;

    let percentage = weight.as_percentage();
    let color = if percentage >= 30.0 {
        RED
    } else if percentage >= 15.0 {
        YELLOW
    } else {
        GREEN
    };

    format!("{}{:>5.1}%{}", color, percentage, RESET)
}

/// 格式化任务树
pub fn format_task_tree(tasks: &[Task], level: usize) -> String {
    use colors::*;

    let mut output = String::new();
    let indent = "  ".repeat(level);

    for (i, task) in tasks.iter().enumerate() {
        let is_last = i == tasks.len() - 1;
        let branch = if is_last { "└── " } else { "├── " };

        let status_icon = format_status_icon(task.status);
        let weight_str = format_weight(task.local_weight);

        output.push_str(&format!(
            "{}{}{} {} {} {}\n",
            indent, branch, status_icon, weight_str, BOLD, task.name
        ));

        // 显示描述（如果有）
        if !task.description.is_empty() {
            let desc_indent = format!("{}    ", indent);
            output.push_str(&format!("{}{}{}\n", desc_indent, DIM, task.description));
        }

        // 递归格式化子任务
        if !task.children.is_empty() {
            let child_output = format_task_tree(&task.children, level + 1);
            output.push_str(&child_output);
        }
    }

    output
}

/// 格式化项目概览
pub fn format_project_overview(project: &Project) -> String {
    use colors::*;

    let mut output = String::new();

    output.push_str(&format!("\n{}📁 项目: {}{}\n", BOLD, project.name, RESET));

    if !project.description.is_empty() {
        output.push_str(&format!("{}   {}{}\n", DIM, project.description, RESET));
    }

    let progress = project.overall_progress();
    output.push_str(&format!(
        "\n{}   总体进度: {}\n",
        "",
        format_progress(progress, 30)
    ));

    let all_tasks = project.all_tasks();
    let completed = all_tasks
        .iter()
        .filter(|t| matches!(t.status, TaskStatus::Completed))
        .count();

    output.push_str(&format!(
        "{}   任务完成: {}/{}\n",
        "",
        completed,
        all_tasks.len()
    ));

    output.push('\n');
    output.push_str(&format!("{}📋 任务列表:{}\n", BOLD, RESET));
    output.push_str(&format_task_tree(&project.root_tasks, 0));

    output
}

/// 格式化任务详情
pub fn format_task_detail(task: &Task) -> String {
    use colors::*;

    let mut output = String::new();

    output.push_str(&format!("\n{}📝 任务详情{}\n", BOLD, RESET));
    output.push_str(&format!("{}ID: {}{}\n", CYAN, task.id, RESET));
    output.push_str(&format!("{}名称: {}{}\n", BOLD, task.name, RESET));
    output.push_str(&format!("{}状态: {}\n", "", format_status(task.status)));
    output.push_str(&format!(
        "{}权重: {} (绝对: {:.1}%)\n",
        "",
        format_weight(task.local_weight),
        task.absolute_weight.as_percentage()
    ));

    if !task.description.is_empty() {
        output.push_str(&format!("\n{}描述:{}\n{}", DIM, RESET, task.description));
    }

    if !task.dependencies.is_empty() {
        output.push_str(&format!("\n{}依赖:{}{}\n", YELLOW, "", RESET));
        for dep in &task.dependencies {
            output.push_str(&format!("  • {}\n", dep));
        }
    }

    if !task.children.is_empty() {
        output.push_str(&format!("\n{}子任务:{}{}\n", CYAN, "", RESET));
        output.push_str(&format_task_tree(&task.children, 0));
    }

    output
}

/// 格式化优先级队列
pub fn format_priority_queue(tasks: &[(String, f64)]) -> String {
    use colors::*;

    let mut output = String::new();

    output.push_str(&format!("\n{}🎯 优先级队列{}\n", BOLD, RESET));
    output.push_str(&format!("{}排名  权重      任务{}\n", DIM, RESET));
    output.push_str(&format!("{}────  ────────  ─────────────{}\n", DIM, RESET));

    for (i, (name, weight)) in tasks.iter().enumerate() {
        let rank = i + 1;
        let color = match rank {
            1 => RED,
            2 => YELLOW,
            3 => GREEN,
            _ => RESET,
        };

        output.push_str(&format!(
            "{}{:>3}{}  {:>6.1}%  {}\n",
            color,
            rank,
            RESET,
            weight * 100.0,
            name
        ));
    }

    output
}

/// 清除终端
pub fn clear_screen() {
    print!("\x1b[2J\x1b[1;1H");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_progress() {
        let bar = format_progress(0.5, 20);
        assert!(bar.contains("50.0%"));
    }

    #[test]
    fn test_format_weight() {
        let w = Weight::new(0.25);
        let formatted = format_weight(w);
        assert!(formatted.contains("25.0%"));
    }
}
