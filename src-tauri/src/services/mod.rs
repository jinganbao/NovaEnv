//! 服务类组件聚合：Redis（后续扩展 MySQL、PostgreSQL 等）。
//!
//! 接入步骤：
//! 1. 在 `models::ServiceKind` 增加枚举值；
//! 2. 在 `services/` 新建 `<name>.rs` 实现安装/启停逻辑；
//! 3. 在 `list_all()` 中注册。

pub mod redis;

use crate::models::ServiceInfo;

/// 全部服务组件状态
pub fn list_all() -> Vec<ServiceInfo> {
    vec![redis::info()]
}
