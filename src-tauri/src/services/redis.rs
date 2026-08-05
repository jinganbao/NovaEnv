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

use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::path::Path;
#[cfg(target_os = "macos")]
use std::os::unix::process::CommandExt;
use std::process::Command;
#[cfg(target_os = "macos")]
use std::time::Duration;

#[cfg(target_os = "macos")]
use tauri::Emitter;

use crate::models::{
    AvailableVersionGroup, ServiceConfig, ServiceInfo, ServiceKind, ServiceVersionInfo,
};
#[cfg(target_os = "macos")]
use crate::models::ServiceProgress;

const NAME: &str = "Redis";
const DEFAULT_PORT: u16 = 6379;
/// 服务根目录 ~/.novaenv/services
#[cfg(target_os = "macos")]
fn services_dir() -> PathBuf {
    crate::installer::installs_dir()
        .parent()
        .unwrap()
        .join("services")
}

/// 数据根目录 ~/.novaenv/data
#[cfg(target_os = "macos")]
fn data_root() -> PathBuf {
    crate::installer::installs_dir()
        .parent()
        .unwrap()
        .join("data")
}

/// 日志根目录 ~/.novaenv/logs
#[cfg(target_os = "macos")]
fn logs_dir() -> PathBuf {
    crate::installer::installs_dir()
        .parent()
        .unwrap()
        .join("logs")
}

/// PID 根目录 ~/.novaenv/run
#[cfg(target_os = "macos")]
fn run_dir() -> PathBuf {
    crate::installer::installs_dir()
        .parent()
        .unwrap()
        .join("run")
}

/// 某版本的安装根目录
#[cfg(target_os = "macos")]
fn version_dir(version: &str) -> PathBuf {
    services_dir().join("redis").join(version)
}

#[cfg(target_os = "macos")]
fn pid_file(version: &str) -> PathBuf {
    run_dir().join(format!("redis-{version}.pid"))
}

#[cfg(target_os = "macos")]
fn conf_file(version: &str) -> PathBuf {
    version_dir(version).join("redis.conf")
}

/// 当前状态（macOS/Windows 通用：Windows 恒为未安装 + 不支持说明）
pub fn info() -> ServiceInfo {
    #[cfg(target_os = "macos")]
    {
        let versions = installed_versions()
            .into_iter()
            .map(|v| {
                let conf = read_conf(&v);
                ServiceInfo {
                    kind: ServiceKind::Redis,
                    name: NAME.to_string(),
                    installed: true,
                    version: Some(v.clone()),
                    versions: Vec::new(),
                    running: is_running(&v),
                    port: conf.as_ref().map(|c| c.port).unwrap_or(DEFAULT_PORT),
                    pid: read_pid(&v),
                    password: conf.map(|c| c.password).unwrap_or_default(),
                    autostart: crate::services::launchd::plist_path("redis", &v).exists(),
                    data_dir: data_root()
                        .join("redis")
                        .join(&v)
                        .to_string_lossy()
                        .into_owned(),
                    note: None,
                }
            })
            .collect::<Vec<_>>();
        let latest = versions.last().cloned().unwrap_or_else(|| ServiceInfo {
            kind: ServiceKind::Redis,
            name: NAME.to_string(),
            installed: false,
            version: None,
            versions: Vec::new(),
            running: false,
            port: DEFAULT_PORT,
            pid: None,
            password: String::new(),
            autostart: false,
            data_dir: String::new(),
            note: None,
        });
        ServiceInfo {
            versions: versions
                .iter()
                .map(|v| ServiceVersionInfo {
                    version: v.version.clone().unwrap_or_default(),
                    running: v.running,
                    port: v.port,
                    autostart: v.autostart,
                })
                .collect(),
            ..latest
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        ServiceInfo {
            kind: ServiceKind::Redis,
            name: NAME.to_string(),
            installed: false,
            version: None,
            versions: Vec::new(),
            running: false,
            port: DEFAULT_PORT,
            pid: None,
            password: String::new(),
            autostart: false,
            data_dir: String::new(),
            note: Some("当前平台暂不支持 Redis（官方无 Windows 发行版）".to_string()),
        }
    }
}

/// 全部已安装版本（按版本升序；无则空列表）
#[cfg(target_os = "macos")]
fn installed_versions() -> Vec<String> {
    let dir = services_dir().join("redis");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
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
    versions.into_iter().map(|(v, _)| v).collect()
}

/// 端口是否可连接（服务运行判定）
#[cfg(target_os = "macos")]
pub fn is_port_open(port: u16) -> bool {
    std::net::TcpStream::connect(("127.0.0.1", port)).is_ok()
}

/// 某版本是否运行中：仅认 NovaEnv 启动的实例（pid 文件 + 进程存活）
/// 不能再用 conf 端口探测——修改端口后 conf 已变，但进程仍监听启动时端口，
/// 会导致误判并跳过配置修改后的自动重启
#[cfg(target_os = "macos")]
fn is_running(version: &str) -> bool {
    let Some(pid) = read_pid(version) else {
        return false;
    };
    unsafe { libc::kill(pid as i32, 0) == 0 }
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
    "8.10.0", "8.8.1", "8.6.5", "8.4.0", "8.2.1", "8.0.4", "7.4.4", "7.2.8", "7.0.15", "6.2.17",
    "6.0.20",
];

/// 服务可安装版本（按大版本分组，最新在前；官方源不可达时回退内置版本表）
pub fn available_version_groups() -> Result<Vec<AvailableVersionGroup>, String> {
    let versions = available_versions()?;
    Ok(group_versions(&versions))
}

/// 扁平版本列表 → 按大版本分组（versions 已倒序，每组首个即最新）
fn group_versions(versions: &[String]) -> Vec<AvailableVersionGroup> {
    let mut groups: Vec<AvailableVersionGroup> = Vec::new();
    for v in versions {
        let major = v.split('.').next().unwrap_or(v).to_string();
        if let Some(g) = groups.iter_mut().find(|g| g.major == major) {
            g.versions.push(v.clone());
        } else {
            groups.push(AvailableVersionGroup {
                major,
                is_lts: false,
                versions: vec![v.clone()],
                latest: v.clone(),
            });
        }
    }
    groups
}

/// 可用版本列表（官方 download.redis.io/releases/ 目录解析，最新在前；
/// 官方源不可达时回退内置版本表，保证功能可用）
pub fn available_versions() -> Result<Vec<String>, String> {
    match fetch_versions_from_official() {
        Ok(versions) if !versions.is_empty() => Ok(versions),
        _ => Ok(FALLBACK_VERSIONS.iter().map(|s| s.to_string()).collect()),
    }
}

/// 解析官方版本目录页（纯函数，便于单元测试）
fn parse_version_html(html: &str) -> Vec<String> {
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
        // 收集可能含尾点（".tar.gz" 前的点），需去掉后再校验
        let ver = ver.trim_end_matches('.').to_string();
        // 形如 X.Y.Z 且以 .tar.gz 结尾才接受
        if ver.matches('.').count() >= 2
            && rest[ver.len()..].starts_with(".tar.gz")
            && !versions.contains(&ver)
        {
            versions.push(ver);
        }
    }
    versions
}

/// 解析官方版本目录页
fn fetch_versions_from_official() -> Result<Vec<String>, String> {
    let html = crate::installer::http_get_text("https://download.redis.io/releases/")?;
    let mut versions = parse_version_html(&html);
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
/// `port`/`password` 为安装配置（缺省用默认端口/无密码）。
#[cfg(target_os = "macos")]
pub fn install(
    app: &tauri::AppHandle,
    kind: ServiceKind,
    version: &str,
    port: Option<u16>,
    password: Option<String>,
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

    // 3) 编译（make -j，仅构建核心二进制目标——默认 make 会附带构建
    //    可选模块（redisearch/redisjson 等），其依赖较新 make 4.x 与额外
    //    依赖，在 macOS 自带 make 3.81 上必然失败；核心 server 不受影响）
    emit(
        app,
        kind,
        version,
        "compiling",
        None,
        "编译中（约 1-2 分钟）…",
    );
    let jobs = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(8);
    let status = Command::new("make")
        .args([
            "-j",
            &jobs.to_string(),
            "redis-server",
            "redis-cli",
            "redis-check-aof",
            "redis-check-rdb",
        ])
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
    for bin in [
        "redis-server",
        "redis-cli",
        "redis-check-aof",
        "redis-check-rdb",
    ] {
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
    let config = ServiceConfig {
        port: port.unwrap_or(DEFAULT_PORT),
        password: password.unwrap_or_default(),
        old_password: String::new(),
    };
    write_conf(version, &dest, &config)?;

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
        emit(
            app,
            kind,
            version,
            "downloading",
            None,
            "官方源不可用，切换 GitHub 镜像…",
        );
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
    let total = resp
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok());
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
                let pct = written.checked_mul(100).map(|v| v / total).unwrap_or(0) as i64;
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
            if written / (4 * 1024 * 1024) as u64 > (written - n as u64) / (4 * 1024 * 1024) as u64
            {
                let mb = format!("{:.1}", written as f64 / 1024.0 / 1024.0);
                emit(
                    app,
                    kind,
                    version,
                    "downloading",
                    None,
                    &format!("已下载 {mb} MB"),
                );
            }
        }
    }
    Ok(())
}

/// 生成运行配置（daemonize + pidfile + 数据/日志目录 + 端口/密码）
#[cfg(target_os = "macos")]
fn write_conf(version: &str, dest: &Path, config: &ServiceConfig) -> Result<(), String> {
    let data_dir = data_root().join("redis").join(version);
    let log_file = logs_dir().join(format!("redis-{version}.log"));
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
    std::fs::create_dir_all(logs_dir()).map_err(|e| format!("创建日志目录失败: {e}"))?;
    std::fs::create_dir_all(run_dir()).map_err(|e| format!("创建运行目录失败: {e}"))?;

    let mut conf = format!(
        "port {}\ndaemonize yes\npidfile {}\ndir {}\nlogfile {}\nsave 900 1\nsave 300 10\nsave 60 10000\nappendonly yes\nappendfilename appendonly.aof\n",
        config.port,
        pid_file(version).display(),
        data_dir.display(),
        log_file.display(),
    );
    if !config.password.is_empty() {
        conf.push_str(&format!("requirepass {}\n", config.password));
    }
    std::fs::write(dest.join("redis.conf"), conf).map_err(|e| format!("写配置文件失败: {e}"))
}

/// 读取版本实际运行配置（端口 / 密码），无配置文件时返回默认值
#[cfg(target_os = "macos")]
pub fn read_conf(version: &str) -> Option<ServiceConfig> {
    let content = std::fs::read_to_string(conf_file(version)).ok()?;
    Some(parse_conf(&content))
}

/// 解析 redis.conf（纯函数，便于单元测试）
fn parse_conf(content: &str) -> ServiceConfig {
    let mut port = DEFAULT_PORT;
    let mut password = String::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(' ') else {
            continue;
        };
        match key.trim() {
            "port" => {
                if let Ok(p) = value.trim().parse::<u16>() {
                    port = p;
                }
            }
            "requirepass" => password = value.trim().to_string(),
            _ => {}
        }
    }
    ServiceConfig { port, password, old_password: String::new() }
}


/// 修改运行配置（端口 / 密码）：校验新端口未被占用 → 重写配置 → 运行中自动重启生效
#[cfg(target_os = "macos")]
pub fn update_config(version: &str, config: &ServiceConfig) -> Result<(), String> {
    let dir = version_dir(version);
    if !dir.join("bin").join("redis-server").is_file() {
        return Err(format!("Redis {version} 未安装"));
    }
    if config.port == 0 {
        return Err("端口不能为 0".to_string());
    }
    let old = read_conf(version).unwrap_or(ServiceConfig {
        port: DEFAULT_PORT,
        password: String::new(),
        old_password: String::new(),
    });
    // 新端口与旧端口不同且已被占用 → 冲突
    if config.port != old.port && is_port_open(config.port) {
        return Err(format!("端口 {} 已被占用，请换一个端口", config.port));
    }
    // 密码留空 = 不修改（保留旧密码），避免误删已设置密码
    let mut effective = config.clone();
    if effective.password.is_empty() {
        effective.password = old.password.clone();
    }
    write_conf(version, &dir, &effective)?;
    // 运行中自动重启使新配置生效
    if is_running(version) {
        restart(version)?;
    }
    Ok(())
}

// ---------- 进程管理 ----------

/// 启动服务：launchd 托管时走 launchctl，否则 daemonize 配置拉起
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
    // 端口预检：conf 端口被其他进程占用且不是本实例 → 明确报错
    let port = read_conf(version).map(|c| c.port).unwrap_or(DEFAULT_PORT);
    if is_port_open(port) {
        return Err(format!(
            "端口 {port} 已被占用（可能是旧实例未退出或其他程序），请先停止占用方或更换端口"
        ));
    }
    let status = Command::new(&server)
        .arg(conf.as_os_str())
        .process_group(0) // 独立进程组，避免 app/终端退出信号波及服务进程
        .status()
        .map_err(|e| format!("启动失败: {e}"))?;
    if !status.success() {
        // 附带日志尾部定位失败原因
        let log = crate::services::tail_log_file(&logs_dir().join(format!("redis-{version}.log")), 8).unwrap_or_default();
        return Err(format!("redis-server 启动失败：{log}"));
    }
    // 轮询端口就绪（最多 5s）
    let port = read_conf(version).map(|c| c.port).unwrap_or(DEFAULT_PORT);
    for _ in 0..25 {
        if is_port_open(port) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Err(format!("Redis 启动超时（端口 {port} 未就绪）"))
}

/// 停止服务：launchd 托管时 bootout，否则 SIGTERM → SIGKILL 兜底
#[cfg(target_os = "macos")]
pub fn stop(version: &str) -> Result<(), String> {
    if autostart_enabled(version) {
        return crate::services::launchd::stop("redis", version);
    }
    if !is_running(version) {
        // 可能只有端口被占（非本服务管理）——不强行处理
        return Ok(());
    }
    let Some(pid) = read_pid(version) else {
        return Ok(());
    };
    // SIGTERM
    unsafe { libc::kill(pid as i32, libc::SIGTERM) };
    // 优雅关闭通常 1-2 秒；4 秒（20×200ms）进程未退出则 SIGKILL 兜底
    // 完成判断用进程存活（不用端口——改端口后 conf 端口≠进程实际端口）
    for _ in 0..20 {
        if !is_running(version) {
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
    if autostart_enabled(version) {
        return crate::services::launchd::restart("redis", version);
    }
    let running = is_running(version);
    if running {
        stop(version)?;
        std::thread::sleep(Duration::from_millis(300));
    }
    start(version)
}

// ---------- 开机自启（launchd） ----------

/// 是否已开启自启（plist 存在）
#[cfg(target_os = "macos")]
fn autostart_enabled(version: &str) -> bool {
    crate::services::launchd::plist_path("redis", version).exists()
}

/// 设置/取消开机自启（launchd 托管：RunAtLoad 自启 + KeepAlive 崩溃拉起）
#[cfg(target_os = "macos")]
pub fn set_autostart(version: &str, enabled: bool) -> Result<(), String> {
    let server = version_dir(version).join("bin").join("redis-server");
    if !server.is_file() {
        return Err(format!("Redis {version} 未安装"));
    }
    if enabled {
        // launchd 托管要求前台运行：生成 daemonize no 的专用配置
        let conf = read_conf(version).unwrap_or(ServiceConfig {
            port: DEFAULT_PORT,
            password: String::new(),
            old_password: String::new(),
        });
        let launchd_conf = version_dir(version).join("redis.launchd.conf");
        let data_dir = data_root().join("redis").join(version);
        let log_file = logs_dir().join(format!("redis-{version}.log"));
        let mut content = format!(
            "port {}\ndaemonize no\npidfile {}\ndir {}\nlogfile {}\n",
            conf.port,
            pid_file(version).display(),
            data_dir.display(),
            log_file.display(),
        );
        if !conf.password.is_empty() {
            content.push_str(&format!("requirepass {}\n", conf.password));
        }
        std::fs::write(&launchd_conf, content).map_err(|e| format!("写 launchd 配置失败: {e}"))?;

        crate::services::launchd::enable(
            "redis",
            version,
            &[
                server.to_string_lossy().into_owned(),
                launchd_conf.to_string_lossy().into_owned(),
            ],
            &logs_dir().to_string_lossy(),
        )?;
        // 原方式运行的旧实例交给 launchd 接管
        if is_running(version) && !crate::services::launchd::is_loaded("redis", version) {
            stop_legacy(version)?;
            crate::services::launchd::start("redis", version)?;
        }
    } else {
        crate::services::launchd::disable("redis", version)?;
        let _ = std::fs::remove_file(version_dir(version).join("redis.launchd.conf"));
    }
    Ok(())
}

/// 非 launchd 方式的停止（供接管/回退使用）
#[cfg(target_os = "macos")]
fn stop_legacy(version: &str) -> Result<(), String> {
    if !is_running(version) {
        return Ok(());
    }
    let Some(pid) = read_pid(version) else {
        return Ok(());
    };
    unsafe { libc::kill(pid as i32, libc::SIGTERM) };
    let port = read_conf(version).map(|c| c.port).unwrap_or(DEFAULT_PORT);
    for _ in 0..20 {
        if !is_port_open(port) {
            let _ = std::fs::remove_file(pid_file(version));
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    unsafe { libc::kill(pid as i32, libc::SIGKILL) };
    let _ = std::fs::remove_file(pid_file(version));
    Ok(())
}

/// 读取服务日志尾部（默认 200 行）
#[cfg(target_os = "macos")]
pub fn tail_log(version: &str, lines: usize) -> Result<String, String> {
    crate::services::tail_log_file(&logs_dir().join(format!("redis-{version}.log")), lines)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ServiceKind;

    #[test]
    fn parses_redis_conf() {
        let conf = parse_conf(
            "port 6380\ndaemonize yes\npidfile /x/run/redis.pid\nrequirepass secret123\nsave 900 1\n",
        );
        assert_eq!(conf.port, 6380);
        assert_eq!(conf.password, "secret123");
    }

    #[test]
    fn defaults_when_missing() {
        let conf = parse_conf("daemonize yes\n");
        assert_eq!(conf.port, DEFAULT_PORT);
        assert_eq!(conf.password, "");
    }

    #[test]
    fn ignores_comments_and_invalid_port() {
        let conf = parse_conf("# port 9999\nport not-a-number\n");
        assert_eq!(conf.port, DEFAULT_PORT);
    }

    #[test]
    fn version_comparison() {
        assert!(cmp_versions("8.10.0", "8.8.1").is_gt());
        assert!(cmp_versions("7.4.4", "8.0.1").is_lt());
        assert!(cmp_versions("6.2.17", "6.2.17").is_eq());
    }

    #[test]
    fn parses_version_list_html() {
        let html = "<html><a href=\"redis-8.10.0.tar.gz\">..</a>\n<a href=\"redis-7.4.4.tar.gz\">..</a>\n<a href=\"redis-7.4.4.tar.gz\">dup</a>\n<a href=\"redis-8.0.0-alpha.tar.gz\">..</a>\n<a href=\"redis-6.2.17.tar.gz\">..</a></html>";
        let mut versions = parse_version_html(html);
        versions.sort_by(|a, b| cmp_versions(b, a));
        assert_eq!(versions, vec!["8.10.0", "7.4.4", "6.2.17"]);
    }

    #[test]
    fn kind_meta() {
        assert_eq!(ServiceKind::Redis.display_name(), "Redis");
        assert_eq!(ServiceKind::Redis.default_port(), 6379);
    }
}
