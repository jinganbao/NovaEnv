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
    #[cfg(target_os = "linux")]
    {
        Err("当前平台暂不支持切换默认版本（支持 macOS / Windows）".to_string())
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
    #[cfg(target_os = "linux")]
    {
        Err("当前平台暂不支持切换默认版本（支持 macOS / Windows）".to_string())
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
        RuntimeKind::Maven => vec![
            format!("export MAVEN_HOME=\"{}\"", version.path),
            "export PATH=\"$MAVEN_HOME/bin:$PATH\"".to_string(),
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
    Ok(ActivationPreview {
        config_file: Some(config.to_string_lossy().into_owned()),
        lines: shell_lines(version),
        backup_path: None,
        note: "切换后自动执行 source ~/.zshrc 刷新 shell 配置。".to_string(),
    })
}

#[cfg(target_os = "macos")]
fn activate_macos(version: &RuntimeVersion) -> Result<(), String> {
    let config = zshrc_path();
    let lines = shell_lines(version);

    // 1) 读取现有内容（文件不存在视为空）
    let existing = std::fs::read_to_string(&config).unwrap_or_default();

    // 2) 按运行时粒度更新 NovaEnv 管理块（Java/Node/Go 默认互不覆盖）
    let new_content = upsert_managed_block(&existing, &lines, version.kind)?;

    // 3) 写回（临时文件 + 原子替换，避免写入中断损坏配置）
    let tmp = config.with_extension("zshrc.tmp");
    std::fs::write(&tmp, &new_content).map_err(|e| format!("写入配置失败: {e}"))?;
    std::fs::rename(&tmp, &config).map_err(|e| format!("替换配置失败: {e}"))?;

    // 4) 同步当前进程环境，立即生效
    for line in &lines {
        if let Some((k, v)) = parse_export(line) {
            std::env::set_var(k, v);
        }
    }
    if let Some(home_line) = lines.first() {
        if let Some((_k, v)) = parse_export(home_line) {
            let bin = std::path::Path::new(&v).join("bin");
            if let Some(path) = std::env::var_os("PATH") {
                let mut paths: Vec<_> = std::env::split_paths(&path).collect();
                paths.insert(0, bin);
                if let Ok(new_path) = std::env::join_paths(paths) {
                    std::env::set_var("PATH", new_path);
                }
            }
        }
    }

    // 5) 当前进程环境已由步骤 4 同步；新终端启动时自动读取新配置
    Ok(())
}

/// 解析 `export K="V"` 行
fn parse_export(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("export ")?;
    let (k, v) = rest.split_once('=')?;
    Some((
        k.trim().to_string(),
        v.trim().trim_matches('"').trim_matches('\'').to_string(),
    ))
}

/// 更新 .zshrc 中的 NovaEnv 管理块：
/// - 块内已存在目标运行时的行 → 替换为新的两行
/// - 块内不存在 → 追加到块尾（其他运行时的默认保留）
/// - 无块 → 创建新块
#[cfg(target_os = "macos")]
fn upsert_managed_block(
    existing: &str,
    lines: &[String],
    kind: RuntimeKind,
) -> Result<String, String> {
    let var = kind.env_var_name();
    let home_prefix = format!("export {var}=");
    let path_prefix = format!("export PATH=\"${var}/bin");

    let Some(start) = existing.find(BLOCK_START) else {
        // 无管理块 → 在文件末尾追加
        let block = format!("{BLOCK_START}\n{}\n{BLOCK_END}\n", lines.join("\n"));
        return Ok(format!("{existing}\n{block}"));
    };

    let rest = &existing[start..];
    let end_offset = rest
        .find(BLOCK_END)
        .ok_or("~/.zshrc 中的 NovaEnv 管理块缺少结束标记，请手动检查该文件")?;
    let end = start + end_offset + BLOCK_END.len();
    let body = &existing[start + BLOCK_START.len()..end - BLOCK_END.len()];

    // 保留块内非目标运行时的行，剔除目标运行时的旧行
    let mut kept: Vec<&str> = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with(&home_prefix) || line.starts_with(&path_prefix) {
            continue; // 目标运行时旧行，将被替换
        }
        kept.push(line);
    }
    // 追加目标运行时新行
    kept.extend(lines.iter().map(String::as_str));

    let block = format!("{BLOCK_START}\n{}\n{BLOCK_END}\n", kept.join("\n"));
    Ok(format!(
        "{}{}{}",
        &existing[..start],
        block,
        &existing[end..]
    ))
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
