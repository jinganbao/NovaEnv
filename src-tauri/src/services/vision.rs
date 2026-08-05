//! Vision MCP 服务（AI 视觉能力）
//!
//! 提供 MCP stdio server（FastMCP + 智谱 GLM-4.6V-Flash）：
//! Reasonix / Cursor 等 MCP 客户端可调用其 analyze_image / ocr / describe 工具，
//! 让无视觉能力的模型（如 DeepSeek）也能「看图」。
//!
//! 首次启动自动完成：
//! 1. 从应用资源/仓库复制 `mcp-vision/` → `~/.novaenv/vision-mcp/`
//! 2. 创建 venv 并安装依赖（fastmcp / openai / Pillow）
//! 3. 后台启动 server.py（stdio，写 pid 与日志文件）

use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(target_os = "macos")]
use std::time::Duration;

use serde::Serialize;

/// Vision 服务状态（前端展示）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisionInfo {
    pub running: bool,
    pub pid: Option<u32>,
    pub log_file: String,
    pub python: Option<String>,
    pub deps_ready: bool,
}

/// 服务根目录 ~/.novaenv/vision-mcp
fn base_dir() -> PathBuf {
    crate::installer::installs_dir().parent().unwrap().join("vision-mcp")
}

fn venv_python() -> PathBuf {
    base_dir().join("venv/bin/python")
}

fn server_file() -> PathBuf {
    base_dir().join("server.py")
}

fn pid_file() -> PathBuf {
    crate::installer::installs_dir().parent().unwrap().join("run").join("vision-mcp.pid")
}

/// venv 的 Python 是否可用且版本 >= 3.10（防止旧版残留 3.9 venv 反复失败）
fn venv_ok() -> bool {
    let py = venv_python();
    if !py.is_file() {
        return false;
    }
    let Ok(out) = Command::new(&py).arg("--version").output() else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    let ver = String::from_utf8_lossy(&out.stdout);
    let v = ver.split_whitespace().nth(1).unwrap_or("0");
    let mut parts = v.split('.');
    let major: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let minor: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    major > 3 || (major == 3 && minor >= 10)
}

fn log_file() -> PathBuf {
    crate::installer::installs_dir().parent().unwrap().join("logs").join("vision-mcp.log")
}

/// NovaEnv 自管 Python（~/.novaenv/installs/python/<ver>/bin/python3），取最高版本
fn novaenv_python() -> Option<String> {
    let dir = crate::installer::installs_dir().join("python");
    let mut best: Option<(String, String)> = None;
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return None;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        let py = p.join("bin").join("python3");
        if !py.is_file() {
            continue;
        }
        let v = entry.file_name().to_string_lossy().into_owned();
        let better = best
            .as_ref()
            .is_none_or(|(bv, _)| crate::installer::compare_versions(&v, bv).is_gt());
        if better {
            best = Some((v, py.to_string_lossy().into_owned()));
        }
    }
    best.map(|(_, p)| p)
}

/// 找到的 Python 3.10+（NovaEnv 自管优先，再 Homebrew 绝对路径，回退 PATH 候选；返回绝对路径）
fn find_python() -> Option<String> {
    if let Some(p) = novaenv_python() {
        return Some(p);
    }
    const CANDIDATES: &[&str] = &[
        "/opt/homebrew/bin/python3.14",
        "/opt/homebrew/bin/python3.13",
        "/opt/homebrew/bin/python3.12",
        "/opt/homebrew/bin/python3.11",
        "/opt/homebrew/bin/python3.10",
        "/opt/homebrew/bin/python3",
        "/usr/local/bin/python3",
        "python3.14",
        "python3.13",
        "python3.12",
        "python3.11",
        "python3.10",
        "python3",
    ];
    for name in CANDIDATES {
        let Ok(out) = Command::new(name).arg("--version").output() else {
            continue;
        };
        if !out.status.success() {
            continue;
        }
        let ver = String::from_utf8_lossy(&out.stdout);
        let v = ver.split_whitespace().nth(1).unwrap_or("0");
        let mut parts = v.split('.');
        let major: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        let minor: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        if major > 3 || (major == 3 && minor >= 10) {
            // 解析绝对路径（which；绝对路径候选直接用）
            if name.starts_with('/') {
                return Some((*name).to_string());
            }
            if let Ok(w) = Command::new("which").arg(name).output() {
                let p = String::from_utf8_lossy(&w.stdout).trim().to_string();
                if !p.is_empty() {
                    return Some(p);
                }
            }
            return Some((*name).to_string());
        }
    }
    None
}

/// pid 进程是否存活
fn pid_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

fn read_pid() -> Option<u32> {
    std::fs::read_to_string(pid_file()).ok()?.trim().parse().ok()
}

/// 当前状态
pub fn info() -> VisionInfo {
    let pid = read_pid();
    let running = pid.is_some_and(|p| pid_alive(p));
    VisionInfo {
        running,
        pid: running.then_some(pid.unwrap_or(0)),
        log_file: log_file().to_string_lossy().into_owned(),
        python: find_python(),
        deps_ready: venv_python().is_file(),
    }
}

/// 复制 mcp-vision 源文件到管理目录（source_dir：打包资源或 dev 仓库目录）
fn deploy_sources(source_dir: &Path) -> Result<(), String> {
    if !source_dir.join("server.py").is_file() {
        return Err("未找到 mcp-vision/server.py（资源或仓库目录均缺失）".to_string());
    }
    let target = base_dir();
    std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
    for name in ["server.py", "vision.py", "requirements.txt"] {
        std::fs::copy(source_dir.join(name), target.join(name))
            .map_err(|e| format!("复制 {name} 失败: {e}"))?;
    }
    Ok(())
}

/// 创建 venv 并安装依赖（首次或 venv 损坏时；python 为解析出的 Python 3.10+ 绝对路径）
fn ensure_deps(source_dir: &Path, python: &str) -> Result<(), String> {
    let dir = base_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {e}"))?;
    if !server_file().is_file() {
        deploy_sources(source_dir)?;
    }
    let venv = dir.join("venv");
    if !venv_ok() {
        // 重建：清理异常/旧版残留（如 Python 3.9 建的 venv 装不上 fastmcp）
        if venv.exists() {
            let _ = std::fs::remove_dir_all(&venv);
        }
        let status = Command::new(python)
            .args(["-m", "venv", venv.to_str().unwrap()])
            .status()
            .map_err(|e| format!("python3 -m venv 失败: {e}"))?;
        if !status.success() {
            return Err("创建 Python venv 失败（请确认已安装 Python 3.10+）".to_string());
        }
    }
    if !venv_ok() {
        return Err("venv 不可用（Python 版本过低或损坏），请检查 Python 3.10+".to_string());
    }
    let req = dir.join("requirements.txt");
    if !req.is_file() {
        deploy_sources(source_dir)?;
    }
    // 检查 fastmcp 是否可用
    let check = Command::new(venv_python())
        .arg("-c")
        .arg("import fastmcp")
        .output()
        .map_err(|e| e.to_string())?;
    if !check.status.success() {
        // 国内镜像多级回退（实测清华 packages 下载 403，阿里云稳定），最后官方源
        let mut last_err = "未知错误".to_string();
        for index in [
            Some("https://mirrors.aliyun.com/pypi/simple"),
            Some("https://mirrors.cloud.tencent.com/pypi/simple"),
            Some("https://pypi.tuna.tsinghua.edu.cn/simple"),
            None, // 官方源
        ] {
            let mut cmd = Command::new(venv_python());
            cmd.args(["-m", "pip", "install", "-q"]);
            if let Some(url) = index {
                cmd.args(["-i", url]);
            }
            cmd.args(["-r", req.to_str().unwrap()]);
            let out = cmd.output().map_err(|e| format!("pip install 失败: {e}"))?;
            if out.status.success() {
                last_err.clear();
                break;
            }
            last_err = String::from_utf8_lossy(&out.stderr)
                .lines()
                .rev()
                .take(6)
                .collect::<Vec<_>>()
                .join(" | ");
        }
        if !last_err.is_empty() {
            // 失败详情写入日志文件，面板「查看日志」可查
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_file())
            {
                use std::io::Write;
                let _ = writeln!(f, "[{}] 依赖安装失败: {last_err}", chrono::Local::now().format("%H:%M:%S"));
            }
            return Err(format!(
                "安装依赖失败（fastmcp/openai/Pillow）：{last_err}\n（已尝试阿里云/腾讯云/清华/官方源，详情见日志）"
            ));
        }
    }
    Ok(())
}

/// 启动 Vision MCP 服务（source_dir：mcp-vision 源目录）
pub fn start(source_dir: &Path, api_key: Option<String>) -> Result<(), String> {
    if info().running {
        return Ok(());
    }
    let python = find_python()
        .ok_or_else(|| "未找到 Python 3.10+（已尝试 Homebrew 与 PATH 中的 python3.10~3.14）".to_string())?;
    ensure_deps(source_dir, &python)?;
    let key = match api_key {
        Some(k) if !k.trim().is_empty() => k.trim().to_string(),
        _ => return Err("请先填写智谱 API Key".to_string()),
    };

    let dir = base_dir();
    let logs = crate::installer::installs_dir().parent().unwrap().join("logs");
    std::fs::create_dir_all(&logs).map_err(|e| e.to_string())?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file())
        .map_err(|e| e.to_string())?;

    // stdio server：后台运行，日志写文件
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::process::CommandExt;
        let child = Command::new(venv_python())
            .arg(dir.join("server.py"))
            .env("ZHIPU_API_KEY", &key)
            .env("VISION_MODEL", "glm-4.6v-flash")
            .current_dir(&dir)
            .stdout(std::process::Stdio::from(log.try_clone().map_err(|e| e.to_string())?))
            .stderr(std::process::Stdio::from(log))
            .process_group(0)
            .spawn()
            .map_err(|e| format!("启动 Vision 服务失败: {e}"))?;
        let pid = child.id();
        let run = crate::installer::installs_dir().parent().unwrap().join("run");
        std::fs::create_dir_all(&run).map_err(|e| e.to_string())?;
        std::fs::write(pid_file(), pid.to_string()).map_err(|e| e.to_string())?;
        // 子进程脱离父进程生命周期
        std::mem::forget(child);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let mut child = Command::new(venv_python())
            .arg(dir.join("server.py"))
            .env("ZHIPU_API_KEY", &key)
            .env("VISION_MODEL", "glm-4.6v-flash")
            .current_dir(&dir)
            .stdout(std::process::Stdio::from(log.try_clone().map_err(|e| e.to_string())?))
            .stderr(std::process::Stdio::from(log))
            .spawn()
            .map_err(|e| format!("启动 Vision 服务失败: {e}"))?;
        let pid = child.id();
        let run = crate::installer::installs_dir().parent().unwrap().join("run");
        std::fs::create_dir_all(&run).map_err(|e| e.to_string())?;
        std::fs::write(pid_file(), pid.to_string()).map_err(|e| e.to_string())?;
        let _ = child; // 非 macOS 不脱离，随父进程
    }
    Ok(())
}

/// 停止 Vision MCP 服务
pub fn stop() -> Result<(), String> {
    let Some(pid) = read_pid() else {
        return Ok(());
    };
    if pid_alive(pid) {
        unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        #[cfg(target_os = "macos")]
        for _ in 0..20 {
            if !pid_alive(pid) {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        if pid_alive(pid) {
            unsafe { libc::kill(pid as i32, libc::SIGKILL) };
        }
    }
    let _ = std::fs::remove_file(pid_file());
    Ok(())
}

/// 服务日志尾部（供面板查看）
pub fn logs() -> Result<String, String> {
    crate::services::tail_log_file(&log_file(), 100)
}
