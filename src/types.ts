/**
 * 桌面 IPC 数据契约。密钥只通过保存命令写入系统凭据管理器，不属于快照。
 * 历史记录保存规则快照，避免用户修改题目后误读旧结果。
 * @author ahui
 */
export interface ModelProfile {
    id: string;
    name: string;
    baseUrl: string;
    model: string;
    hasKey: boolean;
}

/** 严格连续字符串规则；keyword 保留前后空格，匹配区分大小写。 */
export interface TestRule {
    prompt: string;
    keyword: string;
}

/** 超时单位为秒；透明度范围为 80—100%，仅影响面板背景。 */
export interface Preferences {
    alwaysOnTop: boolean;
    snapToEdge: boolean;
    opacity: number;
    timeoutSeconds: number;
    shortcutEnabled: boolean;
    /** 自动调用当前模型；默认启用，无模型配置时不执行。 */
    autoTestEnabled: boolean;
    /** 分钟，1—1440，默认 3；手动开始测试或保存设置后重新计时。 */
    autoTestIntervalMinutes: number;
}

/** API 访问配置和测试规则相互独立，变更规则不改变历史。 */
export interface AppConfig {
    profiles: ModelProfile[];
    selectedProfileId: string | null;
    rule: TestRule;
    preferences: Preferences;
}

export type ResultStatus = 'passed' | 'failed' | 'error' | 'cancelled';

/** error/cancelled 没有模型能力结论；answer 可保留不完整正文用于排查。 */
export interface TestRecord {
    id: string;
    profileId: string;
    profileName: string;
    model: string;
    startedAt: string;
    durationMs: number;
    status: ResultStatus;
    answer: string;
    error: string | null;
    prompt: string;
    keyword: string;
    ruleVersion: string;
}

/** 后端持有的进行中任务，所有窗口共享，防止跨窗口重复请求。 */
export interface ActiveRun {
    id: string;
    profileName: string;
    model: string;
    startedAt: string;
}

export interface AppSnapshot {
    config: AppConfig;
    history: TestRecord[];
    running: ActiveRun | null;
    /** 后端单调时钟距离自动期限的毫秒数；关闭/无配置为 null，旧快照可缺省。 */
    autoTestRemainingMs?: number | null;
}

export interface ProfileInput {
    id: string;
    name: string;
    baseUrl: string;
    model: string;
    /** 空字符串表示保留现有密钥；没有保存过密钥时允许留空以连接本地服务。 */
    apiKey: string;
    removeKey: boolean;
}

/** GitHub 正式发布检查结果；无 Release 与网络失败不能被显示为“已是最新版”。 */
export interface UpdateResult {
    status: 'available' | 'current' | 'no-release';
    currentVersion: string;
    latestVersion: string | null;
    releaseUrl: string;
    /** 最多 4000 字符的发布说明，以纯文本呈现，不执行或自动获取其中资源。 */
    notes: string;
}

/** 临时目录只用于下拉选择，不落盘，不代表模型配额或调用权限。 */
export interface FetchedModel {
    id: string;
    ownedBy: string | null;
}
/** 独立连接测试耗时，单位毫秒；不会产生测智历史或结论。 */
export interface ConnectionResult {
    durationMs: number;
}
