/**
 * README 图片专用内存桥接；由截图脚本的独立 Vite 插件替换，生产入口不引用此文件。
 * 配置和记录为虚构示例；HTML 原样使用用户提供的海岸骑行动画，不读数据库或系统凭据。
 * 仅提供只读快照及 Blob 预览；未知命令一律拒绝，防止文档生成意外测试或发信。
 * @author ahui
 */
import html from '/scripts/fixtures/pelican-coast.html?raw';
import { CANDY_PROMPT, DEFAULT_PROMPT, PELICAN_PROMPT } from '/src/bridge.ts?readme-original';
export { CANDY_RULE_ID, CANDY_PROMPT, DEFAULT_PROMPT, PELICAN_PROMPT } from '/src/bridge.ts?readme-original';

// 只模拟桌面组件的展示状态；原生窗口动作均为空实现，不启动或操控桌面程序。
export const desktop = true;
const htmlMode = new URLSearchParams(location.search).get('example') === 'html';
const profiles = [
    {
        id: 'demo-a',
        name: '演示接口 A',
        baseUrl: 'https://api.example.com/v1',
        model: 'gpt-6',
        hasKey: false,
        interfaceMode: 'responses',
        protocolFallbackEnabled: false,
        reasoningEffort: 'low',
        rulePresetId: htmlMode ? 'builtin-pelican-test' : 'builtin-candy-test',
        catalogToken: 'readme-catalog',
    },
    {
        id: 'demo-b',
        name: '演示接口 B',
        baseUrl: 'https://backup.example.com/v1',
        model: 'gpt-6',
        hasKey: false,
        interfaceMode: 'responses',
        protocolFallbackEnabled: false,
        reasoningEffort: 'medium',
        rulePresetId: 'builtin-candy-test',
        catalogToken: 'readme-catalog',
    },
];
const record = {
    id: 'demo-candy',
    profileId: 'demo-a',
    profileName: '演示接口 A',
    model: 'gpt-6',
    startedAt: '2026-10-27T08:00:00Z',
    durationMs: 14734,
    status: 'passed',
    answer: '21颗',
    error: null,
    prompt: CANDY_PROMPT,
    keyword: '21颗',
    ruleName: '糖果测试',
    ruleVersion: 'literal-contains-v1',
    source: 'manual',
    interfaceMode: 'responses',
    protocolFallbackEnabled: false,
    attemptedModes: ['responses'],
    reasoningEffort: 'low',
    effectiveTimeoutSeconds: 300,
    conditionFingerprint: 'readme-candy',
    httpStatus: 200,
    answerType: 'text',
    answerParsingVersion: 'html-v1',
};
const snapshot = {
    config: {
        profiles,
        selectedProfileId: 'demo-a',
        rule: { prompt: DEFAULT_PROMPT, keyword: '高市早苗' },
        defaultRuleName: '日本首相测试',
        builtinRulesVersion: 2,
        rulePresets: [
            { id: 'builtin-candy-test', name: '糖果测试', prompt: CANDY_PROMPT, keyword: '21颗' },
            {
                id: 'builtin-pelican-test',
                name: '鹈鹕测试',
                prompt: PELICAN_PROMPT,
                keyword: '<svg',
                timeoutSeconds: 600,
            },
        ],
        preferences: {
            alwaysOnTop: true,
            snapToEdge: true,
            opacity: 97,
            timeoutSeconds: 300,
            autoTestEnabled: true,
            autoTestIntervalMinutes: 3,
            historyMaxRecords: 500,
            historyRetentionDays: null,
        },
        emailNotifications: {
            enabled: false,
            smtpHost: 'smtp.qq.com',
            smtpPort: 465,
            security: 'tls',
            sender: '',
            username: '',
            recipients: [],
            onDegraded: true,
            onRecovered: true,
            onFailure: true,
            hasPassword: false,
        },
    },
    history: [
        {
            ...record,
            id: 'demo-html',
            startedAt: '2026-10-27T08:03:00Z',
            durationMs: 21280,
            status: 'html',
            answer: html,
            prompt: PELICAN_PROMPT,
            keyword: '<svg',
            ruleName: '鹈鹕测试',
            conditionFingerprint: 'readme-html',
            answerType: 'html',
            effectiveTimeoutSeconds: 600,
        },
        record,
        {
            ...record,
            id: 'demo-error',
            profileId: 'demo-b',
            profileName: '演示接口 B',
            startedAt: '2026-10-27T07:57:00Z',
            status: 'error',
            answer: '',
            error: '请求超过 300 秒，已停止等待，未执行判定。',
            errorKind: 'timeout',
            httpStatus: null,
            source: 'automatic',
            reasoningEffort: 'medium',
            durationMs: 300000,
        },
    ],
    running: null,
    conditionFingerprints: { 'demo-a': htmlMode ? 'readme-html' : 'readme-candy' },
    autoTestRemainingMs: 137000,
    emailNotificationError: null,
};

/**
 * 仅返回白名单演示数据；不提供保存、测试、联网、邮件或系统操作能力。
 * @param command 当前组件传入的只读/预览命令名；其他命令明确拒绝。
 * @param args 预览文档或待释放 URL；默认空对象，不接受或保存请求凭据。
 * @returns 快照、模型候选或临时 Blob URL；释放及布局空操作返回 undefined。
 * @throws 命令不在文档渲染白名单时拒绝，避免后续组件新增动作被静默伪造成功。
 */
export async function call(command: string, args: Record<string, unknown> = {}): Promise<unknown> {
    if (command === 'get_snapshot') return structuredClone(snapshot);
    if (command === 'get_autostart') return false;
    if (command === 'get_model_catalog')
        return [
            { id: 'gpt-6', label: 'gpt-6' },
            { id: 'demo-model', label: 'demo-model' },
        ];
    if (command === 'get_update_status') return null;
    if (command === 'prepare_html_preview')
        return URL.createObjectURL(new Blob([String(args.document)], { type: 'text/html' }));
    if (command === 'release_html_preview') {
        URL.revokeObjectURL(String(args.url));
        return;
    }
    if (command === 'set_html_preview_expanded') return;
    throw new Error(`README 渲染禁止执行命令：${command}`);
}

/** 演示没有后台事件或原生窗口，保留实际组件的订阅/清理签名。 */
export async function subscribe(_handler: () => void): Promise<() => void> {
    return () => {};
}
export async function subscribeDock(
    _handler: (dock: { side: string | null; tucked: boolean }) => void,
): Promise<() => void> {
    return () => {};
}
export async function subscribeUpdateStatus(handler: (value: null) => void): Promise<() => void> {
    handler(null);
    return () => {};
}
export async function protectWindowClose(_handler: () => Promise<void>): Promise<() => void> {
    return () => {};
}
export async function resizeWidget(_expanded: boolean, _menuHeight = 0, _tucked = false): Promise<void> {}
export async function dragWindow(): Promise<void> {}
export async function dragPetWindow(): Promise<void> {}
/** 文档预览不调整原生 Region；仍允许场景注册与卸载，兼容桌宠组件导入。 */
export function registerSceneLayout(
    _handler: (expanded: boolean, menuHeight: number) => Promise<void>,
): () => void {
    return () => {};
}
export async function subscribeDragEnd(_handler: () => void): Promise<() => void> {
    return () => {};
}
export async function openSettings(): Promise<void> {}
export async function windowAction(_action: 'close' | 'minimize' | 'maximize'): Promise<void> {}
export async function closeSettingsWindow(): Promise<void> {}
