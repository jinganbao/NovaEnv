//! Windows 平台辅助：JDK 扫描依赖注册表 `HKLM\SOFTWARE\JavaSoft`。

use winreg::enums::HKEY_LOCAL_MACHINE;
use winreg::RegKey;

/// 查询注册表 JavaSoft 键下的 JDK 安装信息。
///
/// 返回 (安装路径, 版本号, 厂商) 列表。
/// 注册表结构：`HKLM\SOFTWARE\JavaSoft\JDK\<version>\JavaHome`
/// 同时检查 64 位与 WOW6432Node（32 位视图）两处，避免遗漏。
pub fn java_homes_from_registry() -> Vec<(String, String, String)> {
    let mut result = Vec::new();
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    let mut roots = Vec::new();
    if let Ok(root) = hklm.open_subkey("SOFTWARE\\JavaSoft") {
        roots.push(root);
    }
    if let Ok(root) = hklm.open_subkey("SOFTWARE\\WOW6432Node\\JavaSoft") {
        roots.push(root);
    }

    for root in roots {
        for family in ["JDK", "Java Development Kit"] {
            let Ok(jdk_key) = root.open_subkey(family) else {
                continue;
            };
            // winreg 0.55：enum_keys() 直接返回迭代器，每项为 io::Result<String>
            for version_entry in jdk_key.enum_keys().flatten() {
                let Ok(vkey) = jdk_key.open_subkey(&version_entry) else {
                    continue;
                };
                let Ok(home) = vkey.get_value::<String, _>("JavaHome") else {
                    continue;
                };
                let vendor = vkey
                    .get_value::<String, _>("Vendor")
                    .unwrap_or_else(|_| "Unknown".to_string());
                result.push((home, version_entry, vendor));
            }
        }
    }
    result
}

/// 读取用户级环境变量中的激活路径
/// （NovaEnv 通过 `[Environment]::SetEnvironmentVariable(name, value, 'User')` 写入）。
/// 进程环境为系统级 + 用户级合并视图，重启应用后即可读到。
pub fn active_config_paths() -> Vec<String> {
    ["JAVA_HOME", "NODE_HOME", "GOROOT", "MAVEN_HOME", "PYTHON_HOME"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .filter(|p| !p.is_empty())
        .collect()
}
