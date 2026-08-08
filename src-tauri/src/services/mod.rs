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
pub mod vision;

use crate::models::ServiceInfo;

/// 全部服务组件状态
pub fn list_all() -> Vec<ServiceInfo> {
    vec![redis::info(), mysql::info()]
}

/// 读取日志文件尾部（默认 200 行）。
/// 只从文件尾部倒读最多 256KB 再按行截取，避免长期运行的大日志被全量读入。
#[cfg(target_os = "macos")]
pub fn tail_log_file(path: &std::path::Path, lines: usize) -> Result<String, String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).map_err(|e| format!("读取日志失败: {e}"))?;
    let lines = lines.max(1);
    let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);
    const MAX_TAIL_BYTES: u64 = 256 * 1024;
    let start = file_len.saturating_sub(MAX_TAIL_BYTES);
    file.seek(SeekFrom::Start(start))
        .map_err(|e| format!("读取日志失败: {e}"))?;
    let mut buf = Vec::with_capacity((file_len - start) as usize);
    file.read_to_end(&mut buf)
        .map_err(|e| format!("读取日志失败: {e}"))?;
    let content = String::from_utf8_lossy(&buf);
    let all: Vec<&str> = content.lines().collect();
    // 截断处首行不完整，丢弃（但尾部 256KB 内无换行时保留仅有的内容行）
    let slice = if start > 0 && all.len() > 1 { &all[1..] } else { &all[..] };
    let n = slice.len().min(lines);
    let mut out = slice[slice.len() - n..].join("\n");
    if start > 0 {
        out = format!("…（日志较长，仅显示末尾 {n} 行）\n{out}");
    }
    Ok(out)
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

    #[test]
    fn large_log_tails_from_end() {
        // 超过 256KB 的日志：只读尾部，且带截断提示前缀
        let path = std::env::temp_dir().join(format!("novaenv-test-large-{}", std::process::id()));
        let line = "0123456789".repeat(3); // 30 字节/行
        let content: String = (0..20_000)
            .map(|i| format!("{i} {line}"))
            .collect::<Vec<_>>()
            .join("\n"); // 20_000 行 × ~37 字节 ≈ 740KB > 256KB
        std::fs::write(&path, &content).unwrap();
        let tail = tail_log_file(&path, 3).unwrap();
        assert!(tail.starts_with("…（日志较长"), "应带截断提示: {tail}");
        assert!(tail.contains(&format!("19997 {line}")));
        assert!(tail.contains(&format!("19999 {line}")));
        assert!(!tail.contains("line 0"), "不应包含文件开头内容");
        let _ = std::fs::remove_file(&path);
    }
}
