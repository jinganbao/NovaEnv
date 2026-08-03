//! Go 适配器。
//!
//! 支持常见安装来源：官方安装（/usr/local/go、C:\Program Files\Go）、
//! Homebrew（go@ 版本化安装）、goenv（版本目录）。
//! 版本优先读 `VERSION` 文件（内容形如 `go1.23.4`），失败时从目录名推断。

use std::path::PathBuf;

use crate::adapter::RuntimeAdapter;
use crate::models::{RuntimeKind, RuntimeVersion};
use crate::platform;

pub struct GoAdapter;

impl RuntimeAdapter for GoAdapter {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Go
    }

    // Linux 平台暂无候选目录扫描（CI 打包用），dirs 在 macOS/Windows 下需要可变
    #[allow(unused_mut)]
    fn scan(&self) -> Vec<RuntimeVersion> {
        let mut dirs: Vec<(PathBuf, &str)> = Vec::new();

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
            let Some(version) = read_version_file(&dir).or_else(|| infer_from_dir_name(&dir))
            else {
                continue;
            };
            versions.push(RuntimeVersion {
                kind: RuntimeKind::Go,
                version,
                vendor: vendor.to_string(),
                is_default: crate::platform::is_active_dir(&dir),
                managed: crate::installer::is_managed(&dir.to_string_lossy()),
                path: dir.to_string_lossy().into_owned(),
            });
        }

        versions
    }

    fn active_version(&self) -> Option<String> {
        // `go version` 输出形如：go version go1.23.4 darwin/arm64
        let out = platform::run_capture("go", &["version"])?;
        let token = out.split_whitespace().nth(2)?;
        token.strip_prefix("go").map(|s| s.to_string())
    }
}

/// 读取 GOROOT/VERSION 文件（内容形如 `go1.23.4`），返回 `1.23.4`
fn read_version_file(dir: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(dir.join("VERSION")).ok()?;
    let line = content.trim();
    line.strip_prefix("go")
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

/// 从目录名推断版本：go1.23.4 / go@1.23 / 1.23.4（goenv 布局）
fn infer_from_dir_name(dir: &std::path::Path) -> Option<String> {
    let name = dir.file_name()?.to_string_lossy().into_owned();
    if let Some(rest) = name.strip_prefix("go") {
        // go1.23.4 或 go@1.23
        let rest = rest.strip_prefix('@').unwrap_or(rest);
        if rest.chars().next()?.is_ascii_digit() {
            return Some(rest.to_string());
        }
    }
    crate::runtimes::java::extract_version(&name)
}

#[cfg(target_os = "macos")]
fn candidate_dirs_macos() -> Vec<(PathBuf, &'static str)> {
    let mut dirs = Vec::new();
    // 官方安装
    dirs.push((PathBuf::from("/usr/local/go"), "official"));
    // Homebrew 版本化安装：/opt/homebrew/opt/go@1.23/libexec 或 /usr/local/opt/...
    for opt in ["/opt/homebrew/opt", "/usr/local/opt"] {
        let Ok(entries) = std::fs::read_dir(opt) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == "go" || name.starts_with("go@") {
                // brew go 的实际 GOROOT 在 libexec
                let libexec = entry.path().join("libexec");
                if libexec.is_dir() {
                    dirs.push((libexec, "homebrew"));
                } else {
                    dirs.push((entry.path(), "homebrew"));
                }
            }
        }
    }
    // goenv
    if let Some(home) = platform::home_dir() {
        if let Ok(entries) = std::fs::read_dir(home.join(".goenv/versions")) {
            for entry in entries.flatten() {
                dirs.push((entry.path(), "goenv"));
            }
        }
    }
    // NovaEnv 管理安装（枚举每个已安装版本目录）
    let installs_go = crate::installer::installs_dir().join("go");
    if let Ok(entries) = std::fs::read_dir(&installs_go) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                dirs.push((entry.path(), "novaenv"));
            }
        }
    }
    dirs
}

#[cfg(target_os = "windows")]
fn candidate_dirs_windows() -> Vec<(PathBuf, &'static str)> {
    let mut dirs = Vec::new();
    if let Some(pf) = std::env::var_os("ProgramFiles") {
        dirs.push((PathBuf::from(&pf).join("Go"), "official"));
    }
    if let Some(user) = platform::home_dir() {
        if let Ok(entries) = std::fs::read_dir(user.join(".goenv/versions")) {
            for entry in entries.flatten() {
                dirs.push((entry.path(), "goenv"));
            }
        }
        dirs.push((user.join("go"), "user")); // 用户安装的 go（go.exe 直接位于根目录）
    }
    // NovaEnv 管理安装（枚举每个已安装版本目录）
    let installs_go = crate::installer::installs_dir().join("go");
    if let Ok(entries) = std::fs::read_dir(&installs_go) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                dirs.push((entry.path(), "novaenv"));
            }
        }
    }
    dirs
}
