//! 交互式提示模块

use priority_core::weight_engine::types::{
    Project, Task, TaskId, TaskStatus, Weight,
};
use std::io::{self, Write};
use std::path::PathBuf;

/// 获取项目存储目录
fn projects_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".priority-agent")
        .join("projects")
}

/// 获取项目文件路径
fn project_path(project_id: &str) -> PathBuf {
    projects_dir().join(format!("{}.json", project_id))
}

/// 确保项目目录存在
fn ensure_projects_dir() -> io::Result<()> {
    let dir = projects_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(())
}

/// 保存项目到磁盘
fn save_project(project: &Project) -> io::Result<()> {
    ensure_projects_dir()?;
    let path = project_path(&project.id.0);
    let json = serde_json::to_string_pretty(project)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// 加载项目
fn load_project(project_id: &str) -> io::Result<Project> {
    let path = project_path(project_id);
    let json = std::fs::read_to_string(&path)?;
    let project: Project = serde_json::from_str(&json)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(project)
}

/// 列出所有项目
fn list_projects() -> io::Result<Vec<Project>> {
    let dir = projects_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut projects = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            if let Ok(json) = std::fs::read_to_string(&path) {
                if let Ok(project) = serde_json::from_str::<Project>(&json) {
                    projects.push(project);
                }
            }
        }
    }
    Ok(projects)
}

/// 选择项目
fn select_project(projects: &[Project]) -> io::Result<Option<String>> {
    if projects.is_empty() {
        println!("没有可用的项目，请先创建项目");
        return Ok(None);
    }

    println!("\n可用项目:");
    for (i, project) in projects.iter().enumerate() {
        println!("  {}. {} ({})", i + 1, project.name, project.id);
    }

    print!("\n选择项目编号 (1-{}): ", projects.len());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    match input.trim().parse::<usize>() {
        Ok(n) if n > 0 && n <= projects.len() => Ok(Some(projects[n - 1].id.0.clone())),
        _ => {
            println!("无效的选择");
            Ok(None)
        }
    }
}

/// 在项目中查找任务（可变引用）
fn find_task_mut<'a>(project: &'a mut Project, task_id: &str) -> Option<&'a mut Task> {
    for task in &mut project.root_tasks {
        if task.id.0 == task_id {
            return Some(task);
        }
        if let Some(found) = find_task_in_children_mut(task, task_id) {
            return Some(found);
        }
    }
    None
}

fn find_task_in_children_mut<'a>(task: &'a mut Task, task_id: &str) -> Option<&'a mut Task> {
    for child in &mut task.children {
        if child.id.0 == task_id {
            return Some(child);
        }
        if let Some(found) = find_task_in_children_mut(child, task_id) {
            return Some(found);
        }
    }
    None
}

/// 打印任务树
fn print_task_tree(tasks: &[Task], indent: usize) {
    for task in tasks {
        let prefix = "  ".repeat(indent);
        let status_icon = match task.status {
            TaskStatus::Pending => "⏳",
            TaskStatus::InProgress => "🔄",
            TaskStatus::Completed => "✅",
            TaskStatus::Blocked => "🚫",
        };
        println!(
            "{}{} {} (权重: {:.0}%, 进度: {:.0}%)",
            prefix,
            status_icon,
            task.name,
            task.local_weight.value() * 100.0,
            task.progress() * 100.0
        );
        if !task.description.is_empty() {
            println!("{}   {}", prefix, task.description);
        }
        if !task.children.is_empty() {
            print_task_tree(&task.children, indent + 1);
        }
    }
}

/// 获取下一个待处理的任务（按权重排序）
fn next_pending_task<'a>(project: &'a Project) -> Option<&'a Task> {
    let mut candidates: Vec<&Task> = project
        .all_tasks()
        .into_iter()
        .filter(|t| t.status == TaskStatus::Pending)
        .collect();

    candidates.sort_by(|a, b| {
        b.absolute_weight
            .value()
            .partial_cmp(&a.absolute_weight.value())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    candidates.first().copied()
}

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
        println!("  2. 添加任务到项目");
        println!("  3. 查看任务列表");
        println!("  4. 查看下一个任务");
        println!("  5. 标记任务完成");
        println!("  6. 查看项目进度");
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
                        match save_project(&project) {
                            Ok(()) => {
                                println!(
                                    "\n✅ 项目 '{}' 已创建并保存!",
                                    project.name
                                );
                            }
                            Err(e) => {
                                println!("\n⚠️ 项目创建成功但保存失败: {}", e);
                            }
                        }
                    }
                    Err(e) => println!("错误: {}", e),
                }
            }
            "2" => {
                match list_projects() {
                    Ok(projects) => {
                        match select_project(&projects) {
                            Ok(Some(project_id)) => {
                                match prompt_task() {
                                    Ok(task) => {
                                        match load_project(&project_id) {
                                            Ok(mut project) => {
                                                project.add_task(task);
                                                match save_project(&project) {
                                                    Ok(()) => {
                                                        println!(
                                                            "\n✅ 任务已添加到项目 '{}'",
                                                            project.name
                                                        );
                                                    }
                                                    Err(e) => {
                                                        println!("保存失败: {}", e);
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                println!("加载项目失败: {}", e);
                                            }
                                        }
                                    }
                                    Err(e) => println!("错误: {}", e),
                                }
                            }
                            Ok(None) => {}
                            Err(e) => println!("错误: {}", e),
                        }
                    }
                    Err(e) => println!("列出项目失败: {}", e),
                }
            }
            "3" => {
                match list_projects() {
                    Ok(projects) => {
                        match select_project(&projects) {
                            Ok(Some(project_id)) => {
                                match load_project(&project_id) {
                                    Ok(project) => {
                                        println!(
                                            "\n📋 项目: {} ({})",
                                            project.name, project.id
                                        );
                                        if !project.description.is_empty() {
                                            println!("   {}", project.description);
                                        }
                                        println!(
                                            "   总体进度: {:.1}%",
                                            project.overall_progress() * 100.0
                                        );
                                        if project.root_tasks.is_empty() {
                                            println!("\n   暂无任务");
                                        } else {
                                            println!("\n   任务列表:");
                                            print_task_tree(&project.root_tasks, 1);
                                        }
                                    }
                                    Err(e) => println!("加载项目失败: {}", e),
                                }
                            }
                            Ok(None) => {}
                            Err(e) => println!("错误: {}", e),
                        }
                    }
                    Err(e) => println!("列出项目失败: {}", e),
                }
            }
            "4" => {
                match list_projects() {
                    Ok(projects) => {
                        match select_project(&projects) {
                            Ok(Some(project_id)) => {
                                match load_project(&project_id) {
                                    Ok(project) => {
                                        match next_pending_task(&project) {
                                            Some(task) => {
                                                println!("\n🎯 下一个推荐任务:");
                                                println!("   名称: {}", task.name);
                                                println!(
                                                    "   权重: {:.0}%",
                                                    task.absolute_weight.value() * 100.0
                                                );
                                                if !task.description.is_empty() {
                                                    println!(
                                                        "   描述: {}",
                                                        task.description
                                                    );
                                                }
                                            }
                                            None => {
                                                println!("\n🎉 所有任务已完成或进行中!");
                                            }
                                        }
                                    }
                                    Err(e) => println!("加载项目失败: {}", e),
                                }
                            }
                            Ok(None) => {}
                            Err(e) => println!("错误: {}", e),
                        }
                    }
                    Err(e) => println!("列出项目失败: {}", e),
                }
            }
            "5" => {
                match list_projects() {
                    Ok(projects) => {
                        match select_project(&projects) {
                            Ok(Some(project_id)) => {
                                match load_project(&project_id) {
                                    Ok(mut project) => {
                                        let all_tasks: Vec<&Task> =
                                            project.all_tasks().into_iter().collect();
                                        if all_tasks.is_empty() {
                                            println!("\n项目中没有任务");
                                            continue;
                                        }

                                        println!("\n任务列表:");
                                        for (i, task) in all_tasks.iter().enumerate() {
                                            let status_icon = match task.status {
                                                TaskStatus::Pending => "⏳",
                                                TaskStatus::InProgress => "🔄",
                                                TaskStatus::Completed => "✅",
                                                TaskStatus::Blocked => "🚫",
                                            };
                                            println!(
                                                "  {}. {} {} ({})",
                                                i + 1,
                                                status_icon,
                                                task.name,
                                                task.id
                                            );
                                        }

                                        print!("\n选择要标记完成的任务编号 (1-{}): ", all_tasks.len());
                                        io::stdout().flush()?;
                                        let mut task_input = String::new();
                                        io::stdin().read_line(&mut task_input)?;

                                        if let Ok(n) = task_input.trim().parse::<usize>() {
                                            if n > 0 && n <= all_tasks.len() {
                                                let task_id = all_tasks[n - 1].id.0.clone();
                                                let task_name = if let Some(task) =
                                                    find_task_mut(&mut project, &task_id)
                                                {
                                                    task.status = TaskStatus::Completed;
                                                    Some(task.name.clone())
                                                } else {
                                                    None
                                                };

                                                if let Some(name) = task_name {
                                                    match save_project(&project) {
                                                        Ok(()) => {
                                                            println!(
                                                                "\n✅ 任务 '{}' 已标记为完成!",
                                                                name
                                                            );
                                                        }
                                                        Err(e) => {
                                                            println!("保存失败: {}", e);
                                                        }
                                                    }
                                                }
                                            } else {
                                                println!("无效的选择");
                                            }
                                        }
                                    }
                                    Err(e) => println!("加载项目失败: {}", e),
                                }
                            }
                            Ok(None) => {}
                            Err(e) => println!("错误: {}", e),
                        }
                    }
                    Err(e) => println!("列出项目失败: {}", e),
                }
            }
            "6" => {
                match list_projects() {
                    Ok(projects) => {
                        match select_project(&projects) {
                            Ok(Some(project_id)) => {
                                match load_project(&project_id) {
                                    Ok(project) => {
                                        println!("\n📊 项目进度报告");
                                        println!("================");
                                        println!("项目名称: {}", project.name);
                                        println!(
                                            "总体进度: {:.1}%",
                                            project.overall_progress() * 100.0
                                        );

                                        let all_tasks = project.all_tasks();
                                        let total = all_tasks.len();
                                        let completed = all_tasks
                                            .iter()
                                            .filter(|t| t.status == TaskStatus::Completed)
                                            .count();
                                        let pending = all_tasks
                                            .iter()
                                            .filter(|t| t.status == TaskStatus::Pending)
                                            .count();
                                        let in_progress = all_tasks
                                            .iter()
                                            .filter(|t| t.status == TaskStatus::InProgress)
                                            .count();
                                        let blocked = all_tasks
                                            .iter()
                                            .filter(|t| t.status == TaskStatus::Blocked)
                                            .count();

                                        println!("\n任务统计:");
                                        println!("  总数: {}", total);
                                        println!("  ✅ 已完成: {}", completed);
                                        println!("  ⏳ 待处理: {}", pending);
                                        println!("  🔄 进行中: {}", in_progress);
                                        println!("  🚫 阻塞中: {}", blocked);
                                    }
                                    Err(e) => println!("加载项目失败: {}", e),
                                }
                            }
                            Ok(None) => {}
                            Err(e) => println!("错误: {}", e),
                        }
                    }
                    Err(e) => println!("列出项目失败: {}", e),
                }
            }
            "7" => {
                match list_projects() {
                    Ok(projects) => {
                        match select_project(&projects) {
                            Ok(Some(project_id)) => {
                                match load_project(&project_id) {
                                    Ok(project) => {
                                        println!("\n📈 项目分析");
                                        println!("============");
                                        println!("项目名称: {}", project.name);

                                        let all_tasks = project.all_tasks();
                                        if all_tasks.is_empty() {
                                            println!("\n暂无任务可分析");
                                            continue;
                                        }

                                        // 找出权重最高的任务
                                        let highest_weight = all_tasks
                                            .iter()
                                            .max_by(|a, b| {
                                                a.absolute_weight
                                                    .value()
                                                    .partial_cmp(&b.absolute_weight.value())
                                                    .unwrap_or(std::cmp::Ordering::Equal)
                                            });

                                        if let Some(task) = highest_weight {
                                            println!("\n最高权重任务: {}", task.name);
                                            println!(
                                                "  权重: {:.1}%",
                                                task.absolute_weight.value() * 100.0
                                            );
                                            println!(
                                                "  状态: {:?}",
                                                task.status
                                            );
                                        }

                                        // 找出阻塞的任务
                                        let blocked_tasks: Vec<&&Task> = all_tasks
                                            .iter()
                                            .filter(|t| t.status == TaskStatus::Blocked)
                                            .collect();

                                        if !blocked_tasks.is_empty() {
                                            println!("\n⚠️ 阻塞中的任务:");
                                            for task in blocked_tasks {
                                                println!("  - {}", task.name);
                                            }
                                        }

                                        // 建议
                                        if project.overall_progress() < 0.3 {
                                            println!("\n💡 建议: 项目刚启动，建议先完成高权重任务以建立 momentum");
                                        } else if project.overall_progress() > 0.8 {
                                            println!("\n💡 建议: 项目即将完成，关注剩余任务的收尾质量");
                                        }
                                    }
                                    Err(e) => println!("加载项目失败: {}", e),
                                }
                            }
                            Ok(None) => {}
                            Err(e) => println!("错误: {}", e),
                        }
                    }
                    Err(e) => println!("列出项目失败: {}", e),
                }
            }
            "8" => {
                match list_projects() {
                    Ok(projects) => {
                        match select_project(&projects) {
                            Ok(Some(project_id)) => {
                                match load_project(&project_id) {
                                    Ok(project) => {
                                        let snapshot_id = format!(
                                            "snap_{}",
                                            std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap_or_default()
                                                .as_secs()
                                        );
                                        let snapshot_path = projects_dir()
                                            .join(format!("{}_{}.json", project_id, snapshot_id));

                                        let json = serde_json::to_string_pretty(&project)
                                            .map_err(|e| {
                                                io::Error::new(
                                                    io::ErrorKind::InvalidData,
                                                    e,
                                                )
                                            })?;
                                        std::fs::write(&snapshot_path, json)?;
                                        println!(
                                            "\n📸 快照 '{}' 已创建!",
                                            snapshot_id
                                        );
                                    }
                                    Err(e) => println!("加载项目失败: {}", e),
                                }
                            }
                            Ok(None) => {}
                            Err(e) => println!("错误: {}", e),
                        }
                    }
                    Err(e) => println!("列出项目失败: {}", e),
                }
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
