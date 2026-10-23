<!--
    悬浮窗和设置页共用深色配置选择器，键盘焦点保留在触发按钮。
    只有悬浮窗上下文调整真实窗口尺寸；设置页的新建项仅清空草稿，不保存或调用模型。@author ahui
-->
<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, useId, watch } from 'vue';
import { Box, Check, ChevronDown } from 'lucide-vue-next';
import { resizeWidget } from '../bridge';
import type { ModelProfile } from '../types';

const props = withDefaults(
    defineProps<{
        profiles: Pick<ModelProfile, 'id' | 'name' | 'model'>[];
        selectedId: string | null;
        expanded: boolean;
        disabled: boolean;
        /** 设置页复用外观和键盘逻辑，但不调整悬浮窗尺寸。默认 widget。 */
        context?: 'widget' | 'settings';
        /** 是否在列表开头提供新建草稿项；该项以空 ID 传出，不作为模型保存。 */
        allowNew?: boolean;
        /** 触发器的可访问名称，区分切换当前模型和编辑接口。 */
        label?: string;
        /** 模型目录仅显示 ID；配置选择仍默认显示“分组 : 模型”。 */
        showModel?: boolean;
    }>(),
    { context: 'widget', allowNew: false, label: '当前模型配置', showModel: true },
);
const emit = defineEmits<{
    select: [id: string];
    configure: [];
    openChange: [open: boolean];
    error: [message: string];
}>();
const root = ref<HTMLElement>();
const pickerId = useId();
const open = ref(false);
const activeIndex = ref(0);
// 新建项只携带渲染和选择所需字段，避免构造可被误保存的虚拟接口配置。
const options = computed(() =>
    props.allowNew
        ? [{ id: '', name: '新建接口配置', model: '清空表单，创建新配置' }, ...props.profiles]
        : props.profiles,
);
const selected = computed(() => options.value.find((item) => item.id === props.selectedId));
/** 同时显示分组名称与真实模型 ID；新建草稿提示不是模型名，不拼入标题。 */
const selectedLabel = computed(() => {
    const item = selected.value;
    return item ? (item.id && props.showModel ? `${item.name} : ${item.model}` : item.name) : '配置一个模型';
});
let generation = 0;

/** 窗口尺寸异常交由悬浮卡片显示，避免异步事件形成未处理拒绝。 */
function report(cause: unknown): void {
    emit('error', String(cause));
}

/** 取消尚未完成的打开操作；关闭可由外部展开按钮等待，确保最终窗口尺寸一致。 */
async function close(): Promise<void> {
    generation++;
    open.value = false;
    emit('openChange', false);
    if (props.context === 'widget') await resizeWidget(props.expanded);
}

/** 最多显示五项，其余滚动；窗口只为菜单增加必要高度。 */
async function toggle(): Promise<void> {
    if (props.disabled) return;
    if (!options.value.length) {
        emit('configure');
        return;
    }
    if (open.value) return close();
    const current = ++generation;
    activeIndex.value = Math.max(
        0,
        options.value.findIndex((item) => item.id === props.selectedId),
    );
    if (props.context === 'widget') {
        await resizeWidget(props.expanded, Math.min(options.value.length, 5) * 40 + 8);
    }
    if (current !== generation || props.disabled) return;
    open.value = true;
    emit('openChange', true);
    await scrollActive();
}

/** 仅改变选择，不发起测试；测试期间禁止从鼠标或键盘提交。 */
async function choose(index: number): Promise<void> {
    const item = options.value[index];
    if (props.disabled || !item) return;
    await close();
    if (item.id !== props.selectedId) emit('select', item.id);
}

/** 保持键盘高亮项在五行滚动视口内。 */
async function scrollActive(): Promise<void> {
    await nextTick();
    root.value
        ?.querySelector(`[data-option-index="${activeIndex.value}"]`)
        ?.scrollIntoView?.({ block: 'nearest' });
}

/** 支持方向键、首尾、Enter/空格选择、Escape/Tab 关闭，不拦截正常 Tab 顺序。 */
async function onKey(event: KeyboardEvent): Promise<void> {
    if (props.disabled) return;
    if (event.key === 'Tab' || event.key === 'Escape') {
        if (open.value) await close();
        if (event.key === 'Escape') event.preventDefault();
        return;
    }
    if (!['ArrowDown', 'ArrowUp', 'Home', 'End', 'Enter', ' '].includes(event.key)) return;
    event.preventDefault();
    if (!open.value) return toggle();
    if (event.key === 'Enter' || event.key === ' ') return choose(activeIndex.value);
    const last = options.value.length - 1;
    if (event.key === 'Home') activeIndex.value = 0;
    else if (event.key === 'End') activeIndex.value = last;
    else
        activeIndex.value =
            (activeIndex.value + (event.key === 'ArrowDown' ? 1 : -1) + last + 1) % (last + 1);
    await scrollActive();
}

/** 点击其他区域或窗口失焦时收回菜单，监听器随组件销毁清理。 */
function outside(event: PointerEvent): void {
    if (open.value && !root.value?.contains(event.target as Node)) void close().catch(report);
}
function blur(): void {
    if (open.value) void close().catch(report);
}
watch(
    () => props.disabled,
    (value) => {
        if (value && open.value) void close().catch(report);
    },
);
onMounted(() => {
    document.addEventListener('pointerdown', outside);
    window.addEventListener('blur', blur);
});
onUnmounted(() => {
    generation++;
    document.removeEventListener('pointerdown', outside);
    window.removeEventListener('blur', blur);
});
defineExpose({ close });
</script>

<template>
    <div ref="root" class="model-select" :class="{ 'picker-open': open }">
        <button
            type="button"
            class="model-trigger"
            role="combobox"
            :aria-label="label"
            aria-haspopup="listbox"
            :aria-expanded="open"
            :aria-controls="open ? `${pickerId}-options` : undefined"
            :aria-activedescendant="open ? `${pickerId}-option-${activeIndex}` : undefined"
            :disabled="disabled"
            :title="selectedLabel"
            @click="toggle().catch(report)"
            @keydown="onKey($event).catch(report)"
        >
            <Box :size="16" aria-hidden="true" />
            <span>{{ selectedLabel }}</span>
            <ChevronDown :size="15" class="picker-chevron" aria-hidden="true" />
        </button>
        <div
            v-if="open"
            :id="`${pickerId}-options`"
            class="model-menu"
            role="listbox"
            :aria-label="`${label}列表`"
        >
            <button
                v-for="(item, index) in options"
                :id="`${pickerId}-option-${index}`"
                :data-option-index="index"
                :key="item.id"
                type="button"
                class="model-option"
                :class="{ active: activeIndex === index, selected: item.id === selectedId }"
                role="option"
                :aria-selected="item.id === selectedId"
                :title="`${item.name} · ${item.model}`"
                tabindex="-1"
                @pointermove="activeIndex = index"
                @pointerdown.prevent
                @click="choose(index).catch(report)"
            >
                <span class="option-copy"
                    ><span>{{ item.name }}</span
                    ><small>{{ item.model }}</small></span
                >
                <Check v-if="item.id === selectedId" :size="14" aria-hidden="true" />
            </button>
        </div>
    </div>
</template>
