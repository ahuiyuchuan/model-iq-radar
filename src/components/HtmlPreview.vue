<!-- 轻量 HTML 画面：无外层工具栏/说明；小预览点击放大，页内弹层不创建原生子窗口。@author ahui -->
<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { LoaderCircle, X } from 'lucide-vue-next';
import { call } from '../bridge';
import type { HtmlCandidate } from '../htmlAnswer';
import {
    HTML_VIEWPORT,
    HTML_MAX_VIEWPORT,
    preparePreview,
    previewMessage,
    type PreparedPreview,
} from '../htmlSandbox';

const props = withDefaults(
    defineProps<{
        candidates: HtmlCandidate[];
        executable?: boolean;
    }>(),
    { executable: true },
);
const emit = defineEmits<{ failure: [message: string]; expandedChange: [expanded: boolean] }>();
const index = ref(0);
const expanded = ref(false);
const resizing = ref(false);
const loading = ref(false);
const message = ref('');
const background = ref('');
const backgroundStyle = computed(() =>
    background.value ? { '--html-preview-background': background.value } : undefined,
);
const url = ref('');
const frame = ref<HTMLIFrameElement>();
const stage = ref<HTMLElement>();
const dialog = ref<HTMLDialogElement>();
const scale = ref(0);
const viewport = ref<{ width: number; height: number }>({ ...HTML_VIEWPORT });
const candidate = computed(() => props.candidates[index.value] ?? props.candidates[0]);
// 失败/取消的部分回答和不完整候选只显示源码，原文仍由调用方负责完整复制。
const sourceOnly = computed(() => !props.executable || Boolean(candidate.value?.issue));
const canvasStyle = computed(() => ({
    width: `${viewport.value.width * scale.value}px`,
    height: `${viewport.value.height * scale.value}px`,
}));
let session: PreparedPreview | undefined;
let generation = 0;
let disposed = false;
let observer: ResizeObserver | undefined;
let deadline: ReturnType<typeof setTimeout> | undefined;
// 三处共用最短等待反馈，快速完成补足 300ms；慢加载只等真实 ready，不再额外延时。
const MIN_LOADING_MS = 300;
let loadingStartedAt = 0;
let loadingFinish: ReturnType<typeof setTimeout> | undefined;
let focusBefore: HTMLElement | null = null;

/** 连续等待只计算首次可见时刻；新的 load 取消旧就绪计时，避免旧页面提前撤掉遮罩。 */
function beginLoading(): void {
    clearTimeout(loadingFinish);
    if (!loading.value) loadingStartedAt = performance.now();
    loading.value = true;
}

/** 页面已就绪后仅补足最短反馈；切换/卸载/失败会取消计时，不能结束下一次加载。 */
function finishLoading(): void {
    clearTimeout(loadingFinish);
    const remaining = Math.max(0, MIN_LOADING_MS - (performance.now() - loadingStartedAt));
    const turn = generation;
    if (remaining === 0) loading.value = false;
    else
        loadingFinish = setTimeout(() => {
            if (!disposed && turn === generation) loading.value = false;
        }, remaining);
}

/** 先移除 iframe 再释放文档；世代隔离迟到的 prepare，不能复活旧回答。 */
function stop(): void {
    generation++;
    url.value = '';
    session?.dispose();
    session = undefined;
    loading.value = false;
    clearTimeout(deadline);
    clearTimeout(loadingFinish);
}

/** 只运行当前已保存的候选；切换记录或卸载时释放，不调用模型、不生成新历史。 */
async function start(): Promise<void> {
    stop();
    message.value = '';
    background.value = '';
    viewport.value = { ...HTML_VIEWPORT };
    layout();
    if (disposed) return;
    if (sourceOnly.value) {
        if (candidate.value?.issue) emit('failure', candidate.value.issue);
        return;
    }
    const source = candidate.value;
    if (!source) return;
    const turn = generation;
    beginLoading();
    try {
        const prepared = await preparePreview(source.source);
        if (disposed || turn !== generation) {
            prepared.dispose();
            return;
        }
        session = prepared;
        url.value = prepared.url;
        deadline = setTimeout(() => fail('HTML 加载超时'), 12000);
    } catch (cause) {
        if (!disposed && turn === generation) fail(String(cause));
    }
}
function fail(error: string): void {
    stop();
    message.value = error;
    emit('failure', error);
}

/** iframe 随放大/收回移入 dialog 时可能重新加载；每次真实 load 都重新等待已绘制消息。 */
function frameLoaded(): void {
    if (disposed || !session) return;
    beginLoading();
    clearTimeout(deadline);
    deadline = setTimeout(() => fail('HTML 加载超时'), 12000);
}

/** 完整桌面画布按宽高共同适配并居中；容器变大只改变缩放，避免裁掉正文底部或窄屏重排。 */
function layout(): void {
    const box = stage.value?.getBoundingClientRect();
    scale.value = box
        ? Math.max(0, Math.min(box.width / viewport.value.width, box.height / viewport.value.height))
        : 0;
}

/** dialog 约束焦点并支持 Escape；只临时调整同一宿主，重复点击互斥，关闭恢复原位置/尺寸。 */
async function setExpanded(value: boolean): Promise<void> {
    if (resizing.value || disposed || expanded.value === value) return;
    resizing.value = true;
    // 原生尺寸等待也属于这次可见加载时间，不能在操作返回后重新追加 300ms。
    if (session) beginLoading();
    if (value) {
        focusBefore = document.activeElement as HTMLElement | null;
        emit('expandedChange', true);
    }
    try {
        await call('set_html_preview_expanded', { expanded: value });
        if (disposed) {
            if (value) void call('set_html_preview_expanded', { expanded: false }).catch(() => {});
            return;
        }
        // Chromium 移动 iframe 会重建子文档；load 事件晚于清空画面，必须在 Teleport 前
        // 就进入等待态，并一直保留到新页面 ready，不能仅依靠两帧布局遮罩。
        frameLoaded();
        expanded.value = value;
        await nextTick();
        if (value) dialog.value?.showModal();
        else {
            dialog.value?.close();
            focusBefore?.focus();
            emit('expandedChange', false);
        }
        layout();
        // 原生窗口和 Teleport 都会改变布局；保留加载遮罩到新布局至少完成两帧，避免短暂空白。
        await new Promise<void>((resolve) =>
            requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
        );
    } catch (cause) {
        message.value = String(cause);
        clearTimeout(loadingFinish);
        loading.value = false;
        if (value) {
            // 原生尺寸操作可能部分成功；失败仍尝试恢复，不能持久化临时预览位置。
            void call('set_html_preview_expanded', { expanded: false }).catch(() => {});
            emit('expandedChange', false);
        }
    } finally {
        resizing.value = false;
    }
}

function receive(event: MessageEvent): void {
    if (!session || !previewMessage(event, frame.value, session.token)) return;
    if (event.data.type === 'dimensions') {
        const dimensions = event.data.value;
        const width: unknown = dimensions?.width;
        const height: unknown = dimensions?.height;
        // 模型脚本同样能发送消息；尺寸必须为有界整数，不能注入样式或创建无界巨大画布。
        if (
            typeof width !== 'number' ||
            typeof height !== 'number' ||
            !Number.isInteger(width) ||
            !Number.isInteger(height)
        )
            return;
        if (width < HTML_VIEWPORT.width || height < HTML_VIEWPORT.height) return;
        if (width > HTML_MAX_VIEWPORT.width || height > HTML_MAX_VIEWPORT.height) {
            fail('HTML 页面尺寸超过预览上限，请复制原始回答查看。');
            return;
        }
        viewport.value = { width, height };
        layout();
        return;
    }
    // 仅接受浏览器计算出的纯色格式；不接收背景图片、URL、变量或任意样式片段。
    // 透明/未声明背景回退到应用卡片色，不改写作品内部样式或原始回答。
    if (event.data.type === 'ready' || event.data.type === 'background') {
        const color: unknown = event.data.value;
        if (typeof color === 'string' && color.length <= 96 && /^rgba?\([\d.,% /]+\)$/.test(color)) {
            background.value = color;
        }
    }
    if (event.data.type === 'ready') {
        finishLoading();
        clearTimeout(deadline);
        emit('failure', '');
    } else if (event.data.type === 'escape' && expanded.value) void setExpanded(false);
}
watch(
    () => props.candidates,
    () => {
        index.value = 0;
        void start();
    },
);
watch([index, () => props.executable], () => void start());
watch(stage, (element) => {
    observer?.disconnect();
    if (element) observer?.observe(element);
    layout();
});
onMounted(() => {
    if (typeof ResizeObserver !== 'undefined') {
        observer = new ResizeObserver(layout);
        if (stage.value) observer.observe(stage.value);
    }
    window.addEventListener('message', receive);
    window.addEventListener('resize', layout);
    layout();
    void start();
});
onBeforeUnmount(() => {
    disposed = true;
    stop();
    observer?.disconnect();
    window.removeEventListener('message', receive);
    window.removeEventListener('resize', layout);
    if (expanded.value) {
        dialog.value?.close();
        void call('set_html_preview_expanded', { expanded: false }).catch(() => {});
    }
    emit('expandedChange', false);
});
</script>
<template>
    <div class="html-preview-host">
        <dialog
            ref="dialog"
            class="html-preview-dialog"
            :style="backgroundStyle"
            aria-label="放大 HTML 预览"
            @cancel.prevent="setExpanded(false)"
            @click="$event.target === dialog && setExpanded(false)"
        />
        <Teleport :to="dialog ?? 'body'" :disabled="!expanded">
            <section
                class="html-preview"
                :class="{ 'html-preview-expanded': expanded }"
                :style="backgroundStyle"
                aria-label="HTML 实际效果预览"
            >
                <button
                    v-if="expanded"
                    type="button"
                    class="html-preview-close"
                    :disabled="resizing"
                    aria-label="关闭放大预览"
                    @click="setExpanded(false)"
                >
                    <X :size="18" />
                </button>
                <!-- 仅多片段回答保留必要的切换入口；单片段没有任何附加工具栏或说明层。 -->
                <select
                    v-if="candidates.length > 1"
                    v-model.number="index"
                    class="html-preview-fragments"
                    aria-label="选择 HTML 片段"
                >
                    <option v-for="(item, i) in candidates" :key="i" :value="i">{{ item.label }}</option>
                </select>
                <pre v-if="sourceOnly" class="html-source" tabindex="0">{{
                    candidate?.source ?? '没有可展示的 HTML。'
                }}</pre>
                <div
                    v-else
                    ref="stage"
                    class="html-preview-stage"
                    :class="{ 'html-preview-clickable': !expanded && !message }"
                    :aria-busy="loading || resizing"
                    :role="expanded || message ? undefined : 'button'"
                    :tabindex="expanded || message ? undefined : 0"
                    :aria-label="expanded ? undefined : '放大 HTML 预览'"
                    @click="!expanded && !message && setExpanded(true)"
                    @keydown.enter.self.prevent="!expanded && !message && setExpanded(true)"
                    @keydown.space.self.prevent="!expanded && !message && setExpanded(true)"
                >
                    <div v-if="url" class="html-preview-canvas" :style="canvasStyle">
                        <iframe
                            ref="frame"
                            :src="url"
                            sandbox="allow-scripts"
                            scrolling="no"
                            referrerpolicy="no-referrer"
                            title="HTML 沙箱效果"
                            @load="frameLoaded"
                            :tabindex="expanded ? 0 : -1"
                            :style="{
                                width: `${viewport.width}px`,
                                height: `${viewport.height}px`,
                                transform: `scale(${scale})`,
                                pointerEvents: expanded ? 'auto' : 'none',
                            }"
                        />
                    </div>
                    <div v-if="message" class="html-preview-error" role="status">
                        {{ message
                        }}<button type="button" class="button secondary" @click.stop="start">重新加载</button>
                    </div>
                    <span v-else-if="loading || resizing" class="html-preview-loading" role="status">
                        <span class="html-preview-loading-content">
                            <LoaderCircle class="html-preview-spinner" :size="22" aria-hidden="true" />
                            <span>正在加载 HTML…</span>
                        </span>
                    </span>
                </div>
            </section>
        </Teleport>
    </div>
</template>
