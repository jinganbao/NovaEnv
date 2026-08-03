//! 切换默认环境引擎。
//!
//! macOS：幂等更新 `~/.zshrc`（NovaEnv 管理块替换/追加，写入前自动备份）。
//! Windows：PowerShell 写入用户级环境变量（`[Environment]::SetEnvironmentVariable`，
//! 规避 setx 的 1024 字符 PATH 截断问题）。
//!
//! 流程：前端先调 `preview_activation` 展示变更预览，用户确认后调 `activate` 执行。

use serde::Serialize;

use crate::models::{RuntimeKind, RuntimeVersion};

#[cfg(target_os = "macos")]
use crate::platform;
#[cfg(target_os = "macos")]
use std::path::PathBuf;

/// 切换操作的变更预览（不执行任何写入）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationPreview {
    /// 将要修改的配置文件路径（macOS 为 ~/.zshrc；Windows 无此概念）
    pub config_file: Option<String>,
    /// 将要写入的配置行（展示给用户确认）
    pub lines: Vec<String>,
    /// 备份文件路径（macOS 写入前生成）
    pub backup_path: Option<String>,
    /// 平台说明
    pub note: String,
}

/// 生成切换预览
pub fn preview(version: &RuntimeVersion) -> Result<ActivationPreview, String> {
    validate(version)?;
    #[cfg(target_os = "macos")]
    {
        preview_macos(version)
    }
    #[cfg(target_os = "windows")]
    {
        preview_windows(version)
    }
}

/// 执行切换（写入配置）
pub fn activate(version: &RuntimeVersion) -> Result<(), String> {
    validate(version)?;
    #[cfg(target_os = "macos")]
    {
        activate_macos(version)
    }
    #[cfg(target_os = "windows")]
    {
        activate_windows(version)
    }
}

/// 切换前置校验：安装路径必须存在
fn validate(version: &RuntimeVersion) -> Result<(), String> {
    let path = std::path::Path::new(&version.path);
    if !path.is_dir() {
        return Err(format!("安装路径不存在: {}", version.path));
    }
    Ok(())
}

/// 依据运行时类型生成 shell 配置行
fn shell_lines(version: &RuntimeVersion) -> Vec<String> {
    match version.kind {
        RuntimeKind::Java => vec![
            format!("export JAVA_HOME=\"{}\"", version.path),
            "export PATH=\"$JAVA_HOME/bin:$PATH\"".to_string(),
        ],
        RuntimeKind::Node => vec![
            format!("export NODE_HOME=\"{}\"", version.path),
            "export PATH=\"$NODE_HOME/bin:$PATH\"".to_string(),
        ],
        RuntimeKind::Go => vec![
            format!("export GOROOT=\"{}\"", version.path),
            "export PATH=\"$GOROOT/bin:$PATH\"".to_string(),
        ],
    }
}

// ---------- macOS ----------

const BLOCK_START: &str = "# >>> NovaEnv managed >>>";
const BLOCK_END: &str = "# <<< NovaEnv managed <<<";

#[cfg(target_os = "macos")]
fn zshrc_path() -> PathBuf {
    platform::home_dir()
        .map(|home| home.join(".zshrc"))
        .unwrap_or_else(|| PathBuf::from(".zshrc"))
}

#[cfg(target_os = "macos")]
fn preview_macos(version: &RuntimeVersion) -> Result<ActivationPreview, String> {
    let config = zshrc_path();
    let backup = PathBuf::from(format!("{}.novaenv.bak", config.display()));
    Ok(ActivationPreview {
        config_file: Some(config.to_string_lossy().into_owned()),
        lines: shell_lines(version),
        backup_path: Some(backup.to_string_lossy().into_owned()),
        note: "切换后请重新打开终端（或执行 source ~/.zshrc）生效；原配置已备份。".to_string(),
    })
}

#[cfg(target_os = "macos")]
fn activate_macos(version: &RuntimeVersion) -> Result<(), String> {
    let config = zshrc_path();
    let backup = PathBuf::from(format!("{}.novaenv.bak", config.display()));
    let lines = shell_lines(version);

    // 1) 读取现有内容（文件不存在视为空）
    let existing = std::fs::read_to_string(&config).unwrap_or_default();

    // 2) 写入前备份
    std::fs::write(&backup, &existing).map_err(|e| format!("备份失败（{}）: {e}", backup.display()))?;

    // 3) 构造 NovaEnv 管理块并替换/追加
    let block = format!("{BLOCK_START}\n{}\n{BLOCK_END}\n", lines.join("\n"));
    let new_content = if let Some(start) = existing.find(BLOCK_START) {
        let rest = &existing[start..];
        let end_offset = rest
            .find(BLOCK_END)
            .ok_or("~/.zshrc 中的 NovaEnv 管理块缺少结束标记，请手动检查该文件")?;
        let end = start + end_offset + BLOCK_END.len();
        format!("{}{}{}", &existing[..start], block, &existing[end..])
    } else {
        let mut content = existing;
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(&block);
        content
    };

    // 4) 写回
    std::fs::write(&config, new_content).map_err(|e| format!("写入失败（{}）: {e}", config.display()))?;
    Ok(())
}

// ---------- Windows ----------

#[cfg(target_os = "windows")]
fn preview_windows(version: &RuntimeVersion) -> Result<ActivationPreview, String> {
    let var = version.kind.env_var_name();
    Ok(ActivationPreview {
        config_file: None,
        lines: vec![
            format!("set {var} = \"{}\"（用户级）", version.path),
            format!("PATH 前置 {}\\bin（用户级）", version.path),
        ],
        backup_path: None,
        note: "通过 PowerShell 写入用户级环境变量，重新打开终端后生效。".to_string(),
    })
}

/// PowerShell 单引号字符串转义
#[cfg(target_os = "windows")]
fn ps_escape(s: &str) -> String {
    s.replace('\'', "''")
}

#[cfg(target_os = "windows")]
fn activate_windows(version: &RuntimeVersion) -> Result<(), String> {
    let var = version.kind.env_var_name();
    let path = ps_escape(&version.path);
    // 单条命令完成：设置 HOME 变量 + PATH 前置 bin（含幂等判断）
    let script = format!(
        "[Environment]::SetEnvironmentVariable('{var}', '{path}', 'User'); \
         $bin = '{path}\\bin'; \
         $p = [Environment]::GetEnvironmentVariable('Path', 'User'); \
         if ($p -notmatch [regex]::Escape($bin)) {{ [Environment]::SetEnvironmentVariable('Path', ($bin + ';' + $p), 'User') }}",
        var = var,
        path = path
    );
    let status = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()
        .map_err(|e| format!("无法启动 PowerShell: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("PowerShell 执行失败: {status:?}"))
    }
}
