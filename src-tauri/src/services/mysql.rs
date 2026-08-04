//! MySQL 服务适配器：官方预编译包安装（下载 → 解压 → 数据目录初始化）+ 进程管理。
//!
//! 布局（macOS）：
//! - 程序：`~/.novaenv/services/mysql/<version>/`（官方包解压内容，bin/ 等）
//! - 配置：`~/.novaenv/services/mysql/<version>/my.cnf`
//! - 数据：`~/.novaenv/data/mysql/<version>/`（mysqld --initialize-insecure 初始化）
//! - 日志：`~/.novaenv/logs/mysql-<version>.log`
//! - PID： `~/.novaenv/run/mysql-<version>.pid`
//!
//! 说明：
//! - 官方 CDN 目录列表有反爬，版本列表使用内置版本表（发布节奏慢，随版本维护）
//! - 包名含 macOS 代号（macos14/macos15…），下载时按探测链回退
//! - Windows：官方有 winx64 包但服务化差异大，首版暂不支持（note 提示）

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use tauri::Emitter;

use crate::models::{ServiceConfig, ServiceInfo, ServiceKind, ServiceProgress};

const NAME: &str = "MySQL";
const DEFAULT_PORT: u16 = 3306;

/// 内置版本表（已确认官方 CDN 存在的版本；版本号 → 发布时的 macOS 代号）
const KNOWN_VERSIONS: &[(&str, &str)] = &[
    ("8.4.0", "macos14"), // LTS
    ("9.0.1", "macos14"), // Innovation
];

/// 包名探测回退链（未来版本可能用更新的 macOS 代号）
const MACOS_TAGS: &[&str] = &["macos15", "macos14", "macos13", "macos12"];

/// 服务根目录 ~/.novaenv/services
fn services_dir() -> PathBuf {
    crate::installer::installs_dir()
        .parent()
        .unwrap()
        .join("services")
}

#[cfg(target_os = "macos")]
fn data_root() -> PathBuf {
    crate::installer::installs_dir()
        .parent()
        .unwrap()
        .join("data")
}

#[cfg(target_os = "macos")]
fn logs_dir() -> PathBuf {
    crate::installer::installs_dir()
        .parent()
        .unwrap()
        .join("logs")
}

#[cfg(target_os = "macos")]
fn run_dir() -> PathBuf {
    crate::installer::installs_dir()
        .parent()
        .unwrap()
        .join("run")
}

#[cfg(target_os = "macos")]
fn version_dir(version: &str) -> PathBuf {
    services_dir().join("mysql").join(version)
}

#[cfg(target_os = "macos")]
fn pid_file(version: &str) -> PathBuf {
    run_dir().join(format!("mysql-{version}.pid"))
}

#[cfg(target_os = "macos")]
fn conf_file(version: &str) -> PathBuf {
    version_dir(version).join("my.cnf")
}

#[cfg(target_os = "macos")]
fn bin_dir(version: &str) -> PathBuf {
    version_dir(version).join("bin")
}

/// 当前状态（macOS/Windows 通用：Windows 恒为未安装 + 不支持说明）
pub fn info() -> ServiceInfo {
    #[cfg(target_os = "macos")]
    {
        let installed = latest_installed();
        let running = installed.as_ref().is_some_and(|v| is_running(v));
        let conf = installed.as_ref().and_then(|v| read_conf(v));
        let autostart = installed
            .as_ref()
            .map(|v| crate::services::launchd::plist_path("mysql", v).exists())
            .unwrap_or(false);
        ServiceInfo {
            kind: ServiceKind::MySql,
            name: NAME.to_string(),
            installed: installed.is_some(),
            version: installed.clone(),
            running,
            port: conf.as_ref().map(|c| c.port).unwrap_or(DEFAULT_PORT),
            pid: installed.as_ref().and_then(|v| read_pid(v)),
            password: conf.map(|c| c.password).unwrap_or_default(),
            autostart,
            data_dir: installed
                .as_ref()
                .map(|v| {
                    data_root()
                        .join("mysql")
                        .join(v)
                        .to_string_lossy()
                        .into_owned()
                })
                .unwrap_or_default(),
            note: None,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        ServiceInfo {
            kind: ServiceKind::MySql,
            name: NAME.to_string(),
            installed: false,
            version: None,
            running: false,
            port: DEFAULT_PORT,
            pid: None,
            password: String::new(),
            autostart: false,
            data_dir: String::new(),
            note: Some("当前平台暂不支持 MySQL（Windows 支持规划中）".to_string()),
        }
    }
}

/// 已安装的最高版本（无则 None）
#[cfg(target_os = "macos")]
fn latest_installed() -> Option<String> {
    let dir = services_dir().join("mysql");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return None;
    };
    let mut versions: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let v = e.file_name().to_string_lossy().into_owned();
            e.path().join("bin").join("mysqld").is_file().then_some(v)
        })
        .collect();
    versions.sort_by(|a, b| cmp_versions(a, b));
    versions.pop()
}

/// 端口是否可连接（服务运行判定）
#[cfg(target_os = "macos")]
pub fn is_port_open(port: u16) -> bool {
    std::net::TcpStream::connect(("127.0.0.1", port)).is_ok()
}

#[cfg(target_os = "macos")]
fn read_pid(version: &str) -> Option<u32> {
    std::fs::read_to_string(pid_file(version))
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// 某版本是否运行中：仅认 NovaEnv 启动的实例（pid 文件 + 进程存活 + 端口开放）
/// 不靠端口猜测——避免把用户已有的 MySQL（3306）误判为自己的实例
#[cfg(target_os = "macos")]
fn is_running(version: &str) -> bool {
    let Some(pid) = read_pid(version) else {
        return false;
    };
    let alive = unsafe { libc::kill(pid as i32, 0) == 0 };
    if !alive {
        return false;
    }
    let port = read_conf(version).map(|c| c.port).unwrap_or(DEFAULT_PORT);
    is_port_open(port)
}

/// socket 文件路径（客户端强制走 socket，绝不连到用户已有实例）
#[cfg(target_os = "macos")]
fn socket_file(version: &str) -> PathBuf {
    run_dir().join(format!("mysql-{version}.sock"))
}

/// 安装/配置前的端口冲突检测：端口被占用且不是本实例 → 明确报错
#[cfg(target_os = "macos")]
fn ensure_port_free(version: &str, port: u16) -> Result<(), String> {
    if is_port_open(port) && !is_running(version) {
        return Err(format!(
            "端口 {port} 已被其他进程占用（可能是您已有的 MySQL/其他服务），请更换端口或先停止占用方"
        ));
    }
    Ok(())
}

/// 版本号比较
fn cmp_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let pa: Vec<u32> = a.split('.').filter_map(|s| s.parse().ok()).collect();
    let pb: Vec<u32> = b.split('.').filter_map(|s| s.parse().ok()).collect();
    pa.cmp(&pb)
}

// ---------- 版本源 ----------

/// 可用版本列表（内置表，官方 CDN 已确认存在）
pub fn available_version_groups() -> Result<Vec<crate::models::AvailableVersionGroup>, String> {
    let mut groups: Vec<crate::models::AvailableVersionGroup> = Vec::new();
    for (version, _tag) in KNOWN_VERSIONS {
        let major = version.split('.').next().unwrap_or(version).to_string();
        if let Some(g) = groups.iter_mut().find(|g| g.major == major) {
            g.versions.push(version.to_string());
        } else {
            groups.push(crate::models::AvailableVersionGroup {
                major,
                is_lts: version.starts_with("8.4"),
                versions: vec![version.to_string()],
                latest: version.to_string(),
            });
        }
    }
    Ok(groups)
}

/// 探测包名并返回下载 URL（按 macOS 代号回退链）
#[cfg(target_os = "macos")]
fn resolve_download_url(version: &str) -> Result<String, String> {
    // 官方目录按大版本前两段组织（MySQL-8.4 / MySQL-9.0），与 Go 的 major 规则一致
    let major = version.split('.').take(2).collect::<Vec<_>>().join(".");
    // 已知版本直接使用记录在案的包名（跳过网络探测，兼容慢网络/超时）
    if let Some((_, tag)) = KNOWN_VERSIONS.iter().find(|(v, _)| *v == version) {
        return Ok(format!(
            "https://cdn.mysql.com/Downloads/MySQL-{major}/mysql-{version}-{tag}-arm64.tar.gz"
        ));
    }
    // 未知版本走探测链
    let mut tags: Vec<&str> = MACOS_TAGS.to_vec();
    for tag in tags.iter_mut() {
        let url = format!(
            "https://cdn.mysql.com/Downloads/MySQL-{major}/mysql-{version}-{tag}-arm64.tar.gz"
        );
        if crate::installer::url_exists(&url) {
            return Ok(url);
        }
    }
    Err(format!(
        "未找到 MySQL {version} 的 macOS arm64 官方包（探测失败）"
    ))
}

// ---------- 安装 ----------

/// 安装 MySQL 版本（下载 → 解压 → 初始化数据目录 → 布局）。
/// 仅 macOS；进度经 `service-progress` 事件推送。
/// `port`/`password`：root 初始密码在初始化后通过 mysql 客户端设置。
#[cfg(target_os = "macos")]
pub fn install(
    app: &tauri::AppHandle,
    kind: ServiceKind,
    version: &str,
    port: Option<u16>,
    password: Option<String>,
) -> Result<(), String> {
    emit(app, kind, version, "downloading", Some(0), "开始下载");

    // 1) 下载（官方 CDN，包名探测）
    let downloads = crate::installer::installs_dir().join("_downloads");
    std::fs::create_dir_all(&downloads).map_err(|e| format!("创建下载目录失败: {e}"))?;
    let archive = downloads.join(format!("mysql-{version}.tar.gz"));
    let url = resolve_download_url(version)?;
    download(app, kind, version, &url, &archive)?;

    // 2) 解压
    let tmp = downloads.join(format!("extract-mysql-{version}"));
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp).map_err(|e| format!("清理临时目录失败: {e}"))?;
    }
    emit(app, kind, version, "extracting", None, "解压中…");
    crate::installer::extract(&archive, &tmp)?;
    let src_dir = first_subdir(&tmp).ok_or("MySQL 包结构异常，请重试")?;

    // 3) 布局：拷贝到安装目录
    emit(app, kind, version, "installing", None, "写入安装目录…");
    let dest = version_dir(version);
    if dest.exists() {
        let _ = std::fs::remove_dir_all(&tmp);
        let _ = std::fs::remove_file(&archive);
        return Err(format!("版本 {version} 已安装"));
    }
    // 目标父目录首次安装时不存在（services/mysql/），必须先创建
    std::fs::create_dir_all(dest.parent().ok_or("安装目录无效")?)
        .map_err(|e| format!("创建安装目录失败: {e}"))?;
    std::fs::rename(&src_dir, &dest).map_err(|e| format!("移动安装目录失败: {e}"))?;

    // 4) 初始化数据目录（root 免密初始化，随后按配置设置密码）
    emit(app, kind, version, "installing", None, "初始化数据目录…");
    let data_dir = data_root().join("mysql").join(version);
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
    let config = ServiceConfig {
        port: port.unwrap_or(DEFAULT_PORT),
        password: password.unwrap_or_default(),
    };
    // 端口冲突检测（避免连到/占用用户已有的 MySQL）
    ensure_port_free(version, config.port)?;
    write_conf(version, &dest, &config)?;

    let init = Command::new(bin_dir(version).join("mysqld"))
        .args([
            "--initialize-insecure",
            &format!("--datadir={}", data_dir.display()),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("初始化失败（无法启动 mysqld）: {e}"))?;
    if !init.success() {
        let _ = std::fs::remove_dir_all(&dest);
        let _ = std::fs::remove_dir_all(&data_dir);
        let _ = std::fs::remove_file(&archive);
        return Err("MySQL 数据目录初始化失败，请查看日志后重试".to_string());
    }

    // 5) 设置 root 密码（用户提供时）：启动临时实例 → ALTER USER → 停止
    //    客户端强制走本实例 socket（绝不连到用户已有的 MySQL）
    if !config.password.is_empty() {
        start(version)?;
        let alter = format!(
            "ALTER USER 'root'@'localhost' IDENTIFIED BY '{}';",
            config.password.replace('\'', "''")
        );
        let ok = Command::new(bin_dir(version).join("mysql"))
            .args([
                "-u",
                "root",
                "--skip-password",
                &format!("--socket={}", socket_file(version).display()),
                "-e",
                &alter,
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        stop(version)?;
        if !ok {
            return Err("设置 root 密码失败，请卸载后重新安装".to_string());
        }
    }

    // 6) 清理
    let _ = std::fs::remove_dir_all(&tmp);
    let _ = std::fs::remove_file(&archive);

    emit(app, kind, version, "done", Some(100), "安装完成");
    Ok(())
}

/// 解压目录中第一个子目录
fn first_subdir(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.is_dir())
}

/// 生成 my.cnf（端口 / 数据目录 / socket / 日志 / pid）
#[cfg(target_os = "macos")]
fn write_conf(version: &str, dest: &Path, config: &ServiceConfig) -> Result<(), String> {
    let data_dir = data_root().join("mysql").join(version);
    let log_file = logs_dir().join(format!("mysql-{version}.log"));
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
    std::fs::create_dir_all(logs_dir()).map_err(|e| format!("创建日志目录失败: {e}"))?;
    std::fs::create_dir_all(run_dir()).map_err(|e| format!("创建运行目录失败: {e}"))?;

    let conf = format!(
        "[mysqld]\nport={}\ndatadir={}\nsocket={}\nlog_error={}\npid-file={}\n",
        config.port,
        data_dir.display(),
        run_dir().join(format!("mysql-{version}.sock")).display(),
        log_file.display(),
        pid_file(version).display(),
    );
    std::fs::write(dest.join("my.cnf"), conf).map_err(|e| format!("写配置文件失败: {e}"))
}

/// 读取版本实际运行配置（端口 / 密码）
#[cfg(target_os = "macos")]
pub fn read_conf(version: &str) -> Option<ServiceConfig> {
    let content = std::fs::read_to_string(conf_file(version)).ok()?;
    Some(parse_conf(&content))
}

/// 解析 my.cnf（纯函数，便于单元测试）：仅解析 [mysqld] 段内的配置
fn parse_conf(content: &str) -> ServiceConfig {
    let mut port = DEFAULT_PORT;
    let password = String::new();
    let mut in_mysqld = false;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            in_mysqld = line.trim_start_matches('[').trim_end_matches(']') == "mysqld";
            continue;
        }
        if !in_mysqld {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() == "port" {
            if let Ok(p) = value.trim().parse::<u16>() {
                port = p;
            }
        }
    }
    // 密码不落盘：通过 mysql 客户端查询
    ServiceConfig { port, password }
}


// ---------- 进程管理 ----------

/// 启动服务：launchd 托管时走 launchctl，否则 mysqld 后台拉起
#[cfg(target_os = "macos")]
pub fn start(version: &str) -> Result<(), String> {
    if autostart_enabled(version) {
        return crate::services::launchd::start("mysql", version);
    }
    let mysqld = bin_dir(version).join("mysqld");
    if !mysqld.is_file() {
        return Err(format!("MySQL {version} 未安装"));
    }
    if is_running(version) {
        return Ok(());
    }
    let conf = conf_file(version);
    if !conf.is_file() {
        return Err("配置文件缺失，请重新安装".to_string());
    }
    let log_file = logs_dir().join(format!("mysql-{version}.log"));
    std::fs::create_dir_all(logs_dir()).map_err(|e| format!("创建日志目录失败: {e}"))?;
    let log = std::fs::File::create(&log_file).map_err(|e| format!("创建日志文件失败: {e}"))?;

    let child = Command::new(&mysqld)
        .args([&format!("--defaults-file={}", conf.display())])
        .stdout(std::process::Stdio::from(
            log.try_clone().map_err(|e| format!("日志文件错误: {e}"))?,
        ))
        .stderr(std::process::Stdio::from(log))
        .spawn()
        .map_err(|e| format!("启动失败: {e}"))?;
    let _ = std::fs::write(pid_file(version), child.id().to_string());

    // 轮询端口就绪（最多 15s，mysqld 启动较慢）
    let port = read_conf(version).map(|c| c.port).unwrap_or(DEFAULT_PORT);
    for _ in 0..75 {
        if is_port_open(port) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Err(format!("MySQL 启动超时（端口 {port} 未就绪），请查看日志"))
}

/// 停止服务：launchd 托管时 bootout，否则 mysqladmin shutdown → SIGTERM 兜底
#[cfg(target_os = "macos")]
pub fn stop(version: &str) -> Result<(), String> {
    if autostart_enabled(version) {
        return crate::services::launchd::stop("mysql", version);
    }
    if !is_running(version) {
        return Ok(());
    }
    let conf = read_conf(version).unwrap_or(ServiceConfig {
        port: DEFAULT_PORT,
        password: String::new(),
    });
    // 优雅关闭（走本实例 socket；有密码时携带）
    let admin = bin_dir(version).join("mysqladmin");
    let mut args: Vec<String> = vec![
        "-u".into(),
        "root".into(),
        format!("--socket={}", socket_file(version).display()),
    ];
    if !conf.password.is_empty() {
        args.push(format!("-p{}", conf.password));
    }
    args.push("shutdown".into());
    let _ = Command::new(&admin).args(&args).status();

    let port = conf.port;
    // 优雅关闭通常 1-2 秒；4 秒（20×200ms）未关闭则 SIGTERM 兜底
    for _ in 0..20 {
        if !is_port_open(port) {
            let _ = std::fs::remove_file(pid_file(version));
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    // 兜底 SIGTERM
    if let Some(pid) = read_pid(version) {
        unsafe { libc::kill(pid as i32, libc::SIGTERM) };
    }
    let _ = std::fs::remove_file(pid_file(version));
    Ok(())
}

/// 重启
#[cfg(target_os = "macos")]
pub fn restart(version: &str) -> Result<(), String> {
    if autostart_enabled(version) {
        return crate::services::launchd::restart("mysql", version);
    }
    let running = is_running(version);
    if running {
        stop(version)?;
        std::thread::sleep(Duration::from_millis(400));
    }
    start(version)
}

// ---------- 开机自启（launchd） ----------

/// 是否已开启自启（plist 存在）
#[cfg(target_os = "macos")]
fn autostart_enabled(version: &str) -> bool {
    crate::services::launchd::plist_path("mysql", version).exists()
}

/// 设置/取消开机自启（launchd 托管：RunAtLoad 自启 + KeepAlive 崩溃拉起）
#[cfg(target_os = "macos")]
pub fn set_autostart(version: &str, enabled: bool) -> Result<(), String> {
    let mysqld = bin_dir(version).join("mysqld");
    if !mysqld.is_file() {
        return Err(format!("MySQL {version} 未安装"));
    }
    if enabled {
        crate::services::launchd::enable(
            "mysql",
            version,
            &[
                mysqld.to_string_lossy().into_owned(),
                format!("--defaults-file={}", conf_file(version).display()),
            ],
            &logs_dir().to_string_lossy(),
        )?;
        // 原方式运行的旧实例交给 launchd 接管
        if is_running(version) && !crate::services::launchd::is_loaded("mysql", version) {
            stop_legacy(version)?;
            crate::services::launchd::start("mysql", version)?;
        }
    } else {
        crate::services::launchd::disable("mysql", version)?;
    }
    Ok(())
}

/// 非 launchd 方式的停止（供接管使用）
#[cfg(target_os = "macos")]
fn stop_legacy(version: &str) -> Result<(), String> {
    if !is_running(version) {
        return Ok(());
    }
    let conf = read_conf(version).unwrap_or(ServiceConfig {
        port: DEFAULT_PORT,
        password: String::new(),
    });
    let admin = bin_dir(version).join("mysqladmin");
    let mut args: Vec<String> = vec![
        "-u".into(),
        "root".into(),
        format!("--socket={}", socket_file(version).display()),
    ];
    if !conf.password.is_empty() {
        args.push(format!("-p{}", conf.password));
    }
    args.push("shutdown".into());
    let _ = Command::new(&admin).args(&args).status();
    let port = conf.port;
    for _ in 0..20 {
        if !is_port_open(port) {
            let _ = std::fs::remove_file(pid_file(version));
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    if let Some(pid) = read_pid(version) {
        unsafe { libc::kill(pid as i32, libc::SIGTERM) };
    }
    let _ = std::fs::remove_file(pid_file(version));
    Ok(())
}

/// 读取服务日志尾部（默认 200 行）
#[cfg(target_os = "macos")]
pub fn tail_log(version: &str, lines: usize) -> Result<String, String> {
    crate::services::tail_log_file(&logs_dir().join(format!("mysql-{version}.log")), lines)
}

/// 修改运行配置（端口 / 密码）：校验端口 → 重写配置 → 运行中自动重启
#[cfg(target_os = "macos")]
pub fn update_config(version: &str, config: &ServiceConfig) -> Result<(), String> {
    let dir = version_dir(version);
    if !dir.join("bin").join("mysqld").is_file() {
        return Err(format!("MySQL {version} 未安装"));
    }
    if config.port == 0 {
        return Err("端口不能为 0".to_string());
    }
    let old = read_conf(version).unwrap_or(ServiceConfig {
        port: DEFAULT_PORT,
        password: String::new(),
    });
    if config.port != old.port {
        ensure_port_free(version, config.port)?;
    }
    write_conf(version, &dir, config)?;
    // 密码变更：运行中通过 ALTER USER 应用
    if config.password != old.password {
        let running = is_running(version);
        if !running {
            start(version)?;
        }
        let cmd = format!(
            "ALTER USER 'root'@'localhost' IDENTIFIED BY '{}';",
            config.password.replace('\'', "''")
        );
        let mut args = vec![
            "-u".to_string(),
            "root".to_string(),
            format!("--socket={}", socket_file(version).display()),
        ];
        if !old.password.is_empty() {
            args.push(format!("-p{}", old.password));
        }
        args.push("-e".to_string());
        args.push(cmd);
        let ok = Command::new(bin_dir(version).join("mysql"))
            .args(&args)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            return Err("修改 root 密码失败（旧密码可能不正确）".to_string());
        }
        if !running {
            stop(version)?;
        }
    }
    // 端口变更：运行中自动重启生效
    if config.port != old.port && is_running(version) {
        restart(version)?;
    }
    Ok(())
}

/// 卸载：停止服务 + 删除程序目录（数据目录保留）
#[cfg(target_os = "macos")]
pub fn uninstall(version: &str) -> Result<(), String> {
    if is_running(version) {
        stop(version)?;
    }
    let dir = version_dir(version);
    if !dir.exists() {
        return Err(format!("MySQL {version} 未安装"));
    }
    std::fs::remove_dir_all(&dir).map_err(|e| format!("卸载失败: {e}"))?;
    Ok(())
}

/// 流式下载并推送进度事件（MB 显示）
#[cfg(target_os = "macos")]
fn download(
    app: &tauri::AppHandle,
    kind: ServiceKind,
    version: &str,
    url: &str,
    dest: &Path,
) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(20))
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
        }
    }
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
    fn parses_my_cnf() {
        let conf = parse_conf(
            "[mysqld]\nport=3307\ndatadir=/x/data\nsocket=/x/run/mysql.sock\nlog_error=/x/log\n",
        );
        assert_eq!(conf.port, 3307);
    }

    #[test]
    fn defaults_when_missing() {
        assert_eq!(parse_conf("[mysqld]\ndatadir=/x\n").port, DEFAULT_PORT);
    }

    #[test]
    fn ignores_section_and_comments() {
        assert_eq!(
            parse_conf("# port=1234\n[client]\nport=9999\n").port,
            DEFAULT_PORT
        );
    }

    #[test]
    fn version_comparison() {
        assert!(cmp_versions("8.4.0", "8.4.0").is_eq());
        assert!(cmp_versions("8.4.1", "8.4.0").is_gt());
        assert!(cmp_versions("9.0.1", "8.4.6").is_gt());
    }

    #[test]
    fn kind_meta() {
        assert_eq!(ServiceKind::MySql.display_name(), "MySQL");
        assert_eq!(ServiceKind::MySql.default_port(), 3306);
    }
}
