use serde::{Deserialize, Serialize};

/// 运行时类型 —— 新增语言（如 Python、Rust）时在此扩展枚举，
/// 并在 `runtimes` 模块新增对应适配器即可接入。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeKind {
    Java,
    Node,
    Go,
    Maven,
}

impl RuntimeKind {
    /// 界面展示用名称（前端有对应元数据；保留供后续扩展使用）
    #[allow(dead_code)]
    pub fn display_name(self) -> &'static str {
        match self {
            RuntimeKind::Java => "Java",
            RuntimeKind::Node => "Node.js",
            RuntimeKind::Go => "Go",
            RuntimeKind::Maven => "Maven",
        }
    }

    /// 对应环境变量名（切换时写入；Windows 构建使用，macOS 保留备用）
    #[allow(dead_code)]
    pub fn env_var_name(self) -> &'static str {
        match self {
            RuntimeKind::Java => "JAVA_HOME",
            RuntimeKind::Node => "NODE_HOME",
            RuntimeKind::Go => "GOROOT",
            RuntimeKind::Maven => "MAVEN_HOME",
        }
    }
}

/// 单个已安装的运行时版本
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeVersion {
    pub kind: RuntimeKind,
    /// 版本号（如 17.0.10 / 22.11.0 / 1.23.4）
    pub version: String,
    /// 发行版 / 来源（如 Temurin、OpenJDK、nvm、brew、官方安装）
    pub vendor: String,
    /// 安装根目录（JDK 即 JAVA_HOME，Node 为安装目录，Go 即 GOROOT）
    pub path: String,
    /// 是否为当前默认版本
    pub is_default: bool,
    /// 是否为 NovaEnv 管理目录（~/.novaenv/installs）安装的版本（可卸载）
    pub managed: bool,
}

/// 可安装的版本（来自官方发行源，内部使用）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableVersion {
    /// 版本标识（Java 为具体版本如 21.0.6+7；Node/Go 如 22.11.0 / 1.23.4）
    pub version: String,
    /// 是否为 LTS（Java 的 LTS / Node 的 LTS 标记；Go 无此概念恒为 false）
    pub is_lts: bool,
}

/// 按大版本分组的可安装版本（前端列表展示；可反序列化用于磁盘缓存）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableVersionGroup {
    /// 大版本标识（Java/Node 为第一段如 21 / 24；Go 为前两段如 1.24）
    pub major: String,
    /// 该大版本是否为 LTS
    pub is_lts: bool,
    /// 该大版本下的具体版本（最新在前）
    pub versions: Vec<String>,
    /// 该大版本最新版本号
    pub latest: String,
}

/// 安装请求
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRequest {
    pub kind: RuntimeKind,
    /// Java 为 feature 版本（如 21）；Node/Go 为具体版本号
    pub version: String,
}

/// 安装结果：同大版本旧版本替换信息
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    /// 被自动替换移除的旧版本号列表
    pub removed: Vec<String>,
    /// 旧版本曾是默认，已自动把新版本设为默认
    pub promoted: bool,
}

/// 安装进度事件（通过 tauri 事件 `install-progress` 推送）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgress {
    pub kind: RuntimeKind,
    pub version: String,
    /// 阶段：downloading / extracting / installing / done / error
    pub stage: String,
    /// 下载进度百分比（0-100），非下载阶段为 None
    pub percent: Option<u32>,
    pub message: String,
}

/// 概览：当前生效的环境信息
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOverview {
    /// 当前生效的版本号（来自实际检测，如 `java -version` 输出）
    pub java: Option<String>,
    pub node: Option<String>,
    pub go: Option<String>,
    pub maven: Option<String>,
    /// 当前 JAVA_HOME 环境变量值
    pub java_home: Option<String>,
}

/// `list_runtimes` 命令的返回负载
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimesPayload {
    pub overview: RuntimeOverview,
    pub versions: Vec<RuntimeVersion>,
}

/// 单个运行时在管理目录中的已安装版本
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedRuntimeInfo {
    pub kind: RuntimeKind,
    /// 已安装的版本号（排序后）
    pub versions: Vec<String>,
}

/// 管理目录信息（设置页展示）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManageInfo {
    /// 管理目录路径（~/.novaenv/installs）
    pub path: String,
    /// 已管理版本总数
    pub version_count: usize,
    /// 占用磁盘空间（字节）
    pub size_bytes: u64,
    /// 各运行时的管理版本
    pub runtimes: Vec<ManagedRuntimeInfo>,
}
