//! Python 适配器。
//!
//! 支持常见安装来源：NovaEnv 管理安装（python-build-standalone）、
//! pyenv（~/.pyenv/versions）、Homebrew（python@3.x）、系统自带（/usr/bin/python3）。
//! 版本通过运行 `bin/python3 --version` 获取，失败时从目录名推断。

use std::path::PathBuf;

use crate::adapter::RuntimeAdapter;
use crate::models::{RuntimeKind, RuntimeVersion};
use crate::platform;

pub struct PythonAdapter;

impl RuntimeAdapter for PythonAdapter {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Python
    }

    #[allow(unused_mut)]
    fn scan(&self) -> Vec<RuntimeVersion> {
        let mut dirs: Vec<(PathBuf, &str)> = Vec::new(); // (候选安装目录, 来源名)

        #[cfg(target_os = "macos")]
        {
            dirs.extend(candidate_dirs_macos());
        }
        #[cfg(target_os = "windows")]
        {
            dirs.extend(candidate_dirs_windows());
        }

        let mut seen = Vec::new();
        let mut versions = Vec::new();

        for (dir, vendor) in dirs {
            if !platform::is_dir(&dir) || seen.contains(&dir) {
                continue;
            }
            seen.push(dir.clone());
            let bin = python_bin(&dir);
            let version = if bin.is_file() {
                bin.to_str()
                    .and_then(|p| platform::run_capture(p, &["--version"]))
                    .and_then(|v| parse_python_version(&v))
                    .or_else(|| infer_from_dir_name(&dir))
            } else {
                infer_from_dir_name(&dir)
            };
            if let Some(version) = version {
                versions.push(RuntimeVersion {
                    kind: RuntimeKind::Python,
                    version,
                    vendor: vendor.to_string(),
                    path: dir.to_string_lossy().into_owned(),
                    is_default: crate::platform::is_active_dir(&dir),
                    managed: crate::installer::is_managed(&dir.to_string_lossy()),
                });
            }
        }

        versions
    }

    fn active_version(&self) -> Option<String> {
        platform::run_capture("python3", &["--version"]).and_then(|v| parse_python_version(&v))
    }
}

/// python 可执行文件路径（Windows 为 python.exe）
fn python_bin(dir: &std::path::Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        dir.join("bin").join("python.exe")
    }
    #[cfg(not(target_os = "windows"))]
    {
        dir.join("bin").join("python3")
    }
}

/// 解析 `python3 --version` 输出：`Python 3.13.1`
fn parse_python_version(v: &str) -> Option<String> {
    let first = v.lines().next()?;
    let token = first.strip_prefix("Python ")?;
    let token = token.trim();
    // 可能带 "+" 后缀（如 3.13.1+）或 "rc1"
    let token = token.split(['+', ' ', '-']).next().unwrap_or(token);
    if token.matches('.').count() >= 2 && token.chars().all(|c| c.is_ascii_digit() || c == '.') {
        Some(token.to_string())
    } else {
        None
    }
}

/// 从目录名推断版本：3.13.1 / python@3.13
fn infer_from_dir_name(dir: &std::path::Path) -> Option<String> {
    let name = dir.file_name()?.to_string_lossy().into_owned();
    // pyenv 版本目录即版本号
    if name.chars().all(|c| c.is_ascii_digit() || c == '.') && name.matches('.').count() >= 2 {
        return Some(name);
    }
    // Homebrew python@3.13
    if let Some(rest) = name.strip_prefix("python@") {
        if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit() || c == '.') {
            return Some(rest.to_string());
        }
    }
    crate::runtimes::java::extract_version(&name)
}

#[cfg(target_os = "macos")]
fn candidate_dirs_macos() -> Vec<(PathBuf, &'static str)> {
    let mut dirs = Vec::new();
    if let Some(home) = platform::home_dir() {
        // pyenv
        if let Ok(entries) = std::fs::read_dir(home.join(".pyenv/versions")) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    dirs.push((entry.path(), "pyenv"));
                }
            }
        }
        // NovaEnv 管理安装（枚举每个已安装版本目录）
        let installs_python = crate::installer::installs_dir().join("python");
        if let Ok(entries) = std::fs::read_dir(&installs_python) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    dirs.push((entry.path(), "novaenv"));
                }
            }
        }
    }
    // Homebrew（版本化与默认链接）
    for opt in ["/opt/homebrew/opt", "/usr/local/opt"] {
        if let Ok(entries) = std::fs::read_dir(opt) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with("python@") || name == "python" {
                    dirs.push((entry.path(), "homebrew"));
                }
            }
        }
    }
    // 系统自带 Python（macOS /usr/bin/python3）
    if std::path::Path::new("/usr/bin/python3").is_file() {
        dirs.push((PathBuf::from("/usr/bin"), "system"));
    }
    dirs
}

#[cfg(target_os = "windows")]
fn candidate_dirs_windows() -> Vec<(PathBuf, &'static str)> {
    let mut dirs = Vec::new();
    // 官方安装 / pyenv-win
    if let Some(home) = platform::home_dir() {
        if let Ok(entries) = std::fs::read_dir(home.join(".pyenv/pyenv-win/versions")) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    dirs.push((entry.path(), "pyenv"));
                }
            }
        }
        let installs_python = crate::installer::installs_dir().join("python");
        if let Ok(entries) = std::fs::read_dir(&installs_python) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    dirs.push((entry.path(), "novaenv"));
                }
            }
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push((PathBuf::from(&local).join("Programs/Python"), "official"));
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version_output() {
        assert_eq!(
            parse_python_version("Python 3.13.1\n"),
            Some("3.13.1".to_string())
        );
        assert_eq!(
            parse_python_version("Python 3.9.6"),
            Some("3.9.6".to_string())
        );
        assert_eq!(parse_python_version("command not found"), None);
    }

    #[test]
    fn infers_from_dir_names() {
        assert_eq!(
            infer_from_dir_name(std::path::Path::new("/Users/x/.pyenv/versions/3.13.1")),
            Some("3.13.1".to_string())
        );
        assert_eq!(
            infer_from_dir_name(std::path::Path::new("/opt/homebrew/opt/python@3.13")),
            Some("3.13".to_string())
        );
        assert_eq!(
            infer_from_dir_name(std::path::Path::new("/usr/bin")),
            None
        );
    }
}
