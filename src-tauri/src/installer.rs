//! 安装 / 卸载引擎。
//!
//! 安装目录：`~/.novaenv/installs/<kind>/<version>/`（应用自行管理，卸载安全）。
//! 下载源：
//! - Java：Adoptium Temurin（`api.adoptium.net`）
//! - Node：官方 dist（`nodejs.org/dist`）
//! - Go：官方 dl（`go.dev/dl`）
//!
//! 安装流程：下载（流式 + 进度事件）→ 解压到临时目录 → 定位实际内容
//! → 移动到安装目录 → 清理。进度通过 tauri 事件 `install-progress` 推送。

use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};

use crate::models::{
    AvailableVersion, AvailableVersionGroup, InstallProgress, InstallRequest, RuntimeKind,
    RuntimeVersion,
};

/// 安装互斥锁：同一时间只允许一个安装任务
static INSTALL_LOCK: Mutex<()> = Mutex::new(());

/// NovaEnv 管理根目录：~/.novaenv
pub fn novaenv_dir() -> PathBuf {
    crate::platform::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".novaenv")
}

/// 管理安装目录：~/.novaenv/installs
pub fn installs_dir() -> PathBuf {
    novaenv_dir().join("installs")
}

/// 判断路径是否为 NovaEnv 管理的安装（可卸载）
pub fn is_managed(path: &str) -> bool {
    Path::new(path).starts_with(installs_dir())
}

/// 各运行时在安装目录下的子目录名
fn kind_dir(kind: RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Java => "java",
        RuntimeKind::Node => "node",
        RuntimeKind::Go => "go",
        RuntimeKind::Maven => "maven",
        RuntimeKind::Python => "python",
        RuntimeKind::Rust => "rust",
    }
}

// ---------- 下载 URL 构造 ----------

/// Azul Zulu 平台参数：(os, arch, archive_type)
/// 注意 Zulu 的 arch 命名是 x86_64（非 x64），archive_type 为 tar.gz/zip
fn zulu_platform() -> (&'static str, &'static str, &'static str) {
    #[cfg(target_os = "macos")]
    let os = "macos";
    #[cfg(target_os = "windows")]
    let os = "windows";
    #[cfg(target_os = "linux")]
    let os = "linux";
    #[cfg(target_arch = "aarch64")]
    let arch = "aarch64";
    #[cfg(not(target_arch = "aarch64"))]
    let arch = "x86_64";
    let archive_type = if cfg!(target_os = "windows") {
        "zip"
    } else {
        "tar.gz"
    };
    (os, arch, archive_type)
}

/// 请求 Azul Zulu packages API（可按 java_version 过滤）
fn zulu_packages(java_version: Option<&str>) -> Result<Vec<serde_json::Value>, String> {
    let (os, arch, archive_type) = zulu_platform();
    let mut url = format!(
        "https://api.azul.com/metadata/v1/zulu/packages/?os={os}&arch={arch}&archive_type={archive_type}&java_package_type=jdk&release_status=ga&page_size=10"
    );
    if let Some(v) = java_version {
        url.push_str(&format!("&java_version={v}"));
    } else {
        url.push_str("&latest=true");
    }
    let json = http_get_json(&url)?;
    let arr = json.as_array().ok_or("Zulu 响应格式异常")?.clone();
    Ok(arr)
}

/// 从 Zulu 包列表中挑选标准 JDK 包（排除 fx / crac 变体）
fn zulu_pick(packages: &[serde_json::Value]) -> Option<&serde_json::Value> {
    zulu_pick_all(packages).into_iter().next()
}

/// 收集所有标准 JDK 包（排除 fx / crac 变体）
fn zulu_pick_all(packages: &[serde_json::Value]) -> Vec<&serde_json::Value> {
    packages
        .iter()
        .filter(|p| {
            p.get("name")
                .and_then(|n| n.as_str())
                .map(|n| !n.contains("fx") && !n.contains("crac"))
                .unwrap_or(false)
        })
        .collect()
}

/// 解析 Zulu java_version 数组（[17, 0, 20] → "17.0.20"）
fn zulu_java_version(pkg: &serde_json::Value) -> Option<String> {
    let arr = pkg.get("java_version")?.as_array()?;
    let parts: Vec<String> = arr
        .iter()
        .filter_map(|n| n.as_i64().map(|i| i.to_string()))
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("."))
    }
}

/// 构造下载 URL 与本地归档文件路径
fn download_url(kind: RuntimeKind, version: &str) -> Result<(String, PathBuf), String> {
    let downloads = installs_dir().join("_downloads");
    std::fs::create_dir_all(&downloads).map_err(|e| format!("创建下载目录失败: {e}"))?;

    match kind {
        RuntimeKind::Java => {
            // Azul Zulu：请求 packages API 获取标准 JDK 包（排除 fx/crac）的下载地址
            let feature = version.split('.').next().unwrap_or(version);
            let packages = zulu_packages(Some(feature))?;
            let pkg = zulu_pick(&packages).ok_or("Zulu 未返回可用 JDK 包")?;
            let url = pkg
                .get("download_url")
                .and_then(|v| v.as_str())
                .ok_or("Zulu 下载地址缺失")?
                .to_string();
            let file_name = url
                .rsplit('/')
                .next()
                .unwrap_or("zulu-jdk.tar.gz")
                .to_string();
            Ok((url, downloads.join(file_name)))
        }
        RuntimeKind::Node => {
            let (os_name, arch_name) = if cfg!(target_os = "macos") {
                (
                    "darwin",
                    if cfg!(target_arch = "aarch64") {
                        "arm64"
                    } else {
                        "x64"
                    },
                )
            } else if cfg!(target_os = "windows") {
                ("win", "x64")
            } else {
                (
                    "linux",
                    if cfg!(target_arch = "aarch64") {
                        "arm64"
                    } else {
                        "x64"
                    },
                )
            };
            let ext = if cfg!(target_os = "windows") {
                "zip"
            } else {
                "tar.gz"
            };
            let url = format!(
                "https://nodejs.org/dist/v{version}/node-v{version}-{os_name}-{arch_name}.{ext}"
            );
            Ok((url, downloads.join(format!("node-v{version}.{ext}"))))
        }
        RuntimeKind::Go => {
            let (os_name, arch_name) = if cfg!(target_os = "macos") {
                (
                    "darwin",
                    if cfg!(target_arch = "aarch64") {
                        "arm64"
                    } else {
                        "amd64"
                    },
                )
            } else if cfg!(target_os = "windows") {
                ("windows", "amd64")
            } else {
                (
                    "linux",
                    if cfg!(target_arch = "aarch64") {
                        "arm64"
                    } else {
                        "amd64"
                    },
                )
            };
            let ext = if cfg!(target_os = "windows") {
                "zip"
            } else {
                "tar.gz"
            };
            let url = format!("https://go.dev/dl/go{version}.{os_name}-{arch_name}.{ext}");
            Ok((url, downloads.join(format!("go{version}.{ext}"))))
        }
        RuntimeKind::Maven => {
            // Apache Maven（纯 Java 工具，无平台/架构区分）。
            // 官方源（dlcdn/archive）到国内网络极慢，优先国内镜像，全挂后回退官方。
            let ext = if cfg!(target_os = "windows") {
                "zip"
            } else {
                "tar.gz"
            };
            let file = format!("apache-maven-{version}-bin.{ext}");
            let candidates = [
                format!("https://mirrors.tuna.tsinghua.edu.cn/apache/maven/maven-3/{version}/binaries/{file}"),
                format!("https://mirrors.aliyun.com/apache/maven/maven-3/{version}/binaries/{file}"),
                format!("https://mirrors.huaweicloud.com/apache/maven/maven-3/{version}/binaries/{file}"),
                format!("https://dlcdn.apache.org/maven/maven-3/{version}/binaries/{file}"),
                format!("https://archive.apache.org/dist/maven/maven-3/{version}/binaries/{file}"),
            ];
            let url = candidates
                .into_iter()
                .find(|u| url_exists(u))
                .ok_or("Maven 各下载源均不可用，请稍后重试")?;
            Ok((url, downloads.join(file)))
        }
        RuntimeKind::Python => {
            // python-build-standalone（GitHub Releases，预编译 install_only 包）
            let url = python_download_url(version)?;
            let file = format!("cpython-{version}.tar.gz");
            Ok((url, downloads.join(file)))
        }
        RuntimeKind::Rust => {
            // Rust 官方 dist（static.rust-lang.org，预编译 toolchain 包）
            let target = if cfg!(target_os = "macos") {
                if cfg!(target_arch = "aarch64") {
                    "aarch64-apple-darwin"
                } else {
                    "x86_64-apple-darwin"
                }
            } else if cfg!(target_os = "windows") {
                "x86_64-pc-windows-msvc"
            } else {
                return Err("当前平台暂不支持 Rust 安装".to_string());
            };
            let url = format!(
                "https://static.rust-lang.org/dist/rust-{version}-{target}.tar.gz"
            );
            Ok((url, downloads.join(format!("rust-{version}-{target}.tar.gz"))))
        }
    }
}

/// Python 下载地址解析：请求 python-build-standalone 最新 release，
/// 匹配 `cpython-<version>+<tag>-<platform>-install_only.tar.gz` 资产。
fn python_download_url(version: &str) -> Result<String, String> {
    let json = http_get_json("https://api.github.com/repos/astral-sh/python-build-standalone/releases?per_page=1")?;
    let Some(release) = json.as_array().and_then(|a| a.first()) else {
        return Err("python-build-standalone 响应格式异常".to_string());
    };
    let Some(tag) = release.get("tag_name").and_then(|v| v.as_str()) else {
        return Err("python-build-standalone 响应缺少 tag".to_string());
    };
    let Some(assets) = release.get("assets").and_then(|v| v.as_array()) else {
        return Err("python-build-standalone 响应缺少资产".to_string());
    };
    // 平台标识（与上游命名一致）
    let platform = if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "aarch64-apple-darwin"
        } else {
            "x86_64-apple-darwin"
        }
    } else if cfg!(target_os = "windows") {
        "x86_64-pc-windows-msvc"
    } else {
        return Err("当前平台暂不支持 Python 安装".to_string());
    };
    let prefix = format!("cpython-{version}+{tag}-{platform}-install_only.tar.gz");
    for asset in assets {
        if let Some(name) = asset.get("name").and_then(|v| v.as_str()) {
            if name == prefix {
                if let Some(url) = asset.get("browser_download_url").and_then(|v| v.as_str()) {
                    return Ok(url.to_string());
                }
            }
        }
    }
    Err(format!(
        "未找到 Python {version} 的 {platform} 官方包（可能尚未发布）"
    ))
}

/// HEAD 探测 URL 是否可用（镜像源选择用；跟随重定向，最终 200 视为可用）
pub(crate) fn url_exists(url: &str) -> bool {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(20))
        .build();
    agent
        .head(url)
        .call()
        .map(|r| r.status() == 200)
        .unwrap_or(false)
}

// ---------- 下载 ----------

/// 流式下载并推送进度事件（百分比变化时触发）。
/// 注意：ureq 的 Request::timeout 是总超时，大文件下载不能用；
/// 这里仅限制连接超时，读取不设上限。
pub(crate) fn download(
    app: &AppHandle,
    kind: RuntimeKind,
    version: &str,
    url: &str,
    dest: &Path,
) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .build();
    let resp = agent
        .get(url)
        .call()
        .map_err(|e| format!("下载失败（{url}）: {e}"))?;
    let total = resp
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok());

    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(dest).map_err(|e| format!("创建文件失败: {e}"))?;

    let mut buf = [0u8; 128 * 1024];
    let mut downloaded: u64 = 0;
    let mut last_pct: u32 = 0;
    let mut last_emitted: u64 = 0;
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("下载中断: {e}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| format!("写入文件失败: {e}"))?;
        downloaded += n as u64;
        if let Some(total) = total {
            let pct = ((downloaded as f64 / total as f64) * 100.0).min(100.0) as u32;
            if pct != last_pct {
                last_pct = pct;
                emit_progress(
                    app,
                    kind,
                    version,
                    "downloading",
                    Some(pct),
                    format!(
                        "下载中 {:.1} / {:.1} MB",
                        downloaded as f64 / 1024.0 / 1024.0,
                        total as f64 / 1024.0 / 1024.0
                    ),
                );
            }
        } else if downloaded - last_emitted >= 2 * 1024 * 1024 {
            // 无 Content-Length 时每 2MB 上报一次已下载量（前端显示不定进度）
            last_emitted = downloaded;
            emit_progress(
                app,
                kind,
                version,
                "downloading",
                None,
                format!("已下载 {:.1} MB", downloaded as f64 / 1024.0 / 1024.0),
            );
        }
    }
    Ok(())
}

// ---------- 解压 ----------

pub(crate) fn extract(archive: &Path, target: &Path) -> Result<(), String> {
    std::fs::create_dir_all(target).map_err(|e| format!("创建解压目录失败: {e}"))?;
    let ext = archive
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();

    if ext == "gz" || archive.to_string_lossy().ends_with(".tar.gz") {
        let file = std::fs::File::open(archive).map_err(|e| format!("打开归档失败: {e}"))?;
        let gz = flate2::read::GzDecoder::new(file);
        let mut tar = tar::Archive::new(gz);
        tar.unpack(target).map_err(|e| format!("解压失败: {e}"))?;
    } else if ext == "zip" {
        let file = std::fs::File::open(archive).map_err(|e| format!("打开归档失败: {e}"))?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("读取 zip 失败: {e}"))?;
        zip.extract(target).map_err(|e| format!("解压失败: {e}"))?;
    } else {
        return Err(format!("不支持的归档格式: {ext}"));
    }
    Ok(())
}

/// 解压目录中的第一个子目录（Java/Node 归档的顶层结构）
fn first_subdir(dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            return Some(path);
        }
    }
    None
}

/// 定位解压后的实际内容目录与最终版本号
fn locate_extracted(
    kind: RuntimeKind,
    requested: &str,
    tmp: &Path,
) -> Result<(String, PathBuf), String> {
    match kind {
        RuntimeKind::Java => {
            let sub = first_subdir(tmp).ok_or("解压结果为空，请重试")?;
            let name = sub
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let version = crate::runtimes::java::parse_release(&sub)
                .map(|(v, _)| v)
                .or_else(|| crate::runtimes::java::extract_version(&name))
                .ok_or("无法识别 JDK 版本，请检查下载完整性")?;
            Ok((version, sub))
        }
        RuntimeKind::Node => {
            let sub = first_subdir(tmp).ok_or("解压结果为空，请重试")?;
            Ok((requested.to_string(), sub))
        }
        RuntimeKind::Go => {
            let go_dir = tmp.join("go");
            if !go_dir.is_dir() {
                return Err("Go 归档结构异常，请重试".to_string());
            }
            Ok((requested.to_string(), go_dir))
        }
        RuntimeKind::Maven => {
            // apache-maven-3.9.16/ 顶层目录（与 Node 同构）
            let sub = first_subdir(tmp).ok_or("解压结果为空，请重试")?;
            Ok((requested.to_string(), sub))
        }
        RuntimeKind::Python => {
            // install_only 包顶层为 python/ 目录（与 Go 同构）
            let python_dir = tmp.join("python");
            if !python_dir.is_dir() {
                return Err("Python 包结构异常，请重试".to_string());
            }
            Ok((requested.to_string(), python_dir))
        }
        RuntimeKind::Rust => {
            // 官方包解压后为组件分目录（rustc/ cargo/ rust-std-*/），
            // 先尝试 install.sh --prefix=DIR 组装成 bin/ + lib/ 标准布局，
            // 失败则手动合并组件目录（兜底）
            let root = first_subdir(tmp).ok_or("解压结果为空，请重试")?;
            let dest = tmp.join("_assembled");
            assemble_rust(&root, &dest)?;
            // 组装后 bin/ 下应有 rustc 与 cargo
            if !dest.join("bin").join("rustc").is_file() || !dest.join("bin").join("cargo").is_file()
            {
                return Err("Rust 组装结果异常（缺少 rustc/cargo），请重试".to_string());
            }
            Ok((requested.to_string(), dest))
        }
    }
}

/// 组装 Rust 官方包组件到目标目录。
/// 优先运行 install.sh（注意参数须为 `--prefix=DIR` 等号形式）；
/// 失败时手动合并 rustc/ cargo/ rust-std-*/ 组件（保留可执行权限）。
fn assemble_rust(root: &std::path::Path, dest: &std::path::Path) -> Result<(), String> {
    let install_sh = root.join("install.sh");
    if install_sh.is_file() {
        let ok = std::process::Command::new(&install_sh)
            .arg(format!("--prefix={}", dest.display()))
            .arg("--disable-ldconfig")
            .current_dir(root)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return Ok(());
        }
    }
    // 兜底：手动合并组件
    std::fs::create_dir_all(dest).map_err(|e| format!("创建组装目录失败: {e}"))?;
    let mut merged = false;
    for entry in std::fs::read_dir(root).map_err(|e| format!("读取包目录失败: {e}"))? {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "rustc" || name == "cargo" || name.starts_with("rust-std-") {
            copy_tree(&entry.path(), dest)?;
            merged = true;
        }
    }
    if !merged {
        return Err("Rust 包内未找到可组装组件".to_string());
    }
    Ok(())
}

/// 递归复制目录树（保留文件权限）
fn copy_tree(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    if src.is_dir() {
        std::fs::create_dir_all(dst).map_err(|e| format!("创建目录失败 {dst:?}: {e}"))?;
        for entry in std::fs::read_dir(src).map_err(|e| format!("读取目录失败 {src:?}: {e}"))? {
            let entry = entry.map_err(|e| e.to_string())?;
            copy_tree(&entry.path(), &dst.join(entry.file_name()))?;
        }
    } else {
        std::fs::copy(src, dst).map_err(|e| format!("复制失败 {src:?}: {e}"))?;
    }
    Ok(())
}

// ---------- 安装 / 卸载主流程 ----------

/// 安装指定版本的运行时。
/// 同一大版本只保留最新小版本：安装成功后自动移除同大版本的旧版本；
/// 若被移除的旧版本是默认版本，则自动把新版本设为默认。
pub fn install(
    app: &AppHandle,
    request: &InstallRequest,
) -> Result<crate::models::InstallResult, String> {
    let _guard = INSTALL_LOCK
        .try_lock()
        .map_err(|_| "已有安装任务正在进行中，请稍候".to_string())?;

    let kind = request.kind;
    let version = request.version.trim().to_string();
    if version.is_empty() {
        return Err("版本号不能为空".to_string());
    }

    let (url, archive) = download_url(kind, &version)?;

    emit_progress(app, kind, &version, "downloading", Some(0), "开始下载");

    // 1) 下载（失败自动重试 2 次，应对网络中断导致的文件损坏）
    let mut last_err = String::new();
    let mut downloaded = false;
    for attempt in 0..3 {
        match download(app, kind, &version, &url, &archive) {
            Ok(()) => {
                downloaded = true;
                break;
            }
            Err(e) => {
                last_err = e;
                if attempt < 2 {
                    emit_progress(
                        app,
                        kind,
                        &version,
                        "downloading",
                        None,
                        format!("下载中断，重试第 {} 次…", attempt + 1),
                    );
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
        }
    }
    if !downloaded {
        let _ = std::fs::remove_file(&archive);
        return Err(format!("下载失败: {last_err}"));
    }

    // 2) 解压到临时目录
    let tmp = installs_dir()
        .join("_downloads")
        .join(format!("extract-{}", kind_dir(kind)));
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp).map_err(|e| format!("清理临时目录失败: {e}"))?;
    }
    emit_progress(app, kind, &version, "extracting", None, "解压中…");
    extract(&archive, &tmp).map_err(|e| {
        // 解压失败多为下载文件损坏/磁盘空间不足
        format!("{e}（可能是下载不完整或磁盘空间不足，请重试）")
    })?;

    // 3) 定位实际内容与最终版本
    let (final_version, src) = locate_extracted(kind, &version, &tmp)?;

    // 4) 移动到安装目录
    let dest = installs_dir().join(kind_dir(kind)).join(&final_version);
    if dest.exists() {
        let _ = std::fs::remove_dir_all(&tmp);
        let _ = std::fs::remove_file(&archive);
        return Err(format!("版本 {final_version} 已安装"));
    }
    std::fs::create_dir_all(dest.parent().ok_or("安装目录无效")?)
        .map_err(|e| format!("创建安装目录失败: {e}"))?;
    emit_progress(
        app,
        kind,
        &final_version,
        "installing",
        None,
        "写入安装目录…",
    );
    std::fs::rename(&src, &dest).map_err(|e| format!("移动到安装目录失败: {e}"))?;

    // 5) 清理
    let _ = std::fs::remove_dir_all(&tmp);
    let _ = std::fs::remove_file(&archive);

    emit_progress(app, kind, &final_version, "done", Some(100), "安装完成");

    // 6) 同大版本只保留最新：自动替换旧小版本
    Ok(cleanup_same_major(kind, &final_version, &dest))
}

/// 同大版本只保留最新小版本：移除同大版本的旧版本；
/// 若被移除的旧版本是默认版本，自动把新版本设为默认。
fn cleanup_same_major(
    kind: RuntimeKind,
    new_version: &str,
    new_path: &std::path::Path,
) -> crate::models::InstallResult {
    use crate::models::InstallResult;

    let mut result = InstallResult {
        removed: Vec::new(),
        promoted: false,
    };

    // 安装前的版本快照（用于判断旧版本是否默认）
    let before = crate::runtimes::scan_all()
        .ok()
        .into_iter()
        .flat_map(|p| p.versions)
        .filter(|v| v.kind == kind && v.version != new_version)
        .collect::<Vec<_>>();
    let old_default = before
        .iter()
        .find(|v| v.is_default && major_of(kind, &v.version) == major_of(kind, new_version));

    // 枚举同大版本的其他版本目录并移除
    let dir = installs_dir().join(kind_dir(kind));
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                continue;
            };
            if name == new_version {
                continue;
            }
            if major_of(kind, &name) == major_of(kind, new_version)
                && std::fs::remove_dir_all(&path).is_ok()
            {
                result.removed.push(name);
            }
        }
    }

    // 旧版本曾是默认 → 新版本自动接管默认
    if old_default.is_some() && !result.removed.is_empty() {
        let version = crate::models::RuntimeVersion {
            kind,
            version: new_version.to_string(),
            vendor: "novaenv".to_string(),
            path: new_path.to_string_lossy().into_owned(),
            is_default: false,
            managed: true,
        };
        if crate::activation::activate(&version).is_ok() {
            result.promoted = true;
        }
    }

    result
}

/// 卸载 NovaEnv 管理的版本
pub fn uninstall(version: &RuntimeVersion) -> Result<(), String> {
    let path = Path::new(&version.path);
    if !is_managed(&version.path) {
        return Err("仅支持卸载 NovaEnv 安装的版本（~/.novaenv/installs 下）".to_string());
    }
    if version.is_default {
        return Err("该版本为当前默认版本，请先切换默认后再卸载".to_string());
    }
    if !path.exists() {
        return Err("安装目录不存在".to_string());
    }
    std::fs::remove_dir_all(path).map_err(|e| format!("卸载失败: {e}"))?;
    Ok(())
}

// ---------- 管理目录信息 ----------

/// 收集管理目录信息：路径 / 已管理版本数 / 占用空间
pub fn manage_info() -> crate::models::ManageInfo {
    use crate::models::{ManageInfo, ManagedRuntimeInfo};

    let installs = installs_dir();
    let mut runtimes = Vec::new();
    let mut version_count = 0usize;
    let mut size_bytes = 0u64;

    for (kind, dir_name) in [
        (RuntimeKind::Java, "java"),
        (RuntimeKind::Node, "node"),
        (RuntimeKind::Go, "go"),
        (RuntimeKind::Maven, "maven"),
        (RuntimeKind::Python, "python"),
        (RuntimeKind::Rust, "rust"),
    ] {
        let dir = installs.join(dir_name);
        let mut versions = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        versions.push(name.to_string());
                        size_bytes += dir_size(&entry.path());
                    }
                }
            }
        }
        versions.sort();
        version_count += versions.len();
        runtimes.push(ManagedRuntimeInfo { kind, versions });
    }

    ManageInfo {
        path: installs.to_string_lossy().into_owned(),
        version_count,
        size_bytes,
        runtimes,
    }
}

/// 递归计算目录占用字节数
fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += dir_size(&p);
            } else if let Ok(meta) = p.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

// ---------- 可用版本列表 ----------

/// 可用版本缓存条目
struct CacheEntry {
    fetched_at: Instant,
    groups: Vec<AvailableVersionGroup>,
}

/// 可用版本内存缓存（按运行时类型，TTL 5 分钟；切换环境/刷新界面不再重复请求网络）
static VERSIONS_CACHE: Mutex<Option<HashMap<RuntimeKind, CacheEntry>>> = Mutex::new(None);
const CACHE_TTL: Duration = Duration::from_secs(300);

/// 磁盘缓存有效期（版本列表更新不频繁，半天内直接复用；「刷新列表」可强制更新）
const DISK_CACHE_TTL: Duration = Duration::from_secs(6 * 3600);

/// 获取某运行时官方源的可安装版本列表（按大版本分组）。
/// 命中顺序：内存缓存 → 磁盘缓存 → 网络拉取。
/// `refresh = true` 时绕过缓存强制重新拉取。
pub fn available_versions(
    kind: RuntimeKind,
    refresh: bool,
) -> Result<Vec<AvailableVersionGroup>, String> {
    // 1) 内存缓存
    if !refresh {
        if let Some(entry) = VERSIONS_CACHE
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|map| map.get(&kind))
        {
            if entry.fetched_at.elapsed() < CACHE_TTL {
                return Ok(entry.groups.clone());
            }
        }
        // 2) 磁盘缓存（内存没有或过期时）
        if let Some(groups) = load_disk_cache(kind) {
            store_memory_cache(kind, &groups);
            return Ok(groups);
        }
    }

    // 3) 网络拉取
    let flat = match kind {
        RuntimeKind::Java => available_java()?,
        RuntimeKind::Node => available_node()?,
        RuntimeKind::Go => available_go()?,
        RuntimeKind::Maven => available_maven()?,
        RuntimeKind::Python => available_python()?,
        RuntimeKind::Rust => available_rust()?,
    };
    let groups = group_versions(kind, flat);

    store_memory_cache(kind, &groups);
    save_disk_cache(kind, &groups);
    Ok(groups)
}

fn store_memory_cache(kind: RuntimeKind, groups: &[AvailableVersionGroup]) {
    VERSIONS_CACHE
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(
            kind,
            CacheEntry {
                fetched_at: Instant::now(),
                groups: groups.to_vec(),
            },
        );
}

/// 磁盘缓存路径：~/.novaenv/cache/versions-<kind>.json
fn cache_path(kind: RuntimeKind) -> PathBuf {
    novaenv_dir()
        .join("cache")
        .join(format!("versions-{}.json", kind_name(kind)))
}

fn kind_name(kind: RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Java => "java",
        RuntimeKind::Node => "node",
        RuntimeKind::Go => "go",
        RuntimeKind::Maven => "maven",
        RuntimeKind::Python => "python",
        RuntimeKind::Rust => "rust",
    }
}

/// 读取磁盘缓存（过期或损坏视为无缓存）
fn load_disk_cache(kind: RuntimeKind) -> Option<Vec<AvailableVersionGroup>> {
    load_disk_cache_at(&cache_path(kind))
}

/// 从指定路径读取磁盘缓存（纯函数，便于测试）
fn load_disk_cache_at(path: &std::path::Path) -> Option<Vec<AvailableVersionGroup>> {
    let content = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let fetched_at = json.get("fetched_at")?.as_u64()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    if now.saturating_sub(fetched_at) > DISK_CACHE_TTL.as_secs() {
        return None;
    }
    serde_json::from_value(json.get("groups")?.clone()).ok()
}

/// 写磁盘缓存（失败静默，不影响主流程）
fn save_disk_cache(kind: RuntimeKind, groups: &[AvailableVersionGroup]) {
    save_disk_cache_at(&cache_path(kind), groups);
}

/// 写磁盘缓存到指定路径（纯函数，便于测试）
fn save_disk_cache_at(path: &std::path::Path, groups: &[AvailableVersionGroup]) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let json = serde_json::json!({
        "fetched_at": now,
        "groups": groups,
    });
    let dir = path.parent().map(|p| p.to_path_buf());
    if let Some(dir) = dir {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(path, serde_json::to_vec(&json).unwrap_or_default());
    }
}

/// 扁平版本列表按大版本分组（Go 的大版本为前两段，如 1.24；其余为第一段）
fn group_versions(
    kind: RuntimeKind,
    versions: Vec<AvailableVersion>,
) -> Vec<AvailableVersionGroup> {
    let mut groups: Vec<AvailableVersionGroup> = Vec::new();
    for v in versions {
        let major = major_of(kind, &v.version);
        if let Some(g) = groups.iter_mut().find(|g| g.major == major) {
            if !g.versions.contains(&v.version) {
                g.versions.push(v.version.clone());
            }
            if v.is_lts {
                g.is_lts = true;
            }
        } else {
            groups.push(AvailableVersionGroup {
                major: major.clone(),
                is_lts: v.is_lts,
                versions: vec![v.version.clone()],
                latest: String::new(),
            });
        }
    }
    // 组内小版本倒序（最新在前）+ 大版本倒序（最新在前）
    for g in &mut groups {
        g.versions.sort_by(|a, b| compare_versions(b, a));
        g.latest = g.versions.first().cloned().unwrap_or_default();
    }
    groups.sort_by(|a, b| compare_versions(&b.major, &a.major));
    groups
}

/// 数字段逐位比较版本号（21.0.12 > 21.0.9；1.24 > 1.23）
fn compare_versions(a: &str, b: &str) -> Ordering {
    let nums = |s: &str| -> Vec<u64> {
        s.split(|c: char| !c.is_ascii_digit())
            .filter_map(|p| p.parse().ok())
            .collect()
    };
    let na = nums(a);
    let nb = nums(b);
    let len = na.len().max(nb.len());
    for i in 0..len {
        let x = na.get(i).copied().unwrap_or(0);
        let y = nb.get(i).copied().unwrap_or(0);
        if x != y {
            return x.cmp(&y);
        }
    }
    Ordering::Equal
}

/// 计算版本的大版本标识
fn major_of(kind: RuntimeKind, version: &str) -> String {
    match kind {
        RuntimeKind::Go => version.split('.').take(2).collect::<Vec<_>>().join("."),
        _ => version.split('.').next().unwrap_or(version).to_string(),
    }
}

fn http_get_json(url: &str) -> Result<serde_json::Value, String> {
    let body = ureq::get(url)
        .timeout(Duration::from_secs(12))
        .call()
        .map_err(|e| format!("请求版本源失败（{url}）: {e}"))?
        .into_string()
        .map_err(|e| format!("读取版本源响应失败: {e}"))?;
    serde_json::from_str(&body).map_err(|e| format!("解析版本源失败: {e}"))
}

/// GET 文本内容（用于解析 HTML 目录页，如 Apache Maven 版本列表）
pub(crate) fn http_get_text(url: &str) -> Result<String, String> {
    ureq::get(url)
        .set("User-Agent", "NovaEnv/1.0")
        .timeout(Duration::from_secs(20))
        .call()
        .map_err(|e| format!("请求版本源失败（{url}）: {e}"))?
        .into_string()
        .map_err(|e| format!("读取版本源响应失败: {e}"))
}

/// Azul Zulu：LTS（8/11/17/21/25）+ 最新非 LTS，
/// 逐 major 并行请求 packages API 获取标准 JDK 版本列表
fn available_java() -> Result<Vec<AvailableVersion>, String> {
    const LTS: [i64; 5] = [8, 11, 17, 21, 25];
    let mut majors: Vec<i64> = LTS.to_vec();

    // 最新非 LTS major：不带 java_version 的 latest 请求
    if let Ok(packages) = zulu_packages(None) {
        if let Some(pkg) = zulu_pick(&packages) {
            if let Some(version) = zulu_java_version(pkg) {
                if let Some(m) = version
                    .split('.')
                    .next()
                    .and_then(|s| s.parse::<i64>().ok())
                {
                    if !majors.contains(&m) {
                        majors.push(m);
                    }
                }
            }
        }
    }
    majors.sort_unstable();

    // 并行请求各 major（网络往返从串行 N 次降为 1 次）
    let results: Mutex<Vec<AvailableVersion>> = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for major in &majors {
            let major = *major;
            let results = &results;
            s.spawn(move || {
                let Ok(packages) = zulu_packages(Some(&major.to_string())) else {
                    return;
                };
                for pkg in zulu_pick_all(&packages) {
                    let Some(version) = zulu_java_version(pkg) else {
                        continue;
                    };
                    results.lock().unwrap().push(AvailableVersion {
                        version,
                        is_lts: LTS.contains(&major),
                    });
                }
            });
        }
    });

    let versions = results.into_inner().unwrap();
    if versions.is_empty() {
        return Err("Azul Zulu 未返回可用版本".to_string());
    }
    Ok(versions)
}

/// nodejs.org：dist/index.json，取最新 80 个（含 LTS 标记）
fn available_node() -> Result<Vec<AvailableVersion>, String> {
    let json = http_get_json("https://nodejs.org/dist/index.json")?;
    let arr = json.as_array().ok_or("nodejs.org 响应格式异常")?;
    let mut versions = Vec::new();
    for entry in arr.iter().take(80) {
        let Some(version) = entry.get("version").and_then(|v| v.as_str()) else {
            continue;
        };
        let version = version.strip_prefix('v').unwrap_or(version).to_string();
        let is_lts = entry.get("lts").is_some_and(|v| !v.is_null());
        versions.push(AvailableVersion { version, is_lts });
    }
    if versions.is_empty() {
        return Err("nodejs.org 未返回可用版本".to_string());
    }
    Ok(versions)
}

/// go.dev：dl/?mode=json，全部 stable 版本
fn available_go() -> Result<Vec<AvailableVersion>, String> {
    let json = http_get_json("https://go.dev/dl/?mode=json")?;
    let arr = json.as_array().ok_or("go.dev 响应格式异常")?;
    let mut versions = Vec::new();
    for entry in arr.iter() {
        let stable = entry
            .get("stable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !stable {
            continue;
        }
        let Some(version) = entry.get("version").and_then(|v| v.as_str()) else {
            continue;
        };
        let version = version.strip_prefix("go").unwrap_or(version).to_string();
        versions.push(AvailableVersion {
            version,
            is_lts: false,
        });
    }
    if versions.is_empty() {
        return Err("go.dev 未返回可用版本".to_string());
    }
    Ok(versions)
}

/// Apache Maven 可用版本：解析镜像/官方的 maven-3 目录页（完整历史版本）。
/// 版本列表同样优先国内镜像（响应快），失败回退官方 archive。
fn available_maven() -> Result<Vec<AvailableVersion>, String> {
    let html = http_get_text("https://mirrors.tuna.tsinghua.edu.cn/apache/maven/maven-3/")
        .or_else(|_| http_get_text("https://archive.apache.org/dist/maven/maven-3/"))?;
    let mut versions = Vec::new();
    // 目录条目形如：<a href="3.9.16/">3.9.16/</a>
    for cap in html.match_indices("href=\"").filter_map(|(i, _)| {
        let rest = &html[i + 6..];
        let end = rest.find('"')?;
        let href = &rest[..end];
        href.strip_suffix('/').map(|d| d.to_string())
    }) {
        // 仅保留纯版本号目录（形如 3.9.16 / 3.0.4）
        let version = cap;
        let mut chars = version.chars();
        let mut ok = true;
        let mut dots = 0;
        let mut digits = 0;
        for c in chars.by_ref() {
            if c.is_ascii_digit() {
                digits += 1;
            } else if c == '.' {
                dots += 1;
            } else {
                ok = false;
                break;
            }
        }
        if !ok || dots == 0 || digits == 0 {
            continue;
        }
        // 过滤异常目录（如 "3.0-alpha" 会被上面规则排除；"3.9.16/" 正常）
        versions.push(AvailableVersion {
            version,
            is_lts: false,
        });
    }
    // 版本号倒序（最新在前）
    versions.sort_by(|a, b| {
        let va: Vec<u32> = a
            .version
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect();
        let vb: Vec<u32> = b
            .version
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect();
        vb.cmp(&va)
    });
    if versions.is_empty() {
        return Err("Apache Maven 目录页未解析到可用版本".to_string());
    }
    Ok(versions)
}

// ---------- 进度事件 ----------

fn emit_progress(
    app: &AppHandle,
    kind: RuntimeKind,
    version: &str,
    stage: &str,
    percent: Option<u32>,
    message: impl Into<String>,
) {
    let _ = app.emit(
        "install-progress",
        InstallProgress {
            kind,
            version: version.to_string(),
            stage: stage.to_string(),
            percent,
            message: message.into(),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 磁盘缓存序列化往返测试（临时文件路径，不改全局 HOME 环境变量）
    #[test]
    fn disk_cache_roundtrip() {
        let path = std::env::temp_dir().join(format!(
            "novaenv-cache-test-{}",
            std::process::id()
        ));
        let groups = vec![AvailableVersionGroup {
            major: "21".to_string(),
            is_lts: true,
            versions: vec!["21.0.12".to_string(), "21.0.11".to_string()],
            latest: "21.0.12".to_string(),
        }];
        save_disk_cache_at(&path, &groups);
        let loaded = load_disk_cache_at(&path);
        assert!(loaded.is_some(), "磁盘缓存应可读回");
        let loaded = loaded.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].major, "21");
        assert_eq!(loaded[0].latest, "21.0.12");
        assert_eq!(loaded[0].versions.len(), 2);
        // 清理测试文件
        let _ = std::fs::remove_file(&path);
    }
}

#[cfg(test)]
mod tests_core {
    use super::*;
    use crate::models::{AvailableVersion, RuntimeKind};

    fn av(version: &str, is_lts: bool) -> AvailableVersion {
        AvailableVersion {
            version: version.to_string(),
            is_lts,
        }
    }

    #[test]
    fn major_of_rules() {
        assert_eq!(major_of(RuntimeKind::Java, "17.0.12"), "17");
        assert_eq!(major_of(RuntimeKind::Node, "22.11.0"), "22");
        assert_eq!(major_of(RuntimeKind::Go, "1.24.3"), "1.24");
        assert_eq!(major_of(RuntimeKind::Maven, "3.9.16"), "3");
    }

    #[test]
    fn groups_by_major_with_lts_merge() {
        let versions = vec![
            av("22.11.0", false),
            av("22.12.0", true),
            av("24.4.1", false),
            av("24.4.1", false), // 去重
        ];
        let groups = group_versions(RuntimeKind::Node, versions);
        assert_eq!(groups.len(), 2);
        let g22 = groups.iter().find(|g| g.major == "22").unwrap();
        assert!(g22.is_lts);
        // 组内按版本倒序（最新在前），latest 取首个
        assert_eq!(g22.versions, vec!["22.12.0", "22.11.0"]);
        assert_eq!(g22.latest, "22.12.0");
        let g24 = groups.iter().find(|g| g.major == "24").unwrap();
        assert!(!g24.is_lts);
        assert_eq!(g24.versions, vec!["24.4.1"]);
    }

    #[test]
    fn managed_path_check() {
        let dir = installs_dir();
        let managed = dir.join("node").join("26.6.0");
        assert!(is_managed(&managed.to_string_lossy()));
        assert!(!is_managed("/usr/local/node"));
    }
}

/// Python 可用版本：python-build-standalone 最新 release 的 install_only 资产
/// （cpython-3.13.1+20260728-aarch64-apple-darwin-install_only.tar.gz），
/// 失败回退内置版本表。
fn available_python() -> Result<Vec<AvailableVersion>, String> {
    const FALLBACK: &[&str] = &[
        "3.14.1", "3.13.1", "3.12.8", "3.11.11", "3.10.16", "3.9.20",
    ];
    let platform = if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "aarch64-apple-darwin"
        } else {
            "x86_64-apple-darwin"
        }
    } else if cfg!(target_os = "windows") {
        "x86_64-pc-windows-msvc"
    } else {
        return Ok(FALLBACK
            .iter()
            .map(|v| AvailableVersion {
                version: v.to_string(),
                is_lts: false,
            })
            .collect());
    };
    let suffix = format!("-{platform}-install_only.tar.gz");

    let json = http_get_json(
        "https://api.github.com/repos/astral-sh/python-build-standalone/releases?per_page=1",
    )?;
    let Some(release) = json.as_array().and_then(|a| a.first()) else {
        return Err("python-build-standalone 响应格式异常".to_string());
    };
    let Some(assets) = release.get("assets").and_then(|v| v.as_array()) else {
        return Err("python-build-standalone 响应缺少资产".to_string());
    };
    let mut versions = Vec::new();
    for asset in assets {
        let Some(name) = asset.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(rest) = name.strip_prefix("cpython-") else {
            continue;
        };
        let Some(version) = rest.split('+').next() else {
            continue;
        };
        if name.ends_with(&suffix)
            && version.matches('.').count() >= 2
            && !versions
                .iter()
                .any(|v: &AvailableVersion| v.version == version)
        {
            versions.push(AvailableVersion {
                version: version.to_string(),
                is_lts: false,
            });
        }
    }
    if versions.is_empty() {
        // 回退内置版本表
        return Ok(FALLBACK
            .iter()
            .map(|v| AvailableVersion {
                version: v.to_string(),
                is_lts: false,
            })
            .collect());
    }
    versions.sort_by(|a, b| compare_versions(&b.version, &a.version));
    Ok(versions)
}

/// Rust 可用版本：rust-lang/rust GitHub Releases（stable 版本号），
/// 失败回退内置版本表。
fn available_rust() -> Result<Vec<AvailableVersion>, String> {
    const FALLBACK: &[&str] = &["1.97.1", "1.96.0", "1.95.1", "1.94.0", "1.93.1"];
    let json = http_get_json(
        "https://api.github.com/repos/rust-lang/rust/releases?per_page=60",
    )?;
    let Some(arr) = json.as_array() else {
        return Err("rust-lang/rust 响应格式异常".to_string());
    };
    let mut versions = Vec::new();
    for release in arr {
        let Some(tag) = release.get("tag_name").and_then(|v| v.as_str()) else {
            continue;
        };
        // 只接受形如 1.88.0 的 stable 版本（跳过 beta/rc 等 tag）
        if tag.matches('.').count() == 2 && tag.chars().all(|c| c.is_ascii_digit() || c == '.') {
            versions.push(AvailableVersion {
                version: tag.to_string(),
                is_lts: false,
            });
        }
    }
    if versions.is_empty() {
        return Ok(FALLBACK
            .iter()
            .map(|v| AvailableVersion {
                version: v.to_string(),
                is_lts: false,
            })
            .collect());
    }
    versions.sort_by(|a, b| compare_versions(&b.version, &a.version));
    Ok(versions)
}
