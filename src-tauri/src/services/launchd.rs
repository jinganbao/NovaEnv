//! macOS launchd 辅助：为服务生成 LaunchAgent plist 并托管。
//!
//! 通过 launchd 托管带来：
//! - 开机自启（RunAtLoad）
//! - 崩溃自动拉起（KeepAlive）
//!
//! 每个服务实例一个 plist：
//! `~/Library/LaunchAgents/com.novahub.novaenv.<kind>-<version>.plist`

use std::path::PathBuf;
use std::process::Command;

/// launchd Label 前缀
const LABEL_PREFIX: &str = "com.novahub.novaenv";

/// plist 目录（LaunchAgents）
fn agents_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("Library/LaunchAgents")
}

/// 某服务实例的 Label
pub fn label(kind: &str, version: &str) -> String {
    format!("{LABEL_PREFIX}.{kind}-{version}")
}

/// 某服务实例的 plist 路径
pub fn plist_path(kind: &str, version: &str) -> PathBuf {
    agents_dir().join(format!("{}.plist", label(kind, version)))
}

/// 生成 LaunchAgent plist 内容（RunAtLoad + KeepAlive）
pub fn plist_content(kind: &str, version: &str, program_args: &[String], log_dir: &str) -> String {
    let label = label(kind, version);
    let args_xml: Vec<String> = program_args
        .iter()
        .map(|a| format!("    <string>{}</string>", escape_xml(a)))
        .collect();
    let out = format!("{log_dir}/{kind}-{version}.launchd.out");
    let err = format!("{log_dir}/{kind}-{version}.launchd.err");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
{}
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{out}</string>
    <key>StandardErrorPath</key>
    <string>{err}</string>
    <key>WorkingDirectory</key>
    <string>/</string>
</dict>
</plist>
"#,
        args_xml.join("\n"),
    )
}

/// XML 转义（路径中可能含 & < > "）
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// 写入 plist 并 bootstrap（开启自启）
pub fn enable(
    kind: &str,
    version: &str,
    program_args: &[String],
    log_dir: &str,
) -> Result<(), String> {
    let path = plist_path(kind, version);
    std::fs::create_dir_all(agents_dir())
        .map_err(|e| format!("创建 LaunchAgents 目录失败: {e}"))?;
    std::fs::write(&path, plist_content(kind, version, program_args, log_dir))
        .map_err(|e| format!("写入 plist 失败: {e}"))?;
    // 已加载则先卸载（避免 bootstrap 冲突）
    if is_loaded(kind, version) {
        disable(kind, version)?;
    }
    let status = Command::new("launchctl")
        .args(["bootstrap", "gui/501", path.to_str().unwrap_or_default()])
        .status()
        .map_err(|e| format!("launchctl 不可用: {e}"))?;
    if !status.success() {
        // gui/501 的 uid 硬编码在部分系统不匹配，改用当前 uid
        let uid = unsafe { libc::getuid() };
        let status = Command::new("launchctl")
            .args([
                "bootstrap",
                &format!("gui/{uid}"),
                path.to_str().unwrap_or_default(),
            ])
            .status()
            .map_err(|e| format!("launchctl 不可用: {e}"))?;
        if !status.success() {
            let _ = std::fs::remove_file(&path);
            return Err("launchctl bootstrap 失败".to_string());
        }
    }
    Ok(())
}

/// 停止并卸载（关闭自启 + 停止服务）
pub fn disable(kind: &str, version: &str) -> Result<(), String> {
    let label = label(kind, version);
    let uid = unsafe { libc::getuid() };
    let _ = Command::new("launchctl")
        .args(["bootout", &format!("gui/{uid}/{label}")])
        .status();
    let _ = std::fs::remove_file(plist_path(kind, version));
    Ok(())
}

/// 是否已加载（launchd 托管中）
pub fn is_loaded(kind: &str, version: &str) -> bool {
    let label = label(kind, version);
    let uid = unsafe { libc::getuid() };
    Command::new("launchctl")
        .args(["print", &format!("gui/{uid}/{label}")])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// 通过 launchd 启动（未加载则 bootstrap；已加载则 kickstart 重启）
pub fn start(kind: &str, version: &str) -> Result<(), String> {
    let label = label(kind, version);
    let uid = unsafe { libc::getuid() };
    let path = plist_path(kind, version);
    if !is_loaded(kind, version) {
        let status = Command::new("launchctl")
            .args([
                "bootstrap",
                &format!("gui/{uid}"),
                path.to_str().unwrap_or_default(),
            ])
            .status()
            .map_err(|e| format!("launchctl 不可用: {e}"))?;
        if !status.success() {
            return Err("launchctl bootstrap 失败".to_string());
        }
        return Ok(());
    }
    // 已加载 → kickstart 拉起进程
    let status = Command::new("launchctl")
        .args(["kickstart", &format!("gui/{uid}/{label}")])
        .status()
        .map_err(|e| format!("launchctl 不可用: {e}"))?;
    if !status.success() {
        return Err("launchctl kickstart 失败".to_string());
    }
    Ok(())
}

/// 通过 launchd 停止（不卸载 plist，KeepAlive 会阻止纯 kill）
pub fn stop(kind: &str, version: &str) -> Result<(), String> {
    let label = label(kind, version);
    let uid = unsafe { libc::getuid() };
    // 移除 KeepAlive 后再停：先用 bootout 卸载，之后 start 会重新 bootstrap
    let _ = Command::new("launchctl")
        .args(["bootout", &format!("gui/{uid}/{label}")])
        .status();
    Ok(())
}

/// 重启（-k：先终止再启动）
pub fn restart(kind: &str, version: &str) -> Result<(), String> {
    let label = label(kind, version);
    let uid = unsafe { libc::getuid() };
    if !is_loaded(kind, version) {
        return start(kind, version);
    }
    let status = Command::new("launchctl")
        .args(["kickstart", "-k", &format!("gui/{uid}/{label}")])
        .status()
        .map_err(|e| format!("launchctl 不可用: {e}"))?;
    if !status.success() {
        return Err("launchctl kickstart 失败".to_string());
    }
    Ok(())
}
