//! Maven 适配器。
//!
//! 支持常见安装来源：Homebrew（/opt/homebrew/opt/maven）、
//! 官方手动安装（/opt/local/apache-maven-*）、NovaEnv 管理安装。
//! 版本优先通过运行 `bin/mvn -v` 获取（准确），失败时从目录名推断。

use std::path::PathBuf;

use crate::adapter::RuntimeAdapter;
use crate::models::{RuntimeKind, RuntimeVersion};
use crate::platform;

pub struct MavenAdapter;

impl RuntimeAdapter for MavenAdapter {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Maven
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
            let bin = mvn_bin(&dir);
            let version = if bin.is_file() {
                bin.to_str()
                    .and_then(|p| platform::run_capture(p, &["-v"]))
                    .and_then(|v| parse_mvn_version(&v))
                    .or_else(|| infer_from_dir_name(&dir))
            } else {
                infer_from_dir_name(&dir)
            };
            if let Some(version) = version {
                versions.push(RuntimeVersion {
                    kind: RuntimeKind::Maven,
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
        platform::run_capture("mvn", &["-v"]).and_then(|v| parse_mvn_version(&v))
    }
}

/// mvn 可执行文件路径（Windows 为 mvn.cmd）
fn mvn_bin(dir: &std::path::Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        dir.join("bin").join("mvn.cmd")
    }
    #[cfg(not(target_os = "windows"))]
    {
        dir.join("bin").join("mvn")
    }
}

/// 解析 `mvn -v` 输出首行：`Apache Maven 3.9.16 (…)`
fn parse_mvn_version(v: &str) -> Option<String> {
    let first = v.lines().next()?;
    let token = first.split_whitespace().nth(2)?;
    let token = token.trim();
    if token.chars().next()?.is_ascii_digit() {
        Some(token.to_string())
    } else {
        None
    }
}

/// 从目录名推断版本：apache-maven-3.9.16 / maven
fn infer_from_dir_name(dir: &std::path::Path) -> Option<String> {
    let name = dir.file_name()?.to_string_lossy().into_owned();
    crate::runtimes::java::extract_version(&name)
}

#[cfg(target_os = "macos")]
fn candidate_dirs_macos() -> Vec<(PathBuf, &'static str)> {
    let mut dirs = Vec::new();
    // Homebrew（Apple Silicon 与 Intel 路径）
    for opt in ["/opt/homebrew/opt/maven", "/usr/local/opt/maven"] {
        if std::path::Path::new(opt).is_dir() {
            dirs.push((PathBuf::from(opt), "homebrew"));
        }
    }
    // 官方手动安装（/opt/local/apache-maven-*）
    if let Ok(entries) = std::fs::read_dir("/opt/local") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("apache-maven") {
                dirs.push((entry.path(), "official"));
            }
        }
    }
    // NovaEnv 管理安装（枚举每个已安装版本目录）
    let installs_maven = crate::installer::installs_dir().join("maven");
    if let Ok(entries) = std::fs::read_dir(&installs_maven) {
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
        dirs.push((PathBuf::from(&pf).join("apache-maven"), "official"));
    }
    // Chocolatey：C:\ProgramData\chocolatey\lib\maven\apache-maven-*
    if let Some(pd) = std::env::var_os("ProgramData") {
        let choco = PathBuf::from(&pd).join("chocolatey/lib/maven");
        if let Ok(entries) = std::fs::read_dir(&choco) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    dirs.push((entry.path(), "chocolatey"));
                }
            }
        }
    }
    // NovaEnv 管理安装（枚举每个已安装版本目录）
    let installs_maven = crate::installer::installs_dir().join("maven");
    if let Ok(entries) = std::fs::read_dir(&installs_maven) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                dirs.push((entry.path(), "novaenv"));
            }
        }
    }
    dirs
}
