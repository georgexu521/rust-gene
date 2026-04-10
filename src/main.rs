//! Priority Agent - 加权优先级桌面 Agent
//!
//! 解决 AI Agent 抓不住重点的问题，通过显式的权重系统让 AI 始终专注于最重要的事项。

mod cli;
mod context_manager;
mod task_analyzer;
mod weight_engine;

use cli::commands::{print_help, Cli, Commands};
use cli::display::{format_priority_queue, format_project_overview, print_banner};
use context_manager::persistence::PersistenceManager;
use context_manager::state::SessionState;
use weight_engine::calculator::{ProgressReport, WeightCalculator};
use weight_engine::types::{Project, Task, TaskId, TaskStatus};

fn main() {
    let cli = Cli::parse();

    if cli.verbose {
        println!("详细模式已启用");
    }

    // 初始化持久化管理器
    let data_dir = dirs::data_dir()
        .map(|d| d.join("priority-agent"))
        .unwrap_or_else(|| std::path::PathBuf::from(".priority-agent"));
    
    let mut persistence = PersistenceManager::with_file_storage(&data_dir);
    
    // 尝试加载已有状态
    let mut state = persistence.load_state()
        .unwrap_or_else(|e| {
            eprintln!("加载状态失败: {}", e);
            None
        })
        .unwrap_or_else(SessionState::new);

    match cli.command {
        Commands::Help => {
            print_help();
        }
        Commands::Init => {
            print_banner();
            println!("初始化新项目...");
            
            let project = Project::new(
                "default",
                "我的项目"
            );
            
            state.set_project(project);
            
            if let Err(e) = persistence.save_state(&state) {
                eprintln!("保存项目失败: {}", e);
            } else {
                println!("✓ 项目初始化完成!");
                println!("  数据目录: {}", data_dir.display());
            }
        }
        Commands::AddTask { name } => {
            if state.current_project.is_none() {
                println!("错误: 没有活动项目。请先运行 'priority-agent init'");
                return;
            }
            
            let task_id = format!("task_{}", 
                std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            );
            
            let task = Task::new(&task_id, &name)
                .with_weight(1.0);
            
            if let Some(ref mut project) = state.current_project {
                project.add_task(task);
                
                if let Err(e) = persistence.save_state(&state) {
                    eprintln!("保存失败: {}", e);
                } else {
                    println!("✓ 添加任务: {} (ID: {})", name, task_id);
                }
            }
        }
        Commands::List => {
            if let Some(ref project) = state.current_project {
                println!("{}", format_project_overview(project));
            } else {
                println!("没有活动项目。请先运行 'priority-agent init'");
            }
        }
        Commands::Next => {
            if let Some(ref project) = state.current_project {
                let calculator = WeightCalculator::new();
                
                if let Some(task) = calculator.next_task(project) {
                    println!("🎯 下一个推荐任务:");
                    println!("   名称: {}", task.task_name);
                    println!("   权重: {:.1}%", task.absolute_weight.as_percentage());
                    println!("   ID: {}", task.task_id);
                    
                    if task.blocking_count > 0 {
                        println!("   阻塞 {} 个其他任务", task.blocking_count);
                    }
                } else {
                    println!("✓ 所有任务已完成或无法执行!");
                }
            } else {
                println!("没有活动项目。请先运行 'priority-agent init'");
            }
        }
        Commands::CompleteTask { id } => {
            if let Some(ref mut project) = state.current_project {
                let task_id = TaskId::new(&id);
                
                // 查找并更新任务状态
                let found = update_task_status(&mut project.root_tasks, &task_id, TaskStatus::Completed);
                
                if found {
                    state.complete_task(task_id);
                    
                    if let Err(e) = persistence.save_state(&state) {
                        eprintln!("保存失败: {}", e);
                    } else {
                        println!("✓ 标记任务完成: {}", id);
                    }
                } else {
                    println!("错误: 未找到任务 '{}'", id);
                }
            } else {
                println!("没有活动项目。请先运行 'priority-agent init'");
            }
        }
        Commands::Progress => {
            if let Some(ref project) = state.current_project {
                let mut calculator = WeightCalculator::new();
                
                // 恢复已完成的任务状态
                for task_id in &state.completed_tasks {
                    calculator.mark_completed(task_id.clone());
                }
                
                let report = ProgressReport::generate(&calculator, project);
                
                println!("📊 项目进度报告");
                println!("================");
                println!("整体进度: {:.1}%", report.overall_progress * 100.0);
                println!();
                println!("任务统计:");
                println!("  已完成: {}", report.completed_count);
                println!("  进行中: {}", report.in_progress_count);
                println!("  待处理: {}", report.pending_count);
                println!("  阻塞中: {}", report.blocked_count);
                
                if let Some(ref next_task) = report.next_recommended_task {
                    println!();
                    println!("🎯 推荐下一个任务: {}", next_task);
                }
            } else {
                println!("没有活动项目。请先运行 'priority-agent init'");
            }
        }
        Commands::Analyze => {
            if let Some(ref project) = state.current_project {
                let calculator = WeightCalculator::new();
                let weights = calculator.calculate_absolute_weights(project);
                
                println!("📈 项目分析报告");
                println!("================");
                println!("项目名称: {}", project.name);
                println!("总任务数: {}", project.all_tasks().len());
                println!();
                println!("权重分布:");
                
                let mut weight_list: Vec<_> = weights.iter().collect();
                weight_list.sort_by(|a, b| b.1.value().partial_cmp(&a.1.value()).unwrap());
                
                for (task_id, weight) in weight_list.iter().take(10) {
                    println!("  {}: {:.1}%", task_id, weight.as_percentage());
                }
                
                if weight_list.len() > 10 {
                    println!("  ... 还有 {} 个任务", weight_list.len() - 10);
                }
            } else {
                println!("没有活动项目。请先运行 'priority-agent init'");
            }
        }
        Commands::Snapshot { name } => {
            let description = name.unwrap_or_else(|| {
                format!("快照 {}", 
                    std::time::SystemTime::now()
                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs())
            });
            
            let snapshot = state.create_snapshot(&description);
            let snapshot_id = snapshot.id.clone();
            
            if let Err(e) = persistence.save_snapshot(&snapshot) {
                eprintln!("创建快照失败: {}", e);
            } else {
                println!("✓ 创建快照: {} (ID: {})", description, snapshot_id);
            }
        }
        Commands::Restore { id } => {
            match persistence.load_snapshot(&id) {
                Ok(Some(snapshot)) => {
                    state.restore_from_snapshot(&snapshot);
                    
                    if let Err(e) = persistence.save_state(&state) {
                        eprintln!("保存状态失败: {}", e);
                    } else {
                        println!("✓ 恢复快照: {}", snapshot.description);
                    }
                }
                Ok(None) => {
                    println!("错误: 未找到快照 '{}'", id);
                }
                Err(e) => {
                    eprintln!("加载快照失败: {}", e);
                }
            }
        }
        Commands::Interactive => {
            println!("进入交互模式...");
            println!("提示: 输入 'help' 查看可用命令，'quit' 退出");
            
            // 简单的交互循环
            loop {
                print!("> ");
                use std::io::Write;
                std::io::stdout().flush().unwrap();
                
                let mut input = String::new();
                if std::io::stdin().read_line(&mut input).is_err() {
                    break;
                }
                
                let input = input.trim();
                
                match input {
                    "quit" | "exit" | "q" => {
                        println!("再见!");
                        break;
                    }
                    "help" | "h" => {
                        println!("可用命令:");
                        println!("  add <名称>    - 添加任务");
                        println!("  list          - 列出任务");
                        println!("  next          - 显示下一个任务");
                        println!("  done <ID>     - 完成任务");
                        println!("  progress      - 显示进度");
                        println!("  quit          - 退出");
                    }
                    "" => continue,
                    _ => {
                        if input.starts_with("add ") {
                            let name = &input[4..];
                            // 复用 AddTask 逻辑
                            println!("添加任务: {}", name);
                        } else {
                            println!("未知命令: {}", input);
                        }
                    }
                }
            }
        }
    }
}

/// 递归更新任务状态
fn update_task_status(tasks: &mut [Task], target_id: &TaskId, status: TaskStatus) -> bool {
    for task in tasks.iter_mut() {
        if task.id == *target_id {
            task.status = status;
            return true;
        }
        
        if !task.children.is_empty() {
            if update_task_status(&mut task.children, target_id, status) {
                return true;
            }
        }
    }
    
    false
}
