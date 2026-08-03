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

/// 解析 ~/.zshrc 中 NovaEnv 管理块（`# >>> NovaEnv managed >>>`），
/// 提取激活的运行时路径（JAVA_HOME / NODE_HOME / GOROOT）。
/// 不依赖进程 PATH，切换默认后任何时刻都能读到准确状态。
pub fn active_config_paths() -> Vec<String> {
    const BLOCK_START: &str = "# >>> NovaEnv managed >>>";
    const BLOCK_END: &str = "# <<< NovaEnv managed <<<";
    const VARS: [&str; 4] = ["JAVA_HOME", "NODE_HOME", "GOROOT", "MAVEN_HOME"];

    let Some(home) = super::home_dir() else {
        return Vec::new();
    };
    let Ok(content) = std::fs::read_to_string(home.join(".zshrc")) else {
        return Vec::new();
    };

    let mut in_block = false;
    let mut paths = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with(BLOCK_START) {
            in_block = true;
            continue;
        }
        if line.starts_with(BLOCK_END) {
            break;
        }
        if !in_block {
            continue;
        }
        // 仅解析管理块内的 export（避免误读用户自行配置的变量）
        let Some(rest) = line.strip_prefix("export ") else {
            continue;
        };
        let Some((var, value)) = rest.split_once('=') else {
            continue;
        };
        if VARS.contains(&var.trim()) {
            let value = value.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                paths.push(value.to_string());
            }
        }
    }
    paths
}
