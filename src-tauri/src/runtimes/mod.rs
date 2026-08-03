//! 运行时适配器聚合：JDK / Node / Go。
//!
//! 新增语言（如 Python、Rust）的接入步骤：
//! 1. 在 `models::RuntimeKind` 增加枚举值；
//! 2. 在 `runtimes/` 新建 `<name>.rs` 实现 `RuntimeAdapter`；
//! 3. 在 `all()` 中注册；
//! 4. 在 `overview()` 中补充概览字段。

pub mod go;
pub mod java;
pub mod maven;
pub mod node;

use crate::adapter::RuntimeAdapter;
use crate::models::{RuntimeKind, RuntimeOverview, RuntimesPayload};

/// 全部运行时适配器
pub fn all() -> Vec<Box<dyn RuntimeAdapter>> {
    vec![
        Box::new(java::JavaAdapter),
        Box::new(node::NodeAdapter),
        Box::new(go::GoAdapter),
        Box::new(maven::MavenAdapter),
    ]
}

/// 扫描全部运行时，返回概览 + 完整版本列表
pub fn scan_all() -> Result<RuntimesPayload, String> {
    let mut versions = Vec::new();
    for adapter in all() {
        versions.extend(adapter.scan());
    }
    Ok(RuntimesPayload {
        overview: overview(),
        versions,
    })
}

/// 收集当前生效环境的概览信息
fn overview() -> RuntimeOverview {
    let mut overview = RuntimeOverview::default();
    for adapter in all() {
        let active = adapter.active_version();
        match adapter.kind() {
            RuntimeKind::Java => overview.java = active,
            RuntimeKind::Node => overview.node = active,
            RuntimeKind::Go => overview.go = active,
            RuntimeKind::Maven => overview.maven = active,
        }
    }
    overview.java_home = std::env::var("JAVA_HOME").ok();
    overview
}
