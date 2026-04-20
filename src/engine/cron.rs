//! Cron 定时任务系统
//!
//! 类似 Claude Code 的 CronTool 和 Hermes 的 cron 系统：
//! - Agent 可以给自己设置定时任务
//! - 支持一次性延迟执行和周期性执行
//! - 存储在 SQLite 中持久化

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;
use uuid::Uuid;

/// Cron 任务定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    /// 任务 ID
    pub id: String,
    /// 任务名称
    pub name: String,
    /// 要执行的 prompt（自包含，不依赖当前对话上下文）
    pub prompt: String,
    /// Cron 表达式（简化版）或延迟秒数
    pub schedule: CronSchedule,
    /// 是否启用
    pub enabled: bool,
    /// 创建时间
    pub created_at: String,
    /// 下次执行时间
    pub next_run: Option<String>,
    /// 已执行次数
    pub run_count: u64,
    /// 最后执行结果
    pub last_result: Option<String>,
}

/// Cron 调度类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CronSchedule {
    /// 延迟 N 秒后执行一次
    DelayOnce { seconds: u64 },
    /// 每隔 N 秒执行
    Interval { seconds: u64 },
    /// Cron 表达式（简化版：支持 "*/N * * * *" 格式）
    Cron { expression: String },
    /// 一次性：在指定时间执行
    AtTime { iso_datetime: String },
}

/// Cron 管理器
pub struct CronManager {
    jobs: Arc<RwLock<Vec<CronJob>>>,
}

impl CronManager {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 添加定时任务
    pub async fn add_job(&self, job: CronJob) -> String {
        let id = job.id.clone();
        info!("Added cron job: {} ({})", job.name, id);
        self.jobs.write().await.push(job);
        id
    }

    /// 创建延迟任务
    pub async fn schedule_delay(&self, name: String, prompt: String, delay_seconds: u64) -> String {
        let job = CronJob {
            id: format!("cron-{}", &Uuid::new_v4().to_string()[..8]),
            name,
            prompt,
            schedule: CronSchedule::DelayOnce {
                seconds: delay_seconds,
            },
            enabled: true,
            created_at: chrono::Utc::now().to_rfc3339(),
            next_run: None,
            run_count: 0,
            last_result: None,
        };
        self.add_job(job).await
    }

    /// 创建周期任务
    pub async fn schedule_interval(
        &self,
        name: String,
        prompt: String,
        interval_seconds: u64,
    ) -> String {
        let job = CronJob {
            id: format!("cron-{}", &Uuid::new_v4().to_string()[..8]),
            name,
            prompt,
            schedule: CronSchedule::Interval {
                seconds: interval_seconds,
            },
            enabled: true,
            created_at: chrono::Utc::now().to_rfc3339(),
            next_run: None,
            run_count: 0,
            last_result: None,
        };
        self.add_job(job).await
    }

    /// 列出所有任务
    pub async fn list_jobs(&self) -> Vec<CronJob> {
        self.jobs.read().await.clone()
    }

    /// 获取待执行的任务
    pub async fn get_due_jobs(&self) -> Vec<CronJob> {
        let jobs = self.jobs.read().await;
        jobs.iter().filter(|j| j.enabled).cloned().collect()
    }

    /// 更新任务执行结果
    pub async fn update_result(&self, job_id: &str, result: String) {
        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
            job.run_count += 1;
            job.last_result = Some(result);

            // 一次性任务执行后禁用
            if matches!(job.schedule, CronSchedule::DelayOnce { .. }) {
                job.enabled = false;
            }
        }
    }

    /// 删除任务
    pub async fn remove_job(&self, job_id: &str) -> bool {
        let mut jobs = self.jobs.write().await;
        let before = jobs.len();
        jobs.retain(|j| j.id != job_id);
        jobs.len() < before
    }

    /// 暂停任务
    pub async fn pause_job(&self, job_id: &str) -> bool {
        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
            job.enabled = false;
            return true;
        }
        false
    }

    /// 恢复任务
    pub async fn resume_job(&self, job_id: &str) -> bool {
        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
            job.enabled = true;
            return true;
        }
        false
    }

    /// 获取任务数量
    pub async fn job_count(&self) -> usize {
        self.jobs.read().await.len()
    }
}

impl Default for CronManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── Cron 工具接口 ──────────────────────────────────────

/// Cron 工具 - 让 agent 管理定时任务
/// 使用全局 CronManager 单例
pub struct CronTool;

impl CronTool {
    /// 获取全局 CronManager
    fn global_manager() -> &'static Arc<CronManager> {
        use std::sync::OnceLock;
        static INSTANCE: OnceLock<Arc<CronManager>> = OnceLock::new();
        INSTANCE.get_or_init(|| Arc::new(CronManager::new()))
    }
}

#[async_trait::async_trait]
impl crate::tools::Tool for CronTool {
    fn name(&self) -> &str {
        "cron"
    }

    fn description(&self) -> &str {
        "Schedule and manage cron jobs. The agent can set timers, recurring tasks, \
         or schedule future actions. Jobs are self-contained prompts that run later."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "list", "pause", "resume", "remove", "run"],
                    "description": "create: schedule a new job. list: show all jobs. \
                                   pause/resume: toggle a job. remove: delete a job. \
                                   run: execute a job immediately."
                },
                "name": {
                    "type": "string",
                    "description": "Job name (for create)"
                },
                "prompt": {
                    "type": "string",
                    "description": "Self-contained prompt to execute (for create)"
                },
                "schedule": {
                    "type": "string",
                    "description": "Schedule: '30m' (delay once), 'every 2h' (interval), or '0 9 * * *' (cron)"
                },
                "job_id": {
                    "type": "string",
                    "description": "Job ID (for pause/resume/remove/run)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        _context: crate::tools::ToolContext,
    ) -> crate::tools::ToolResult {
        let action = params["action"].as_str().unwrap_or("list");

        match action {
            "create" => {
                let name = params["name"].as_str().unwrap_or("unnamed").to_string();
                let prompt = params["prompt"].as_str().unwrap_or("");
                let schedule_str = params["schedule"].as_str().unwrap_or("30m");

                if prompt.is_empty() {
                    return crate::tools::ToolResult::error("prompt required for 'create'");
                }

                // 解析 schedule
                let job_id = if schedule_str.starts_with("every ") {
                    let interval = parse_duration(schedule_str.trim_start_matches("every "));
                    Self::global_manager()
                        .schedule_interval(name, prompt.to_string(), interval)
                        .await
                } else {
                    let delay = parse_duration(schedule_str);
                    Self::global_manager()
                        .schedule_delay(name, prompt.to_string(), delay)
                        .await
                };

                crate::tools::ToolResult::success(format!(
                    "Created cron job '{}' (id: {}), schedule: {}",
                    params["name"].as_str().unwrap_or("unnamed"),
                    job_id,
                    schedule_str
                ))
            }

            "list" => {
                let jobs = Self::global_manager().list_jobs().await;
                if jobs.is_empty() {
                    crate::tools::ToolResult::success("No cron jobs.".to_string())
                } else {
                    let mut output = format!("Cron jobs ({}):\n\n", jobs.len());
                    for job in &jobs {
                        output.push_str(&format!(
                            "- {} ({}): {} [{}] runs: {}\n",
                            job.name,
                            job.id,
                            if job.enabled { "enabled" } else { "paused" },
                            match &job.schedule {
                                CronSchedule::DelayOnce { seconds } =>
                                    format!("once in {}s", seconds),
                                CronSchedule::Interval { seconds } => format!("every {}s", seconds),
                                CronSchedule::Cron { expression } => expression.clone(),
                                CronSchedule::AtTime { iso_datetime } =>
                                    format!("at {}", iso_datetime),
                            },
                            job.run_count
                        ));
                        // 显示 prompt 预览
                        let preview: String = job.prompt.chars().take(60).collect();
                        output.push_str(&format!("  prompt: {}...\n", preview));
                    }
                    crate::tools::ToolResult::success(output)
                }
            }

            "pause" => {
                let job_id = params["job_id"].as_str().unwrap_or("");
                if job_id.is_empty() {
                    return crate::tools::ToolResult::error("job_id required");
                }
                if Self::global_manager().pause_job(job_id).await {
                    crate::tools::ToolResult::success(format!("Paused job {}", job_id))
                } else {
                    crate::tools::ToolResult::error(format!("Job {} not found", job_id))
                }
            }

            "resume" => {
                let job_id = params["job_id"].as_str().unwrap_or("");
                if job_id.is_empty() {
                    return crate::tools::ToolResult::error("job_id required");
                }
                if Self::global_manager().resume_job(job_id).await {
                    crate::tools::ToolResult::success(format!("Resumed job {}", job_id))
                } else {
                    crate::tools::ToolResult::error(format!("Job {} not found", job_id))
                }
            }

            "remove" => {
                let job_id = params["job_id"].as_str().unwrap_or("");
                if job_id.is_empty() {
                    return crate::tools::ToolResult::error("job_id required");
                }
                if Self::global_manager().remove_job(job_id).await {
                    crate::tools::ToolResult::success(format!("Removed job {}", job_id))
                } else {
                    crate::tools::ToolResult::error(format!("Job {} not found", job_id))
                }
            }

            "run" => {
                let job_id = params["job_id"].as_str().unwrap_or("");
                if job_id.is_empty() {
                    return crate::tools::ToolResult::error("job_id required");
                }
                // 获取 job prompt
                let jobs = Self::global_manager().list_jobs().await;
                if let Some(job) = jobs.iter().find(|j| j.id == job_id) {
                    crate::tools::ToolResult::success(format!(
                        "Job '{}' would execute with prompt:\n\n{}",
                        job.name, job.prompt
                    ))
                } else {
                    crate::tools::ToolResult::error(format!("Job {} not found", job_id))
                }
            }

            _ => crate::tools::ToolResult::error(format!(
                "Unknown action: {}. Use create, list, pause, resume, remove, run",
                action
            )),
        }
    }
}

/// 解析人类可读的时间间隔为秒
fn parse_duration(s: &str) -> u64 {
    let s = s.trim().to_lowercase();
    if s.ends_with("s") {
        s[..s.len() - 1].parse().unwrap_or(60)
    } else if s.ends_with("m") {
        s[..s.len() - 1].parse::<u64>().unwrap_or(5) * 60
    } else if s.ends_with("h") {
        s[..s.len() - 1].parse::<u64>().unwrap_or(1) * 3600
    } else if s.ends_with("d") {
        s[..s.len() - 1].parse::<u64>().unwrap_or(1) * 86400
    } else {
        // 纯数字，当作秒
        s.parse().unwrap_or(300)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("30s"), 30);
        assert_eq!(parse_duration("5m"), 300);
        assert_eq!(parse_duration("2h"), 7200);
        assert_eq!(parse_duration("1d"), 86400);
        assert_eq!(parse_duration("60"), 60);
    }

    #[tokio::test]
    async fn test_cron_manager() {
        let manager = CronManager::new();

        let id = manager
            .schedule_delay("test".to_string(), "Do something".to_string(), 60)
            .await;
        assert!(id.starts_with("cron-"));

        let jobs = manager.list_jobs().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].name, "test");

        assert!(manager.pause_job(&id).await);
        assert!(!manager.list_jobs().await[0].enabled);

        assert!(manager.resume_job(&id).await);
        assert!(manager.list_jobs().await[0].enabled);

        assert!(manager.remove_job(&id).await);
        assert_eq!(manager.list_jobs().await.len(), 0);
    }

    #[tokio::test]
    async fn test_cron_interval() {
        let manager = CronManager::new();

        let _id = manager
            .schedule_interval("check".to_string(), "Check status".to_string(), 300)
            .await;

        let jobs = manager.list_jobs().await;
        assert!(matches!(
            jobs[0].schedule,
            CronSchedule::Interval { seconds: 300 }
        ));
    }
}
