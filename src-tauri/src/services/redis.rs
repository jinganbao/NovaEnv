//! Redis 服务适配器：源码安装（官方 release 包 → make 编译）+ 进程管理。
//!
//! 布局（macOS）：
//! - 程序：`~/.novaenv/services/redis/<version>/bin/{redis-server,redis-cli}`
//! - 配置：`~/.novaenv/services/redis/<version>/redis.conf`（daemonize yes + pidfile）
//! - 数据：`~/.novaenv/data/redis/<version>/`
//! - 日志：`~/.novaenv/logs/redis-<version>.log`
//! - PID： `~/.novaenv/run/redis-<version>.pid`
//!
//! 依赖：Xcode CommandLine Tools（make / cc）。
//! Windows：官方无 Redis 发行版，暂不支持（note 提示）。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use tauri::Emitter;

use crate::models::{ServiceInfo, ServiceKind, ServiceProgress};

const NAME: &str = "Redis";
const PORT: u16 = 6379;
/// 服务根目录 ~/.novaenv/services
fn services_dir() -> PathBuf {
    crate::installer::installs_dir().parent().unwrap().join("services")
}

/// 数据根目录 ~/.novaenv/data
fn data_root() -> PathBuf {
    crate::installer::installs_dir().parent().unwrap().join("data")
}

/// 日志根目录 ~/.novaenv/logs
fn logs_dir() -> PathBuf {
    crate::installer::installs_dir().parent().unwrap().join("logs")
}

/// PID 根目录 ~/.novaenv/run
fn run_dir() -> PathBuf {
    crate::installer::installs_dir().parent().unwrap().join("run")
}

/// 某版本的安装根目录
fn version_dir(version: &str) -> PathBuf {
    services_dir().join("redis").join(version)
}

fn pid_file(version: &str) -> PathBuf {
    run_dir().join(format!("redis-{version}.pid"))
}

fn conf_file(version: &str) -> PathBuf {
    version_dir(version).join("redis.conf")
}

/// 当前状态（macOS/Windows 通用：Windows 恒为未安装 + 不支持说明）
pub fn info() -> ServiceInfo {
    #[cfg(target_os = "macos")]
    {
        let installed = latest_installed();
        let running = installed.as_ref().is_some_and(|v| is_running(v));
        ServiceInfo {
            kind: ServiceKind::Redis,
            name: NAME.to_string(),
            installed: installed.is_some(),
            version: installed.clone(),
            running,
            port: PORT,
            pid: installed.as_ref().and_then(|v| read_pid(v)),
            data_dir: installed
                .as_ref()
                .map(|v| data_root().join("redis").join(v).to_string_lossy().into_owned())
                .unwrap_or_default(),
            note: None,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        ServiceInfo {
            kind: ServiceKind::Redis,
            name: NAME.to_string(),
            installed: false,
            version: None,
            running: false,
            port: PORT,
            pid: None,
            data_dir: String::new(),
            note: Some("当前平台暂不支持 Redis（官方无 Windows 发行版）".to_string()),
        }
    }
}

/// 已安装的最高版本（无则 None）
#[cfg(target_os = "macos")]
fn latest_installed() -> Option<String> {
    let dir = services_dir().join("redis");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return None;
    };
    let mut versions: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let v = e.file_name().to_string_lossy().into_owned();
            let bin = e.path().join("bin").join("redis-server");
            bin.is_file().then_some((v, e.path()))
        })
        .collect();
    versions.sort_by(|a, b| cmp_versions(&a.0, &b.0));
    versions.pop().map(|(v, _)| v)
}

/// 端口是否可连接（服务运行判定）
#[cfg(target_os = "macos")]
pub fn is_port_open(port: u16) -> bool {
    std::net::TcpStream::connect(("127.0.0.1", port)).is_ok()
}

/// 某版本是否运行中：pid 文件存在且进程存活
#[cfg(target_os = "macos")]
fn is_running(version: &str) -> bool {
    match read_pid(version) {
        Some(pid) => {
            // macOS 下 kill(pid, 0) 检测进程存活
            let alive = unsafe { libc::kill(pid as i32, 0) == 0 };
            alive && is_port_open(PORT)
        }
        None => is_port_open(PORT),
    }
}

#[cfg(target_os = "macos")]
fn read_pid(version: &str) -> Option<u32> {
    std::fs::read_to_string(pid_file(version))
        .ok()?
        .trim()
        .parse()
        .ok()
}

// ---------- 版本源 ----------

/// 内置兜底版本列表（官方源不可达时使用；均为 download.redis.io 已发布版本）
const FALLBACK_VERSIONS: &[&str] = &[
    "8.10.0", "8.8.1", "8.6.5", "8.4.0", "8.2.1", "8.0.4", "7.4.4", "7.2.8", "7.0.15",
    "6.2.17", "6.0.20",
];

/// 可用版本列表（官方 download.redis.io/releases/ 目录解析，最新在前；
/// 官方源不可达时回退内置版本表，保证功能可用）
pub fn available_versions() -> Result<Vec<String>, String> {
    match fetch_versions_from_official() {
        Ok(versions) if !versions.is_empty() => Ok(versions),
        _ => Ok(FALLBACK_VERSIONS.iter().map(|s| s.to_string()).collect()),
    }
}

/// 解析官方版本目录页
fn fetch_versions_from_official() -> Result<Vec<String>, String> {
    let html = crate::installer::http_get_text("https://download.redis.io/releases/")?;
    let mut versions = Vec::new();
    for (i, _) in html.match_indices("redis-") {
        let rest = &html[i + 6..];
        let mut ver = String::new();
        for c in rest.chars() {
            if c.is_ascii_digit() || c == '.' {
                ver.push(c);
            } else {
                break;
            }
        }
        // 形如 X.Y.Z 且以 .tar.gz 结尾才接受
        if ver.matches('.').count() >= 2 && rest[ver.len()..].starts_with(".tar.gz") {
            if !versions.contains(&ver) {
                versions.push(ver);
            }
        }
    }
    versions.sort_by(|a, b| cmp_versions(b, a));
    if versions.is_empty() {
        return Err("Redis 版本源未解析到可用版本".to_string());
    }
    Ok(versions)
}

/// 版本号比较（8.10.0 > 8.8.1）
fn cmp_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let pa: Vec<u32> = a.split('.').filter_map(|s| s.parse().ok()).collect();
    let pb: Vec<u32> = b.split('.').filter_map(|s| s.parse().ok()).collect();
    pa.cmp(&pb)
}

// ---------- 安装 ----------

/// 安装 Redis 版本（下载 → 解压 → 编译 → 布局）。
/// 仅 macOS；进度经 `service-progress` 事件推送。
#[cfg(target_os = "macos")]
pub fn install(
    app: &tauri::AppHandle,
    kind: ServiceKind,
    version: &str,
) -> Result<(), String> {
    // 编译环境检查
    for tool in ["make", "cc"] {
        if std::process::Command::new(tool)
            .arg("--version")
            .output()
            .map(|o| !o.status.success())
            .unwrap_or(true)
        {
            return Err(format!(
                "缺少编译工具 {tool}，请先安装 Xcode 命令行工具（xcode-select --install）"
            ));
        }
    }

    emit(app, kind, version, "downloading", Some(0), "开始下载");

    // 1) 下载源码包（官方源，回退 GitHub）
    let downloads = crate::installer::installs_dir().join("_downloads");
    std::fs::create_dir_all(&downloads).map_err(|e| format!("创建下载目录失败: {e}"))?;
    let archive = downloads.join(format!("redis-{version}.tar.gz"));
    let url = format!("https://download.redis.io/releases/redis-{version}.tar.gz");
    download_with_fallback(app, kind, version, &url, &archive)?;

    // 2) 解压
    let tmp = downloads.join(format!("extract-redis-{version}"));
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp).map_err(|e| format!("清理临时目录失败: {e}"))?;
    }
    emit(app, kind, version, "extracting", None, "解压中…");
    crate::installer::extract(&archive, &tmp)?;
    let src_dir = tmp.join(format!("redis-{version}"));
    if !src_dir.is_dir() {
        return Err("Redis 源码目录结构异常，请重试".to_string());
    }

    // 3) 编译（make -j）
    emit(app, kind, version, "compiling", None, "编译中（约 1-2 分钟）…");
    let jobs = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(8);
    let status = Command::new("make")
        .args(["-j", &jobs.to_string()])
        .current_dir(&src_dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("编译失败（无法启动 make）: {e}"))?;
    if !status.success() {
        let _ = std::fs::remove_dir_all(&tmp);
        let _ = std::fs::remove_file(&archive);
        return Err("Redis 编译失败，请查看系统日志后重试".to_string());
    }

    // 4) 布局：拷贝产物到安装目录
    emit(app, kind, version, "installing", None, "写入安装目录…");
    let dest = version_dir(version);
    if dest.exists() {
        let _ = std::fs::remove_dir_all(&tmp);
        let _ = std::fs::remove_file(&archive);
        return Err(format!("版本 {version} 已安装"));
    }
    std::fs::create_dir_all(dest.join("bin")).map_err(|e| format!("创建安装目录失败: {e}"))?;
    for bin in ["redis-server", "redis-cli", "redis-check-aof", "redis-check-rdb"] {
        let src = src_dir.join("src").join(bin);
        if src.is_file() {
            std::fs::copy(&src, dest.join("bin").join(bin))
                .map_err(|e| format!("拷贝 {bin} 失败: {e}"))?;
        }
    }
    // redis.conf 模板拷入（供参考）+ 生成运行配置
    if src_dir.join("redis.conf").is_file() {
        let _ = std::fs::copy(src_dir.join("redis.conf"), dest.join("redis.conf.default"));
    }
    write_conf(version, &dest)?;

    // 5) 清理
    let _ = std::fs::remove_dir_all(&tmp);
    let _ = std::fs::remove_file(&archive);

    emit(app, kind, version, "done", Some(100), "安装完成");
    Ok(())
}

/// 下载（官方源失败回退 GitHub archive；进度经 service-progress 推送，MB 显示）
#[cfg(target_os = "macos")]
fn download_with_fallback(
    app: &tauri::AppHandle,
    kind: ServiceKind,
    version: &str,
    primary: &str,
    dest: &Path,
) -> Result<(), String> {
    let fallback = format!("https://github.com/redis/redis/archive/refs/tags/{version}.tar.gz");
    if download(app, kind, version, primary, dest).is_err() {
        emit(app, kind, version, "downloading", None, "官方源不可用，切换 GitHub 镜像…");
        download(app, kind, version, &fallback, dest)?;
    }
    Ok(())
}

/// 流式下载并推送进度事件（百分比变化时触发；MB 显示）
#[cfg(target_os = "macos")]
fn download(
    app: &tauri::AppHandle,
    kind: ServiceKind,
    version: &str,
    url: &str,
    dest: &Path,
) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .build();
    let resp = agent
        .get(url)
        .set("User-Agent", "NovaEnv/1.0")
        .call()
        .map_err(|e| format!("下载失败（{url}）: {e}"))?;
    let total = resp.header("Content-Length").and_then(|v| v.parse::<u64>().ok());
    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(dest).map_err(|e| format!("创建文件失败: {e}"))?;
    let mut buf = [0u8; 64 * 1024];
    let mut written: u64 = 0;
    let mut last_pct: i64 = -1;
    loop {
        let n = std::io::Read::read(&mut reader, &mut buf)
            .map_err(|e| format!("读取下载流失败: {e}"))?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n])
            .map_err(|e| format!("写入文件失败: {e}"))?;
        written += n as u64;
        if let Some(total) = total {
            if total > 0 {
                let pct = ((written * 100) / total) as i64;
                if pct != last_pct {
                    last_pct = pct;
                    let mb = |b: u64| format!("{:.1}", b as f64 / 1024.0 / 1024.0);
                    emit(
                        app,
                        kind,
                        version,
                        "downloading",
                        Some(pct as u32),
                        &format!("下载中 {}/{} MB", mb(written), mb(total)),
                    );
                }
            }
        } else {
            // 无 Content-Length：每 4MB 报一次已下载量（indeterminate 进度）
            if written / (4 * 1024 * 1024) as u64 > (written - n as u64) / (4 * 1024 * 1024) as u64 {
                let mb = format!("{:.1}", written as f64 / 1024.0 / 1024.0);
                emit(app, kind, version, "downloading", None, &format!("已下载 {mb} MB"));
            }
        }
    }
    Ok(())
}

/// 生成运行配置（daemonize + pidfile + 数据/日志目录）
#[cfg(target_os = "macos")]
fn write_conf(version: &str, dest: &Path) -> Result<(), String> {
    let data_dir = data_root().join("redis").join(version);
    let log_file = logs_dir().join(format!("redis-{version}.log"));
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
    std::fs::create_dir_all(logs_dir()).map_err(|e| format!("创建日志目录失败: {e}"))?;
    std::fs::create_dir_all(run_dir()).map_err(|e| format!("创建运行目录失败: {e}"))?;

    let conf = format!(
        "port {PORT}\ndaemonize yes\npidfile {}\ndir {}\nlogfile {}\nsave 900 1\nsave 300 10\nsave 60 10000\nappendonly yes\nappendfilename appendonly.aof\n",
        pid_file(version).display(),
        data_dir.display(),
        log_file.display(),
    );
    std::fs::write(dest.join("redis.conf"), conf).map_err(|e| format!("写配置文件失败: {e}"))
}

// ---------- 进程管理 ----------

/// 启动服务：以配置文件的 daemonize 模式拉起，轮询端口就绪
#[cfg(target_os = "macos")]
pub fn start(version: &str) -> Result<(), String> {
    let server = version_dir(version).join("bin").join("redis-server");
    if !server.is_file() {
        return Err(format!("Redis {version} 未安装"));
    }
    if is_running(version) {
        return Ok(()); // 已在运行
    }
    let conf = conf_file(version);
    if !conf.is_file() {
        return Err("配置文件缺失，请重新安装".to_string());
    }
    let status = Command::new(&server)
        .arg(conf.as_os_str())
        .status()
        .map_err(|e| format!("启动失败: {e}"))?;
    if !status.success() {
        return Err("redis-server 启动失败，请查看日志".to_string());
    }
    // 轮询端口就绪（最多 5s）
    for _ in 0..25 {
        if is_port_open(PORT) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Err(format!("Redis 启动超时（端口 {PORT} 未就绪）"))
}

/// 停止服务：读 pid → SIGTERM → 等待退出 → SIGKILL 兜底
#[cfg(target_os = "macos")]
pub fn stop(version: &str) -> Result<(), String> {
    if !is_running(version) {
        // 可能只有端口被占（非本服务管理）——不强行处理
        return Ok(());
    }
    let Some(pid) = read_pid(version) else {
        return Ok(());
    };
    // SIGTERM
    unsafe { libc::kill(pid as i32, libc::SIGTERM) };
    for _ in 0..50 {
        if !is_port_open(PORT) {
            // 清理残留 pid 文件
            let _ = std::fs::remove_file(pid_file(version));
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    // 超时 SIGKILL
    unsafe { libc::kill(pid as i32, libc::SIGKILL) };
    let _ = std::fs::remove_file(pid_file(version));
    Ok(())
}

/// 重启
#[cfg(target_os = "macos")]
pub fn restart(version: &str) -> Result<(), String> {
    let running = is_running(version);
    if running {
        stop(version)?;
        std::thread::sleep(Duration::from_millis(300));
    }
    start(version)
}

/// 卸载：停止服务 + 删除程序目录（数据目录保留）
#[cfg(target_os = "macos")]
pub fn uninstall(version: &str) -> Result<(), String> {
    if is_running(version) {
        stop(version)?;
    }
    let dir = version_dir(version);
    if !dir.exists() {
        return Err(format!("Redis {version} 未安装"));
    }
    std::fs::remove_dir_all(&dir).map_err(|e| format!("卸载失败: {e}"))?;
    Ok(())
}

/// 推送进度事件
#[cfg(target_os = "macos")]
fn emit(
    app: &tauri::AppHandle,
    kind: ServiceKind,
    version: &str,
    stage: &str,
    percent: Option<u32>,
    message: &str,
) {
    let _ = app.emit(
        "service-progress",
        ServiceProgress {
            kind,
            version: version.to_string(),
            stage: stage.to_string(),
            percent,
            message: message.to_string(),
        },
    );
}
