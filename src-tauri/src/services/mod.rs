//! 服务类组件聚合：Redis（后续扩展 MySQL、PostgreSQL 等）。
//!
//! 接入步骤：
//! 1. 在 `models::ServiceKind` 增加枚举值；
//! 2. 在 `services/` 新建 `<name>.rs` 实现安装/启停逻辑；
//! 3. 在 `list_all()` 中注册。

pub mod launchd;
pub mod mysql;
pub mod redis;

use crate::models::ServiceInfo;

/// 全部服务组件状态
pub fn list_all() -> Vec<ServiceInfo> {
    vec![redis::info(), mysql::info()]
}

/// 读取日志文件尾部（默认 200 行）
pub fn tail_log_file(path: &std::path::Path, lines: usize) -> Result<String, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("读取日志失败: {e}"))?;
    let all: Vec<&str> = content.lines().collect();
    let n = all.len().min(lines.max(1));
    Ok(all[all.len() - n..].join("\n"))
}
