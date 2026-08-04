//! 服务类组件聚合：Redis（后续扩展 MySQL、PostgreSQL 等）。
//!
//! 接入步骤：
//! 1. 在 `models::ServiceKind` 增加枚举值；
//! 2. 在 `services/` 新建 `<name>.rs` 实现安装/启停逻辑；
//! 3. 在 `list_all()` 中注册。

#[cfg(target_os = "macos")]
pub mod launchd;
pub mod mysql;
pub mod redis;

use crate::models::ServiceInfo;

/// 全部服务组件状态
pub fn list_all() -> Vec<ServiceInfo> {
    vec![redis::info(), mysql::info()]
}

/// 读取日志文件尾部（默认 200 行）
#[cfg(target_os = "macos")]
pub fn tail_log_file(path: &std::path::Path, lines: usize) -> Result<String, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("读取日志失败: {e}"))?;
    let all: Vec<&str> = content.lines().collect();
    let n = all.len().min(lines.max(1));
    Ok(all[all.len() - n..].join("\n"))
}

#[cfg(test)]
mod tests {
    use super::tail_log_file;
    use std::path::PathBuf;

    #[test]
    fn tails_last_lines() {
        let path = std::env::temp_dir().join(format!("novaenv-test-log-{}", std::process::id()));
        let lines: Vec<String> = (0..10).map(|i| format!("line {i}")).collect();
        std::fs::write(&path, lines.join("\n")).unwrap();
        let tail = tail_log_file(&path, 3).unwrap();
        assert_eq!(tail, "line 7\nline 8\nline 9");
        let all = tail_log_file(&path, 100).unwrap();
        assert_eq!(all.lines().count(), 10);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_log_errors() {
        let path: PathBuf = "/nonexistent/novaenv-test.log".into();
        assert!(tail_log_file(&path, 10).is_err());
    }
}
