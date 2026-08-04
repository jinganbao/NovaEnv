//! Rust 适配器。
//!
//! 支持常见安装来源：NovaEnv 管理安装（官方 dist 预编译 toolchain）、
//! rustup（~/.rustup/toolchains）、Homebrew（/opt/homebrew/opt/rust）。
//! 版本通过运行 `bin/rustc --version` 获取，失败时从目录名推断。

use std::path::PathBuf;

use crate::adapter::RuntimeAdapter;
use crate::models::{RuntimeKind, RuntimeVersion};
use crate::platform;

pub struct RustAdapter;

impl RuntimeAdapter for RustAdapter {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Rust
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
            let bin = rustc_bin(&dir);
            let version = if bin.is_file() {
                bin.to_str()
                    .and_then(|p| platform::run_capture(p, &["--version"]))
                    .and_then(|v| parse_rustc_version(&v))
                    .or_else(|| infer_from_dir_name(&dir))
            } else {
                infer_from_dir_name(&dir)
            };
            if let Some(version) = version {
                versions.push(RuntimeVersion {
                    kind: RuntimeKind::Rust,
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
        platform::run_capture("rustc", &["--version"]).and_then(|v| parse_rustc_version(&v))
    }
}

/// rustc 可执行文件路径（Windows 为 rustc.exe）
fn rustc_bin(dir: &std::path::Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        dir.join("bin").join("rustc.exe")
    }
    #[cfg(not(target_os = "windows"))]
    {
        dir.join("bin").join("rustc")
    }
}

/// 解析 `rustc --version` 输出：`rustc 1.88.0 (a1c84a5e0 2025-07-24)`
fn parse_rustc_version(v: &str) -> Option<String> {
    let first = v.lines().next()?;
    let rest = first.strip_prefix("rustc ")?;
    let token = rest.split(' ').next()?;
    if token.matches('.').count() >= 2 && token.chars().all(|c| c.is_ascii_digit() || c == '.') {
        Some(token.to_string())
    } else {
        None
    }
}

/// 从目录名推断版本：1.88.0 / 1.88.0-aarch64-apple-darwin
fn infer_from_dir_name(dir: &std::path::Path) -> Option<String> {
    let name = dir.file_name()?.to_string_lossy().into_owned();
    // rustup 工具链目录：1.88.0-aarch64-apple-darwin
    let head = name.split('-').next()?;
    if head.matches('.').count() >= 2 && head.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return Some(head.to_string());
    }
    crate::runtimes::java::extract_version(&name)
}

#[cfg(target_os = "macos")]
fn candidate_dirs_macos() -> Vec<(PathBuf, &'static str)> {
    let mut dirs = Vec::new();
    if let Some(home) = platform::home_dir() {
        // rustup 工具链
        if let Ok(entries) = std::fs::read_dir(home.join(".rustup/toolchains")) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    dirs.push((entry.path(), "rustup"));
                }
            }
        }
        // NovaEnv 管理安装（枚举每个已安装版本目录）
        let installs_rust = crate::installer::installs_dir().join("rust");
        if let Ok(entries) = std::fs::read_dir(&installs_rust) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    dirs.push((entry.path(), "novaenv"));
                }
            }
        }
    }
    // Homebrew
    for opt in ["/opt/homebrew/opt", "/usr/local/opt"] {
        let d = std::path::Path::new(opt).join("rust");
        if d.join("bin").join("rustc").is_file() {
            dirs.push((d, "homebrew"));
        }
    }
    dirs
}

#[cfg(target_os = "windows")]
fn candidate_dirs_windows() -> Vec<(PathBuf, &'static str)> {
    let mut dirs = Vec::new();
    if let Some(home) = platform::home_dir() {
        // rustup 工具链（Windows：~/.rustup/toolchains/<ver>-x86_64-pc-windows-msvc）
        if let Ok(entries) = std::fs::read_dir(home.join(".rustup/toolchains")) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    dirs.push((entry.path(), "rustup"));
                }
            }
        }
        let installs_rust = crate::installer::installs_dir().join("rust");
        if let Ok(entries) = std::fs::read_dir(&installs_rust) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    dirs.push((entry.path(), "novaenv"));
                }
            }
        }
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rustc_version() {
        assert_eq!(
            parse_rustc_version("rustc 1.88.0 (a1c84a5e0 2025-07-24)\n"),
            Some("1.88.0".to_string())
        );
        assert_eq!(parse_rustc_version("command not found"), None);
    }

    #[test]
    fn infers_from_dir_names() {
        assert_eq!(
            infer_from_dir_name(std::path::Path::new("/Users/x/.rustup/toolchains/1.88.0-aarch64-apple-darwin")),
            Some("1.88.0".to_string())
        );
        assert_eq!(
            infer_from_dir_name(std::path::Path::new("/Users/x/.novaenv/installs/rust/1.88.0")),
            Some("1.88.0".to_string())
        );
        assert_eq!(
            infer_from_dir_name(std::path::Path::new("/opt/homebrew/opt/rust")),
            None
        );
    }
}
