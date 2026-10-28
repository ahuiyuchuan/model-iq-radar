/**
 * 唯一桌面桥接入口。浏览器仅提供可交互的布局预览，绝不冒充真实 API 测试。
 * 预览只在内存保存配置，不保留密钥，不向第三方发起请求。
 * @author ahui
 */
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { AppSnapshot, DataResetSelection, ModelProfile, ProfileInput, UpdateResult } from './types';

export const desktop = isTauri();
/** 新建接口的首选题目；按稳定 ID 判断，已有绑定和用户删除的预设不被隐式覆盖/复建。 */
export const CANDY_RULE_ID = 'builtin-candy-test';
/** 与 Rust 默认正文一致；已保存的自定义问题保留，不按名称猜测覆盖。 */
export const DEFAULT_PROMPT =
    '仅依据你的记忆，你所知道的最新一位日本首相是谁？\n\n要求：\n- 全程禁止联网、搜索互联网、读取本机文件。\n- 只能依据已有的模型知识回答，不得查询或核实实时信息。';
/** 题目原文只约束被测模型；HTML 回答走已有中性展示，备用关键词不评估动画质量。 */
export const PELICAN_PROMPT =
    '创建一个HTML，内容是SVG绘制一个黑天鹅骑自行车的2D动画，你不需要任何测试。\n\n要求：\n- 全程禁止联网、搜索互联网、读取本机文件。\n- 只能依据已有的模型知识回答，不得查询或核实实时信息。';
/** 浏览器布局预览使用与 Rust 同源文案；题目只发送正文，不附加 21颗 判定答案。 */
export const CANDY_PROMPT =
    '请完成回答下面的问题，答案要求只能由数字和“颗”组成，例如10颗、20颗、30颗；数字与“颗”之间不得有空格：\n\n在一个黑色的袋子里放有三种口味的糖果，每种糖果有两种不同的形状（圆形和五角星形，不同的形状靠手感可以分辨）。现已知不同口味的糖和不同形状的数量统计如下表。参赛者需要在活动前决定摸出的糖果数目，那么，最少取出多少个糖果才能保证手中同时拥有不同形状的苹果味和桃子味的糖？（同时手中有圆形苹果味匹配五角星桃子味糖果，或者有圆形桃子味匹配五角星苹果味糖果都满足要求）\n苹果味 桃子味 西瓜味\n圆形 7 9 8\n五角星形 7 6 4';

const preview: AppSnapshot = {
    config: {
        profiles: [],
        selectedProfileId: null,
        rule: { prompt: DEFAULT_PROMPT, keyword: '高市早苗' },
        defaultRuleName: '日本首相测试',
        builtinRulesVersion: 2,
        rulePresets: [
            { id: CANDY_RULE_ID, name: '糖果测试', prompt: CANDY_PROMPT, keyword: '21颗' },
            { id: 'builtin-pelican-test', name: '鹈鹕测试', prompt: PELICAN_PROMPT, keyword: '<svg' },
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
    },
    history: [],
    running: null,
};
// 分类重置只作用于浏览器布局预览的内存，不接触桌面数据库或系统凭据。
const previewDefaults = structuredClone(preview.config);

/**
 * 调用本地命令，失败时将后端可读错误向上传播。
 * @param command 已注册的命令名，非空。
 * @param args 命令参数；不记录到日志，避免泄露保存时的密钥。
 * @returns 命令结果；泛型必须与 Rust 返回契约一致。
 */
export async function call<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
    if (desktop) {
        return invoke<T>(command, args);
    }
    if (command === 'get_snapshot') {
        // 预览也遵循桌面读取范围，不能让布局调试无意依赖悬浮窗全量历史。
        const snapshot = structuredClone(preview);
        snapshot.historyRevision = preview.history.map((record) => record.id).join(':');
        if (args.includeHistory !== true) {
            const id = snapshot.config.selectedProfileId;
            const fingerprint = id ? snapshot.conditionFingerprints?.[id] : undefined;
            const latest =
                snapshot.history.find(
                    (record) =>
                        record.profileId === id && fingerprint && record.conditionFingerprint === fingerprint,
                ) ?? snapshot.history.find((record) => record.profileId === id);
            snapshot.history = latest ? [latest] : [];
        }
        return snapshot as T;
    }
    // 浏览器内存预览遵循摘要/按页正文契约，不接触桌面数据；正式检索由 Rust 后台执行。
    if (command === 'get_history_index')
        return {
            revision: preview.history.map((record) => record.id).join(':'),
            entries: preview.history.map((record, index) => ({
                sequence: index + 1,
                record: { ...record, answer: '', prompt: '' },
            })),
        } as T;
    if (command === 'get_history_records')
        return preview.history.filter((_record, index) =>
            (args.sequences as number[]).includes(index + 1),
        ) as T;
    if (command === 'search_history') {
        const query = String(args.query ?? '').toLocaleLowerCase();
        return preview.history
            .filter((record) =>
                `${record.profileName} ${record.model} ${record.reasoningEffort ?? (record.conditionFingerprint ? '不指定' : '未记录')} ${record.answer} ${record.ruleName ?? ''} ${[record.keyword, ...(record.alternativeKeywords ?? [])].join(' ')}`
                    .toLocaleLowerCase()
                    .includes(query),
            )
            .map((record) => record.id) as T;
    }
    if (command === 'get_model_catalog') return [] as T;
    if (command === 'get_update_status') return null as T;
    if (command === 'start_test') {
        throw new Error('请在桌面应用中测试。浏览器仅预览界面，不调用模型 API。');
    }
    // 普通浏览器不能读写 Windows 自启，不在预览中伪造开关已生效。
    if (['get_autostart', 'set_autostart'].includes(command)) {
        throw new Error('开机自启仅在 Windows 桌面应用中可用。');
    }
    if (
        [
            'open_external_url',
            'prepare_html_preview',
            'release_html_preview',
            'set_html_preview_expanded',
            'fetch_models',
            'fetch_profile_models',
            'test_connection',
            'check_updates',
            'send_test_email',
            'save_email_settings',
            'export_config_backup',
            'preview_config_import',
            'import_config_backup',
        ].includes(command)
    ) {
        throw new Error('请在桌面应用中使用此功能，浏览器预览不会联网检查或打开外部页面。');
    }
    if (command === 'reset_selected_data') {
        const selected = args.selection as DataResetSelection;
        if (
            !selected ||
            !['api', 'rules', 'widget', 'history', 'mail'].some(
                (key) => selected[key as keyof DataResetSelection] === true,
            )
        )
            throw new Error('请至少选择一个需要清空的分类。');
        if (preview.running) throw new Error('测试正在进行，请先完成或取消。');
        if (selected.api) {
            preview.config.profiles = [];
            preview.config.selectedProfileId = null;
        }
        if (selected.rules) {
            preview.config.rule = structuredClone(previewDefaults.rule);
            preview.config.defaultRuleName = previewDefaults.defaultRuleName;
            preview.config.rulePresets = structuredClone(previewDefaults.rulePresets);
            preview.config.builtinRulesVersion = 2;
            for (const profile of preview.config.profiles) profile.rulePresetId = CANDY_RULE_ID;
        }
        if (selected.widget) {
            preview.config.preferences = structuredClone(previewDefaults.preferences);
            preview.config.widgetPosition = null;
        }
        if (selected.history) preview.history = [];
        if (selected.mail) preview.config.emailNotifications = undefined;
        if (selected.api || selected.history || selected.mail) preview.emailNotificationError = null;
    } else if (command === 'save_rule') {
        preview.config.rule = structuredClone(args.rule) as typeof preview.config.rule;
        if (typeof args.name === 'string') preview.config.defaultRuleName = args.name;
    } else if (command === 'set_profile_rule') {
        const profile = preview.config.profiles.find((item) => item.id === args.profileId);
        if (!profile) throw new Error('模型配置已不存在，请刷新后重试。');
        if (
            args.rulePresetId != null &&
            !preview.config.rulePresets?.some((item) => item.id === args.rulePresetId)
        )
            throw new Error('规则预设已不存在，请刷新后重试。');
        profile.rulePresetId = (args.rulePresetId as string | null) ?? null;
    } else if (command === 'set_profile_model') {
        // 预览没有远端模型目录，不伪造可选择项或绕过桌面目录校验。
        throw new Error('请在桌面应用中拉取并切换模型。');
    } else if (command === 'set_profile_effort') {
        const profile = preview.config.profiles.find((item) => item.id === args.profileId);
        if (!profile) throw new Error('模型配置已不存在，请刷新后重试。');
        const value = args.reasoningEffort;
        if (value != null && !['low', 'medium', 'high', 'xhigh'].includes(String(value)))
            throw new Error('思考强度无效。');
        profile.reasoningEffort = value as ModelProfile['reasoningEffort'];
    } else if (command === 'save_preferences') {
        preview.config.preferences = structuredClone(args.preferences) as typeof preview.config.preferences;
    } else if (command === 'set_auto_test_enabled') {
        preview.config.preferences.autoTestEnabled = Boolean(args.enabled);
    } else if (command === 'preview_history_retention') {
        return { removedCount: 0, remainingCount: preview.history.length } as T;
    } else if (command === 'save_rule_preset') {
        const preset = structuredClone(args.preset) as NonNullable<typeof preview.config.rulePresets>[number];
        const presets = (preview.config.rulePresets ??= []);
        const index = presets.findIndex((item) => item.id === preset.id);
        if (index < 0) presets.push(preset);
        else presets[index] = preset;
    } else if (command === 'delete_rule_preset') {
        preview.config.rulePresets = preview.config.rulePresets?.filter((item) => item.id !== args.id);
        for (const profile of preview.config.profiles) {
            if (profile.rulePresetId === args.id) profile.rulePresetId = null;
        }
    } else if (command === 'save_profile') {
        const input = args.input as ProfileInput;
        const interfaceMode: ModelProfile['interfaceMode'] =
            input.interfaceMode === 'responses' ? 'responses' : 'chat_completions';
        const reasoningEffort: ModelProfile['reasoningEffort'] =
            input.reasoningEffort === 'low' ||
            input.reasoningEffort === 'medium' ||
            input.reasoningEffort === 'high' ||
            input.reasoningEffort === 'xhigh'
                ? input.reasoningEffort
                : null;
        const profile = {
            id: input.id || crypto.randomUUID(),
            name: input.name,
            baseUrl: input.baseUrl,
            model: input.model,
            hasKey: false,
            interfaceMode,
            protocolFallbackEnabled: input.protocolFallbackEnabled ?? false,
            reasoningEffort,
            rulePresetId: input.rulePresetId || null,
            catalogToken: input.catalogToken || null,
        };
        const index = preview.config.profiles.findIndex((item) => item.id === profile.id);
        if (index < 0) preview.config.profiles.push(profile);
        else preview.config.profiles[index] = profile;
        preview.config.selectedProfileId = profile.id;
    } else if (command === 'select_profile') {
        preview.config.selectedProfileId = String(args.id);
    } else if (command === 'delete_profile') {
        preview.config.profiles = preview.config.profiles.filter((item) => item.id !== args.id);
        preview.config.selectedProfileId = preview.config.profiles[0]?.id ?? null;
    } else if (command === 'clear_history') {
        preview.history = [];
    }
    window.dispatchEvent(new Event('preview-changed'));
    return undefined as T;
}

/**
 * 订阅跨窗口刷新事件；调用方卸载时必须执行返回的清理函数。
 * @param handler 只刷新快照，不在事件内自动发起测试。
 * @returns 解除监听函数。
 */
export async function subscribe(handler: () => void): Promise<() => void> {
    if (desktop) return listen('snapshot-changed', handler);
    window.addEventListener('preview-changed', handler);
    return () => window.removeEventListener('preview-changed', handler);
}

/**
 * 订阅进程级更新缓存；先监听后读取，避免打开设置时旧缓存撤销刚收到的新版本标记。
 * @param handler 仅呈现更新标记，不弹窗、不触发网络请求；null 表示尚无成功检查结果。
 * @returns 卸载设置页时必须调用的清理函数；浏览器只提供空缓存预览。
 * @throws IPC/事件订阅失败时拒绝，调用方按静默检查约定忽略，不影响其他设置。
 */
export async function subscribeUpdateStatus(
    handler: (result: UpdateResult | null) => void,
): Promise<() => void> {
    if (!desktop) {
        handler(null);
        return () => {};
    }
    let receivedEvent = false;
    const cleanup = await listen<UpdateResult>('update-status-changed', (event) => {
        receivedEvent = true;
        handler(event.payload);
    });
    try {
        const initial = await call<UpdateResult | null>('get_update_status');
        if (!receivedEvent) handler(initial);
        return cleanup;
    } catch (cause) {
        cleanup();
        throw cause;
    }
}

let layoutQueue: Promise<unknown> = Promise.resolve();

/**
 * 串行调整卡片、菜单和边签，防止快速切换时旧尺寸覆盖新状态。
 * @param expanded 是否显示完整回答卡片。
 * @param menuHeight 菜单所需逻辑像素，0 表示关闭菜单。
 * @param tucked 是否收成屏幕边缘小图标（侧边 28×48、顶部 48×28 逻辑像素）；仅已吸附时生效。
 */
export async function resizeWidget(expanded: boolean, menuHeight = 0, tucked = false): Promise<void> {
    if (!desktop) return;
    const update = layoutQueue
        .catch(() => {})
        .then(() => call('set_widget_layout', { expanded, menuHeight, tucked }));
    layoutQueue = update;
    await update;
}

/**
 * 监听左、右、顶部边缘吸附及图标状态，先注册事件再读取初始状态，优先采用新事件。
 * @param handler 接收边缘方向（未吸附时 null）与是否收为图标的状态，不应发起测试。
 * @returns 卸载时必须调用的清理函数；浏览器预览返回无副作用空函数。
 * @throws 桌面监听或初始读取失败时拒绝 Promise；已注册的监听会先清理。
 */
export async function subscribeDock(
    handler: (dock: { side: string | null; tucked: boolean }) => void,
): Promise<() => void> {
    if (!desktop) return () => {};
    let receivedEvent = false;
    const cleanup = await listen('widget-dock', (event) => {
        receivedEvent = true;
        handler(event.payload as { side: string | null; tucked: boolean });
    });
    try {
        const initial = await call<{ side: string | null; tucked: boolean }>('get_widget_dock');
        // 读取期间收到的新事件优先，防止旧的启动快照撤销刚发生的收起/恢复。
        if (!receivedEvent) handler(initial);
        return cleanup;
    } catch (cause) {
        cleanup();
        throw cause;
    }
}

/** 只有标题栏可主动发起拖动，按钮不会成为隐含拖动区域。 */
export async function dragWindow(): Promise<void> {
    if (desktop) {
        const window = getCurrentWindow();
        if (window.label === 'widget') await call('prepare_widget_drag');
        await window.startDragging();
    }
}

/** 桌面模式打开复用的设置窗口；浏览器通过独立地址展示相同组件。 */
export async function openSettings(): Promise<void> {
    if (desktop) await call('open_settings');
    else window.open('/?view=settings', 'model-probe-settings');
}

/**
 * 执行原生窗口操作；设置页关闭仍经过草稿保护，悬浮窗关闭由后端转为隐藏到托盘。
 * @param action 关闭、最小化或切换最大化；浏览器预览不执行原生操作。
 * @throws 原生操作失败时拒绝 Promise，交由调用方提供可见反馈。
 */
export async function windowAction(action: 'close' | 'minimize' | 'maximize'): Promise<void> {
    if (!desktop) return;
    const current = getCurrentWindow();
    if (action === 'minimize') await current.minimize();
    else if (action === 'maximize') await current.toggleMaximize();
    else await current.close();
}

/**
 * 拦截标题栏、Alt+F4 等原生关闭路径，让设置页统一保护草稿。
 * 监听注册前组件可能卸载，调用方仍须在 Promise 完成后清理。
 * @param handler 统一关闭流程，负责保存/放弃/取消及最终销毁窗口。
 * @returns 解除原生监听的函数，浏览器预览返回无副作用空函数。
 */
export async function protectWindowClose(handler: () => Promise<void>): Promise<() => void> {
    if (!desktop) return () => {};
    return getCurrentWindow().onCloseRequested((event) => {
        event.preventDefault();
        void handler();
    });
}

/** 用户已完成保存/放弃选择后销毁设置窗口，绕过 close 事件防止重复确认。 */
export async function closeSettingsWindow(): Promise<void> {
    if (desktop) await getCurrentWindow().destroy();
}
