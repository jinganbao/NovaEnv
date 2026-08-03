// 与 Rust 后端 models.rs 对应的类型定义（serde camelCase 契约）

export type RuntimeKind = "java" | "node" | "go";

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

/** 运行时展示元信息 */
export const RUNTIME_META: Record<
  RuntimeKind,
  { name: string; icon: string; desc: string }
> = {
  java: { name: "Java", icon: "☕", desc: "JDK 运行时" },
  node: { name: "Node.js", icon: "🟢", desc: "JavaScript 运行时" },
  go: { name: "Go", icon: "🐹", desc: "Go 编程语言" },
};
