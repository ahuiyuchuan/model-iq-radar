<!--
    悬浮窗和设置页共用深色配置选择器，键盘焦点保留在触发按钮或可编辑输入框。
    只有悬浮窗上下文调整真实窗口尺寸；列表只提供已有项，新建草稿由外部按钮负责。@author ahui
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
        /** 当前值没有对应选项时的触发器文案；仅说明空态/草稿，不作为可选列表项。 */
        placeholder?: string;
        /** 触发器的可访问名称，区分切换当前模型和编辑接口。 */
        label?: string;
        /** 模型目录仅显示 ID；配置选择仍默认显示“分组 : 模型”。 */
        showModel?: boolean;
        /** 窄型强度选择器省去装饰图标，保留文字与箭头；默认仍显示模型图标。 */
        showIcon?: boolean;
        /** 已核实能力与兼容提示，随触发器完整值显示，不触发参数降级或请求。 */
        description?: string;
        /** 打开前关闭同组其他菜单；在窗口扩高前等待，避免两层菜单或旧尺寸互相覆盖。 */
        beforeOpen?: () => Promise<void>;
        /** 卡片模型菜单的规则/强度摘要；只展示元数据，不改变选择行为或其他页面布局。 */
        summaries?: Record<string, { rule: string; effort: string }>;
        /** 设置页模型名称复用同一列表，可手输任意模型 ID；不因失焦自动选择候选。 */
        editable?: boolean;
        inputId?: string;
        modelValue?: string;
    }>(),
    {
        context: 'widget',
        placeholder: '配置一个模型',
        label: '当前模型配置',
        showModel: true,
        showIcon: true,
    },
);
const emit = defineEmits<{
    select: [id: string];
    configure: [];
    openChange: [open: boolean];
    error: [message: string];
    'update:modelValue': [value: string];
}>();
const root = ref<HTMLElement>();
const pickerId = useId();
const open = ref(false);
const opening = ref(false);
let selecting = false;
const activeIndex = ref(0);
const opensUp = ref(false);
const settingsMenuHeight = ref(210);
// 仅主动输入触发过滤；已保存的自定义 ID 不应遮住服务商返回的其他候选。
const filterText = ref<string | null>(null);
// 下拉仅选择调用方提供的数据；不能为展示未保存草稿而注入一个可被选中的“新增”动作。
const options = computed(() => {
    const query = props.editable ? (filterText.value?.toLocaleLowerCase() ?? '') : '';
    // 已选完整 ID 时打开全部缓存，便于换模型；输入搜索时只过滤展示，不删除缓存。
    return !query || props.profiles.some((item) => item.id.toLocaleLowerCase() === query)
        ? props.profiles
        : props.profiles.filter((item) => `${item.id} ${item.model}`.toLocaleLowerCase().includes(query));
});
const selected = computed(() => options.value.find((item) => item.id === props.selectedId));
/** 有选中项时显示名称及可选模型 ID；无匹配项时使用独立草稿提示，不伪造数据项。 */
const selectedLabel = computed(() => {
    const item = selected.value;
    return item ? (props.showModel ? `${item.name} : ${item.model}` : item.name) : props.placeholder;
});
let generation = 0;

/** 窗口尺寸异常交由悬浮卡片显示，避免异步事件形成未处理拒绝。 */
function report(cause: unknown): void {
    emit('error', String(cause));
}

/** 取消尚未完成的打开操作；关闭可由外部展开按钮等待，确保最终窗口尺寸一致。 */
async function close(): Promise<void> {
    if (!open.value && !opening.value) return;
    generation++;
    open.value = false;
    filterText.value = null;
    emit('openChange', false);
    if (props.context === 'widget') await resizeWidget(props.expanded);
}

/** 只过滤当前展示，原缓存和已输入模型 ID 均保留；父级更新其他模型时清除旧查询。 */
function editModel(event: Event): void {
    const value = (event.target as HTMLInputElement).value;
    filterText.value = value;
    emit('update:modelValue', value);
}
watch(
    () => props.modelValue,
    (value) => {
        if (value !== filterText.value) filterText.value = null;
    },
);

/** 最多显示五项，其余滚动；窗口只为菜单增加必要高度。 */
async function toggle(): Promise<void> {
    if (props.disabled || opening.value || selecting) return;
    if (!options.value.length) {
        emit('configure');
        return;
    }
    if (open.value) return close();
    const current = ++generation;
    opening.value = true;
    activeIndex.value = Math.max(
        0,
        options.value.findIndex((item) => item.id === props.selectedId),
    );
    try {
        await props.beforeOpen?.();
        if (current !== generation || props.disabled) return;
        if (props.context === 'widget') {
            // 五行 + 上下内边距 8px + 边框 2px，不能因漏算边框出现仅 2px 的滚动条。
            await resizeWidget(props.expanded, Math.min(options.value.length, 5) * 40 + 10);
        } else if (root.value) {
            // 设置页只滚动主体。靠近底栏时向上打开，菜单仍限制在可见滚动视口，不能被裁切。
            const rect = root.value.getBoundingClientRect();
            const scroll = root.value.closest('.settings-scroll, .dialog-body')?.getBoundingClientRect();
            const top = Math.max(0, scroll?.top ?? 0);
            const bottom = Math.min(window.innerHeight, scroll?.bottom ?? window.innerHeight);
            const below = bottom - rect.bottom - 8;
            const above = rect.top - top - 8;
            const requested = Math.min(options.value.length, 5) * 40 + 10;
            opensUp.value = below < requested && above > below;
            settingsMenuHeight.value = Math.max(50, Math.min(210, opensUp.value ? above : below));
        }
        if (current !== generation || props.disabled) return;
        open.value = true;
        emit('openChange', true);
        await scrollActive();
    } finally {
        opening.value = false;
    }
}

/** 仅改变选择，不发起测试；测试期间禁止从鼠标或键盘提交。 */
async function choose(index: number): Promise<void> {
    const item = options.value[index];
    if (props.disabled || !open.value || selecting || !item) return;
    selecting = true;
    try {
        await close();
        // 调整原生尺寸期间可能进入测试或删除配置，不能提交旧列表中的过期选项。
        if (
            !props.disabled &&
            options.value.some((option) => option.id === item.id) &&
            item.id !== props.selectedId
        ) {
            if (props.editable) emit('update:modelValue', item.id);
            else emit('select', item.id);
        }
    } finally {
        selecting = false;
    }
}

/** 保持键盘高亮项在五行滚动视口内。 */
async function scrollActive(): Promise<void> {
    await nextTick();
    root.value
        ?.querySelector(`[data-option-index="${activeIndex.value}"]`)
        ?.scrollIntoView?.({ block: 'nearest' });
}

/** 普通选择支持首尾/空格；可编辑模式保留文本操作和输入法，只用上下/Enter 选择。 */
async function onKey(event: KeyboardEvent): Promise<void> {
    if (props.disabled) return;
    // 中文输入法确认不能选中模型；即便列表展开，空格、Home/End 和组合键仍用于编辑。
    if (props.editable) {
        if (event.isComposing) return;
        if (
            (event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) &&
            !['Tab', 'Escape'].includes(event.key)
        )
            return;
        if (!['ArrowDown', 'ArrowUp', 'Enter', 'Escape', 'Tab'].includes(event.key)) return;
        if (!open.value && event.key === 'Enter') return;
    }
    if (event.key === 'Tab' || event.key === 'Escape') {
        if (event.key === 'Escape' && (open.value || opening.value)) event.stopPropagation();
        if (open.value || opening.value) await close();
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
    if ((open.value || opening.value) && !root.value?.contains(event.target as Node))
        void close().catch(report);
}
function blur(): void {
    if (open.value || opening.value) void close().catch(report);
}
watch(
    () => props.disabled,
    (value) => {
        if (value && (open.value || opening.value)) void close().catch(report);
    },
);
// 列表可能由后台快照替换；高亮索引始终指向当前列表，不保留无效 aria-activedescendant。
watch(options, (value) => {
    activeIndex.value = Math.max(0, Math.min(activeIndex.value, value.length - 1));
    if (!value.length && (open.value || opening.value)) void close().catch(report);
});
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
    <div ref="root" class="model-select" :class="{ 'picker-open': open, 'editable-picker': editable }">
        <div v-if="editable" class="model-input-wrap">
            <input
                :id="inputId"
                :value="modelValue"
                type="text"
                role="combobox"
                :aria-label="label"
                aria-autocomplete="list"
                aria-haspopup="listbox"
                :aria-expanded="open"
                :aria-controls="open ? `${pickerId}-options` : undefined"
                :aria-activedescendant="open ? `${pickerId}-option-${activeIndex}` : undefined"
                required
                maxlength="200"
                autocomplete="off"
                spellcheck="false"
                placeholder="输入或选择模型 ID"
                :disabled="disabled"
                @input="editModel"
                @click="!open && options.length && toggle().catch(report)"
                @keydown="onKey($event).catch(report)"
            />
            <button
                type="button"
                class="icon-button"
                aria-label="展开模型列表"
                :disabled="disabled || !profiles.length"
                :aria-expanded="open"
                @click="
                    filterText = null;
                    toggle().catch(report);
                "
            >
                <ChevronDown :size="15" aria-hidden="true" />
            </button>
        </div>
        <button
            v-else
            type="button"
            class="model-trigger"
            role="combobox"
            :aria-label="label"
            aria-haspopup="listbox"
            :aria-expanded="open"
            :aria-controls="open ? `${pickerId}-options` : undefined"
            :aria-activedescendant="open ? `${pickerId}-option-${activeIndex}` : undefined"
            :disabled="disabled"
            :title="`${label}：${selectedLabel}${description ? `\n${description}` : ''}`"
            @click="toggle().catch(report)"
            @keydown="onKey($event).catch(report)"
        >
            <Box v-if="showIcon" :size="16" aria-hidden="true" />
            <span>{{ selectedLabel }}</span>
            <ChevronDown :size="15" class="picker-chevron" aria-hidden="true" />
        </button>
        <div
            v-if="open"
            :id="`${pickerId}-options`"
            class="model-menu"
            :class="{ 'opens-up': context === 'settings' && opensUp }"
            :style="context === 'settings' ? { maxHeight: `${settingsMenuHeight}px` } : undefined"
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
                :class="{
                    active: activeIndex === index,
                    selected: item.id === selectedId,
                    'has-summary': Boolean(summaries?.[item.id]),
                }"
                role="option"
                :aria-selected="item.id === selectedId"
                :title="`${item.name} · ${item.model}${summaries?.[item.id] ? ` · 规则：${summaries[item.id]!.rule} · 思考：${summaries[item.id]!.effort}` : ''}`"
                tabindex="-1"
                @pointermove="activeIndex = index"
                @pointerdown.prevent
                @click="choose(index).catch(report)"
            >
                <span class="option-copy"
                    ><span>{{ summaries?.[item.id] ? `${item.name} : ${item.model}` : item.name }}</span
                    ><small v-if="summaries?.[item.id]">{{ summaries[item.id]!.rule }}</small>
                    <small v-else-if="item.model">{{ item.model }}</small></span
                >
                <span v-if="summaries?.[item.id]" class="option-effort">{{
                    summaries[item.id]!.effort
                }}</span>
                <Check v-if="item.id === selectedId" :size="14" aria-hidden="true" />
            </button>
        </div>
    </div>
</template>
