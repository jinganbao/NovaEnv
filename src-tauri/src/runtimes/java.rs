//! JDK 适配器。
//!
//! macOS：`/usr/libexec/java_home -V` 权威扫描 + 标准目录兜底（含 release 文件解析）。
//! Windows：注册表 `HKLM\SOFTWARE\JavaSoft` + 常见安装目录扫描。

use std::path::{Path, PathBuf};

use crate::adapter::RuntimeAdapter;
use crate::models::{RuntimeKind, RuntimeVersion};
use crate::platform;

pub struct JavaAdapter;

impl RuntimeAdapter for JavaAdapter {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Java
    }

    fn scan(&self) -> Vec<RuntimeVersion> {
        let mut homes: Vec<(String, String, String)> = Vec::new();

        #[cfg(target_os = "macos")]
        {
            homes.extend(scan_macos());
        }
        #[cfg(target_os = "windows")]
        {
            homes.extend(scan_windows());
        }

        // 去重（同一路径可能被多个来源命中）
        homes.sort_by(|a, b| a.0.cmp(&b.0));
        homes.dedup_by(|a, b| a.0 == b.0);

        homes
            .into_iter()
            .map(|(path, version, vendor)| RuntimeVersion {
                kind: RuntimeKind::Java,
                version,
                vendor,
                is_default: crate::platform::is_active_dir(std::path::Path::new(&path)),
                managed: crate::installer::is_managed(&path),
                path,
            })
            .collect()
    }

    fn active_version(&self) -> Option<String> {
        let output = std::process::Command::new("java")
            .arg("-version")
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stderr);
        let first = text.lines().next()?;
        let version = first.split('"').nth(1)?;
        let version = version.trim();
        if version.is_empty() {
            None
        } else {
            Some(version.to_string())
        }
    }
}

// ---------- macOS ----------

#[cfg(target_os = "macos")]
fn scan_macos() -> Vec<(String, String, String)> {
    let mut result = Vec::new();

    // 1) 权威来源：/usr/libexec/java_home -V
    if let Some(listing) = platform::macos::java_home_listing() {
        result.extend(parse_java_home_v(&listing));
    }

    // 2) 兜底：标准目录扫描（release 文件解析）
    //    macOS 的 .jdk 与 Zulu 包均为 Contents/Home 布局，需先检查该层级
    let mut dirs = vec![PathBuf::from("/Library/Java/JavaVirtualMachines")];
    if let Some(home) = platform::home_dir() {
        dirs.push(home.join("Library/Java/JavaVirtualMachines"));
    }
    // NovaEnv 管理安装
    dirs.push(crate::installer::installs_dir().join("java"));
    result.extend(scan_jdk_dirs(&dirs, true));

    result
}

/// 解析 `/usr/libexec/java_home -V` 输出，形如：
/// `17.0.10 (x86_64) "Eclipse Adoptium" - "OpenJDK 17.0.10" [1] /Library/Java/.../Contents/Home`
#[cfg(target_os = "macos")]
fn parse_java_home_v(output: &str) -> Vec<(String, String, String)> {
    let mut result = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("Matching")
            || line.starts_with("No Java")
        {
            continue;
        }
        let path = line.rsplit(' ').next().unwrap_or("");
        let version = line.split_whitespace().next().unwrap_or("");
        if path.starts_with('/') && !version.is_empty() {
            let vendor = extract_quoted(line).unwrap_or_else(|| "Unknown".to_string());
            result.push((path.to_string(), version.to_string(), vendor));
        }
    }
    result
}

#[cfg(target_os = "macos")]
fn extract_quoted(line: &str) -> Option<String> {
    line.split('"').nth(1).map(|s| s.to_string())
}

// ---------- Windows ----------

#[cfg(target_os = "windows")]
fn scan_windows() -> Vec<(String, String, String)> {
    let mut result = platform::windows::java_homes_from_registry();

    // 常见安装目录兜底
    let mut dirs = Vec::new();
    if let Some(pf) = std::env::var_os("ProgramFiles") {
        dirs.push(PathBuf::from(&pf).join("Java"));
        dirs.push(PathBuf::from(&pf).join("Eclipse Adoptium"));
        dirs.push(PathBuf::from(&pf).join("Microsoft"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(PathBuf::from(&local).join("Programs"));
    }
    // NovaEnv 管理安装
    dirs.push(crate::installer::installs_dir().join("java"));
    result.extend(scan_jdk_dirs(&dirs, true));

    result
}

// ---------- 公共扫描逻辑 ----------

/// 扫描目录下的 JDK 安装。`home_inside` 表示是否需进入 `Contents/Home`（macOS .jdk 布局）。
fn scan_jdk_dirs(dirs: &[PathBuf], home_inside: bool) -> Vec<(String, String, String)> {
    let mut result = Vec::new();
    for dir in dirs {
        if !platform::is_dir(dir) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let jdk = entry.path();
            if !jdk.is_dir() {
                continue;
            }
            if let Some((path, version, vendor)) =
                resolve_jdk_home(&jdk, home_inside)
            {
                result.push((path, version, vendor));
            }
        }
    }
    result
}

/// 解析单个 JDK 目录：优先 release 文件，其次目录名兜底。
fn resolve_jdk_home(jdk: &Path, home_inside: bool) -> Option<(String, String, String)> {
    if home_inside {
        let home = jdk.join("Contents").join("Home");
        if let Some((version, vendor)) = parse_release(&home) {
            return Some((home.to_string_lossy().into_owned(), version, vendor));
        }
    }
    if let Some((version, vendor)) = parse_release(jdk) {
        return Some((jdk.to_string_lossy().into_owned(), version, vendor));
    }
    let name = jdk
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if let Some(version) = extract_version(&name) {
        return Some((jdk.to_string_lossy().into_owned(), version, "Unknown".into()));
    }
    None
}

/// 解析 JDK `release` 文件中的 JAVA_VERSION / JAVA_VENDOR。
pub(crate) fn parse_release(home: &Path) -> Option<(String, String)> {
    let content = std::fs::read_to_string(home.join("release")).ok()?;
    let mut version = None;
    let mut vendor = None;
    for line in content.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"');
        match key.trim() {
            "JAVA_VERSION" => version = Some(value.to_string()),
            "JAVA_VENDOR" => vendor = Some(value.to_string()),
            _ => {}
        }
    }
    Some((version?, vendor.unwrap_or_else(|| "Unknown".to_string())))
}

/// 从字符串中提取形如 `\d+(\.\d+)+` 的版本号（目录名兜底用）。
pub(crate) fn extract_version(s: &str) -> Option<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut best: Option<String> = None;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == '.') {
            j += 1;
        }
        let cand = chars[i..j].iter().collect::<String>();
        let cand = cand.trim_end_matches('.').to_string();
        if cand.contains('.')
            && best
                .as_ref()
                .map_or(true, |b: &String| cand.len() > b.len())
        {
            best = Some(cand);
        }
        i = j;
    }
    best
}
