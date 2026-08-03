//! 平台相关实现：macOS / Windows 通过条件编译分离。

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

use std::path::{Path, PathBuf};

/// 用户主目录（跨平台，兼容未设置 HOME 的 Windows 环境）
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

/// 判断路径是否为目录
pub fn is_dir(path: &std::path::Path) -> bool {
    path.is_dir()
}

/// 构造运行时 bin 可执行文件路径（Windows 需 .exe 后缀）
pub fn bin_path(dir: &Path, name: &str) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        dir.join("bin").join(format!("{name}.exe"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        dir.join("bin").join(name)
    }
}

/// 运行命令并捕获 stdout（成功时返回去尾空白后的文本）
pub fn run_capture(program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .ok()?;
    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}

/// 当前激活配置中的运行时路径列表：
/// - macOS：解析 ~/.zshrc 中 NovaEnv 管理块（JAVA_HOME / NODE_HOME / GOROOT）
/// - Windows：读取用户级环境变量
pub fn active_config_paths() -> Vec<String> {
    #[cfg(target_os = "macos")]
    {
        return macos::active_config_paths();
    }
    #[cfg(target_os = "windows")]
    {
        return windows::active_config_paths();
    }
    #[allow(unreachable_code)]
    Vec::new()
}

/// 判断目录是否被激活配置指向（设置默认后即时生效，不依赖进程 PATH）
pub fn is_active_dir(dir: &Path) -> bool {
    let dir = dir.to_path_buf();
    active_config_paths().iter().any(|p| {
        let pp = PathBuf::from(p);
        dir == pp || dir == pp.parent().unwrap_or(&pp)
    })
}

