//! Node.js 适配器。
//!
//! 支持常见安装来源：nvm / fnm / Homebrew / 官方安装（含 Windows 的 nvm-windows）。
//! 版本优先通过运行 `<dir>/bin/node -v` 获取（准确），失败时从目录名推断。

use std::path::PathBuf;

use crate::adapter::RuntimeAdapter;
use crate::models::{RuntimeKind, RuntimeVersion};
use crate::platform;

pub struct NodeAdapter;

impl RuntimeAdapter for NodeAdapter {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Node
    }

    // Linux 平台暂无候选目录扫描（CI 打包用），dirs 在 macOS/Windows 下需要可变
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
            let bin = platform::bin_path(&dir, "node");
            let version = if bin.is_file() {
                bin.to_str()
                    .and_then(|p| platform::run_capture(p, &["-v"]))
                    .and_then(|v| trim_v_prefix(&v))
                    .or_else(|| infer_from_dir_name(&dir))
            } else {
                infer_from_dir_name(&dir)
            };
            if let Some(version) = version {
                versions.push(RuntimeVersion {
                    kind: RuntimeKind::Node,
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
        platform::run_capture("node", &["-v"]).and_then(|v| trim_v_prefix(&v))
    }
}

/// 去掉版本号的 `v` 前缀（v22.11.0 → 22.11.0）
fn trim_v_prefix(v: &str) -> Option<String> {
    let v = v.trim();
    let v = v.strip_prefix('v').unwrap_or(v);
    if v.is_empty() {
        None
    } else {
        Some(v.to_string())
    }
}

/// 从目录名推断版本：v22.11.0 / node@22 / nodejs / fnm 布局的 v22.11.0
fn infer_from_dir_name(dir: &std::path::Path) -> Option<String> {
    let name = dir.file_name()?.to_string_lossy().into_owned();
    // nvm / nvm-windows / fnm 目录名即为 vX.Y.Z
    if let Some(rest) = name.strip_prefix('v') {
        if rest.chars().next()?.is_ascii_digit() {
            return Some(rest.to_string());
        }
    }
    // Homebrew node@22 这类只有主版本号，仍可接受
    if let Some(rest) = name.strip_prefix("node@") {
        if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
            return Some(rest.to_string());
        }
    }
    crate::runtimes::java::extract_version(&name)
}

#[cfg(target_os = "macos")]
fn candidate_dirs_macos() -> Vec<(PathBuf, &'static str)> {
    let mut dirs = Vec::new();
    if let Some(home) = platform::home_dir() {
        // nvm
        if let Ok(entries) = std::fs::read_dir(home.join(".nvm/versions/node")) {
            for entry in entries.flatten() {
                dirs.push((entry.path(), "nvm"));
            }
        }
        // fnm
        let fnm_base = home.join(".local/share/fnm/node-versions");
        if let Ok(entries) = std::fs::read_dir(&fnm_base) {
            for entry in entries.flatten() {
                dirs.push((entry.path().join("installation"), "fnm"));
            }
        }
        // 用户级安装
        dirs.push((home.join(".local/node"), "local"));
    }
    // Homebrew（Apple Silicon 与 Intel 路径）
    for opt in ["/opt/homebrew/opt", "/usr/local/opt"] {
        let Ok(entries) = std::fs::read_dir(opt) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("node") {
                dirs.push((entry.path(), "homebrew"));
            }
        }
    }
    // 官方安装
    dirs.push((PathBuf::from("/usr/local/node"), "official"));
    // NovaEnv 管理安装（枚举每个已安装版本目录，与 java.rs 的 scan_jdk_dirs 同款模式）
    let installs_node = crate::installer::installs_dir().join("node");
    if let Ok(entries) = std::fs::read_dir(&installs_node) {
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
        dirs.push((PathBuf::from(&pf).join("nodejs"), "official"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        // nvm-windows：%APPDATA%\nvm\vX.Y.Z
        if let Ok(entries) = std::fs::read_dir(PathBuf::from(&appdata).join("nvm")) {
            for entry in entries.flatten() {
                dirs.push((entry.path(), "nvm-windows"));
            }
        }
        // fnm-windows：%APPDATA%\fnm\node-versions\vX.Y.Z\installation
        let fnm_base = PathBuf::from(&appdata).join("fnm/node-versions");
        if let Ok(entries) = std::fs::read_dir(&fnm_base) {
            for entry in entries.flatten() {
                dirs.push((entry.path().join("installation"), "fnm"));
            }
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push((PathBuf::from(&local).join("Programs/nodejs"), "official"));
    }
    // NovaEnv 管理安装（枚举每个已安装版本目录）
    let installs_node = crate::installer::installs_dir().join("node");
    if let Ok(entries) = std::fs::read_dir(&installs_node) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                dirs.push((entry.path(), "novaenv"));
            }
        }
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_v_prefix() {
        assert_eq!(trim_v_prefix("v22.11.0"), Some("22.11.0".to_string()));
        assert_eq!(trim_v_prefix("22.11.0"), Some("22.11.0".to_string()));
        assert_eq!(trim_v_prefix(""), None);
    }

    #[test]
    fn infers_from_dir_names() {
        let nvm = std::path::Path::new("/Users/x/.nvm/versions/node/v22.11.0");
        assert_eq!(infer_from_dir_name(nvm), Some("22.11.0".to_string()));
        let brew = std::path::Path::new("/opt/homebrew/opt/node@22");
        assert_eq!(infer_from_dir_name(brew), Some("22".to_string()));
        let plain = std::path::Path::new("/usr/local/node");
        assert!(infer_from_dir_name(plain).is_none());
    }
}
