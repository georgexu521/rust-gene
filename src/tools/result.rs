//! 工具结果与错误类型
//!
//! 从 `tools/mod.rs` 拆分出来的 ToolResult、ToolErrorCode、ToolPermissionLevel。

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 工具错误码
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ToolErrorCode {
    /// 成功
    Success,
    /// 参数验证失败
    InvalidParams,
    /// 权限被拒绝
    PermissionDenied,
    /// 资源不存在
    NotFound,
    /// 执行超时
    Timeout,
    /// 执行失败
    ExecutionFailed,
    /// 工具不可用
    Unavailable,
    /// 取消执行
    Cancelled,
    /// 危险操作被拦截
    DangerousBlocked,
    /// 未知错误
    #[default]
    Unknown,
}

impl ToolErrorCode {
    /// 从错误信息推断错误码
    pub fn from_error(error: &str) -> Self {
        let e = error.to_ascii_lowercase();
        if e.contains("invalid param")
            || e.contains("missing required")
            || e.contains("must be of type")
        {
            ToolErrorCode::InvalidParams
        } else if e.contains("permission denied") || e.contains("denied") {
            ToolErrorCode::PermissionDenied
        } else if e.contains("not found") || e.contains("does not exist") {
            ToolErrorCode::NotFound
        } else if e.contains("timeout") || e.contains("timed out") {
            ToolErrorCode::Timeout
        } else if e.contains("dangerous") || e.contains("blocked") {
            ToolErrorCode::DangerousBlocked
        } else if e.contains("cancelled") || e.contains("canceled") {
            ToolErrorCode::Cancelled
        } else {
            ToolErrorCode::Unknown
        }
    }

    /// 获取错误码的 HTTP 状态码映射
    pub fn http_status(&self) -> u16 {
        match self {
            ToolErrorCode::Success => 200,
            ToolErrorCode::InvalidParams => 400,
            ToolErrorCode::PermissionDenied => 403,
            ToolErrorCode::NotFound => 404,
            ToolErrorCode::Timeout => 408,
            ToolErrorCode::DangerousBlocked => 451,
            ToolErrorCode::Unavailable
            | ToolErrorCode::ExecutionFailed
            | ToolErrorCode::Cancelled
            | ToolErrorCode::Unknown => 500,
        }
    }
}

/// 工具权限等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ToolPermissionLevel {
    /// 只读操作，不会修改任何文件或系统状态
    ReadOnly,
    /// 低风险操作，只会读取或创建临时文件
    #[default]
    LowRisk,
    /// 中等风险操作，会修改项目文件但可撤销
    MediumRisk,
    /// 高风险操作，会修改系统配置或不可撤销的操作
    HighRisk,
    /// 最高风险操作，可能影响系统安全或数据
    Critical,
}

impl ToolPermissionLevel {
    /// 从操作名称推断权限等级
    pub fn from_operation(op: &str) -> Self {
        let op_lower = op.to_ascii_lowercase();
        if op_lower.contains("read")
            || op_lower.contains("get")
            || op_lower.contains("list")
            || op_lower.contains("search")
        {
            ToolPermissionLevel::ReadOnly
        } else if op_lower.contains("write")
            || op_lower.contains("edit")
            || op_lower.contains("create")
        {
            ToolPermissionLevel::MediumRisk
        } else if op_lower.contains("delete")
            || op_lower.contains("remove")
            || op_lower.contains("kill")
        {
            ToolPermissionLevel::HighRisk
        } else if op_lower.contains("exec")
            || op_lower.contains("bash")
            || op_lower.contains("shell")
        {
            ToolPermissionLevel::Critical
        } else {
            ToolPermissionLevel::LowRisk
        }
    }

    /// 是否需要确认提示
    pub fn requires_confirmation(&self) -> bool {
        matches!(
            self,
            ToolPermissionLevel::HighRisk | ToolPermissionLevel::Critical
        )
    }
}

/// 工具执行结果
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolResult {
    /// 是否成功
    pub success: bool,
    /// 输出内容
    pub content: String,
    /// 错误信息（如果有）
    pub error: Option<String>,
    /// 错误码
    #[serde(default)]
    pub error_code: Option<ToolErrorCode>,
    /// 额外数据（JSON 格式）
    pub data: Option<Value>,
    /// 执行耗时（毫秒）
    #[serde(default)]
    pub duration_ms: Option<u64>,
    /// 工具名称（用于审计）
    #[serde(default)]
    pub tool_name: Option<String>,
}

impl ToolResult {
    /// 创建成功结果
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            success: true,
            content: content.into(),
            error_code: Some(ToolErrorCode::Success),
            ..Default::default()
        }
    }

    /// 创建带数据的成功结果
    pub fn success_with_data(content: impl Into<String>, data: Value) -> Self {
        Self {
            success: true,
            content: content.into(),
            error_code: Some(ToolErrorCode::Success),
            data: Some(data),
            ..Default::default()
        }
    }

    /// 创建失败结果
    pub fn error(error: impl Into<String>) -> Self {
        let err_str = error.into();
        Self {
            success: false,
            error: Some(err_str.clone()),
            error_code: Some(ToolErrorCode::from_error(&err_str)),
            ..Default::default()
        }
    }

    /// 创建带内容的失败结果
    pub fn error_with_content(error: impl Into<String>, content: impl Into<String>) -> Self {
        let err_str = error.into();
        Self {
            success: false,
            content: content.into(),
            error: Some(err_str.clone()),
            error_code: Some(ToolErrorCode::from_error(&err_str)),
            ..Default::default()
        }
    }

    /// 从缓存值重建 ToolResult
    pub fn from_cached_value(value: Value) -> Self {
        // 尝试从缓存的 JSON 重建
        if let Ok(result) = serde_json::from_value::<ToolResult>(value.clone()) {
            return result;
        }

        // 如果反序列化失败，创建一个通用的成功结果
        let content = value
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("Cached result")
            .to_string();

        let success = value
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let error = value
            .get("error")
            .and_then(|v| v.as_str())
            .map(String::from);

        let data = value.get("data").cloned();

        Self {
            success,
            content,
            error: error.clone(),
            error_code: error.as_ref().map(|e| ToolErrorCode::from_error(e)),
            data,
            ..Default::default()
        }
    }
}
