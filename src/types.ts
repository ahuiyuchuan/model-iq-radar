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
    /** 本机凭据变化的非敏感修订标识；仅后端维护，不包含密钥，不随备份迁移。 */
    credentialRevision?: string | null;
    /** 请求协议；旧配置缺省为 Chat Completions。 */
    interfaceMode?: 'chat_completions' | 'responses';
    /** 旧配置缺省 false，新建表单默认 true；错误最多切换一次协议，共享总超时，可能产生第二次 API 调用。 */
    protocolFallbackEnabled?: boolean;
    /** 推理强度；null 明确表示不发送，旧配置缺字段时按默认 low 迁移。 */
    reasoningEffort?: 'low' | 'medium' | 'high' | 'xhigh' | null;
    /** 仅兼容旧数据；v1.0.3 起忽略接口超时，改用测试规则/全局值。 */
    timeoutSeconds?: number | null;
    rulePresetId?: string | null;
    /** 本机已拉取模型列表的随机句柄；不随配置备份导出，不含凭据。 */
    catalogToken?: string | null;
}

export interface RulePreset {
    id: string;
    name: string;
    prompt: string;
    keyword: string;
    /** 额外答案，最多 19 项；缺省时保持首版单答案语义。 */
    alternativeKeywords?: string[];
    /** 本题总超时，正整数秒（安全整数）；缺省/null 继承悬浮设置。 */
    timeoutSeconds?: number | null;
}

/** 任一答案连续命中即通过；每项保留空格/换行并区分大小写，不拆分旧 keyword。 */
export interface TestRule {
    prompt: string;
    keyword: string;
    alternativeKeywords?: string[];
    /** 本题总超时，正整数秒（安全整数）；缺省/null 继承悬浮设置。 */
    timeoutSeconds?: number | null;
}

/** 超时单位为秒；透明度范围为 80—100%，仅影响面板背景。 */
export interface Preferences {
    alwaysOnTop: boolean;
    snapToEdge: boolean;
    opacity: number;
    /** 全局请求超时，正整数秒（安全整数），缺字段默认 300；规则未设置时使用。 */
    timeoutSeconds: number;
    /** 自动调用当前模型；默认启用，无模型配置时不执行。 */
    autoTestEnabled: boolean;
    /** 分钟，1—1440，默认 3；每次测试结算、开启自动或修改间隔时重新计时。 */
    autoTestIntervalMinutes: number;
    /** 历史记录最多保留条数；旧配置缺省为 500，null 使用 5000 条硬上限。 */
    historyMaxRecords?: number | null;
    /** 历史记录最多保留天数；空值表示不按时间清理。 */
    historyRetentionDays?: number | null;
}

/** 接口可绑定命名规则或沿用默认规则；变更有效规则不重新判定历史。 */
export interface AppConfig {
    profiles: ModelProfile[];
    selectedProfileId: string | null;
    rule: TestRule;
    /** 仅展示名称；默认规则身份及空绑定不变，旧配置缺省为“日本首相测试”。 */
    defaultRuleName?: string;
    /** 自带规则的一次性补入版本；后端维护，缺省为 0，当前迁移至 2。 */
    builtinRulesVersion?: number;
    rulePresets?: RulePreset[];
    preferences: Preferences;
    /** 窗口物理像素位置；由后端按显示器工作区恢复，前端不直接修改。 */
    widgetPosition?: [number, number] | null;
    /** SMTP 密码独立保存在系统凭据管理器，快照仅包含存在标记。 */
    emailNotifications?: EmailNotificationSettings;
}

export type ResultStatus = 'passed' | 'failed' | 'error' | 'cancelled' | 'html';
export type AnswerType = 'text' | 'html';

/** HTML/error/cancelled 没有模型能力结论；answer 可保留不完整正文用于排查。 */
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
    alternativeKeywords?: string[];
    /** 当次请求的规则名称快照；旧历史缺省时显示未记录。 */
    ruleName?: string | null;
    ruleVersion: string;
    /** 以下字段由 v1.0.1 新记录保存；旧历史不补猜。 */
    source?: 'manual' | 'automatic' | null;
    interfaceMode?: 'chat_completions' | 'responses' | null;
    /** 当次请求策略及实际尝试顺序；旧记录缺失时显示未记录。 */
    protocolFallbackEnabled?: boolean | null;
    attemptedModes?: string[];
    reasoningEffort?: string | null;
    effectiveTimeoutSeconds?: number | null;
    /** 后端按有效测试条件计算；缺失的旧历史不能被标记为当前有效结果。 */
    conditionFingerprint?: string | null;
    errorKind?: string | null;
    httpStatus?: number | null;
    requestId?: string | null;
    /** 新记录由后端自动识别；旧历史缺失时前端只用于展示预览，不重写结论。 */
    answerType?: AnswerType | null;
    answerParsingVersion?: string | null;
}

/** 只允许加密 SMTP；空密码草稿表示保留已有授权码，不从后端读取明文。 */
export interface EmailNotificationSettings {
    enabled: boolean;
    smtpHost: string;
    smtpPort: number;
    security: 'tls' | 'starttls';
    sender: string;
    username: string;
    recipients: string[];
    onDegraded: boolean;
    onRecovered: boolean;
    onFailure: boolean;
    hasPassword: boolean;
}

/** 邮件设置写入/试发契约；试发不保存草稿，失败不得影响模型测试。 */
export interface EmailSettingsInput {
    settings: EmailNotificationSettings;
    password: string;
    removePassword: boolean;
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
    /** 设置页显式请求完整历史；悬浮窗只含当前模型最新的一条，避免传输所有原始回答。 */
    history: TestRecord[];
    running: ActiveRun | null;
    /** 后端单调时钟距离自动期限的毫秒数；测试中/关闭/无配置为 null，旧快照可缺省。 */
    autoTestRemainingMs?: number | null;
    /** 各模型当前条件摘要，仅用于与记录快照比较，不含密钥明文。 */
    conditionFingerprints?: Record<string, string>;
    /** 最近事件邮件发送失败的脱敏提示；不改变模型结果，不进入历史导出。 */
    emailNotificationError?: string | null;
}

export interface ProfileInput {
    id: string;
    name: string;
    baseUrl: string;
    model: string;
    /** 空字符串表示保留现有密钥；没有保存过密钥时允许留空以连接本地服务。 */
    apiKey: string;
    removeKey: boolean;
    /** 新建表单默认 Responses；编辑旧配置时显式传入兼容后的已保存模式。 */
    interfaceMode?: 'chat_completions' | 'responses';
    /** 新建表单默认 true；旧调用方缺字段仍按 false 兼容，不批量启用已有配置。 */
    protocolFallbackEnabled?: boolean;
    reasoningEffort?: 'low' | 'medium' | 'high' | 'xhigh' | null;
    rulePresetId?: string | null;
    catalogToken?: string | null;
}

/** 两个管理入口均清空所选分类并恢复初始值；必须显式确认，空选择由前后端共同拒绝。 */
export interface DataResetSelection {
    api: boolean;
    rules: boolean;
    widget: boolean;
    history: boolean;
    mail: boolean;
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

/** 缓存目录仅提供模型 ID 建议，不代表配额、实时可用性或调用权限。 */
export interface FetchedModel {
    id: string;
    ownedBy: string | null;
}
/** 列表成功拉取后保存本机随机句柄；新配置保存时关联，失败不覆盖上次列表。 */
export interface ModelCatalog {
    token: string;
    models: FetchedModel[];
}
/** 独立连接测试耗时，单位毫秒；不会产生测智历史或结论。 */
export interface ConnectionResult {
    durationMs: number;
    interfaceMode: 'chat_completions' | 'responses';
    attemptedModes: string[];
}
