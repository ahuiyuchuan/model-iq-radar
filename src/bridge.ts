/**
 * 唯一桌面桥接入口。浏览器仅提供可交互的布局预览，绝不冒充真实 API 测试。
 * 预览只在内存保存配置，不保留密钥，不向第三方发起请求。
 * @author ahui
 */
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { AppSnapshot } from './types';

export const desktop = isTauri();
/** 默认五行题目不插入空行；与 Rust 默认值一致，不覆盖用户已保存的自定义问题。 */
export const DEFAULT_PROMPT =
    '请快速按顺序回答：\n1. 仅依据你的记忆，你所知道的最新一位日本首相是谁？\n要求：\n- 全程禁止联网、搜索互联网、读取本机文件。\n- 只能依据已有的模型知识回答，不得查询或核实实时信息。';

const preview: AppSnapshot = {
    config: {
        profiles: [],
        selectedProfileId: null,
        rule: { prompt: DEFAULT_PROMPT, keyword: '高市早苗' },
        preferences: {
            alwaysOnTop: true,
            snapToEdge: true,
            opacity: 97,
            timeoutSeconds: 60,
            shortcutEnabled: true,
            autoTestEnabled: true,
            autoTestIntervalMinutes: 3,
        },
    },
    history: [],
    running: null,
};

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
        return structuredClone(preview) as T;
    }
    if (command === 'start_test') {
        throw new Error('请在桌面应用中测试。浏览器仅预览界面，不调用模型 API。');
    }
    // 普通浏览器不能读写 Windows 自启，不在预览中伪造开关已生效。
    if (['get_autostart', 'set_autostart'].includes(command)) {
        throw new Error('开机自启仅在 Windows 桌面应用中可用。');
    }
    if (['check_updates', 'open_external_url', 'fetch_models', 'test_connection'].includes(command)) {
        throw new Error('请在桌面应用中使用此功能，浏览器预览不会联网检查或打开外部页面。');
    }
    if (command === 'save_rule') {
        preview.config.rule = structuredClone(args.rule) as typeof preview.config.rule;
    } else if (command === 'save_preferences') {
        preview.config.preferences = structuredClone(args.preferences) as typeof preview.config.preferences;
    } else if (command === 'save_profile') {
        const input = args.input as Record<string, string>;
        const profile = {
            id: input.id || crypto.randomUUID(),
            name: input.name,
            baseUrl: input.baseUrl,
            model: input.model,
            hasKey: false,
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

/** 监听左、右、顶部边缘吸附及图标状态，返回的清理函数须在窗口卸载时执行。 */
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

/** 根据窗口角色执行关闭；悬浮窗隐藏到托盘，设置窗口可直接销毁。 */
export async function windowAction(action: 'close' | 'minimize' | 'maximize'): Promise<void> {
    if (!desktop) return;
    const current = getCurrentWindow();
    if (action === 'minimize') await current.minimize();
    else if (action === 'maximize') await current.toggleMaximize();
    else await current.close();
}
