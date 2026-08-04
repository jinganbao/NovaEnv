// 与 Rust 后端 models.rs 对应的类型定义（serde camelCase 契约）

export type RuntimeKind = "java" | "node" | "go" | "maven" | "python" | "rust";

/** 服务类组件类型 */
export type ServiceKind = "redis" | "mysql";

/** 服务状态信息（list_services 返回） */
export interface ServiceInfo {
  kind: ServiceKind;
  /** 展示名称（如 "Redis"） */
  name: string;
  /** 是否已安装（任一版本） */
  installed: boolean;
  /** 已安装版本（最新版本；未安装为 null） */
  version: string | null;
  /** 全部已安装版本（多版本列表） */
  versions: ServiceVersionInfo[];
  /** 是否运行中（最新版本） */
  running: boolean;
  /** 服务端口（最新版本） */
  port: number;
  /** 进程 PID（最新版本运行中时） */
  pid: number | null;
  /** 当前访问密码（最新版本；空表示未设置） */
  password: string;
  /** 是否开启开机自启（最新版本） */
  autostart: boolean;
  /** 数据目录（最新版本） */
  dataDir: string;
  /** 平台支持说明（如 Windows 暂不支持） */
  note: string | null;
}

/** 单个已安装服务版本的状态（多版本列表项） */
export interface ServiceVersionInfo {
  version: string;
  running: boolean;
  port: number;
  autostart: boolean;
}

/** 服务运行配置（端口 / 密码），用于安装与修改 */
export interface ServiceConfig {
  port: number;
  /** 空字符串表示无密码 */
  password: string;
  /** 当前密码（MySQL 修改密码时用于认证；Redis 忽略） */
  oldPassword?: string;
}

/** 服务安装进度事件 */
export interface ServiceProgress {
  kind: ServiceKind;
  version: string;
  /** downloading / compiling / installing / done / error */
  stage: string;
  percent: number | null;
  message: string;
}

export interface RuntimeVersion {
  kind: RuntimeKind;
  /** 版本号，如 17.0.10 / 22.11.0 / 1.23.4 */
  version: string;
  /** 发行版 / 来源，如 Temurin、nvm、homebrew、novaenv */
  vendor: string;
  /** 安装根目录（JAVA_HOME / Node 安装目录 / GOROOT） */
  path: string;
  /** 是否为当前默认版本 */
  isDefault: boolean;
  /** 是否为 NovaEnv 管理目录安装的版本（可卸载） */
  managed: boolean;
}

/** 按大版本分组的可安装版本（官方源） */
export interface AvailableVersionGroup {
  /** 大版本标识（Java/Node 如 21 / 24；Go 如 1.24） */
  major: string;
  isLts: boolean;
  /** 该大版本下具体版本（最新在前） */
  versions: string[];
  /** 该大版本最新版本 */
  latest: string;
}

/** 安装进度事件 */
export interface InstallProgress {
  kind: RuntimeKind;
  version: string;
  /** downloading / extracting / installing / done / error */
  stage: string;
  percent: number | null;
  message: string;
}

/** 安装结果：同大版本旧版本替换信息 */
export interface InstallResult {
  /** 被自动替换移除的旧版本号列表 */
  removed: string[];
  /** 旧版本曾是默认，已自动把新版本设为默认 */
  promoted: boolean;
}

/** 单个运行时在管理目录中的已安装版本 */
export interface ManagedRuntimeInfo {
  kind: RuntimeKind;
  versions: string[];
}

/** 管理目录信息 */
export interface ManageInfo {
  path: string;
  versionCount: number;
  sizeBytes: number;
  runtimes: ManagedRuntimeInfo[];
}

export interface RuntimeOverview {
  java: string | null;
  node: string | null;
  go: string | null;
  maven: string | null;
  python: string | null;
  rust: string | null;
  javaHome: string | null;
}

export interface RuntimesPayload {
  overview: RuntimeOverview;
  versions: RuntimeVersion[];
}

export interface ActivationPreview {
  /** 将要修改的配置文件（macOS ~/.zshrc） */
  configFile: string | null;
  /** 将要写入的配置行 */
  lines: string[];
  /** 备份文件路径 */
  backupPath: string | null;
  /** 平台说明 */
  note: string;
}

/** 运行时展示元信息（含品牌色与字母，用于图标与概览） */
export const RUNTIME_META: Record<
  RuntimeKind,
  { name: string; icon: string; desc: string; letter: string; color: string }
> = {
  java: { name: "Java", icon: "☕", desc: "JDK 运行时", letter: "J", color: "linear-gradient(135deg,#f59e0b,#ea580c)" },
  node: { name: "Node.js", icon: "🟢", desc: "JavaScript 运行时", letter: "N", color: "linear-gradient(135deg,#4ade80,#22c55e)" },
  go: { name: "Go", icon: "🐹", desc: "Go 编程语言", letter: "G", color: "linear-gradient(135deg,#38bdf8,#0891b2)" },
  maven: { name: "Maven", icon: "📦", desc: "Java 构建工具", letter: "M", color: "linear-gradient(135deg,#f87171,#dc2626)" },
  python: { name: "Python", icon: "🐍", desc: "Python 解释器", letter: "P", color: "linear-gradient(135deg,#60a5fa,#2563eb)" },
  rust: { name: "Rust", icon: "🦀", desc: "Rust 编程语言", letter: "R", color: "linear-gradient(135deg,#f97316,#c2410c)" },
};

/** 服务展示元信息（品牌色与字母） */
export const SERVICE_META: Record<
  ServiceKind,
  { name: string; letter: string; color: string }
> = {
  redis: { name: "Redis", letter: "R", color: "linear-gradient(135deg,#f87171,#b91c1c)" },
  mysql: { name: "MySQL", letter: "S", color: "linear-gradient(135deg,#22d3ee,#0e7490)" },
};
