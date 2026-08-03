//! macOS 平台辅助：JDK 扫描依赖 `/usr/libexec/java_home -V`。

use std::process::Command;

/// 运行 `/usr/libexec/java_home -V`，返回其 stderr 输出（版本列表打印在 stderr）。
pub fn java_home_listing() -> Option<String> {
    let output = Command::new("/usr/libexec/java_home")
        .arg("-V")
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stderr).into_owned())
    } else {
        None
    }
}

/// 查询 `/usr/libexec/java_home` 返回的当前默认 JDK 路径。
pub fn default_java_home() -> Option<String> {
    let output = Command::new("/usr/libexec/java_home").output().ok()?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Some(path);
        }
    }
    None
}
