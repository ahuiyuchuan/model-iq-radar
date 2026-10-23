<!--
    悬浮组件：单一主操作、独立拖动区域、可滚动回答。所有模型文本通过 Vue 转义。
    @author ahui
-->
<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import {
    Target,
    Settings2,
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
} from 'lucide-vue-next';
import { call, dragWindow, openSettings, resizeWidget, subscribeDock } from '../bridge';
import { formatTime, useProbe } from '../useProbe';
import ModelPicker from './ModelPicker.vue';
import MarkdownAnswer from './MarkdownAnswer.vue';

const { snapshot, snapshotReceivedAt, error, profile, latest, busy, start, cancel } = useProbe();
const uiNow = ref({ monotonic: performance.now(), wall: Date.now() });
let clockTimer: ReturnType<typeof setInterval> | undefined;
const expanded = ref(false);
const copied = ref(false);
const cancelling = ref(false);
const picker = ref<InstanceType<typeof ModelPicker>>();
const pickerOpen = ref(false);
const selecting = ref(false);
const edgeSide = ref<string | null>(null);
const tucked = ref(false);
const collapsing = ref(false);
const revealing = ref(false);
let animationGeneration = 0;
let hovering = false;
let tuckTimer: ReturnType<typeof setTimeout> | undefined;
let unsubscribeDock: (() => void) | undefined;
let disposed = false;

/** 鼠标离开 250ms 后收回；只缩短等待，保留原有 180/220ms 过渡动画。 */
function scheduleTuck(): void {
    clearTimeout(tuckTimer);
    if (!edgeSide.value || tucked.value) return;
    tuckTimer = setTimeout(() => {
        if (!hovering && !pickerOpen.value && !selecting.value) void setTucked(true);
    }, 250);
}

/** 边签使用真实小窗口，不留下透明的大面积鼠标遮挡。 */
async function setTucked(value: boolean): Promise<void> {
    if (disposed || (!value && revealing.value)) return;
    clearTimeout(tuckTimer);
    const generation = ++animationGeneration;
    try {
        if (value) {
            collapsing.value = true;
            if (!window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) {
                await new Promise((resolve) => setTimeout(resolve, 180));
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

onMounted(async () => {
    clockTimer = setInterval(() => {
        uiNow.value = { monotonic: performance.now(), wall: Date.now() };
    }, 1000);
    try {
        const cleanup = await subscribeDock((dock) => {
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
    clearTimeout(tuckTimer);
    clearInterval(clockTimer);
    unsubscribeDock?.();
});

const state = computed(() => (busy.value ? 'running' : (latest.value?.status ?? 'idle')));
const stateLabels: Record<string, string> = {
    idle: '待测试',
    running: '测试中',
    passed: '不降智',
    failed: '降智',
    error: '测试失败',
    cancelled: '已取消',
};
const resultLabel = computed(() => stateLabels[state.value]);
/** 两种布局共用主按钮文案：任何已完成/失败/取消记录均允许重新测试。 */
const primaryLabel = computed(() =>
    busy.value
        ? cancelling.value
            ? '正在取消'
            : '取消测试'
        : !profile.value
          ? '配置接口'
          : latest.value
            ? '重新测试'
            : '开始测试',
);
const testStartedAt = computed(() =>
    busy.value ? snapshot.value?.running?.startedAt : latest.value?.startedAt,
);
const compactTestTime = computed(() => {
    const value = testStartedAt.value ? new Date(testStartedAt.value) : null;
    return value && !Number.isNaN(value.getTime())
        ? value.toLocaleTimeString('zh-CN', { hour12: false })
        : '尚未测试';
});
const requestDuration = computed(() => {
    if (!busy.value) return latest.value ? `${(latest.value.durationMs / 1000).toFixed(2)}s` : '—';
    const started = Date.parse(testStartedAt.value ?? '');
    return Number.isFinite(started) ? `${(Math.max(0, uiNow.value.wall - started) / 1000).toFixed(1)}s` : '—';
});
const autoTestEnabled = computed(() => snapshot.value?.config.preferences.autoTestEnabled ?? false);
/** 后端提供剩余时长，前端用单调时钟平滑显示；不每秒发 IPC，也不自行触发付费请求。 */
const autoCountdown = computed(() => {
    if (!profile.value) return '待配置';
    const remaining = snapshot.value?.autoTestRemainingMs;
    if (remaining == null) return '同步中';
    const seconds = Math.ceil(
        Math.max(0, remaining - Math.max(0, uiNow.value.monotonic - snapshotReceivedAt.value)) / 1000,
    );
    if (seconds === 0) return busy.value ? '等待空闲' : '即将测试';
    const minutes = Math.floor(seconds / 60);
    return `${minutes.toString().padStart(2, '0')}:${(seconds % 60).toString().padStart(2, '0')}`;
});
const hint = computed(() => {
    if (busy.value) return cancelling.value ? '正在取消请求…' : '正在等待模型完整回答…';
    if (!latest.value) return '配置好接口后，即可开始第一次测试';
    if (latest.value.status === 'passed') return '命中连续字符串：';
    if (latest.value.status === 'failed') return '未命中连续字符串：';
    return latest.value.error || '本次未产生判定结果';
});

/** 保持窗口外框与内容尺寸一致，失败时撤回展开状态并提供可见提示。 */
async function toggleExpanded(): Promise<void> {
    const next = !expanded.value;
    try {
        await picker.value?.close();
        await resizeWidget(next);
        expanded.value = next;
    } catch (cause) {
        error.value = String(cause);
    }
}

/** 没有配置时主操作直接进入设置；不产生无意义的失败历史。 */
async function primaryAction(): Promise<void> {
    if (!profile.value) {
        await safeOpenSettings();
    } else if (busy.value) {
        cancelling.value = true;
        await cancel();
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

/** 模型选择只更新后端选中项，不复制界面中的旧配置覆盖其他窗口。 */
async function selectProfile(id: string): Promise<void> {
    if (selecting.value || busy.value) return;
    selecting.value = true;
    try {
        await call('select_profile', { id });
    } catch (cause) {
        error.value = String(cause);
    } finally {
        selecting.value = false;
    }
}

/** 复制完整正文，剪贴板权限失败时给出可读错误，保留原结果。 */
async function copyAnswer(): Promise<void> {
    try {
        await navigator.clipboard.writeText(latest.value?.answer ?? '');
        copied.value = true;
        window.setTimeout(() => {
            copied.value = false;
        }, 1800);
    } catch {
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
    <button
        v-if="tucked"
        class="edge-tab"
        :class="[edgeSide, `state-${state}`, { 'edge-restoring': revealing }]"
        :aria-label="`显示测智雷达：${resultLabel}`"
        :title="`${resultLabel} · 移入鼠标展开`"
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
        v-else
        class="widget panel"
        :class="[
            { expanded, 'picker-visible': pickerOpen, 'dock-away': collapsing || revealing },
            `state-${state}`,
            edgeSide ? `dock-${edgeSide}` : '',
        ]"
        :style="{ '--panel-opacity': (snapshot?.config.preferences.opacity ?? 97) / 100 }"
        aria-label="测智雷达悬浮组件"
        @mouseenter="pointerEntered"
        @mouseleave="pointerLeft"
    >
        <header class="widget-header drag-area" @mousedown.left="dragWindow().catch(() => {})">
            <div class="brand">
                <Target class="brand-mark" :size="17" aria-hidden="true" />
                <span>测智雷达</span>
            </div>
            <Grip :size="22" class="drag-grip" aria-hidden="true" />
            <div class="widget-tools" @mousedown.stop>
                <button class="icon-button" aria-label="打开设置" title="设置" @click="safeOpenSettings">
                    <Settings2 :size="15" />
                </button>
                <button
                    class="icon-button expand-button"
                    :aria-expanded="expanded"
                    :aria-label="expanded ? '收起结果' : '展开测试详情'"
                    :title="expanded ? '收起' : '展开'"
                    @click="toggleExpanded"
                >
                    <ChevronUp v-if="expanded" :size="16" /><ChevronDown v-else :size="16" />
                </button>
            </div>
        </header>

        <ModelPicker
            ref="picker"
            :profiles="snapshot?.config.profiles ?? []"
            :selected-id="snapshot?.config.selectedProfileId ?? null"
            :expanded="expanded"
            :disabled="busy || selecting"
            @select="selectProfile"
            @configure="safeOpenSettings"
            @open-change="pickerOpen = $event"
            @error="error = $event"
        />

        <section v-if="expanded" class="result-content">
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
                    <p :title="hint + (latest?.keyword ?? '')">
                        {{ hint }}
                        <strong v-if="!busy && (state === 'passed' || state === 'failed')">{{
                            latest?.keyword
                        }}</strong>
                    </p>
                </div>
            </div>

            <div v-if="error" class="inline-error" role="alert">{{ error }}</div>
            <div class="answer-label"><span>模型回答</span><span class="eyebrow">完整正文</span></div>
            <div class="answer-box" tabindex="0" :aria-busy="busy">
                <div v-if="busy" class="answer-placeholder">
                    <LoaderCircle :size="18" class="spin" />等待模型回答
                </div>
                <MarkdownAnswer v-else-if="latest?.answer" :source="latest.answer" />
                <p v-else class="muted">
                    {{ state === 'idle' ? '测试完成后，回答会显示在这里。' : '没有可展示的回答。' }}
                </p>
            </div>
            <div class="metadata" :class="{ 'with-countdown': autoTestEnabled }">
                <div>
                    <Clock3 :size="14" /><span
                        >测试时间<strong :title="formatTime(testStartedAt)">{{
                            formatTime(testStartedAt)
                        }}</strong></span
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
                <button class="button secondary" :disabled="busy || !latest?.answer" @click="copyAnswer">
                    <Check v-if="copied" :size="15" /><Copy v-else :size="15" />{{
                        copied ? '已复制' : '复制回答'
                    }}
                </button>
                <button
                    class="button"
                    :class="{ secondary: busy }"
                    :disabled="cancelling || selecting"
                    @click="primaryAction"
                >
                    <X v-if="busy" :size="15" /><RotateCw v-else-if="latest" :size="15" /><Play
                        v-else
                        :size="15"
                    />
                    {{ primaryLabel }}
                </button>
            </div>
            <footer class="rule-note"><Info :size="12" /><span>结果仅依据自定义连续字符串规则</span></footer>
            <p class="scope-note">适用于 GPT 6 及以上系列模型，其他模型无法保证测试准确度。</p>
        </section>

        <div v-else class="compact-actions">
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
                :title="`测试时间：${formatTime(testStartedAt)}；请求耗时：${requestDuration}`"
            >
                <span
                    >测试时间 {{ compactTestTime }} <span>耗时 {{ requestDuration }}</span></span
                >
                <span v-if="autoTestEnabled" class="auto-countdown">下次测试 {{ autoCountdown }}</span>
            </div>
            <button
                class="button compact-primary"
                :class="{ secondary: busy }"
                :disabled="cancelling || selecting"
                @click="primaryAction"
            >
                <X v-if="busy" :size="13" /><RotateCw v-else-if="latest" :size="13" /><Play
                    v-else
                    :size="13"
                />
                {{ primaryLabel }}
            </button>
        </div>
    </main>
</template>
