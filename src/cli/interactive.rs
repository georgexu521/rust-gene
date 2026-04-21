//! 交互式提示模块

use priority_core::weight_engine::types::{Project, Task, TaskId};
use std::io::{self, Write};

/// 提示用户输入任务信息
pub fn prompt_task() -> io::Result<Task> {
    print!("任务名称: ");
    io::stdout().flush()?;

    let mut name = String::new();
    io::stdin().read_line(&mut name)?;
    let name = name.trim().to_string();

    if name.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "任务名称不能为空",
        ));
    }

    print!("任务描述 (可选): ");
    io::stdout().flush()?;

    let mut description = String::new();
    io::stdin().read_line(&mut description)?;
    let description = description.trim().to_string();

    print!("权重 (0-1, 默认 1.0): ");
    io::stdout().flush()?;

    let mut weight_str = String::new();
    io::stdin().read_line(&mut weight_str)?;
    let weight: f64 = weight_str.trim().parse().unwrap_or(1.0);

    // 生成任务ID
    let id = format!(
        "task_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    );

    let mut task = Task::new(&id, &name).with_weight(weight);

    if !description.is_empty() {
        task = task.with_description(description);
    }

    Ok(task)
}

/// 提示用户输入项目信息
pub fn prompt_project() -> io::Result<Project> {
    print!("项目名称: ");
    io::stdout().flush()?;

    let mut name = String::new();
    io::stdin().read_line(&mut name)?;
    let name = name.trim().to_string();

    if name.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "项目名称不能为空",
        ));
    }

    print!("项目描述 (可选): ");
    io::stdout().flush()?;

    let mut description = String::new();
    io::stdin().read_line(&mut description)?;
    let description = description.trim().to_string();

    // 生成项目ID
    let id = format!(
        "proj_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    );

    let mut project = Project::new(&id, &name);
    project.description = description;

    Ok(project)
}

/// 从列表中选择任务
pub fn select_task(tasks: &[Task]) -> io::Result<Option<TaskId>> {
    if tasks.is_empty() {
        println!("没有可用的任务");
        return Ok(None);
    }

    println!("\n可用任务:");
    for (i, task) in tasks.iter().enumerate() {
        println!("  {}. {} ({})", i + 1, task.name, task.id);
    }

    print!("\n选择任务编号 (1-{}): ", tasks.len());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    match input.trim().parse::<usize>() {
        Ok(n) if n > 0 && n <= tasks.len() => Ok(Some(tasks[n - 1].id.clone())),
        _ => {
            println!("无效的选择");
            Ok(None)
        }
    }
}

/// 确认提示
pub fn confirm(prompt: &str) -> io::Result<bool> {
    print!("{} [y/N]: ", prompt);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().to_lowercase() == "y")
}

/// 输入提示
pub fn input(prompt: &str) -> io::Result<String> {
    print!("{}: ", prompt);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().to_string())
}

/// 显示菜单并获取选择
pub fn menu(options: &[&str]) -> io::Result<usize> {
    println!("\n选项:");
    for (i, option) in options.iter().enumerate() {
        println!("  {}. {}", i + 1, option);
    }

    print!("\n选择 (1-{}): ", options.len());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    match input.trim().parse::<usize>() {
        Ok(n) if n > 0 && n <= options.len() => Ok(n - 1),
        _ => {
            println!("无效的选择");
            Ok(0)
        }
    }
}

/// 交互式主循环
pub fn interactive_loop() -> io::Result<()> {
    use crate::cli::display::{clear_screen, print_banner};

    clear_screen();
    print_banner();

    println!("欢迎使用 Priority Agent 交互模式!\n");

    loop {
        println!("\n主菜单:");
        println!("  1. 初始化新项目");
        println!("  2. 添加任务");
        println!("  3. 查看任务列表");
        println!("  4. 查看下一个任务");
        println!("  5. 标记任务完成");
        println!("  6. 查看进度");
        println!("  7. 分析项目");
        println!("  8. 创建快照");
        println!("  9. 退出");

        print!("\n选择操作 (1-9): ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        match input.trim() {
            "1" => {
                match prompt_project() {
                    Ok(project) => {
                        println!("\n项目 '{}' 已创建!", project.name);
                        // TODO: 保存项目
                    }
                    Err(e) => println!("错误: {}", e),
                }
            }
            "2" => {
                match prompt_task() {
                    Ok(task) => {
                        println!("\n任务 '{}' 已添加!", task.name);
                        // TODO: 添加任务到项目
                    }
                    Err(e) => println!("错误: {}", e),
                }
            }
            "3" => {
                println!("\n任务列表功能待实现");
            }
            "4" => {
                println!("\n下一个任务功能待实现");
            }
            "5" => {
                println!("\n标记完成功能待实现");
            }
            "6" => {
                println!("\n查看进度功能待实现");
            }
            "7" => {
                println!("\n分析项目功能待实现");
            }
            "8" => {
                println!("\n创建快照功能待实现");
            }
            "9" | "q" | "quit" | "exit" => {
                println!("\n再见!");
                break;
            }
            _ => println!("无效的选择，请重试"),
        }
    }

    Ok(())
}
