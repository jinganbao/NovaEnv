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

use std::collections::HashMap;
use std::cmp::Ordering;
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
    Path::new(path).starts_with(&installs_dir())
}

/// 各运行时在安装目录下的子目录名
fn kind_dir(kind: RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Java => "java",
        RuntimeKind::Node => "node",
        RuntimeKind::Go => "go",
    }
}

// ---------- 下载 URL 构造 ----------

/// 目标平台 os/arch 标识（按发行源约定）
fn os_arch() -> (&'static str, &'static str) {
    #[cfg(target_os = "macos")]
    let os = "mac";
    #[cfg(target_os = "windows")]
    let os = "windows";
    #[cfg(target_os = "linux")]
    let os = "linux";
    #[cfg(target_arch = "aarch64")]
    let arch = "aarch64";
    #[cfg(not(target_arch = "aarch64"))]
    let arch = "x64";
    (os, arch)
}

/// 构造下载 URL 与本地归档文件路径
fn download_url(kind: RuntimeKind, version: &str) -> Result<(String, PathBuf), String> {
    let (os, arch) = os_arch();
    let downloads = installs_dir().join("_downloads");
    std::fs::create_dir_all(&downloads)
        .map_err(|e| format!("创建下载目录失败: {e}"))?;

    match kind {
        RuntimeKind::Java => {
            // Adoptium Temurin：latest/<feature>/ga/<os>/<arch>/jdk/hotspot/normal/eclipse
            let url = format!(
                "https://api.adoptium.net/v3/binary/latest/{version}/ga/{os}/{arch}/jdk/hotspot/normal/eclipse"
            );
            let ext = if cfg!(target_os = "windows") { "zip" } else { "tar.gz" };
            Ok((url, downloads.join(format!("temurin-{version}.{ext}"))))
        }
        RuntimeKind::Node => {
            let (os_name, arch_name) = if cfg!(target_os = "macos") {
                ("darwin", if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" })
            } else if cfg!(target_os = "windows") {
                ("win", "x64")
            } else {
                ("linux", if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" })
            };
            let ext = if cfg!(target_os = "windows") { "zip" } else { "tar.gz" };
            let url = format!(
                "https://nodejs.org/dist/v{version}/node-v{version}-{os_name}-{arch_name}.{ext}"
            );
            Ok((url, downloads.join(format!("node-v{version}.{ext}"))))
        }
        RuntimeKind::Go => {
            let (os_name, arch_name) = if cfg!(target_os = "macos") {
                ("darwin", if cfg!(target_arch = "aarch64") { "arm64" } else { "amd64" })
            } else if cfg!(target_os = "windows") {
                ("windows", "amd64")
            } else {
                ("linux", if cfg!(target_arch = "aarch64") { "arm64" } else { "amd64" })
            };
            let ext = if cfg!(target_os = "windows") { "zip" } else { "tar.gz" };
            let url = format!("https://go.dev/dl/go{version}.{os_name}-{arch_name}.{ext}");
            Ok((url, downloads.join(format!("go{version}.{ext}"))))
        }
    }
}

// ---------- 下载 ----------

/// 流式下载并推送进度事件（百分比变化时触发）。
/// 注意：ureq 的 Request::timeout 是总超时，大文件下载不能用；
/// 这里仅限制连接超时，读取不设上限。
fn download(
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
                    format!("下载中 {downloaded}/{total} 字节"),
                );
            }
        }
    }
    Ok(())
}

// ---------- 解压 ----------

fn extract(archive: &Path, target: &Path) -> Result<(), String> {
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
        zip.extract(target)
            .map_err(|e| format!("解压失败: {e}"))?;
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
    }
}

// ---------- 安装 / 卸载主流程 ----------

/// 安装指定版本的运行时
pub fn install(app: &AppHandle, request: &InstallRequest) -> Result<(), String> {
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

    // 1) 下载
    download(app, kind, &version, &url, &archive)?;

    // 2) 解压到临时目录
    let tmp = installs_dir()
        .join("_downloads")
        .join(format!("extract-{}", kind_dir(kind)));
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp).map_err(|e| format!("清理临时目录失败: {e}"))?;
    }
    emit_progress(app, kind, &version, "extracting", None, "解压中…");
    extract(&archive, &tmp)?;

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
    emit_progress(app, kind, &final_version, "installing", None, "写入安装目录…");
    std::fs::rename(&src, &dest).map_err(|e| format!("移动到安装目录失败: {e}"))?;

    // 5) 清理
    let _ = std::fs::remove_dir_all(&tmp);
    let _ = std::fs::remove_file(&archive);

    emit_progress(app, kind, &final_version, "done", Some(100), "安装完成");
    Ok(())
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
    use crate::models::{ManagedRuntimeInfo, ManageInfo};

    let installs = installs_dir();
    let mut runtimes = Vec::new();
    let mut version_count = 0usize;
    let mut size_bytes = 0u64;

    for (kind, dir_name) in [
        (RuntimeKind::Java, "java"),
        (RuntimeKind::Node, "node"),
        (RuntimeKind::Go, "go"),
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
    }
}

/// 读取磁盘缓存（过期或损坏视为无缓存）
fn load_disk_cache(kind: RuntimeKind) -> Option<Vec<AvailableVersionGroup>> {
    let path = cache_path(kind);
    let content = std::fs::read_to_string(&path).ok()?;
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
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let json = serde_json::json!({
        "fetched_at": now,
        "groups": groups,
    });
    let dir = cache_path(kind).parent().map(|p| p.to_path_buf());
    if let Some(dir) = dir {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(cache_path(kind), serde_json::to_vec(&json).unwrap_or_default());
    }
}

/// 扁平版本列表按大版本分组（Go 的大版本为前两段，如 1.24；其余为第一段）
fn group_versions(kind: RuntimeKind, versions: Vec<AvailableVersion>) -> Vec<AvailableVersionGroup> {
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
        RuntimeKind::Go => version
            .split('.')
            .take(2)
            .collect::<Vec<_>>()
            .join("."),
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

/// Adoptium：available_releases 拿 majors（LTS + 最新 2 个非 LTS），
/// 逐 major 并行拉取该 feature 下最近的 GA 版本（semver 如 21.0.12+8.0.LTS）
fn available_java() -> Result<Vec<AvailableVersion>, String> {
    let info = http_get_json("https://api.adoptium.net/v3/info/available_releases")?;
    let mut lts: Vec<i64> = info
        .get("available_lts_releases")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|n| n.as_i64()).collect())
        .unwrap_or_default();
    let mut all: Vec<i64> = info
        .get("available_releases")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|n| n.as_i64()).collect())
        .unwrap_or_default();
    lts.sort_unstable();
    all.sort_unstable();

    // majors：全部 LTS + 最新 1 个非 LTS（减少首次请求数量）
    let mut majors: Vec<i64> = lts.clone();
    for m in all.iter().rev() {
        if !majors.contains(m) {
            majors.push(*m);
        }
        if majors.len() >= lts.len() + 1 {
            break;
        }
    }
    majors.sort_unstable();

    // 并行请求各 major（网络往返从串行 N 次降为 1 次）
    let results: Mutex<Vec<AvailableVersion>> = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for major in &majors {
            let major = *major;
            let results = &results;
            let lts = &lts;
            s.spawn(move || {
                let url = format!(
                    "https://api.adoptium.net/v3/assets/feature_releases/{major}/ga?page_size=5&image_type=jdk"
                );
                let Ok(json) = http_get_json(&url) else {
                    return;
                };
                let Some(arr) = json.as_array() else {
                    return;
                };
                let mut items = Vec::new();
                for item in arr {
                    let Some(semver) = item
                        .get("version_data")
                        .and_then(|v| v.get("semver"))
                        .and_then(|v| v.as_str())
                    else {
                        continue;
                    };
                    // Adoptium semver 形如 "21.0.12+8.0.LTS"，规范化为 3 段
                    // （与安装后 release 文件的 JAVA_VERSION 一致，便于列表对账）
                    let version = semver.split('+').next().unwrap_or(semver).to_string();
                    items.push(AvailableVersion {
                        version,
                        is_lts: lts.contains(&major),
                    });
                }
                results.lock().unwrap().extend(items);
            });
        }
    });

    let versions = results.into_inner().unwrap();
    if versions.is_empty() {
        return Err("Adoptium 未返回可用版本".to_string());
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
        let stable = entry.get("stable").and_then(|v| v.as_bool()).unwrap_or(false);
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

    /// 磁盘缓存序列化往返测试（临时 HOME 避免污染真实用户目录）
    #[test]
    fn disk_cache_roundtrip() {
        let original_home = std::env::var("HOME").ok();
        std::env::set_var("HOME", "/tmp/novaenv-cache-test");
        let groups = vec![AvailableVersionGroup {
            major: "21".to_string(),
            is_lts: true,
            versions: vec!["21.0.12".to_string(), "21.0.11".to_string()],
            latest: "21.0.12".to_string(),
        }];
        save_disk_cache(RuntimeKind::Java, &groups);
        let loaded = load_disk_cache(RuntimeKind::Java);
        assert!(loaded.is_some(), "磁盘缓存应可读回");
        let loaded = loaded.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].major, "21");
        assert_eq!(loaded[0].latest, "21.0.12");
        assert_eq!(loaded[0].versions.len(), 2);
        // 清理测试目录
        let _ = std::fs::remove_dir_all("/tmp/novaenv-cache-test");
        if let Some(home) = original_home {
            std::env::set_var("HOME", home);
        }
    }
}
