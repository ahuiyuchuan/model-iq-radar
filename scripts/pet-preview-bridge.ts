/**
 * 桌宠浏览器验收专用内存桥接；只有 QA Vite 插件加载，不进入生产包。
 * 使用虚构状态驱动真实组件，不读取 SQLite、系统凭据，不调用网络模型/邮件。
 * 原生布局仅记录参数，不能把浏览器验证表述为 Windows 穿透验收。@author ahui
 */
import * as base from './readme-preview-bridge';
import type { AppSnapshot, TestRecord } from '../src/types';
import type { SkinPack } from '../src/pet/types';
export {
    CANDY_RULE_ID,
    CANDY_PROMPT,
    DEFAULT_PROMPT,
    PELICAN_PROMPT,
    subscribeUpdateStatus,
    protectWindowClose,
    windowAction,
    closeSettingsWindow,
} from './readme-preview-bridge';
export const desktop = true;
const state = (await base.call('get_snapshot')) as AppSnapshot;
state.config.preferences.skin = {
    id: 'blue-whale-maid',
    size: 192,
    animation: true,
    bubbles: true,
    autonomous: false,
};
const changed = new Set<() => void>();
const released = new Set<() => void>();
type QaDock = { side: string | null; tucked: boolean };
const dockChanged = new Set<(dock: QaDock) => void>();
let dock: QaDock = { side: null, tucked: false };
let layout: ((expanded: boolean, menuHeight: number) => Promise<void>) | undefined;
export const scenes: unknown[] = [];
/** 与Rust保持相同的矩形边界拒绝规则，避免浏览器吞掉原生会拒绝的布局请求。 */
export const rejectedScenes: unknown[] = [];
/** 工作区相对窗口的验收边界；null使用当前模拟方向，测试不查询真实屏幕。 */
let qaMenuBounds: [number, number, number, number] | null = null;
export function setQaMenuBounds(bounds: [number, number, number, number] | null): void {
    qaMenuBounds = bounds;
}
/** 只供浏览器回归模拟原生窗口移位；生产桥接没有此入口，不能控制真实系统窗口。 */
let sceneObserver: ((scene: unknown) => void | Promise<void>) | undefined;
/** 注册本页几何观察者；传undefined解除，避免将模拟位置带入下一组固定场景用例。 */
export function observeQaScene(handler?: (scene: unknown) => void | Promise<void>): void {
    sceneObserver = handler;
}
/** 经典窗口尺寸替身独立于角色scene；用于复现退出皮肤后详情被156px窗口截断。 */
let classicObserver: ((expanded: boolean) => void | Promise<void>) | undefined;
export function observeQaClassic(handler?: (expanded: boolean) => void | Promise<void>): void {
    classicObserver = handler;
}
/** 只记录命令名而非参数，核对交互不会触发测试/邮件，也避免验收日志收集凭据。 */
export const requests: string[] = [];
const imported = new Map<string, SkinPack>();
// 保留独立合成模板，清空历史/反复切状态也不借用被上个用例改过的记录。
const sampleRecord = structuredClone(state.history.find((record) => record.status === 'passed')!);
const sampleHistory = structuredClone(state.history);
const sampleFingerprints = structuredClone(state.conditionFingerprints);
let qaRun = 0;
// get_snapshot必须像后端一样返回“当前”剩余时间，不能每次轮询都把固定137秒重新发给前端。
let autoDeadline = performance.now() + 137000;

/**
 * 隔离浏览器用逻辑工作区检测已提交的主体触边；这里只模拟方向通知，不移动系统窗口。
 * 独立Rust回归验证实际判定函数；该替身不能证明Win32拖动、DPI或透明穿透已通过。
 * @param x 虚构窗口原点，逻辑像素。
 * @param y 虚构窗口原点，逻辑像素。
 * @param width 虚构工作区宽度，逻辑像素；默认820。
 * @returns 此次主体检测到的方向，远离三边时为null。
 */
export function setQaWindowPosition(x: number, y: number, width = 820): string | null {
    const scene = scenes.at(-1) as { dockBounds?: number[]; actorBounds: number[] };
    const [left, top, extent] = scene.dockBounds ?? scene.actorBounds;
    const side =
        y + top! <= 17
            ? 'top'
            : x + left! <= 17
              ? 'left'
              : x + left! + extent! >= width - 17
                ? 'right'
                : null;
    setQaDock(side, false);
    return side;
}

/** 浏览器矩阵只修改本页虚构偏好；不读写真实桌面设置或凭据。 */
export function setQaAppearance(size: number, bubbles = true): void {
    state.config.preferences.skin!.size = size;
    state.config.preferences.skin!.bubbles = bubbles;
    changed.forEach((f) => f());
}

/** 空历史用于未测试/待机验收；恢复使用初始合成快照而非生产数据。 */
export function setQaHistory(empty: boolean): void {
    state.running = null;
    state.history = empty ? [] : structuredClone(sampleHistory);
    changed.forEach((f) => f());
}

/** 模拟后端条件指纹改变，验证旧通过只能显示为待重新测试。 */
export function setQaStale(stale: boolean): void {
    state.conditionFingerprints = structuredClone(sampleFingerprints);
    const id = state.config.selectedProfileId;
    if (stale && id && state.conditionFingerprints) state.conditionFingerprints[id] = 'qa-new-condition';
    changed.forEach((f) => f());
}

/** 主姿态截图固定关闭自主动作；单独的空闲验收显式启用，防止计时器污染其他截图。 */
export function setQaAutonomous(enabled: boolean): void {
    state.config.preferences.skin!.autonomous = enabled;
    changed.forEach((f) => f());
}

/** 暂停角色动画仅修改本页虚构偏好，用于检查探头恢复不依赖播放器时钟。 */
export function setQaAnimation(enabled: boolean): void {
    state.config.preferences.skin!.animation = enabled;
    changed.forEach((f) => f());
}

/**
 * 注入原生吸附事件的浏览器替身；没有窗口移动能力，不能据此宣称原生贴边已通过。
 * @param side 左、右或顶部的同一探头方向；null 表示拖离边缘。
 * @param tucked 原生当前是否处于收起状态，默认 false，由真实组件决定何时收起。
 */
export function setQaDock(side: 'left' | 'right' | 'top' | null, tucked = false): void {
    dock = { side, tucked };
    dockChanged.forEach((handler) => handler({ ...dock }));
}

/** 只接收固定的合成终态；running 保留到验收脚本显式结算，便于核对拖动优先级。 */
export function setQaRun(status: TestRecord['status'] | 'running'): void {
    state.running =
        status === 'running'
            ? {
                  id: `pet-qa-run-${++qaRun}`,
                  profileName: '演示接口 A',
                  model: 'gpt-6',
                  startedAt: new Date().toISOString(),
              }
            : null;
    if (status !== 'running') {
        state.history = [
            {
                ...sampleRecord,
                id: `pet-qa-run-${++qaRun}`,
                status,
                profileId: state.config.selectedProfileId!,
                conditionFingerprint: state.conditionFingerprints?.[state.config.selectedProfileId!],
            },
            ...state.history,
        ];
    }
    changed.forEach((f) => f());
}
export function finishQaDrag(): void {
    released.forEach((f) => f());
}
export async function call(command: string, args: Record<string, unknown> = {}): Promise<unknown> {
    if (command === 'get_pet_menu_bounds') {
        if (qaMenuBounds) return [...qaMenuBounds];
        const width = (scenes.at(-1) as { width?: number } | undefined)?.width ?? 420;
        return dock.side === 'right' ? [width - 820, 0, width, 920] : [0, 0, 820, 920];
    }
    requests.push(command);
    if (requests.length > 4000) requests.splice(0, requests.length - 4000);
    if (command === 'get_snapshot') {
        const snapshot = structuredClone(state);
        snapshot.autoTestRemainingMs = Math.max(0, autoDeadline - performance.now());
        return snapshot;
    }
    if (command === 'get_widget_dock') return { ...dock };
    // 主按钮和自动测试开关的浏览器验收只改变本页内存状态；绝不执行真实推理或邮件。
    if (command === 'start_test' || command === 'cancel_test') {
        setQaRun(command === 'start_test' ? 'running' : 'cancelled');
        return;
    }
    if (command === 'set_auto_test_enabled') {
        state.config.preferences.autoTestEnabled = Boolean(args.enabled);
        state.autoTestRemainingMs = 137000;
        autoDeadline = performance.now() + 137000;
        changed.forEach((f) => f());
        return;
    }
    if (command === 'select_profile') {
        if (!state.config.profiles.some((p) => p.id === args.id)) throw new Error('QA 接口不存在。');
        state.config.selectedProfileId = String(args.id);
        changed.forEach((f) => f());
        return;
    }
    if (['set_profile_model', 'set_profile_rule', 'set_profile_effort'].includes(command)) {
        const profile = state.config.profiles.find((p) => p.id === args.profileId);
        if (!profile) throw new Error('QA 接口不存在。');
        if (command === 'set_profile_model') profile.model = String(args.model);
        if (command === 'set_profile_rule') profile.rulePresetId = args.rulePresetId as string | null;
        if (command === 'set_profile_effort')
            profile.reasoningEffort = args.reasoningEffort as typeof profile.reasoningEffort;
        // 替身只显式标记条件过期，不复制后端指纹算法，也不宣称验证了真实推理结果。
        state.conditionFingerprints = {
            ...state.conditionFingerprints,
            [profile.id]: `qa-condition-${++qaRun}`,
        };
        changed.forEach((f) => f());
        return;
    }
    // 只确认前端发出了撤销请求；浏览器没有原生拖动线程，不能冒充验证Win32收尾。
    if (command === 'cancel_pet_drag') return;
    if (command === 'set_pet_scene_layout') {
        const candidate = args.scene as {
            width: number;
            height: number;
            actorBounds: number[];
            dockBounds: number[];
            regions: number[][];
        };
        for (const rect of [candidate.actorBounds, candidate.dockBounds, ...candidate.regions]) {
            if (
                rect.length !== 4 ||
                rect.some((n) => !Number.isFinite(n) || n < 0) ||
                rect[2]! <= 0 ||
                rect[3]! <= 0 ||
                rect[0]! + rect[2]! > candidate.width + 1 ||
                rect[1]! + rect[3]! > candidate.height + 1
            ) {
                rejectedScenes.push(candidate);
                throw new Error('桌宠角色或吸附轮廓超出场景。');
            }
        }
        scenes.push(args.scene);
        await sceneObserver?.(args.scene);
        const scene = args.scene as { docking?: string | null };
        dock.tucked = ['left', 'right', 'top'].includes(scene.docking ?? '');
        return;
    }
    if (command === 'clear_pet_scene') {
        await classicObserver?.(Boolean(args.expanded));
        return;
    }
    if (command === 'list_skins')
        return [...imported.values()].map((p) => ({
            id: p.manifest.id,
            name: p.manifest.name,
            author: p.manifest.author,
            builtin: false,
        }));
    if (command === 'get_skin') {
        const pack = imported.get(String(args.id));
        if (!pack) throw new Error('QA 皮肤不存在。');
        return pack;
    }
    if (command === 'install_skin') {
        const pack = args.pack as SkinPack;
        imported.set(pack.manifest.id, pack);
        return;
    }
    if (command === 'remove_skin') {
        imported.delete(String(args.id));
        return;
    }
    if (command === 'preview_history_retention')
        return { removedCount: 0, remainingCount: state.history.length };
    if (command === 'save_preferences') {
        state.config.preferences = structuredClone(args.preferences) as AppSnapshot['config']['preferences'];
        changed.forEach((f) => f());
        return;
    }
    return base.call(command, args);
}
export async function subscribe(handler: () => void): Promise<() => void> {
    changed.add(handler);
    return () => {
        changed.delete(handler);
    };
}
export async function subscribeDragEnd(handler: () => void): Promise<() => void> {
    released.add(handler);
    return () => {
        released.delete(handler);
    };
}
/** 注册后立即给出当前替身状态；卸载删除监听，避免切换皮肤累积回调。 */
export async function subscribeDock(handler: (dock: QaDock) => void): Promise<() => void> {
    dockChanged.add(handler);
    handler({ ...dock });
    return () => {
        dockChanged.delete(handler);
    };
}
export function registerSceneLayout(
    handler: (expanded: boolean, menuHeight: number) => Promise<void>,
): () => void {
    layout = handler;
    return () => {
        if (layout === handler) layout = undefined;
    };
}
export async function resizeWidget(expanded: boolean, menuHeight = 0): Promise<void> {
    if (layout) await layout(expanded, menuHeight);
    else await classicObserver?.(expanded);
}
export async function dragWindow(): Promise<void> {}
/** 只记录实际拖动意图；不移动系统窗口，不能把此断言当成原生抓取点验收。 */
export const dragRequests: [number, number][] = [];
export async function dragPetWindow(pointerDelta: [number, number]): Promise<void> {
    dragRequests.push([...pointerDelta]);
}
export async function openSettings(): Promise<void> {}
