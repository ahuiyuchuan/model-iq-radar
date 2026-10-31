<!-- 桌宠场景：只组合角色与原面板；付费请求和测试调度仍由原来的业务入口负责。@author ahui -->
<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, shallowRef, watch } from 'vue';
import {
    Target,
    Settings2,
    X,
    CircleCheck,
    TriangleAlert,
    CircleX,
    CircleHelp,
    Code2,
    ChevronRight,
    Hand,
} from 'lucide-vue-next';
import {
    call,
    desktop,
    dragPetWindow,
    openSettings,
    registerSceneLayout,
    subscribeDragEnd,
    subscribeDock,
    windowAction,
} from '../bridge';
import { DEFAULT_SKIN, type DockSide, type LoadedSkin, type PetAction, type Rect } from '../pet/types';
import { loadSkin } from '../pet/skinLoader';
import { chooseAction, nextAmbient, petResult, resolveIdleAction, ResultFeedback } from '../pet/controller';
import {
    contextMenuLayout,
    fitSceneLayout,
    peekLayout,
    rightSceneLayout,
    sceneLayout,
    type SceneLayout,
} from '../pet/layout';
import type { AppSnapshot, TestRecord } from '../types';
import PetActor from './PetActor.vue';
import PetOrnaments from './PetOrnaments.vue';

const props = defineProps<{
    snapshot: AppSnapshot | null;
    latest: TestRecord | null;
    busy: boolean;
    previewExpanded: boolean;
    contentExpanded?: boolean;
    /** 当前条件变化时旧记录失效，不能把历史通过继续显示在常驻云朵。 */
    stale?: boolean;
    /** Widget 实测主体高度，不包含下拉菜单。 */
    contentHeight?: number;
    /** 相对卡片左上角的可见浮层，单独扩展原生窗口与输入区域。 */
    floatingBounds?: Rect[];
}>();
const emit = defineEmits<{ active: [value: boolean]; visible: [value: boolean]; error: [message: string] }>();
const skin = shallowRef<LoadedSkin | null>(null);
const panel = ref(false),
    menu = ref(false),
    dragging = ref(false),
    bubble = ref('');
const actionMenu = ref(false);
let actionCloseTimer: ReturnType<typeof setTimeout> | undefined;
// 一次菜单会话预留两层几何；隐藏准备和绘制清理结束后才归还窗口尺寸。
const contextLayout = shallowRef<SceneLayout | null>(null);
const menuReady = ref(false);
const submenuLeft = ref(false);
let menuGeneration = 0;
const actionTrigger = ref<HTMLButtonElement>();
/** 仅列出当前皮肤真正提供的动作；点播只是本地姿态预览，不改写业务结果或模拟真实拖动。 */
const actionLabels: Record<PetAction, string> = {
    idle: '待机',
    petting: '摸摸头',
    wave: '招招手',
    stretch: '伸个懒腰',
    bow: '小幅鞠躬',
    seated: '侧坐休息',
    dragging: '悬空姿态',
    landing: '放下站稳',
    working: '测试中姿态',
    success: '不降智姿态',
    failed: '降智姿态',
    error: '失败姿态',
};
const menuActions = computed(() =>
    (Object.keys(actionLabels) as PetAction[]).filter((kind) => skin.value?.manifest.actions[kind]),
);
const transient = ref<PetAction | null>(null);
// 用户主动动作拥有短暂呈现权；结果和自动收纳不能在点播尚未结束时抢占。
const manualActivity = ref(false);
watch(
    transient,
    (value) => {
        if (value === null) manualActivity.value = false;
    },
    { flush: 'sync' },
);
const ambient = ref<PetAction | null>(null);
const pageVisible = ref(!document.hidden);
// 桌宠贴边由场景独立管理，不能与 Widget 的经典小图标共享尺寸和恢复时序。
const docked = ref(false);
const edgeSide = ref<string | null>(null);
// 完整布局朝向独立于贴边检测；拖到屏幕中央或顶部不会重置到卡片左侧。
const sceneSide = ref<'left' | 'right'>('left');
const stageElement = ref<HTMLElement>();
let dockTimer: ReturnType<typeof setTimeout> | undefined;
let peekClickUntil = 0;
// 布局移到静止光标下也会派发pointerenter；只有屏幕坐标实际移动才是恢复意图。
// clientX会随窗口原点变化，不能用来判断用户是否移动鼠标。
let peekHoverPoint: [number, number] | null = null;
// 收纳只改变呈现，不清除panel：再次悬停恢复用户原来的紧凑/详情卡片。
const dockMotion = ref<'' | 'enter'>('');
let dockTarget = false;
let dockMotionGeneration = 0;
let motionTimer: ReturnType<typeof setTimeout> | undefined;
let finishMotionWait: ((completed: boolean) => void) | undefined;
// 原生拖动拥有窗口与抓取点；期间收到方向变化只暂存，松手后再重新布置场景。
let pendingDockSide: string | null | undefined;
let unsubscribeDock: (() => void) | undefined;
const dockClip = computed(() => skin.value?.manifest.docking?.left);
const canDock = computed(() => Boolean(dockClip.value && skin.value?.dockPoster));
const dockSide = computed<DockSide | null>(() =>
    ['left', 'right', 'top'].includes(edgeSide.value ?? '') ? (edgeSide.value as DockSide) : null,
);
const peek = computed(() =>
    dockClip.value && dockSide.value ? peekLayout(dockClip.value, dockSide.value) : null,
);
const snapEnabled = computed(() => props.snapshot?.config.preferences.snapToEdge !== false);
// 命中轮廓跟随已绘制动作；下一图集加载期间保留当前画面及其可点击范围。
const displayedAction = ref<PetAction>('idle');
const actorBounds = ref<Rect[] | null>(null);
const look = ref<[number, number]>([0, 0]);
const dragVelocity = ref<[number, number]>([0, 0]);
let previousPointer: { x: number; y: number; at: number } | undefined;
let pointerRest: ReturnType<typeof setTimeout> | undefined;
const panelHeight = ref(176);
const preferences = computed(() => ({ ...DEFAULT_SKIN, ...props.snapshot?.config.preferences.skin }));
const active = computed(() => Boolean(skin.value));
const result = computed(() => petResult(props.busy, props.latest, Boolean(props.stale)));
// 点击确认结束本次结果提示和结果姿态，不改写结果。轮询、临时动作、展开收起不复活旧提示；
// 新任务、记录或条件状态变化重新显示，不持久化到皮肤或数据库。
// 例外：收纳期间卡片不可见，进行中的任务必须在探头旁继续显示进度。
const cloudIdentity = computed(() =>
    JSON.stringify([
        result.value.key,
        props.latest?.id,
        props.latest?.status,
        props.busy ? props.snapshot?.running?.id : null,
        Boolean(props.stale),
    ]),
);
const dismissedCloud = ref<string | null>(null);
const cloudVisible = computed(
    () => dismissedCloud.value !== cloudIdentity.value || (docked.value && props.busy),
);
const action = computed(() => {
    // 气泡确认也确认本次表情；不能只隐藏文案，却在收起面板后恢复旧庆祝／错误姿态。
    // busy本身仍用于测试与输入规则，只在呈现优先级中屏蔽已确认的working动作。
    const resting =
        cloudVisible.value && result.value.action && skin.value?.manifest.actions[result.value.action]
            ? result.value.action
            : null;
    const chosen = chooseAction(
        dragging.value,
        panel.value ? null : transient.value,
        props.busy && cloudVisible.value,
        panel.value,
        resting,
        resolveIdleAction(preferences.value.idleAction, Object.keys(skin.value?.manifest.actions ?? {})),
    );
    return !dragging.value && !transient.value && !props.busy && !panel.value
        ? (ambient.value ?? chosen)
        : chosen;
});
const normalLayout = computed(() =>
    sceneLayout(
        preferences.value.size,
        panel.value,
        props.contentHeight && Number.isFinite(props.contentHeight) ? props.contentHeight : panelHeight.value,
        true,
        false,
        props.floatingBounds,
        false,
    ),
);
const layout = computed(() => {
    const normal = normalLayout.value;
    if (!docked.value || !peek.value) {
        const context = contextLayout.value;
        if (!context) return sceneSide.value === 'right' ? rightSceneLayout(normal) : normal;
        // 菜单关闭需保留两帧原几何，但此时面板可能已经展开／增高；不能提交旧范围配新卡片。
        const current: SceneLayout = {
            ...context,
            panel: [context.panel[0], context.panel[1], normal.panel[2], normal.panel[3]],
            floating: normal.floating.map(
                ([x, y, w, h]) =>
                    [
                        context.panel[0] + x - normal.panel[0],
                        context.panel[1] + y - normal.panel[1],
                        w,
                        h,
                    ] as Rect,
            ),
        };
        return fitSceneLayout(current, [
            current.pet,
            current.bubble,
            current.menu,
            current.actions,
            ...(panel.value ? [current.panel, ...current.floating] : []),
        ]);
    }
    return {
        ...normal,
        width: peek.value.width,
        height: peek.value.height,
        pet: [peek.value.peek[0], 0, 96, 96] as Rect,
        bubble: peek.value.bubble,
        floating: [],
    };
});
// 角色是用户主动启用的持续动画；以本应用的明确开关为准，避免系统设置让桌宠无声冻结。
const animate = computed(() => preferences.value.animation);
const feedback = new ResultFeedback();
let generation = 0,
    layoutRevision = 0,
    disposed = false;
const session = crypto.randomUUID();
let nativeQueue: Promise<void> = Promise.resolve();
let bubbleTimer: ReturnType<typeof setTimeout> | undefined;
let transientTimer: ReturnType<typeof setTimeout> | undefined;
let clickTimer: ReturnType<typeof setTimeout> | undefined;
let longPressTimer: ReturnType<typeof setTimeout> | undefined;
let unregister: (() => void) | undefined, unsubscribeDrag: (() => void) | undefined;
let press: { x: number; y: number; moved: boolean } | null = null;
let cloudWasTucked = false;
// 卡片里的文本选择、按钮按下同样拥有输入，不能只保护角色拖动。
let stagePressed = false;
let suppressClickUntil = 0;
let deferred: { id: string; status: TestRecord['status']; at: number } | null = null;
let ambientTimer: ReturnType<typeof setTimeout> | undefined;
let ambientEnd: ReturnType<typeof setTimeout> | undefined;
let previousAmbient: PetAction | null = null;
const canIdle = computed(
    () =>
        active.value &&
        preferences.value.animation &&
        preferences.value.autonomous !== false &&
        pageVisible.value &&
        !props.busy &&
        !dragging.value &&
        !docked.value &&
        !dockSide.value &&
        !dockMotion.value &&
        !transient.value &&
        !panel.value &&
        !menu.value &&
        !props.previewExpanded,
);

/** 撤销过期过渡并释放等待者；换肤、隐藏及拖动不得被旧异步任务重新收起。 */
function cancelDockMotion(): void {
    dockMotionGeneration++;
    clearTimeout(motionTimer);
    finishMotionWait?.(false);
    finishMotionWait = undefined;
    dockMotion.value = '';
}

/** 有界动画等待；取消会立即结束Promise，不残留未完成的换肤/卸载任务。 */
function waitDockMotion(milliseconds: number): Promise<boolean> {
    return new Promise((resolve) => {
        finishMotionWait = resolve;
        motionTimer = setTimeout(() => {
            finishMotionWait = undefined;
            resolve(true);
        }, milliseconds);
    });
}

/**
 * 目标立即接管，保留不透明画面，仅在最终几何内小幅滑入；不排队等待旧动作退场。
 * 新目标取消旧等待，原生提交仍串行采样最新布局，避免迟到任务重新收起。
 * @param tucked true收为探头，false恢复原场景。
 * @param immediate 拖动/禁用/隐藏等高优先级退出跳过装饰动画。
 */
async function changeDock(tucked: boolean, immediate = false): Promise<void> {
    if (dockTarget === tucked && (dockMotion.value || docked.value === tucked) && !immediate) return;
    cancelDockMotion();
    dockTarget = tucked;
    peekHoverPoint = null;
    const current = dockMotionGeneration;
    docked.value = tucked;
    if (immediate || !animate.value || !pageVisible.value || !active.value) {
        return;
    }
    dockMotion.value = 'enter';
    await nextTick();
    await safelyLayout();
    if (disposed || current !== dockMotionGeneration || !active.value) return;
    if (!(await waitDockMotion(160)) || current !== dockMotionGeneration) return;
    dockMotion.value = '';
    scheduleDock();
}

/** 仅离开且无输入占用时延迟收起；普通面板可收纳，菜单、拖拽和键盘焦点优先。 */
function scheduleDock(): void {
    clearTimeout(dockTimer);
    if (
        !canDock.value ||
        !snapEnabled.value ||
        !dockSide.value ||
        docked.value ||
        disposed ||
        manualActivity.value
    )
        return;
    dockTimer = setTimeout(() => {
        // 鼠标点击遗留的焦点不能永久阻止收起；仅保护真实键盘焦点。
        const focused =
            stageElement.value?.contains(document.activeElement) &&
            document.activeElement?.matches(':focus-visible');
        if (
            disposed ||
            dragging.value ||
            menu.value ||
            manualActivity.value ||
            props.floatingBounds?.length ||
            props.previewExpanded ||
            !pageVisible.value
        )
            return;
        // 恢复会改变原生窗口边界，WebView 未必派发 pointerleave。仅靠事件会卡在完整形态。
        // 贴边且可收起时低频重查真实 hover/focus，不依赖闭包里缓存的输入状态。
        // 窗口包含卡片上方和菜单旁的透明空白；它们不拥有交互，不能用整个stage的:hover
        // 阻止收纳。否则鼠标已经离开可见角色/卡片，右侧展开布局仍会永久保持打开。
        const hovered = stageElement.value?.querySelector(
            '.pet-character:hover, .pet-result-cloud:hover, .pet-panel:not([inert]):hover, .pet-edge-peek:hover, .pet-context-menu:hover',
        );
        if (hovered || focused || press || stagePressed) {
            scheduleDock();
            return;
        }
        clearTimeout(clickTimer);
        clearTimeout(longPressTimer);
        clearTimeout(transientTimer);
        clearTimeout(ambientEnd);
        ambient.value = null;
        transient.value = null;
        bubble.value = '';
        void changeDock(true);
    }, 180);
}

/** 悬停恢复上次场景，点击/Enter打开卡片；恢复不触发测试或改写真实结果。 */
function revealDock(openPanel = false, focusCharacter = false, immediate = false): void {
    clearTimeout(dockTimer);
    if (docked.value && !openPanel) peekClickUntil = performance.now() + 550;
    void changeDock(false, immediate);
    if (openPanel) panel.value = true;
    if (focusCharacter)
        void nextTick(() =>
            stageElement.value
                ?.querySelector<HTMLButtonElement>(
                    cloudVisible.value ? '.pet-result-cloud' : '.pet-character',
                )
                ?.focus({ preventScroll: true }),
        );
    scheduleDock();
}
/**
 * 进入只记录屏幕坐标，移动至少2逻辑像素才恢复；几何重排导致的enter/move不会自动弹回。
 * 点击、右键和键盘仍立即恢复，动画关闭时同样使用此门禁，不靠延迟或动画时钟兜底。
 */
function hoverPeek(event: PointerEvent): void {
    if (!docked.value || dragging.value || press) return;
    const point: [number, number] = [event.screenX, event.screenY];
    if (!peekHoverPoint) {
        peekHoverPoint = point;
        return;
    }
    if (Math.hypot(point[0] - peekHoverPoint[0], point[1] - peekHoverPoint[1]) >= 2) revealDock();
}
function enterStage(): void {
    // 鼠标返回完整场景时可逆转正在进行的退场，不能在用户操作中途完成收纳。
    if (dockTarget && !docked.value) revealDock();
    scheduleDock();
}
/** 焦点进入时取消收起，避免键盘操作中窗口突然变成探头。 */
function focusStage(): void {
    scheduleDock();
}
/** 悬停会先替换探头节点；捕获紧随其后的真实点击，避免点击掉进恢复后的透明区域。 */
function clickAfterPeek(event: MouseEvent): void {
    if (event.button !== 0 || performance.now() >= peekClickUntil || dragging.value) return;
    const control = (event.target as Element | null)?.closest(
        'button, input, select, textarea, a, [role="combobox"]',
    );
    // 仅补接落在探头/角色/云朵或透明场景的首次点击。恢复后直接点击真实控件必须交给控件，
    // 否则短过渡让“收起详情”等按钮落入550ms窗口，被误吞成再次打开面板。
    if (control && !control.matches('.pet-character, .pet-result-cloud, .pet-edge-peek')) {
        peekClickUntil = 0;
        return;
    }
    peekClickUntil = 0;
    event.preventDefault();
    event.stopPropagation();
    press = null;
    clearTimeout(longPressTimer);
    revealDock(true);
}
function leaveStage(event: PointerEvent): void {
    // 窗口缩放/捕获释放后可能没有收到按钮自己的 pointerup；未按键时清理过期按下。
    // 真实原生拖动仍由终态事件收尾，不在这里提前释放拖动所有权。
    if (!event.buttons && !dragging.value) {
        press = null;
        clearTimeout(longPressTimer);
    }
    scheduleDock();
}
watch(
    [
        panel,
        menu,
        dragging,
        manualActivity,
        pageVisible,
        () => props.previewExpanded,
        () => props.floatingBounds?.length,
        canDock,
        snapEnabled,
    ],
    () => {
        if (
            !canDock.value ||
            !snapEnabled.value ||
            menu.value ||
            manualActivity.value ||
            props.floatingBounds?.length ||
            dragging.value ||
            props.previewExpanded ||
            !pageVisible.value
        )
            revealDock(false, false, true);
        scheduleDock();
    },
);
// 用户关闭动画时立即完成当前目标，不能留下半透明角色或仍在占用输入的过渡区。
watch(animate, (enabled) => {
    if (!enabled && dockMotion.value) void changeDock(dockTarget, true);
});

/** 自主动作与业务/用户交互分开计时；任何高优先级操作立即取消，恢复空闲后重新等待。
 * 不移动窗口、不触发模型或邮件；卸载、隐藏和关闭动画必须清除计时器。
 */
function scheduleAmbient(): void {
    clearTimeout(ambientTimer);
    if (!canIdle.value || disposed) return;
    ambientTimer = setTimeout(
        () => {
            if (!canIdle.value || !skin.value || disposed) return;
            const next = nextAmbient(
                Object.keys(skin.value.manifest.actions),
                previousAmbient,
                Math.random(),
            );
            if (next) {
                previousAmbient = next;
                ambient.value = next;
                ambientEnd = setTimeout(
                    () => {
                        ambient.value = null;
                        scheduleAmbient();
                    },
                    next === 'seated' ? 4500 : 2600,
                );
            } else scheduleAmbient();
        },
        7000 + Math.random() * 9000,
    );
}
watch([canIdle, () => preferences.value.id], () => {
    clearTimeout(ambientTimer);
    clearTimeout(ambientEnd);
    ambient.value = null;
    scheduleAmbient();
});
function visibleChanged(): void {
    pageVisible.value = !document.hidden;
    if (!pageVisible.value) {
        stagePressed = false;
        cloudWasTucked = false;
        // 隐藏可能丢失pointerup；恢复显示后的普通移动不能消费上次未完成的按下。
        // 已接管的原生拖动仍由finishDrag收尾，不在这里提前归还窗口所有权。
        press = null;
        clearTimeout(dockTimer);
        clearTimeout(clickTimer);
        clearTimeout(longPressTimer);
        clearTimeout(transientTimer);
        transient.value = null;
        bubble.value = '';
        deferred = null;
    }
}

/** 双击或菜单主动邀请活动；不会伪装测试通过，结束后恢复当前业务姿态。 */
function activity(kind: PetAction): void {
    if (dragging.value || performance.now() < suppressClickUntil) return;
    clearTimeout(clickTimer);
    manualActivity.value = true;
    transient.value = skin.value?.manifest.actions[kind] ? kind : 'petting';
    say(kind === 'wave' ? '你好呀～' : kind === 'stretch' ? '伸个懒腰～' : `动作：${actionLabels[kind]}`);
    clearTimeout(transientTimer);
    transientTimer = setTimeout(() => {
        transient.value = null;
    }, 2600);
}

const position = (rect: Rect) => ({
    left: `${rect[0]}px`,
    top: `${rect[1]}px`,
    width: `${rect[2]}px`,
    height: `${rect[3]}px`,
});
const theme = computed(() =>
    skin.value
        ? {
              '--pet-panel': skin.value.manifest.tokens.panelBackground,
              '--pet-border': skin.value.manifest.tokens.panelBorder,
              '--pet-primary': skin.value.manifest.tokens.primary,
              '--pet-text': skin.value.manifest.tokens.textPrimary,
              // 卡片按角色画布计算交互避让，不能仅绕开可见像素后仍被透明按钮挡住。
              '--pet-size': `${preferences.value.size}px`,
          }
        : {},
);

/** 切换采用先加载后替换；迟到结果销毁，失败回退经典界面且不反复重试网络或业务请求。 */
watch(
    () => preferences.value.id,
    async (id) => {
        const current = ++generation;
        if (skin.value && desktop) {
            try {
                // 原生拖动与松手后的延迟保存都独立于DOM；即便已松手，换肤也必须撤销旧意图，
                // 不能让新皮肤复用组件session后消费上一皮肤尚未对齐的保存请求。
                await call('cancel_pet_drag');
            } catch (cause) {
                if (!disposed && current === generation) await fail(`停止拖动失败：${String(cause)}`);
                return;
            }
            if (disposed || current !== generation) return;
        }
        transient.value = null;
        deferred = null;
        dismissedCloud.value = null;
        clearTimeout(transientTimer);
        clearTimeout(bubbleTimer);
        clearTimeout(clickTimer);
        clearTimeout(longPressTimer);
        press = null;
        cloudWasTucked = false;
        stagePressed = false;
        dragging.value = false;
        bubble.value = '';
        menu.value = false;
        menuGeneration++;
        menuReady.value = false;
        contextLayout.value = null;
        docked.value = false;
        dockTarget = false;
        cancelDockMotion();
        edgeSide.value = null;
        pendingDockSide = undefined;
        clearTimeout(dockTimer);
        if (id === 'classic') {
            skin.value?.dispose();
            skin.value = null;
            return;
        }
        try {
            const loaded = await loadSkin(id);
            if (disposed || current !== generation) {
                loaded.dispose();
                return;
            }
            skin.value?.dispose();
            skin.value = loaded;
            panel.value = false;
            actorBounds.value = null;
            panelHeight.value = props.contentExpanded ? 546 : 176;
            // 活动皮肤之间切换不会触发 active 的真假变化；已有监听继续使用，但位置需重新核对。
            if (desktop && unsubscribeDock) {
                await nextTick();
                await applyLayout();
                const dock = await call<{ side: string | null; tucked: boolean }>('get_widget_dock');
                if (disposed || current !== generation) return;
                edgeSide.value = dock.side;
                if (dock.side === 'left' || dock.side === 'right') sceneSide.value = dock.side;
                scheduleDock();
            }
        } catch (cause) {
            if (!disposed && current === generation) {
                await fail(`皮肤不可用：${String(cause)}`);
            }
        }
    },
    { immediate: true },
);

/** 场景所有原生更新使用同一递增序号；位图可见区域和面板/菜单合并后交给 Rust。 */
async function commitLayout(): Promise<void> {
    if (!active.value || disposed || props.previewExpanded) return;
    const l = layout.value;
    const m = skin.value!.manifest;
    const clip =
        docked.value && dockClip.value
            ? dockClip.value
            : (m.actions[displayedAction.value] ?? m.actions.idle);
    const scale = l.pet[2] / clip.frameSize;
    // 实时跟随开始前，悬空姿态可能把鼠标位置裁成透明洞。按住期间暂保留完整输入区域，
    // 松手立即恢复角色轮廓，避免换姿态时失去拖动或把输入送给下面的应用。
    // actorBounds只描述角色，用于吸附后的精确对齐；贴边检测使用独立dockBounds，
    // 让展开卡片或常驻云朵碰到右边缘时也可收纳，不要求用户把角色先拖出屏幕。
    const actorRegions =
        docked.value && peek.value
            ? peek.value.regions
            : (actorBounds.value ?? clip.hitBands).map(
                  ([x, y, w, h]) =>
                      [l.pet[0] + x * scale, l.pet[1] + y * scale, w * scale, h * scale] as Rect,
              );
    // 滑动期间覆盖本场景输入边界，避免移动中的角色失去输入；完成立即恢复精确掩码。
    const regions: Rect[] =
        dragging.value || dockMotion.value ? [[0, 0, l.width, l.height]] : [...actorRegions];
    const left = Math.min(...actorRegions.map(([x]) => x));
    const top = Math.min(...actorRegions.map(([, y]) => y));
    const actorBox = [
        left,
        top,
        Math.max(...actorRegions.map(([x, , w]) => x + w)) - left,
        Math.max(...actorRegions.map(([, y, , h]) => y + h)) - top,
    ];
    if (!docked.value) {
        if (panel.value) regions.push(l.panel, ...l.floating);
        if (menu.value && menuReady.value) {
            regions.push(l.menu);
            if (actionMenu.value) regions.push(l.actions);
        }
    }
    // 已确认的云朵立即退出命中掩码；保留布局留白以固定角色位置，透明处可穿透。
    if (cloudVisible.value) regions.push(l.bubble);
    // 临时菜单/选择器不能触发吸附；只合并本次实际显示的角色、常驻云朵与主体卡片。
    const dockParts: Rect[] = [actorBox as Rect, ...(cloudVisible.value ? [l.bubble] : [])];
    if (panel.value && !docked.value) dockParts.push(l.panel);
    const dockLeft = Math.min(...dockParts.map(([x]) => x));
    const dockTop = Math.min(...dockParts.map(([, y]) => y));
    const dockBounds = [
        dockLeft,
        dockTop,
        Math.max(...dockParts.map(([x, , w]) => x + w)) - dockLeft,
        Math.max(...dockParts.map(([, y, , h]) => y + h)) - dockTop,
    ];
    if (desktop)
        await call('set_pet_scene_layout', {
            scene: {
                session,
                revision: ++layoutRevision,
                width: l.width,
                height: l.height,
                anchor: [l.pet[0] + l.pet[2] / 2, l.pet[1] + l.pet[3] * 0.92],
                // 持久化使用默认朝向的锚点基准，重启默认布局时不会把右侧反射原点当角色位置。
                canonicalAnchor: [
                    normalLayout.value.pet[0] + normalLayout.value.pet[2] / 2,
                    normalLayout.value.pet[1] + normalLayout.value.pet[3] * 0.92,
                ],
                docking: docked.value ? dockSide.value : null,
                actorBounds: actorBox,
                dockBounds,
                regions,
            },
        });
}
/** 串行提交和退出；异步切换不会在经典窗口恢复后再写入迟到的桌宠 Region。 */
function applyLayout(): Promise<void> {
    const next = nativeQueue.catch(() => {}).then(commitLayout);
    nativeQueue = next;
    return next;
}
async function safelyLayout(): Promise<void> {
    try {
        await applyLayout();
    } catch (cause) {
        await fail(`窗口布局失败：${String(cause)}`);
    }
}
/**
 * 任一资源/渲染故障先完成原生恢复，再把错误交给宿主展开详情。
 * 先发错误会让 Widget 的展开与经典 156px 重置竞态，最终截断可见内容；不可倒置此顺序。
 * @param message 不含凭据的可读故障信息；恢复成功后附加经典界面提示。
 */
async function fail(message: string): Promise<void> {
    cancelDockMotion();
    const current = generation;
    const wasActive = active.value;
    skin.value?.dispose();
    skin.value = null;
    // 已激活场景由监听退出；首次加载失败时 active 始终为 false，必须主动恢复。
    await nextTick();
    if (wasActive) await nativeQueue.catch(() => {});
    else await restoreClassicLayout();
    if (!disposed && current === generation && !active.value) emit('error', `${message} 已恢复经典界面。`);
}

/**
 * 串行恢复经典原生窗口，供已激活场景退出和首次加载失败共用。
 * 执行前重查 active，避免排队期间已加载的新皮肤被旧失败回调清除；错误保留为可见提示。
 */
async function restoreClassicLayout(): Promise<void> {
    if (!desktop) return;
    try {
        const next = nativeQueue
            .catch(() => {})
            .then(async () => {
                if (!active.value && !disposed)
                    await call('clear_pet_scene', { expanded: Boolean(props.contentExpanded) });
            });
        nativeQueue = next;
        await next;
    } catch (cause) {
        emit('error', String(cause));
    }
}

watch(active, async (value) => {
    emit('active', value);
    emit('visible', value ? panel.value : true);
    if (value) {
        const activatingSkin = skin.value;
        unregister = registerSceneLayout(async (expanded, menuHeight) => {
            // 旧桥接仍可请求主体模式；菜单高度不能再被当成卡片自身高度。
            void menuHeight;
            panelHeight.value = expanded ? 546 : 176;
            await nextTick();
            await applyLayout();
        });
        if (!unsubscribeDrag) {
            const subscribingFor = skin.value;
            try {
                const cleanup = await subscribeDragEnd(finishDrag, (velocity) => {
                    if (!dragging.value) return;
                    dragVelocity.value = velocity;
                    clearTimeout(pointerRest);
                    pointerRest = setTimeout(() => {
                        dragVelocity.value = [0, 0];
                    }, 100);
                });
                if (disposed || skin.value !== subscribingFor) cleanup();
                else unsubscribeDrag = cleanup;
            } catch (cause) {
                if (!disposed && skin.value === subscribingFor) await fail(String(cause));
            }
        }
        // 首次提交真实角色几何后才读取贴边状态；否则原生会按经典面板宽度夹紧保存位置，
        // 使右侧/顶部重启后丢失方向。等待期间换肤或回退时，不再注册过期场景监听。
        await safelyLayout();
        if (disposed || !active.value || skin.value !== activatingSkin) return;
        if (!unsubscribeDock) {
            const subscribingFor = skin.value;
            try {
                const cleanup = await subscribeDock((dock) => {
                    if (disposed || !active.value) return;
                    if (dragging.value) {
                        pendingDockSide = dock.side;
                        return;
                    }
                    edgeSide.value = dock.side;
                    if (dock.side === 'left' || dock.side === 'right') sceneSide.value = dock.side;
                    if (!dockSide.value || !canDock.value) revealDock();
                    else scheduleDock();
                });
                if (disposed || skin.value !== subscribingFor) cleanup();
                else unsubscribeDock = cleanup;
            } catch (cause) {
                if (!disposed && skin.value === subscribingFor) await fail(`贴边监听失败：${String(cause)}`);
            }
        }
        await safelyLayout();
    } else {
        unregister?.();
        unregister = undefined;
        menu.value = false;
        bubble.value = '';
        unsubscribeDrag?.();
        unsubscribeDrag = undefined;
        unsubscribeDock?.();
        unsubscribeDock = undefined;
        clearTimeout(dockTimer);
        await restoreClassicLayout();
    }
});
watch(
    [
        layout,
        menu,
        actionMenu,
        menuReady,
        displayedAction,
        dragging,
        actorBounds,
        dockMotion,
        cloudVisible,
        () => props.previewExpanded,
    ],
    () => void safelyLayout(),
    { flush: 'post' },
);
watch([panel, docked], ([value, tucked]) => {
    emit('visible', value && !tucked);
    menu.value = false;
});
watch(
    () => [props.snapshot, props.latest],
    () => {
        const status = feedback.update(props.snapshot, props.latest);
        if (!status || !active.value) return;
        if (dragging.value) deferred = { id: props.latest!.id, status, at: performance.now() };
        else showResult(status);
    },
);
watch(
    () => [props.busy, props.stale, props.latest?.id, props.latest?.status],
    () => {
        // 同 ID 的记录也可能更新状态；反馈必须匹配“记录身份+真实结论”，不能只看 ID。
        if (
            props.busy ||
            props.stale ||
            (deferred && (deferred.id !== props.latest?.id || deferred.status !== props.latest?.status))
        ) {
            deferred = null;
        }
        if (
            (props.busy || props.stale || props.latest?.status !== 'passed') &&
            transient.value === 'success'
        ) {
            clearTimeout(transientTimer);
            transient.value = null;
        }
    },
);

/** 气泡仅本机短暂显示；关闭气泡时彻底清理计时器和占位。 */
function say(text: string): void {
    clearTimeout(bubbleTimer);
    bubble.value = preferences.value.bubbles ? text : '';
    bubbleTimer = setTimeout(() => {
        bubble.value = '';
    }, 2800);
}
watch(
    () => preferences.value.bubbles,
    (value) => {
        if (!value) {
            clearTimeout(bubbleTimer);
            bubble.value = '';
        }
    },
);
function showResult(status: TestRecord['status']): void {
    // 只给未确认的新通过短暂反馈；松手后的延迟结算也不能复活已确认的同一次庆祝。
    if (status === 'passed' && cloudVisible.value && !panel.value && !docked.value && !manualActivity.value) {
        transient.value = 'success';
        clearTimeout(transientTimer);
        transientTimer = setTimeout(() => {
            transient.value = null;
        }, 1800);
    }
}
function pet(): void {
    if (dragging.value || performance.now() < suppressClickUntil) return;
    manualActivity.value = true;
    transient.value = 'petting';
    say('嘿嘿～');
    clearTimeout(transientTimer);
    transientTimer = setTimeout(() => {
        transient.value = null;
    }, 1500);
}
/** 延迟单击以区分双击；不会先摸头再立即被招手打断。 */
function clickCharacter(): void {
    if (dragging.value || performance.now() < suppressClickUntil) return;
    clearTimeout(clickTimer);
    clickTimer = setTimeout(pet, 220);
}
function pointerDown(event: PointerEvent, longPress = true): void {
    if (event.button !== 0) return;
    clearTimeout(dockTimer);
    press = { x: event.screenX, y: event.screenY, moved: false };
    clearTimeout(longPressTimer);
    if (longPress)
        longPressTimer = setTimeout(() => {
            if (!press || press.moved || dragging.value) return;
            suppressClickUntil = performance.now() + 350;
            clearTimeout(clickTimer);
            menu.value = true;
            press = null;
        }, 550);
    (event.currentTarget as HTMLElement).setPointerCapture?.(event.pointerId);
}
/** 云朵与角色共用6px拖动阈值；按下不触发测试，探头先恢复以免拖着缩略布局离开边缘。 */
function cloudPointerDown(event: PointerEvent): void {
    if (event.button !== 0) return;
    cloudWasTucked = docked.value;
    if (docked.value) revealDock(false, false, true);
    peekClickUntil = 0;
    // 云朵长按仍允许开始拖动，不抢占为角色菜单；右键有独立入口。
    pointerDown(event, false);
}
/** 松手后的合成click不能再切面板；键盘激活(detail=0)仍可立即操作。 */
function clickCloud(event: MouseEvent): void {
    if (dragging.value || (event.detail > 0 && performance.now() < suppressClickUntil)) return;
    if (event.detail > 0 && cloudWasTucked) revealDock(true);
    else togglePanel();
    dismissedCloud.value = cloudIdentity.value;
    // 隐藏原焦点后键盘仍能继续操作；鼠标点击不强行移动焦点或滚动页面。
    if ((event.currentTarget as HTMLElement)?.matches(':focus-visible'))
        void nextTick(() =>
            stageElement.value
                ?.querySelector<HTMLButtonElement>('.pet-character')
                ?.focus({ preventScroll: true }),
        );
    cloudWasTucked = false;
}
/** 卡片标题栏和角色共用同一跟随链路；工具按钮、选择器和回答选择不会启动拖动。 */
function panelPointerDown(event: PointerEvent): void {
    if (event.button === 0) {
        stagePressed = true;
        if (dockTarget && !docked.value) revealDock(false, false, true);
    }
    const target = event.target as HTMLElement;
    if (active.value && target.closest('.drag-area') && !target.closest('button,input,select,textarea'))
        pointerDown(event);
}
async function pointerMove(event: PointerEvent): Promise<void> {
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    look.value = [
        Math.max(-1, Math.min(1, ((event.clientX - rect.left) / rect.width) * 2 - 1)),
        Math.max(-1, Math.min(1, ((event.clientY - rect.top) / rect.height) * 2 - 1)),
    ];
    const now = performance.now();
    if (previousPointer && now > previousPointer.at) {
        const dt = Math.max(0.016, (now - previousPointer.at) / 1000);
        dragVelocity.value = [
            (event.screenX - previousPointer.x) / dt,
            (event.screenY - previousPointer.y) / dt,
        ];
    }
    previousPointer = { x: event.screenX, y: event.screenY, at: now };
    clearTimeout(pointerRest);
    pointerRest = setTimeout(() => {
        dragVelocity.value = [0, 0];
    }, 90);
    if (!press || press.moved || Math.hypot(event.screenX - press.x, event.screenY - press.y) < 6) return;
    // 在任何 await 前捕获位移；原生接管期间 press 可能已被松手通知清理。
    const pointerDelta: [number, number] = [
        Math.round(event.screenX - press.x),
        Math.round(event.screenY - press.y),
    ];
    press.moved = true;
    peekClickUntil = 0;
    clearTimeout(longPressTimer);
    clearTimeout(clickTimer);
    dragging.value = true;
    menu.value = false;
    transient.value = null;
    clearTimeout(transientTimer);
    try {
        await nextTick();
        await applyLayout();
        await dragPetWindow(pointerDelta);
    } catch (cause) {
        say(`拖动失败：${String(cause)}`);
        finishDrag();
    }
}
/** 原生拖动可能吞掉页面 pointerup；后端终态通知与页面取消均使用幂等收尾。 */
function finishDrag(): void {
    stagePressed = false;
    const wasDragging = dragging.value;
    clearTimeout(longPressTimer);
    if (wasDragging) suppressClickUntil = performance.now() + 220;
    press = null;
    dragging.value = false;
    if (pendingDockSide !== undefined) {
        edgeSide.value = pendingDockSide;
        if (pendingDockSide === 'left' || pendingDockSide === 'right') sceneSide.value = pendingDockSide;
        pendingDockSide = undefined;
    }
    dragVelocity.value = [0, 0];
    previousPointer = undefined;
    if (!wasDragging) return;
    cloudWasTucked = false;
    scheduleDock();
    const pending = deferred;
    deferred = null;
    // 贴边拥有呈现优先级：不再经过landing→结果→idle才收纳；结论仍由常驻云朵表达。
    if (dockSide.value && canDock.value && snapEnabled.value) {
        clearTimeout(transientTimer);
        transient.value = null;
        ambient.value = null;
        return;
    }
    // 静态“放下站稳”驻留是输入收尾，不冒充连续动画；旧包缺少时直接回当前状态。
    const settle = () => {
        transient.value = null;
        if (
            pending &&
            pending.id === props.latest?.id &&
            pending.status === props.latest?.status &&
            !props.stale &&
            !props.busy &&
            performance.now() - pending.at < 8000
        )
            showResult(pending.status);
    };
    if (!panel.value && skin.value?.manifest.actions.landing) {
        transient.value = 'landing';
        clearTimeout(transientTimer);
        transientTimer = setTimeout(settle, 320);
    } else settle();
}
function release(): void {
    releaseStagePress();
    if (!desktop || !dragging.value) finishDrag();
}
/** 鼠标可在卡片外松开；全局监听只释放输入保护，不结束原生拖动所有权。 */
function releaseStagePress(): void {
    stagePressed = false;
    scheduleDock();
}
function openMenu(): void {
    clearTimeout(clickTimer);
    clearTimeout(longPressTimer);
    press = null;
    menu.value = !menu.value;
}
/** 两层之间留600ms通行时间；进入动作入口／子菜单或键盘聚焦子菜单时继续保留。
 * 仅收起二级，不改变预留窗口几何；父菜单关闭、换肤与卸载会取消唯一计时器。
 */
function scheduleActionClose(): void {
    clearTimeout(actionCloseTimer);
    if (!actionMenu.value) return;
    actionCloseTimer = setTimeout(() => {
        const child = stageElement.value?.querySelector<HTMLElement>('.pet-action-submenu');
        const focused =
            child?.contains(document.activeElement) && document.activeElement?.matches(':focus-visible');
        if (
            disposed ||
            !menu.value ||
            actionTrigger.value?.matches(':hover') ||
            child?.matches(':hover') ||
            focused
        )
            return;
        actionMenu.value = false;
    }, 600);
}
/** 悬停／键盘重新进入时撤销旧关闭意图，不堆积计时器。 */
function keepActionOpen(): void {
    clearTimeout(actionCloseTimer);
}
function openActionMenu(): void {
    keepActionOpen();
    actionMenu.value = true;
}
watch(actionMenu, (opened) => {
    if (!opened) keepActionOpen();
});
watch(menu, async (opened) => {
    const current = ++menuGeneration;
    menuReady.value = false;
    keepActionOpen();
    actionMenu.value = false;
    if (!opened) {
        // 先移除菜单并让浏览器提交一次空菜单画面，再缩窗，避免旧菜单合成残影被裁切。
        await nextTick();
        await new Promise<void>((resolve) =>
            requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
        );
        if (!disposed && current === menuGeneration) contextLayout.value = null;
        return;
    }
    try {
        const base = sceneSide.value === 'right' ? rightSceneLayout(normalLayout.value) : normalLayout.value;
        const fallback: Rect = [0, 0, screen.availWidth || innerWidth, screen.availHeight || innerHeight];
        const offset = [base.pet[0] - layout.value.pet[0], base.pet[1] - layout.value.pet[1]];
        const available = desktop ? await call<Rect | undefined>('get_pet_menu_bounds') : fallback;
        if (disposed || current !== menuGeneration || !menu.value) return;
        const bounds: Rect =
            available?.length === 4 && available.every(Number.isFinite)
                ? [
                      available[0] + offset[0]!,
                      available[1] + offset[1]!,
                      available[2] + offset[0]!,
                      available[3] + offset[1]!,
                  ]
                : fallback;
        const placed = contextMenuLayout(base, bounds);
        contextLayout.value = placed.layout;
        submenuLeft.value = placed.left;
        await nextTick();
        await applyLayout();
        if (disposed || current !== menuGeneration || !menu.value) return;
        menuReady.value = true;
        await nextTick();
        const root = stageElement.value?.querySelector<HTMLElement>('.pet-context-menu');
        if (!root?.contains(document.activeElement))
            root?.querySelector<HTMLButtonElement>('button')?.focus({ preventScroll: true });
    } catch (cause) {
        if (disposed || current !== menuGeneration) return;
        menu.value = false;
        say(`菜单布局失败：${String(cause)}`);
    }
});
/** 点播前收起卡片，保持“面板显示始终侧坐”；2600ms后重新读取当前真实业务姿态。 */
function playMenuAction(kind: PetAction): void {
    if (dragging.value || !menuActions.value.includes(kind)) return;
    panel.value = false;
    activity(kind);
    menu.value = false;
}
/** 按层导航：上下/Home/End只在当前层移动，右键展开，左键或Escape返回上层。 */
function menuKey(event: KeyboardEvent): void {
    const target = event.target as HTMLElement;
    const current = target.closest<HTMLElement>('[role="menu"]');
    if (!current) return;
    const nested = current.classList.contains('pet-action-submenu');
    if (event.key === 'Tab') {
        menu.value = false;
        return;
    }
    if (event.key === 'Escape' || (event.key === 'ArrowLeft' && nested)) {
        event.preventDefault();
        event.stopPropagation();
        if (nested) {
            actionMenu.value = false;
            void nextTick(() => actionTrigger.value?.focus());
        } else {
            menu.value = false;
            void nextTick(() =>
                stageElement.value?.querySelector<HTMLButtonElement>('.pet-character')?.focus(),
            );
        }
        return;
    }
    if (event.key === 'ArrowRight' && target === actionTrigger.value) {
        event.preventDefault();
        event.stopPropagation();
        openActionMenu();
        void nextTick(() =>
            stageElement.value?.querySelector<HTMLButtonElement>('.pet-action-submenu button')?.focus(),
        );
        return;
    }
    if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
    event.preventDefault();
    event.stopPropagation();
    const items = [...current.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')].filter(
        (item) => item.closest('[role="menu"]') === current && !item.disabled,
    );
    const index = items.indexOf(target as HTMLButtonElement);
    const next =
        event.key === 'Home'
            ? 0
            : event.key === 'End'
              ? items.length - 1
              : (index + (event.key === 'ArrowUp' ? -1 : 1) + items.length) % items.length;
    items[next]?.focus();
}
function togglePanel(): void {
    const wasTucked = docked.value || dockTarget;
    void changeDock(false);
    peekClickUntil = 0;
    // 展开要立即进入侧坐，不能让之前点击产生的摸头计时器覆盖设计中的展开姿态。
    transient.value = null;
    clearTimeout(transientTimer);
    clearTimeout(clickTimer);
    clearTimeout(longPressTimer);
    clearTimeout(bubbleTimer);
    bubble.value = '';
    panel.value = wasTucked || !panel.value;
}
function outside(event: PointerEvent): void {
    if (!(event.target as HTMLElement).closest('.pet-context-menu')) menu.value = false;
}
function escape(event: KeyboardEvent): void {
    if (event.key === 'Escape') menu.value = false;
}
onMounted(() => {
    document.addEventListener('pointerdown', outside);
    document.addEventListener('keydown', escape);
    document.addEventListener('visibilitychange', visibleChanged);
    document.addEventListener('pointerup', releaseStagePress);
    document.addEventListener('pointercancel', releaseStagePress);
});
onUnmounted(() => {
    disposed = true;
    generation++;
    unregister?.();
    unsubscribeDrag?.();
    unsubscribeDock?.();
    clearTimeout(dockTimer);
    keepActionOpen();
    skin.value?.dispose();
    cancelDockMotion();
    clearTimeout(bubbleTimer);
    clearTimeout(transientTimer);
    clearTimeout(ambientTimer);
    clearTimeout(ambientEnd);
    clearTimeout(pointerRest);
    clearTimeout(clickTimer);
    clearTimeout(longPressTimer);
    document.removeEventListener('pointerdown', outside);
    document.removeEventListener('keydown', escape);
    document.removeEventListener('visibilitychange', visibleChanged);
    document.removeEventListener('pointerup', releaseStagePress);
    document.removeEventListener('pointercancel', releaseStagePress);
});
/** 供宿主隐藏测量菜单后等待真实场景提交；失败由选择器关闭菜单并显示错误。 */
defineExpose({ syncLayout: applyLayout });
</script>
<template>
    <div
        ref="stageElement"
        :data-dock-motion="dockMotion"
        :data-edge="dockSide"
        :data-docked="docked"
        :class="[
            active ? 'pet-stage' : 'classic-stage',
            { 'pet-edge-right': active && sceneSide === 'right' },
        ]"
        :style="active ? { ...theme, width: `${layout.width}px`, height: `${layout.height}px` } : {}"
        @pointerdown="panelPointerDown"
        @pointermove="pointerMove"
        @pointerup="release"
        @pointercancel="release"
        @pointerenter="enterStage"
        @pointerleave="leaveStage"
        @focusin="focusStage"
        @focusout="scheduleDock"
        @click.capture="clickAfterPeek"
    >
        <div
            v-show="!active || !docked || previewExpanded"
            :class="active ? 'pet-panel' : ''"
            :style="active && !previewExpanded ? position(layout.panel) : {}"
            :inert="active && (!panel || docked) && !previewExpanded"
        >
            <slot :toggle-panel="togglePanel" />
            <PetOrnaments
                v-if="
                    skin &&
                    panel &&
                    !previewExpanded &&
                    skin.manifest.decorations &&
                    skin.manifest.decorations !== 'none'
                "
                :preset="skin.manifest.decorations"
                :animate="animate && pageVisible"
            />
        </div>
        <template v-if="skin && !previewExpanded">
            <!-- 鼠标按下不转移焦点：默认focus会提前移除探头，使随后click丢失。
                 键盘聚焦仍恢复场景并交接到云朵，Enter保持可用。 -->
            <button
                v-if="docked && peek && skin.dockPoster"
                class="pet-edge-peek"
                :data-side="dockSide"
                :style="position(peek.peek)"
                :aria-label="`${skin.manifest.name}：${result.label}，展开桌宠与测智面板`"
                :title="`${result.label} · 悬停恢复原场景，点击展开面板`"
                @pointerenter="hoverPeek"
                @pointermove="hoverPeek"
                @pointerleave="peekHoverPoint = null"
                @pointerdown.prevent
                @focus="revealDock(false, true)"
                @click="revealDock(true)"
                @contextmenu.prevent="
                    revealDock();
                    openMenu();
                "
            >
                <img
                    :src="skin.dockPoster"
                    :style="{ left: `${peek.imageLeft}px`, transform: peek.transform }"
                    alt=""
                    draggable="false"
                />
            </button>
            <button
                v-show="!docked"
                class="pet-character"
                :style="position(layout.pet)"
                :aria-label="`${skin.manifest.name}桌宠：${busy ? '测试中' : '点击摸头，按住拖动'}`"
                title="点击摸头 · 按住拖动 · 右键菜单"
                @pointerdown="pointerDown"
                @pointerup="release"
                @pointercancel="release"
                @click="clickCharacter"
                @dblclick="activity('wave')"
                @keydown.enter.prevent="togglePanel"
                @contextmenu.prevent="openMenu"
            >
                <PetActor
                    :skin="skin"
                    :action="action"
                    :animate="animate && pageVisible && !docked"
                    :size="layout.pet[2]"
                    :look="look"
                    :drag-velocity="dragVelocity"
                    @ready="displayedAction = $event"
                    @error="fail"
                    @bounds="actorBounds = $event"
                />
            </button>
            <button
                v-show="cloudVisible"
                class="pet-result-cloud"
                :class="`pet-result-${result.tone}`"
                :data-status="result.key"
                :style="position(layout.bubble)"
                :aria-label="`${result.label}，${panel && !docked ? '收起为桌宠' : '展开测智面板'}`"
                :title="`${result.detail} · 点击展开/收起 · 按住拖动 · 右键菜单`"
                @pointerdown="cloudPointerDown"
                @pointerup="release"
                @pointercancel="release"
                @click="clickCloud"
                @contextmenu.prevent="
                    revealDock();
                    openMenu();
                "
            >
                <svg
                    class="pet-cloud-outline"
                    viewBox="0 0 180 68"
                    preserveAspectRatio="none"
                    aria-hidden="true"
                >
                    <path
                        d="M26 55C12 57 4 49 5 38C-1 29 4 17 15 15C18 5 31 1 43 7C53-1 69 1 77 5C89-1 103 0 112 6C124 1 139 3 145 10C161 8 174 15 173 28C184 39 174 53 163 53C153 62 139 61 131 57C118 63 103 61 95 57C80 63 65 60 57 56C47 59 41 56 36 53L22 65L26 55Z"
                    />
                </svg>
                <span class="pet-cloud-content" role="status" aria-live="polite">
                    <span class="pet-cloud-heading">
                        <CircleCheck v-if="result.key === 'passed'" :size="20" aria-hidden="true" />
                        <TriangleAlert v-else-if="result.key === 'failed'" :size="20" aria-hidden="true" />
                        <CircleX
                            v-else-if="result.key === 'error' || result.key === 'cancelled'"
                            :size="20"
                            aria-hidden="true"
                        />
                        <Code2 v-else-if="result.key === 'html'" :size="20" aria-hidden="true" />
                        <CircleHelp v-else-if="result.key === 'stale'" :size="20" aria-hidden="true" />
                        <Target v-else :size="20" aria-hidden="true" />
                        <strong>{{ result.label }}</strong>
                    </span>
                    <span v-if="bubble" class="pet-cloud-interaction">{{ bubble }}</span>
                </span>
            </button>
            <div
                v-if="menu"
                class="pet-context-menu"
                :data-submenu-side="submenuLeft ? 'left' : 'right'"
                :style="{ ...position(layout.menu), visibility: menuReady ? undefined : 'hidden' }"
                role="menu"
                aria-label="桌宠操作"
                @keydown="menuKey"
                @pointerleave="scheduleActionClose"
            >
                <button
                    role="menuitem"
                    @pointerenter="scheduleActionClose"
                    @click="
                        togglePanel();
                        menu = false;
                    "
                >
                    <Target :size="15" />{{ panel ? '收起为桌宠' : '打开测智面板' }}
                </button>
                <button
                    role="menuitem"
                    @pointerenter="scheduleActionClose"
                    @click="
                        openSettings().catch((cause) => say(String(cause)));
                        menu = false;
                    "
                >
                    <Settings2 :size="15" />设置与皮肤管理
                </button>
                <button
                    role="menuitem"
                    @pointerenter="scheduleActionClose"
                    @click="
                        say(busy ? '正在测试…' : '我在这里陪你～');
                        menu = false;
                    "
                >
                    查看当前状态
                </button>
                <!-- 延迟关闭给斜向移动留出通行时间，进入子菜单取消；停在其他项目才收起。 -->
                <div
                    class="pet-action-group"
                    @pointerenter="keepActionOpen"
                    @pointerleave="scheduleActionClose"
                >
                    <button
                        ref="actionTrigger"
                        role="menuitem"
                        aria-haspopup="menu"
                        :aria-expanded="actionMenu"
                        :aria-controls="`pet-actions-${session}`"
                        @pointerenter="openActionMenu"
                        @click="openActionMenu"
                    >
                        <Hand :size="15" />动作<ChevronRight class="pet-submenu-arrow" :size="15" />
                    </button>
                    <div
                        v-if="actionMenu"
                        :id="`pet-actions-${session}`"
                        role="menu"
                        aria-label="桌宠动作"
                        class="pet-context-menu pet-action-submenu"
                        @pointerenter="keepActionOpen"
                        @pointerleave="scheduleActionClose"
                        @focusin="keepActionOpen"
                        @focusout="scheduleActionClose"
                        :style="{
                            ...position([
                                // 绝对定位原点在一级菜单1px边框内，扣除边框以与场景命中矩形重合。
                                layout.actions[0] - layout.menu[0] - 1,
                                layout.actions[1] - layout.menu[1] - 1,
                                layout.actions[2],
                                layout.actions[3],
                            ]),
                        }"
                    >
                        <button
                            v-for="kind in menuActions"
                            :key="kind"
                            role="menuitem"
                            :title="panel ? '点播时收起面板，测试结论保持不变' : '仅预览动作，不改变测试结论'"
                            @click="playMenuAction(kind)"
                        >
                            {{ actionLabels[kind] }}
                        </button>
                    </div>
                </div>
                <button
                    role="menuitem"
                    @pointerenter="scheduleActionClose"
                    @click="
                        windowAction('close').catch((cause) => say(String(cause)));
                        menu = false;
                    "
                >
                    <X :size="15" />隐藏到托盘
                </button>
            </div>
        </template>
    </div>
</template>
<style scoped>
/* 只在最终布局内滑入，透明度始终为1；气泡固定以免结论随动画闪动。 */
.pet-stage {
    --dock-x: -10px;
    --dock-y: 0px;
}
.pet-stage[data-edge='right'] {
    --dock-x: 10px;
}
.pet-stage[data-edge='top'] {
    --dock-x: 0px;
    --dock-y: -10px;
}
.pet-stage[data-dock-motion='enter'] > .pet-character,
.pet-stage[data-dock-motion='enter'] > .pet-edge-peek,
.pet-stage[data-dock-motion='enter'] > .pet-panel {
    animation: dock-arrive 160ms cubic-bezier(0.22, 1, 0.36, 1) both;
}
@keyframes dock-arrive {
    from {
        transform: translate(var(--dock-x, 0px), var(--dock-y, 0px));
    }
    to {
        transform: translate(0, 0);
    }
}
/* 云朵是程序绘制的状态控件：透明区没有截图残边，文字始终来自当前业务状态。 */
.pet-result-cloud {
    touch-action: none;
    user-select: none;
    --cloud-ink: #156dc0;
    --cloud-line: #86c7fa;
    --cloud-fill: #f1f9ff;
    position: absolute;
    z-index: 5;
    padding: 0 12px 9px;
    border: 0;
    appearance: none;
    -webkit-appearance: none;
    background: transparent;
    color: var(--cloud-ink);
    cursor: pointer;
    display: grid;
    place-items: center;
    isolation: isolate;
}
.pet-result-green {
    --cloud-ink: #268a32;
    --cloud-line: #a2dfaa;
    --cloud-fill: #f1fcf1;
}
.pet-result-amber {
    --cloud-ink: #9f6509;
    --cloud-line: #efc267;
    --cloud-fill: #fff9ec;
}
.pet-result-red {
    --cloud-ink: #d93b48;
    --cloud-line: #f5a0aa;
    --cloud-fill: #fff4f4;
}
.pet-result-neutral {
    --cloud-ink: #5e718a;
    --cloud-line: #b5c6db;
    --cloud-fill: #f5f8fc;
}
.pet-cloud-outline {
    display: block;
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    z-index: 0;
    overflow: visible;
    fill: var(--cloud-fill);
    stroke: var(--cloud-line);
    stroke-width: 1.8;
    stroke-linejoin: round;
}
.pet-cloud-content {
    position: relative;
    z-index: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1px;
    line-height: 1.15;
}
.pet-edge-peek {
    position: absolute;
    overflow: hidden;
    border: 0;
    padding: 0;
    appearance: none;
    background: transparent;
    cursor: pointer;
}
.pet-edge-peek img {
    position: absolute;
    top: 0;
    transform-origin: 48px 48px;
    display: block;
    width: 96px;
    height: 96px;
    max-width: none;
    pointer-events: none;
    user-select: none;
}
/* 右侧完整角色只镜像布局，不翻转角色、文字或按钮；标题工具区避开透明角色按钮。 */
.pet-edge-right :deep(.widget-header) {
    margin-left: 0;
    margin-right: max(0px, calc(var(--pet-size, 192px) * 0.62 - 14px));
}
.pet-edge-right .pet-cloud-outline {
    transform: scaleX(-1);
}
/* 顶部探头倒转云朵轮廓，尾巴向上接近角色；文字与状态图标不倒转。 */
.pet-stage[data-edge='top'][data-docked='true'] .pet-cloud-outline {
    transform: scaleY(-1);
}
/* 轮廓倒转后尾巴在上方，原来的下方留尾空隙也必须对称换到上方。 */
.pet-stage[data-edge='top'][data-docked='true'] .pet-result-cloud {
    padding-top: 9px;
    padding-bottom: 0;
}
/* 两层紧贴无透明间隙；二级菜单限高滚动，窗口和输入掩码由同一矩形扩展。 */
.pet-action-group > button {
    width: 100%;
}
.pet-submenu-arrow {
    margin-left: auto;
}
.pet-context-menu[data-submenu-side='left'] .pet-submenu-arrow {
    transform: scaleX(-1);
}
.pet-action-submenu {
    overflow-y: auto;
    z-index: 7;
}
.pet-action-submenu button {
    flex-shrink: 0;
}
.pet-context-menu button:focus-visible {
    outline: 2px solid var(--pet-primary);
    outline-offset: -2px;
    background: #304c70;
}
/* 角色按钮覆盖整块透明画布；矩形焦点框会超出窗口被截成蓝色折线。
   键盘仍以角色轮廓柔光提示焦点，不把透明留白画成边框。 */
.pet-character:focus {
    outline: none;
}
.pet-character:focus-visible .pet-actor {
    filter: drop-shadow(0 0 2px var(--pet-primary));
}
.pet-edge-peek:focus-visible {
    outline: 2px solid var(--pet-primary);
    outline-offset: -2px;
    border-radius: 8px;
}
.pet-cloud-heading {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    white-space: nowrap;
}
.pet-cloud-heading strong {
    font-size: 15px;
    font-weight: 750;
    letter-spacing: 0;
}
.pet-cloud-heading svg {
    flex-shrink: 0;
    stroke-width: 2.6;
}
.pet-cloud-interaction {
    font-size: 10px;
    max-width: 120px;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    color: var(--cloud-ink);
}
.pet-result-cloud:focus-visible {
    outline: 2px solid var(--cloud-ink);
    outline-offset: 3px;
    border-radius: 24px;
}
</style>
