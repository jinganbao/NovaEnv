use crate::models::{RuntimeKind, RuntimeVersion};

/// 运行时适配器：每种语言一个实现，负责扫描已安装版本与检测当前默认。
///
/// 新增运行时（如 Python）只需实现本 trait 并注册到 `runtimes::all()`，
/// 前端与切换引擎无需改动。
pub trait RuntimeAdapter: Send + Sync {
    /// 本适配器管理的运行时类型
    fn kind(&self) -> RuntimeKind;

    /// 扫描本机已安装的所有版本（含 is_default 标记）
    fn scan(&self) -> Vec<RuntimeVersion>;

    /// 检测当前实际生效的版本号（用于判定 is_default）
    fn active_version(&self) -> Option<String>;
}
