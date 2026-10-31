<!--
    悬浮组件：单一主操作、独立拖动区域、可滚动回答。
    文字使用安全 Markdown，HTML 交给共用沙箱 iframe 预览；状态和复制使用后端结论与原文。
    @author ahui
-->
<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import {
    Target,
    Settings2,
    TimerReset,
    ChevronDown,
    ChevronUp,
    Play,
    Check,
    X,
    Minus,
    LoaderCircle,
    CircleAlert,
    Copy,
    RotateCw,
    Clock3,
    Zap,
    Info,
    Grip,
    PanelLeftClose,
} from 'lucide-vue-next';
import { CANDY_RULE_ID, call, dragWindow, openSettings, resizeWidget, subscribeDock } from '../bridge';
import { formatTime, formatRelativeTime, useProbe } from '../useProbe';
import ModelPicker from './ModelPicker.vue';
import PetStage from './PetStage.vue';
import AnswerContent from './AnswerContent.vue';
import { ruleAnswers } from '../rules';
import { reasoningSupport } from '../reasoning';
import type { FetchedModel, ModelCatalog, ModelProfile } from '../types';
import type { Rect } from '../pet/types';

const {
    snapshot,
    snapshotReceivedAt,
    error,
    profile,
    latest,
    latestRecord,
    stale,
    busy,
    refresh,
    start,
    cancel,
} = useProbe();
const uiNow = ref({ monotonic: performance.now(), wall: Date.now() });
let clockTimer: ReturnType<typeof setInterval> | undefined;
const expanded = ref(false);
const petActive = ref(false);
const petPanelVisible = ref(true);
const card = ref<HTMLElement>();
const petStage = ref<InstanceType<typeof PetStage>>();
const cardHeight = ref(176);
const floatingBounds = ref<Rect[]>([]);
let cardObserver: ResizeObserver | undefined;
const observedMenus = new Set<Element>();
let measuredFrame = 0;
/**
 * 菜单相对卡片测量，交由桌宠场景扩展窗口与输入区域；绝不把菜单高度当作卡片高度。
 * 关闭菜单后清空浮层轮廓，因此边饰始终跟随真实卡片，透明空白也不会永久拦截点击。
 */
async function measureSceneContent(): Promise<void> {
    await nextTick();
    const element = card.value;
    if (disposed || !petActive.value || !element) {
        for (const menu of observedMenus) cardObserver?.unobserve(menu);
        observedMenus.clear();
        floatingBounds.value = [];
        return;
    }
    const origin = element.getBoundingClientRect();
    if (origin.height > 0) cardHeight.value = Math.ceil(origin.height);
    const menus = [...element.querySelectorAll<HTMLElement>('.model-menu')];
    // 后台目录可能在菜单已打开时变化；观察浮层本身而非只观察卡片，防止列表增高后仍沿用旧边界。
    for (const old of observedMenus) {
        if (!menus.includes(old as HTMLElement)) {
            cardObserver?.unobserve(old);
            observedMenus.delete(old);
        }
    }
    for (const menu of menus) {
        if (!observedMenus.has(menu)) {
            cardObserver?.observe(menu);
            observedMenus.add(menu);
        }
    }
    floatingBounds.value = menus
        .map((menu): Rect => {
            const box = menu.getBoundingClientRect();
            // 预留阴影的安全边距，并包含菜单相对选择器的真实偏移。
            return [box.left - origin.left - 3, box.top - origin.top - 3, box.width + 6, box.height + 8];
        })
        .filter((bounds) => bounds[2] > 6 && bounds[3] > 8);
}
function queueSceneMeasure(): void {
    cancelAnimationFrame(measuredFrame);
    measuredFrame = requestAnimationFrame(() => void measureSceneContent());
}
/** 菜单先隐藏测量，再等待场景IPC完成；只预留浮层空间，不拉长卡片或滚动整页。 */
async function prepareSceneMenu(): Promise<void> {
    if (!petActive.value || disposed) return;
    await measureSceneContent();
    await nextTick();
    await petStage.value?.syncLayout();
}
/** 桌宠独立控制面板收放，经典边签逻辑只在经典模式启用。 */
watch(petActive, (value) => {
    if (value) {
        tucked.value = false;
        edgeSide.value = null;
        clearTimeout(tuckTimer);
    }
});
const copied = ref(false);
const showRawAnswer = ref(false);
const previewError = ref('');
// 放大层位于同一页面，打开期间不因 iframe 内的鼠标/焦点变化触发边缘收起。
const previewExpanded = ref(false);
const cancelling = ref(false);
const picker = ref<InstanceType<typeof ModelPicker>>();
const modelPicker = ref<InstanceType<typeof ModelPicker>>();
const modelPickerOpen = ref(false);
const catalogModels = ref<FetchedModel[]>([]);
const catalogLoading = ref(false);
const fetchingModels = ref(false);
let catalogGeneration = 0;
const rulePicker = ref<InstanceType<typeof ModelPicker>>();
const effortPicker = ref<InstanceType<typeof ModelPicker>>();
const rulePickerOpen = ref(false);
const effortPickerOpen = ref(false);
const pickerOpen = ref(false);
const selecting = ref(false);
const autoSwitchPending = ref(false);
const layoutPending = ref(false);
const edgeSide = ref<string | null>(null);
const tucked = ref(false);
const collapsing = ref(false);
const revealing = ref(false);
let animationGeneration = 0;
let hovering = false;
let tuckTimer: ReturnType<typeof setTimeout> | undefined;
let unsubscribeDock: (() => void) | undefined;
let disposed = false;
let copiedTimer: ReturnType<typeof setTimeout> | undefined;
let motionTimer: ReturnType<typeof setTimeout> | undefined;
let releaseMotion: (() => void) | undefined;

/** 键盘或辅助功能点击也必须互斥；不能只依赖 pointerdown 的外部点击关闭。 */
async function closeOtherPickers(current: 'group' | 'model' | 'rule' | 'effort'): Promise<void> {
    if (current !== 'group') await picker.value?.close();
    if (current !== 'model') await modelPicker.value?.close();
    if (current !== 'rule') await rulePicker.value?.close();
    if (current !== 'effort') await effortPicker.value?.close();
}

/** 关闭窗口时可结束等待；后续世代检查保证异步动画不再操作原生窗口。 */
function waitForMotion(): Promise<void> {
    return new Promise((resolve) => {
        releaseMotion = resolve;
        motionTimer = setTimeout(() => {
            releaseMotion = undefined;
            resolve();
        }, 180);
    });
}

/** 鼠标离开 250ms 后收回；只缩短等待，保留原有 180/220ms 过渡动画。 */
function scheduleTuck(): void {
    clearTimeout(tuckTimer);
    if (petActive.value || !edgeSide.value || tucked.value) return;
    tuckTimer = setTimeout(() => {
        if (
            !hovering &&
            !previewExpanded.value &&
            !pickerOpen.value &&
            !modelPickerOpen.value &&
            !rulePickerOpen.value &&
            !effortPickerOpen.value &&
            !selecting.value
        )
            void setTucked(true);
    }, 250);
}

/** 边签使用真实小窗口，不留下透明的大面积鼠标遮挡。 */
async function setTucked(value: boolean): Promise<void> {
    if (value && previewExpanded.value) return;
    if (disposed || (value && (collapsing.value || tucked.value)) || (!value && revealing.value)) return;
    clearTimeout(tuckTimer);
    const generation = ++animationGeneration;
    try {
        if (value) {
            collapsing.value = true;
            if (!window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) {
                await waitForMotion();
            }
            if (generation !== animationGeneration || hovering) {
                collapsing.value = false;
                return;
            }
        } else {
            revealing.value = true;
            collapsing.value = false;
            // 原生窗口扩大前先隐藏旧图标，避免图标在新窗口左上角闪现一帧。
            await nextTick();
            await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
        }
        // 等待动画期间窗口可能已关闭；旧恢复任务不得继续调整原生窗口。
        if (disposed || generation !== animationGeneration) return;
        await resizeWidget(expanded.value, 0, value);
        if (disposed || generation !== animationGeneration) return;
        tucked.value = value;
        collapsing.value = false;
        if (!value) {
            await nextTick();
            requestAnimationFrame(() =>
                requestAnimationFrame(() => {
                    if (disposed || generation !== animationGeneration) return;
                    revealing.value = false;
                }),
            );
            scheduleTuck();
        }
    } catch (cause) {
        collapsing.value = false;
        revealing.value = false;
        error.value = String(cause);
    }
}

function pointerEntered(): void {
    hovering = !tucked.value;
    clearTimeout(tuckTimer);
    clearTimeout(motionTimer);
    releaseMotion?.();
    if (collapsing.value) {
        animationGeneration++;
        collapsing.value = false;
    }
    if (tucked.value) void setTucked(false);
}
function pointerLeft(): void {
    hovering = false;
    scheduleTuck();
}

/** 放大关闭后重新计算悬停，不能沿用 iframe 内操作前的旧鼠标状态。 */
function previewExpandedChanged(value: boolean): void {
    previewExpanded.value = value;
    clearTimeout(tuckTimer);
    if (!value) {
        hovering = Boolean(document.querySelector('.widget:hover'));
        if (!hovering) scheduleTuck();
    }
}

onMounted(async () => {
    if (typeof ResizeObserver !== 'undefined') {
        cardObserver = new ResizeObserver(queueSceneMeasure);
        if (card.value) cardObserver.observe(card.value);
    }
    window.addEventListener('resize', queueSceneMeasure);
    clockTimer = setInterval(() => {
        uiNow.value = { monotonic: performance.now(), wall: Date.now() };
    }, 1000);
    try {
        const cleanup = await subscribeDock((dock) => {
            if (petActive.value) return;
            edgeSide.value = dock.side;
            tucked.value = dock.tucked;
            if (!dock.tucked) scheduleTuck();
        });
        if (disposed) cleanup();
        else unsubscribeDock = cleanup;
    } catch (cause) {
        if (!disposed) error.value = String(cause);
    }
});
onUnmounted(() => {
    disposed = true;
    animationGeneration++;
    catalogGeneration++;
    clearTimeout(tuckTimer);
    clearTimeout(copiedTimer);
    clearTimeout(motionTimer);
    releaseMotion?.();
    clearInterval(clockTimer);
    unsubscribeDock?.();
    cardObserver?.disconnect();
    observedMenus.clear();
    cancelAnimationFrame(measuredFrame);
    window.removeEventListener('resize', queueSceneMeasure);
});

watch(
    [petActive, expanded, petPanelVisible, pickerOpen, modelPickerOpen, rulePickerOpen, effortPickerOpen],
    () => void measureSceneContent(),
    { flush: 'post' },
);

const state = computed(() => (busy.value ? 'running' : (latest.value?.status ?? 'idle')));
const stateLabels: Record<string, string> = {
    idle: '待测试',
    running: '测试中',
    passed: '不降智',
    failed: '降智',
    error: '测试失败',
    cancelled: '已取消',
    html: '已生成 HTML',
};
const resultLabel = computed(() =>
    !busy.value && stale.value
        ? '待重新测试'
        : state.value === 'html' && previewError.value
          ? 'HTML 预览失败'
          : stateLabels[state.value],
);
watch(
    () => latestRecord.value?.id,
    () => {
        previewError.value = '';
        copied.value = false;
        showRawAnswer.value = false;
        clearTimeout(copiedTimer);
    },
);
/** 两种布局共用主按钮文案：任何已完成/失败/取消记录均允许重新测试。 */
const primaryLabel = computed(() =>
    busy.value
        ? cancelling.value
            ? '正在取消'
            : '取消测试'
        : !profile.value
          ? '配置接口'
          : latestRecord.value
            ? '重新测试'
            : '开始测试',
);
const testStartedAt = computed(() =>
    // 条件变化只使结论失效；上次测试的真实时间仍应保留，不能误写成从未测试。
    busy.value ? snapshot.value?.running?.startedAt : latestRecord.value?.startedAt,
);
const compactTestTime = computed(() => {
    return busy.value && !testStartedAt.value ? '正在测试' : formatTime(testStartedAt.value);
});
/** 可见值固定为绝对时间便于截图核对；相对时效仅作为悬停补充，不覆盖真实时间。 */
const testTimeTitle = computed(() =>
    testStartedAt.value
        ? `${compactTestTime.value}（${formatRelativeTime(testStartedAt.value, uiNow.value.wall)}）`
        : compactTestTime.value,
);
const requestDuration = computed(() => {
    // 条件变化撤销当前判定，不删除历史事实；耗时与下方展示的同一条回答保持对应。
    if (!busy.value)
        return latestRecord.value ? `${(latestRecord.value.durationMs / 1000).toFixed(2)}s` : '暂无数据';
    const started = Date.parse(testStartedAt.value ?? '');
    return Number.isFinite(started)
        ? `${(Math.max(0, uiNow.value.wall - started) / 1000).toFixed(1)}s`
        : '计时中';
});
/** 旧配置缺省为 low；显式 null 表示不发送参数，不能用空值合并把它改回 low。 */
const currentEffort = computed(() =>
    profile.value?.reasoningEffort === undefined ? 'low' : (profile.value.reasoningEffort ?? ''),
);
const reasoning = computed(() =>
    reasoningSupport(profile.value?.model ?? '', profile.value?.interfaceMode, currentEffort.value || null),
);
const autoTestEnabled = computed(() => snapshot.value?.config.preferences.autoTestEnabled ?? false);
/** 规则由稳定 ID 解析，失效绑定明确标识；强度缺字段沿用旧配置 low，null 为不指定。 */
const profileSummaries = computed(() =>
    Object.fromEntries(
        (snapshot.value?.config.profiles ?? []).map((item) => [
            item.id,
            {
                rule: item.rulePresetId
                    ? (snapshot.value?.config.rulePresets?.find((preset) => preset.id === item.rulePresetId)
                          ?.name ?? '规则已失效')
                    : (snapshot.value?.config.defaultRuleName ?? '默认规则'),
                effort: item.reasoningEffort === undefined ? 'low' : (item.reasoningEffort ?? '不指定'),
            },
        ]),
    ),
);
const ruleOptions = computed(() => [
    { id: '', name: snapshot.value?.config.defaultRuleName ?? '默认规则', model: '' },
    ...(snapshot.value?.config.rulePresets ?? []).map((item) => ({
        id: item.id,
        name: item.name,
        model: '',
    })),
]);
/** 未建接口时也预选糖果；已有接口始终显示其持久绑定，不以显示默认值改变实际请求。 */
const selectedRuleId = computed(() =>
    profile.value
        ? (profile.value.rulePresetId ?? '')
        : snapshot.value?.config.rulePresets?.some((item) => item.id === CANDY_RULE_ID)
          ? CANDY_RULE_ID
          : '',
);
// 仅解释后端已判定的历史；状态不在前端重判，多答案通过时展示实际命中的项。
const resultAnswers = computed(() => {
    const record = latest.value;
    if (!record) return '';
    const answers = ruleAnswers(record);
    return record.status === 'passed'
        ? (answers.find((answer) => record.answer.includes(answer)) ?? '')
        : answers.join(' / ');
});
/** 后端提供剩余时长，前端用单调时钟平滑显示；不每秒发 IPC，也不自行触发付费请求。 */
const autoCountdown = computed(() => {
    if (!profile.value) return '待配置';
    if (busy.value) return '本次结束后计时';
    const remaining = snapshot.value?.autoTestRemainingMs;
    if (remaining == null) return '同步中';
    const seconds = Math.ceil(
        Math.max(0, remaining - Math.max(0, uiNow.value.monotonic - snapshotReceivedAt.value)) / 1000,
    );
    if (seconds === 0) return '即将测试';
    const minutes = Math.floor(seconds / 60);
    return `${minutes.toString().padStart(2, '0')}:${(seconds % 60).toString().padStart(2, '0')}`;
});
const hint = computed(() => {
    if (busy.value) return cancelling.value ? '正在取消请求…' : '正在等待模型完整回答…';
    if (stale.value) return '展示上次回答；当前条件待重新测试';
    if (!latest.value) return '配置好接口后，即可开始第一次测试';
    if (latest.value.status === 'passed') return '命中连续字符串：';
    if (latest.value.status === 'failed') return '未命中连续字符串：';
    if (latest.value.status === 'html') return '回答已识别为 HTML，可查看实际效果';
    return latest.value.error || '本次未产生判定结果';
});

/** 同分组历史可继续查看，但不能把旧协议结果当作当前协议已测试；旧记录不补猜协议。 */
const answerCaption = computed(() => {
    if (!stale.value || busy.value) return '完整正文';
    const protocol = latestRecord.value?.interfaceMode;
    const label =
        protocol === 'responses'
            ? 'Responses'
            : protocol === 'chat_completions'
              ? 'Chat Completions'
              : '协议未记录';
    return `上次回答 · ${label}`;
});

/** 保持窗口外框与内容尺寸一致，失败时撤回展开状态并提供可见提示。 */
async function toggleExpanded(): Promise<void> {
    if (layoutPending.value || disposed) return;
    layoutPending.value = true;
    const next = !expanded.value;
    try {
        await picker.value?.close();
        await modelPicker.value?.close();
        await rulePicker.value?.close();
        await effortPicker.value?.close();
        await resizeWidget(next);
        expanded.value = next;
    } catch (cause) {
        error.value = String(cause);
    } finally {
        layoutPending.value = false;
    }
}

/** 没有配置时主操作直接进入设置；不产生无意义的失败历史。 */
async function primaryAction(): Promise<void> {
    if (!profile.value) {
        await safeOpenSettings();
    } else if (busy.value) {
        cancelling.value = true;
        if (!(await cancel())) cancelling.value = false;
    } else {
        await start();
    }
}

/** 设置窗口异常不能成为未捕获 Promise，也不应改变测试结果。 */
async function safeOpenSettings(): Promise<void> {
    try {
        await openSettings();
    } catch (cause) {
        error.value = String(cause);
    }
}

/** 悬浮窗快捷切换自动测试；后端负责持久化、重置周期并通知托盘与设置页。 */
async function toggleAutoTest(): Promise<void> {
    if (autoSwitchPending.value) return;
    autoSwitchPending.value = true;
    try {
        await call('set_auto_test_enabled', {
            enabled: !autoTestEnabled.value,
        });
    } catch (cause) {
        error.value = String(cause);
    } finally {
        await refresh();
        autoSwitchPending.value = false;
    }
}

/** 目录只属于当前分组的地址/凭据句柄；切换后旧响应不得回填新分组或已卸载窗口。 */
const catalogIdentity = computed(() =>
    JSON.stringify([
        profile.value?.id,
        profile.value?.baseUrl,
        profile.value?.catalogToken,
        profile.value?.credentialRevision,
    ]),
);
watch(
    catalogIdentity,
    async () => {
        const generation = ++catalogGeneration;
        catalogModels.value = [];
        catalogLoading.value = false;
        const current = profile.value;
        if (!current?.catalogToken) return;
        catalogLoading.value = true;
        try {
            const models = await call<FetchedModel[]>('get_model_catalog', {
                token: current.catalogToken,
                baseUrl: current.baseUrl,
            });
            if (!disposed && generation === catalogGeneration) catalogModels.value = models;
        } catch (cause) {
            if (!disposed && generation === catalogGeneration) error.value = String(cause);
        } finally {
            if (!disposed && generation === catalogGeneration) catalogLoading.value = false;
        }
    },
    { immediate: true },
);

/** 显式拉取只读取目录，不产生推理、历史或自动选中；空目录保留重试入口。 */
async function fetchProfileModels(): Promise<void> {
    const current = profile.value;
    if (!current || fetchingModels.value || catalogLoading.value || selecting.value || busy.value) return;
    fetchingModels.value = true;
    error.value = '';
    const identity = catalogIdentity.value;
    try {
        const catalog = await call<ModelCatalog>('fetch_profile_models', { profileId: current.id });
        if (!disposed && identity === catalogIdentity.value) {
            catalogGeneration++;
            catalogModels.value = catalog.models;
            if (!catalog.models.length) error.value = '接口未返回可用模型，可在设置中手动填写模型 ID。';
        }
    } catch (cause) {
        if (!disposed && identity === catalogIdentity.value) error.value = String(cause);
    } finally {
        if (!disposed) {
            await refresh();
            fetchingModels.value = false;
        }
    }
}

/** 仅提交目录中的模型 ID；后端再次验证最新目录，拒绝过期项，不覆盖其他配置字段。 */
async function selectModel(model: string): Promise<void> {
    const current = profile.value;
    if (
        !current ||
        selecting.value ||
        busy.value ||
        model === current.model ||
        !catalogModels.value.some((item) => item.id === model)
    )
        return;
    selecting.value = true;
    error.value = '';
    try {
        await call('set_profile_model', { profileId: current.id, model });
    } catch (cause) {
        error.value = String(cause);
    } finally {
        await refresh();
        selecting.value = false;
    }
}

/** 模型选择只更新后端选中项，不复制界面中的旧配置覆盖其他窗口。 */
async function selectProfile(id: string): Promise<void> {
    if (selecting.value || busy.value) return;
    selecting.value = true;
    try {
        await call('select_profile', { id });
    } catch (cause) {
        error.value = String(cause);
    } finally {
        await refresh();
        selecting.value = false;
    }
}

/** 只提交当前模型的绑定；等待期间禁止重复切换，读取权威快照后再解除门禁。 */
async function selectRule(id: string): Promise<void> {
    const current = profile.value;
    if (!current || selecting.value || busy.value) return;
    selecting.value = true;
    error.value = '';
    try {
        await call('set_profile_rule', { profileId: current.id, rulePresetId: id || null });
    } catch (cause) {
        error.value = String(cause);
    } finally {
        await refresh();
        selecting.value = false;
    }
}

/** 快捷强度切换只写当前模型这一字段；不重计时、不自动测试，失败保留原值并重新读取。 */
async function selectEffort(id: string): Promise<void> {
    const current = profile.value;
    if (!current || selecting.value || busy.value || id === currentEffort.value) return;
    selecting.value = true;
    error.value = '';
    try {
        await call('set_profile_effort', {
            profileId: current.id,
            reasoningEffort: (id || null) as ModelProfile['reasoningEffort'],
        });
    } catch (cause) {
        error.value = String(cause);
    } finally {
        await refresh();
        selecting.value = false;
    }
}

/** 复制完整正文，剪贴板权限失败时给出可读错误，保留原结果。 */
async function copyAnswer(): Promise<void> {
    const record = latestRecord.value;
    if (busy.value || !record?.answer) return;
    try {
        // 复制与屏幕展示同一条历史原文；切换记录或卸载后，迟到的结果不能回写提示。
        await navigator.clipboard.writeText(record.answer);
        if (disposed || latestRecord.value?.id !== record.id) return;
        copied.value = true;
        clearTimeout(copiedTimer);
        copiedTimer = setTimeout(() => {
            copied.value = false;
        }, 1800);
    } catch {
        if (!disposed && latestRecord.value?.id === record.id)
            error.value = '复制失败，请在回答区域手动选择并复制。';
    }
}

watch(busy, (value) => {
    if (!value) cancelling.value = false;
    if (!value) scheduleTuck();
});
watch(error, (value) => {
    if (value && !expanded.value) void toggleExpanded();
});
</script>

<template>
    <PetStage
        ref="petStage"
        v-slot="{ togglePanel }"
        :snapshot="snapshot"
        :latest="latest"
        :busy="busy"
        :stale="stale"
        :preview-expanded="previewExpanded"
        :content-expanded="expanded"
        :content-height="cardHeight"
        :floating-bounds="floatingBounds"
        @active="petActive = $event"
        @visible="petPanelVisible = $event"
        @error="error = $event"
    >
        <button
            v-if="tucked"
            class="edge-tab"
            :class="[edgeSide, `state-${state}`, { 'edge-restoring': revealing }]"
            :aria-label="`显示测智雷达：${resultLabel}`"
            @mouseenter="pointerEntered"
            @click="setTucked(false)"
            @focus="setTucked(false)"
        >
            <LoaderCircle v-if="busy" :size="20" class="edge-spinner spin" aria-hidden="true" />
            <Target v-else :size="18" aria-hidden="true" /><span class="sr-only" aria-live="polite">{{
                resultLabel
            }}</span>
        </button>
        <main
            ref="card"
            v-else
            v-show="!petActive || petPanelVisible || previewExpanded"
            class="widget panel"
            :class="[
                {
                    expanded,
                    'picker-visible': pickerOpen || modelPickerOpen || rulePickerOpen || effortPickerOpen,
                    'dock-away': collapsing || revealing,
                },
                `state-${state}`,
                edgeSide ? `dock-${edgeSide}` : '',
            ]"
            :style="{ '--panel-opacity': (snapshot?.config.preferences.opacity ?? 97) / 100 }"
            aria-label="测智雷达悬浮组件"
            @mouseenter="pointerEntered"
            @mouseleave="pointerLeft"
        >
            <header
                class="widget-header drag-area"
                @mousedown.left="!petActive && dragWindow().catch(() => {})"
            >
                <div class="brand">
                    <Target class="brand-mark" :size="17" aria-hidden="true" />
                    <span>测智雷达</span>
                </div>
                <Grip :size="22" class="drag-grip" aria-hidden="true" />
                <div class="widget-tools" @mousedown.stop>
                    <button
                        class="icon-button"
                        :class="{ active: autoTestEnabled }"
                        :aria-pressed="autoTestEnabled"
                        :disabled="autoSwitchPending"
                        aria-label="切换自动测试"
                        :title="`${autoTestEnabled ? '关闭' : '开启'}自动测试 · 每 ${snapshot?.config.preferences.autoTestIntervalMinutes ?? 3} 分钟`"
                        @click="toggleAutoTest"
                    >
                        <TimerReset :size="15" />
                    </button>
                    <button class="icon-button" aria-label="打开设置" title="设置" @click="safeOpenSettings">
                        <Settings2 :size="15" />
                    </button>
                    <button
                        class="icon-button expand-button"
                        :aria-expanded="expanded"
                        :disabled="layoutPending"
                        :aria-label="expanded ? '收起结果' : '展开测试详情'"
                        :title="expanded ? '收起' : '展开'"
                        @click="toggleExpanded"
                    >
                        <ChevronUp v-if="expanded" :size="16" /><ChevronDown v-else :size="16" />
                    </button>
                    <button
                        v-if="petActive"
                        class="icon-button pet-collapse"
                        aria-label="收起为桌宠"
                        title="收起为桌宠"
                        @click="togglePanel"
                    >
                        <PanelLeftClose :size="16" />
                    </button>
                </div>
            </header>

            <!-- 模型目录为空才显示拉取按钮；当前手填 ID 只作占位，不伪造目录数据。 -->
            <div class="widget-model-row" :class="{ 'with-fetch': !catalogModels.length }">
                <ModelPicker
                    ref="picker"
                    :profiles="snapshot?.config.profiles ?? []"
                    :context="petActive ? 'scene' : 'widget'"
                    :selected-id="snapshot?.config.selectedProfileId ?? null"
                    :expanded="expanded"
                    :disabled="busy || selecting"
                    :show-model="false"
                    :summaries="profileSummaries"
                    :before-open="() => closeOtherPickers('group')"
                    :prepare-scene-menu="prepareSceneMenu"
                    @select="selectProfile"
                    @configure="safeOpenSettings"
                    @open-change="pickerOpen = $event"
                    @error="error = $event"
                />
                <ModelPicker
                    ref="modelPicker"
                    class="widget-model-picker"
                    label="当前分组模型"
                    :context="petActive ? 'scene' : 'widget'"
                    icon="brain"
                    :profiles="catalogModels.map((item) => ({ id: item.id, name: item.id, model: '' }))"
                    :selected-id="profile?.model ?? null"
                    :placeholder="profile?.model || '无模型'"
                    :expanded="expanded"
                    :disabled="busy || selecting || catalogLoading || fetchingModels || !catalogModels.length"
                    :show-model="false"
                    :show-icon="false"
                    :before-open="() => closeOtherPickers('model')"
                    :prepare-scene-menu="prepareSceneMenu"
                    @select="selectModel"
                    @open-change="modelPickerOpen = $event"
                    @error="error = $event"
                />
                <button
                    v-if="!catalogModels.length"
                    type="button"
                    class="icon-button widget-fetch-models"
                    aria-label="拉取当前分组模型"
                    :title="
                        catalogLoading
                            ? '读取模型缓存中'
                            : fetchingModels
                              ? '正在拉取模型'
                              : '拉取当前分组模型'
                    "
                    :aria-busy="catalogLoading || fetchingModels"
                    :disabled="busy || selecting || catalogLoading || fetchingModels || !profile"
                    @click="fetchProfileModels"
                >
                    <RotateCw
                        :size="14"
                        :class="{ spin: catalogLoading || fetchingModels }"
                        aria-hidden="true"
                    />
                </button>
            </div>

            <div class="widget-rule-row">
                <ModelPicker
                    ref="rulePicker"
                    class="widget-rule-picker"
                    label="当前测试规则"
                    :context="petActive ? 'scene' : 'widget'"
                    icon="list"
                    :profiles="ruleOptions"
                    :selected-id="selectedRuleId"
                    :placeholder="profile?.rulePresetId ? '规则已失效' : '默认规则'"
                    :expanded="expanded"
                    :disabled="busy || selecting || !profile"
                    :show-model="false"
                    :before-open="() => closeOtherPickers('rule')"
                    :prepare-scene-menu="prepareSceneMenu"
                    @select="selectRule"
                    @open-change="rulePickerOpen = $event"
                    @error="error = $event"
                />
                <ModelPicker
                    ref="effortPicker"
                    class="widget-effort-picker"
                    label="当前思考等级"
                    :context="petActive ? 'scene' : 'widget'"
                    icon="sliders"
                    :profiles="reasoning.options"
                    :selected-id="currentEffort"
                    :expanded="expanded"
                    :disabled="busy || selecting || !profile"
                    :show-model="false"
                    :show-icon="false"
                    :description="reasoning.notice"
                    :before-open="() => closeOtherPickers('effort')"
                    :prepare-scene-menu="prepareSceneMenu"
                    @select="selectEffort"
                    @open-change="effortPickerOpen = $event"
                    @error="error = $event"
                />
            </div>

            <!-- 经典与桌宠共用原纵向内容，皮肤只改变颜色与边饰，避免维护两套业务控件。 -->
            <section v-if="expanded" class="result-content" aria-label="本次测试详情">
                <div class="result-heading" aria-live="polite">
                    <span class="status-symbol hero-symbol" :class="state">
                        <LoaderCircle v-if="busy" :size="25" class="spin" />
                        <Check v-else-if="state === 'passed'" :size="25" />
                        <CircleAlert v-else-if="state === 'failed'" :size="25" />
                        <X v-else-if="state === 'error'" :size="25" />
                        <Minus v-else :size="25" />
                    </span>
                    <div class="result-heading-copy">
                        <h1>{{ resultLabel }}</h1>
                        <p :title="hint + resultAnswers">
                            {{ hint }}
                            <strong v-if="!busy && (state === 'passed' || state === 'failed')">{{
                                resultAnswers
                            }}</strong>
                        </p>
                    </div>
                </div>

                <div v-if="error" class="inline-error" role="alert">{{ error }}</div>
                <div class="answer-label">
                    <span>模型回答</span>
                    <div class="answer-label-tools">
                        <span class="eyebrow">{{ answerCaption }}</span>
                        <button
                            v-if="latestRecord?.answerType === 'html'"
                            class="icon-button"
                            :disabled="busy || !latestRecord?.answer"
                            :aria-pressed="showRawAnswer"
                            @click="showRawAnswer = !showRawAnswer"
                        >
                            {{ showRawAnswer ? '返回预览' : '原始回答' }}
                        </button>
                    </div>
                </div>
                <div
                    class="answer-box"
                    :class="{
                        'answer-box-html': latestRecord?.answerType === 'html' && !busy && !showRawAnswer,
                    }"
                    tabindex="0"
                    :aria-busy="busy"
                >
                    <div v-if="busy" class="answer-placeholder">
                        <LoaderCircle :size="18" class="spin" />等待模型回答
                    </div>
                    <pre v-else-if="showRawAnswer && latestRecord?.answer" class="pet-raw-answer">{{
                        latestRecord.answer
                    }}</pre>
                    <AnswerContent
                        v-else-if="latestRecord?.answer"
                        :source="latestRecord.answer"
                        :answer-type="latestRecord.answerType"
                        :executable="latestRecord.status !== 'error' && latestRecord.status !== 'cancelled'"
                        @failure="previewError = $event"
                        @expanded-change="previewExpandedChanged"
                    />
                    <p v-else class="muted">
                        {{ state === 'idle' ? '测试完成后，回答会显示在这里。' : '没有可展示的回答。' }}
                    </p>
                </div>
                <div class="metadata" :class="{ 'with-countdown': autoTestEnabled }">
                    <div class="test-timestamp">
                        <Clock3 :size="14" /><span
                            >{{ stale && !busy ? '上次测试' : '测试时间'
                            }}<strong :title="testTimeTitle">{{ compactTestTime }}</strong></span
                        >
                    </div>
                    <div>
                        <Zap :size="14" /><span
                            >请求耗时<strong>{{ requestDuration }}</strong></span
                        >
                    </div>
                    <div v-if="autoTestEnabled" class="auto-countdown">
                        <Clock3 :size="14" /><span
                            >下次测试<strong>{{ autoCountdown }}</strong></span
                        >
                    </div>
                </div>
                <div class="expanded-actions">
                    <button
                        class="button secondary"
                        :disabled="busy || !latestRecord?.answer"
                        @click="copyAnswer"
                    >
                        <Check v-if="copied" :size="15" /><Copy v-else :size="15" />{{
                            copied ? '已复制' : '复制回答'
                        }}
                    </button>
                    <button
                        class="button"
                        :class="{ secondary: busy }"
                        :disabled="cancelling || selecting || (busy && !snapshot?.running)"
                        @click="primaryAction"
                    >
                        <X v-if="busy" :size="15" /><RotateCw v-else-if="latestRecord" :size="15" /><Play
                            v-else
                            :size="15"
                        />
                        {{ primaryLabel }}
                    </button>
                </div>
                <footer class="rule-note">
                    <Info :size="12" /><span>文字按连续字符串判定；HTML 仅展示效果</span>
                </footer>
                <p class="scope-note">适用于 GPT 6 及以上系列模型，其他模型无法保证测试准确度。</p>
            </section>

            <div v-if="!expanded" class="compact-actions">
                <div class="compact-status" aria-live="polite">
                    <span class="status-symbol" :class="state">
                        <LoaderCircle v-if="busy" :size="15" class="spin" />
                        <Check v-else-if="state === 'passed'" :size="12" />
                        <CircleAlert v-else-if="state === 'failed'" :size="12" />
                        <X v-else-if="state === 'error'" :size="12" />
                        <Minus v-else :size="12" /> </span
                    >{{ resultLabel }}
                </div>
                <div
                    class="compact-timing"
                    :title="`测试时间：${testTimeTitle}；请求耗时：${requestDuration}`"
                >
                    <!-- 标签必须直接可见，不能仅藏在悬停说明中；日期仍与耗时、倒计时分行。 -->
                    <span class="compact-test-time">测试时间 {{ compactTestTime }}</span>
                    <span v-if="busy || latestRecord || autoTestEnabled" class="compact-request-details">
                        <span v-if="busy || latestRecord">耗时 {{ requestDuration }}</span>
                        <span v-if="autoTestEnabled" class="auto-countdown"
                            >{{ busy || latestRecord ? ' · ' : '' }}下次 {{ autoCountdown }}</span
                        >
                    </span>
                </div>
                <button
                    class="button compact-primary"
                    :class="{ secondary: busy }"
                    :disabled="cancelling || selecting || (busy && !snapshot?.running)"
                    @click="primaryAction"
                >
                    <X v-if="busy" :size="13" /><RotateCw v-else-if="latestRecord" :size="13" /><Play
                        v-else
                        :size="13"
                    />
                    {{ primaryLabel }}
                </button>
            </div>
        </main>
    </PetStage>
</template>
