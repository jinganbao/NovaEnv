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
    let Some(home) = super::home_dir() else {
        return Vec::new();
    };
    let Ok(content) = std::fs::read_to_string(home.join(".zshrc")) else {
        return Vec::new();
    };
    parse_active_config(&content)
}

/// 解析 .zshrc 管理块（纯函数，便于单元测试）：
/// 提取 NovaEnv 管理块内的 JAVA_HOME / NODE_HOME / GOROOT / MAVEN_HOME
pub fn parse_active_config(content: &str) -> Vec<String> {
    const BLOCK_START: &str = "# >>> NovaEnv managed >>>";
    const BLOCK_END: &str = "# <<< NovaEnv managed <<<";
    const VARS: [&str; 6] = [
        "JAVA_HOME",
        "NODE_HOME",
        "GOROOT",
        "MAVEN_HOME",
        "PYTHON_HOME",
        "RUST_HOME",
    ];

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

#[cfg(test)]
mod tests {
    use super::parse_active_config;

    #[test]
    fn parses_managed_block_only() {
        let content = "\
# >>> NovaEnv managed >>>
export NODE_HOME=\"/Users/x/.novaenv/installs/node/26.6.0\"
export PATH=\"$NODE_HOME/bin:$PATH\"
export JAVA_HOME='/Users/x/.novaenv/installs/java/25.36.15/Contents/Home'
export PATH=\"$JAVA_HOME/bin:$PATH\"
# <<< NovaEnv managed <<<
export JAVA_HOME=\"/usr/local/java\"  # 块外变量不应被读取
";
        let paths = parse_active_config(content);
        assert_eq!(
            paths,
            vec![
                "/Users/x/.novaenv/installs/node/26.6.0",
                "/Users/x/.novaenv/installs/java/25.36.15/Contents/Home",
            ]
        );
    }

    #[test]
    fn empty_without_block() {
        assert!(parse_active_config("export JAVA_HOME=\"/usr/local/java\"\n").is_empty());
    }

    #[test]
    fn ignores_empty_values() {
        let content = "# >>> NovaEnv managed >>>\nexport NODE_HOME=\"\"\n# <<< NovaEnv managed <<<";
        assert!(parse_active_config(content).is_empty());
    }
}
