<!--
    本机日期筛选：保留原生日期字段的输入/清空能力，日历弹层复用首版深色菜单。
    只传递 YYYY-MM-DD，不经 UTC 转换，不联网；焦点和全局监听随组件生命周期清理。
    @author ahui
-->
<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, useId, watch } from 'vue';
import { CalendarDays, ChevronLeft, ChevronRight, ChevronsLeft, ChevronsRight } from 'lucide-vue-next';

/** 筛选值为空串或本机 YYYY-MM-DD；label 是输入框及日历的无障碍名称，disabled 时关闭弹层。 */
const props = withDefaults(defineProps<{ modelValue: string; label: string; disabled?: boolean }>(), {
    disabled: false,
});
const emit = defineEmits<{ 'update:modelValue': [value: string] }>();
const root = ref<HTMLElement>();
const input = ref<HTMLInputElement>();
const popup = ref<HTMLElement>();
const pickerId = useId();
const open = ref(false);
const popupStyle = ref({ width: '280px', left: '0px', top: 'calc(100% + 6px)', maxHeight: '304px' });
const weekdays = ['日', '一', '二', '三', '四', '五', '六'];

/** 在本机中午构造日期，避免 ISO 字符串的 UTC 偏移及 0—99 年构造器特殊规则。 */
function dateAt(year: number, month: number, day: number): Date {
    const date = new Date();
    date.setHours(12, 0, 0, 0);
    date.setFullYear(year, month, day);
    return date;
}
/** 输入框与日历共用本机日历日；空串和不合法日期均不作为选中值。 */
function parseDate(value: string): Date | null {
    const parts = /^(\d{4})-(\d{2})-(\d{2})$/u.exec(value);
    if (!parts) return null;
    const [year, month, day] = parts.slice(1).map(Number);
    const date = dateAt(year, month - 1, day);
    return year >= 1 && date.getFullYear() === year && date.getMonth() === month - 1 && date.getDate() === day
        ? date
        : null;
}
/** 序列化本机日期，不使用 toISOString，以免负时区或夏令时改变筛选日期。 */
function dateValue(date: Date): string {
    return `${String(date.getFullYear()).padStart(4, '0')}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}
const initial = parseDate(props.modelValue) ?? new Date();
const view = ref(dateAt(initial.getFullYear(), initial.getMonth(), 1));
const activeDate = ref(dateValue(initial));
const today = ref(dateValue(new Date()));
const monthTitle = computed(() => `${view.value.getFullYear()}年${view.value.getMonth() + 1}月`);
const rows = computed(() => {
    const first = view.value;
    const days = Array.from({ length: 42 }, (_, index) => {
        const date = dateAt(first.getFullYear(), first.getMonth(), 1 - first.getDay() + index);
        return {
            value: dateValue(date),
            label: `${date.getFullYear()}年${date.getMonth() + 1}月${date.getDate()}日`,
            day: date.getDate(),
            outside: date.getMonth() !== first.getMonth(),
            disabled: date.getFullYear() < 1 || date.getFullYear() > 9999,
        };
    });
    return Array.from({ length: 6 }, (_, index) => days.slice(index * 7, index * 7 + 7));
});

/**
 * 弹层复用首版菜单并限于滚动主体：优先向下/上打开，右侧向左避让。
 * 两侧均不足时在主体内移位，避免小窗口裁掉日期和操作；极端高度才在弹层内滚动。
 */
async function openCalendar(focusDay = false): Promise<void> {
    if (props.disabled) return;
    if (!open.value) {
        const date = parseDate(props.modelValue) ?? new Date();
        today.value = dateValue(new Date());
        view.value = dateAt(date.getFullYear(), date.getMonth(), 1);
        activeDate.value = dateValue(date);
        const rect = root.value?.getBoundingClientRect();
        const bounds = root.value?.closest('.settings-scroll, .dialog-body')?.getBoundingClientRect();
        if (rect) {
            const left = Math.max(0, bounds?.left ?? 0) + 8;
            const right = Math.min(window.innerWidth, bounds?.right ?? window.innerWidth) - 8;
            const width = Math.min(280, Math.max(0, right - left));
            const top = Math.max(0, bounds?.top ?? 0) + 8;
            const bottom = Math.min(window.innerHeight, bounds?.bottom ?? window.innerHeight) - 8;
            const height = Math.max(0, Math.min(304, bottom - top));
            const below = bottom - rect.bottom - 6;
            const above = rect.top - top - 6;
            const popupTop =
                below >= height
                    ? rect.bottom + 6
                    : above >= height
                      ? rect.top - height - 6
                      : Math.max(top, Math.min(rect.bottom + 6, bottom - height));
            popupStyle.value = {
                width: `${width}px`,
                left: `${Math.max(left, Math.min(rect.left, right - width)) - rect.left}px`,
                top: `${popupTop - rect.top}px`,
                maxHeight: `${height}px`,
            };
        }
        open.value = true;
    }
    if (focusDay) {
        await nextTick();
        focusActiveDay();
    }
}
/** 小窗口内只滚动日历使键盘目标可见，避免滚动外层触发关闭或让焦点藏在裁切区。 */
function focusActiveDay(): void {
    const menu = popup.value;
    const day = menu?.querySelector<HTMLButtonElement>(`[data-date="${activeDate.value}"]`);
    if (!menu || !day) return;
    day.focus({ preventScroll: true });
    const bounds = menu.getBoundingClientRect();
    const target = day.getBoundingClientRect();
    if (target.bottom > bounds.bottom - 8) menu.scrollTop += target.bottom - bounds.bottom + 8;
    else if (target.top < bounds.top + 8) menu.scrollTop -= bounds.top - target.top + 8;
}
function closeCalendar(restoreFocus = false): void {
    open.value = false;
    if (restoreFocus) input.value?.focus({ preventScroll: true });
}
/** 选日、今天和清空均只修改筛选，不请求模型或保存配置。 */
function choose(value: string): void {
    emit('update:modelValue', value);
    closeCalendar(true);
}
async function changeMonth(delta: number): Promise<void> {
    const date = dateAt(view.value.getFullYear(), view.value.getMonth() + delta, 1);
    if (date.getFullYear() < 1 || date.getFullYear() > 9999) return;
    const reachesBoundary =
        Math.abs(delta) === 12
            ? date.getFullYear() === 1 || date.getFullYear() === 9999
            : (date.getFullYear() === 1 && date.getMonth() === 0) ||
              (date.getFullYear() === 9999 && date.getMonth() === 11);
    // 到达年界会禁用当前导航按钮；先移交焦点，避免浏览器失焦导致日历误关闭。
    if (reachesBoundary) popup.value?.focus({ preventScroll: true });
    view.value = date;
    activeDate.value = dateValue(date);
    if (reachesBoundary) {
        await nextTick();
        focusActiveDay();
    }
}
/** 手动编辑沿用原生 date 的有效值语义，空串清除条件；不自动打开系统日历。 */
function editDate(event: Event): void {
    const target = event.target as HTMLInputElement;
    if (target.validity.valid) {
        emit('update:modelValue', target.value);
        closeCalendar();
    }
}
/** 输入框可用 Enter/空格/Alt+↓/F4 打开；日历支持方向键、Home/End 和翻月键。 */
async function keydown(event: KeyboardEvent): Promise<void> {
    if (event.key === 'Escape' && open.value) {
        event.preventDefault();
        event.stopPropagation();
        closeCalendar(true);
        return;
    }
    if (event.target === input.value) {
        // Alt+F4/Ctrl+F4 等窗口快捷键继续交给宿主，只接管日期选择器的打开组合。
        if (
            !event.ctrlKey &&
            !event.metaKey &&
            ((!event.altKey && ['Enter', ' ', 'F4'].includes(event.key)) ||
                (event.altKey && event.key === 'ArrowDown'))
        ) {
            event.preventDefault();
            await openCalendar(true);
        }
        return;
    }
    const value = (event.target as HTMLElement).dataset.date;
    const date = value ? parseDate(value) : null;
    if (
        !date ||
        !['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End', 'PageUp', 'PageDown'].includes(
            event.key,
        )
    )
        return;
    event.preventDefault();
    const days: Record<string, number> = {
        ArrowLeft: -1,
        ArrowRight: 1,
        ArrowUp: -7,
        ArrowDown: 7,
        Home: -date.getDay(),
        End: 6 - date.getDay(),
    };
    if (event.key === 'PageUp' || event.key === 'PageDown') {
        const day = date.getDate();
        date.setDate(1);
        date.setMonth(date.getMonth() + (event.key === 'PageUp' ? -1 : 1) * (event.shiftKey ? 12 : 1));
        date.setDate(Math.min(day, dateAt(date.getFullYear(), date.getMonth() + 1, 0).getDate()));
    } else date.setDate(date.getDate() + days[event.key]);
    if (date.getFullYear() < 1 || date.getFullYear() > 9999) return;
    view.value = dateAt(date.getFullYear(), date.getMonth(), 1);
    activeDate.value = dateValue(date);
    await nextTick();
    focusActiveDay();
}
function outside(event: PointerEvent): void {
    if (!root.value?.contains(event.target as Node)) closeCalendar();
}
function blur(): void {
    closeCalendar();
}
function focusout(event: FocusEvent): void {
    if (!root.value?.contains(event.relatedTarget as Node | null)) closeCalendar();
}
/** 外层滚动或缩窗时关闭过期定位；弹层自身滚动不关闭，仍可访问底部动作。 */
function scroll(event: Event): void {
    if (open.value && !popup.value?.contains(event.target as Node)) closeCalendar();
}
watch(
    () => props.disabled,
    (disabled) => {
        if (disabled) closeCalendar();
    },
);
onMounted(() => {
    document.addEventListener('pointerdown', outside);
    document.addEventListener('scroll', scroll, true);
    window.addEventListener('blur', blur);
    window.addEventListener('resize', blur);
});
onUnmounted(() => {
    document.removeEventListener('pointerdown', outside);
    document.removeEventListener('scroll', scroll, true);
    window.removeEventListener('blur', blur);
    window.removeEventListener('resize', blur);
});
</script>

<template>
    <div ref="root" class="date-picker" @keydown="keydown" @focusout="focusout">
        <input
            ref="input"
            class="date-picker-input"
            type="date"
            :aria-label="label"
            :value="modelValue"
            min="0001-01-01"
            max="9999-12-31"
            :disabled="disabled"
            role="combobox"
            aria-haspopup="dialog"
            :aria-expanded="open"
            :aria-controls="open ? `${pickerId}-calendar` : undefined"
            @input="editDate"
            @click.prevent="openCalendar()"
        />
        <CalendarDays :size="14" class="date-picker-icon" aria-hidden="true" />
        <div
            v-if="open"
            :id="`${pickerId}-calendar`"
            ref="popup"
            class="model-menu date-picker-menu"
            :style="popupStyle"
            role="dialog"
            :aria-label="`${label}选择器`"
            tabindex="-1"
        >
            <div class="date-picker-header">
                <button
                    type="button"
                    class="icon-button"
                    aria-label="上一年"
                    :disabled="view.getFullYear() === 1"
                    @click="changeMonth(-12)"
                >
                    <ChevronsLeft :size="15" aria-hidden="true" />
                </button>
                <button
                    type="button"
                    class="icon-button"
                    aria-label="上一月"
                    :disabled="view.getFullYear() === 1 && view.getMonth() === 0"
                    @click="changeMonth(-1)"
                >
                    <ChevronLeft :size="15" aria-hidden="true" />
                </button>
                <strong aria-live="polite">{{ monthTitle }}</strong>
                <button
                    type="button"
                    class="icon-button"
                    aria-label="下一月"
                    :disabled="view.getFullYear() === 9999 && view.getMonth() === 11"
                    @click="changeMonth(1)"
                >
                    <ChevronRight :size="15" aria-hidden="true" />
                </button>
                <button
                    type="button"
                    class="icon-button"
                    aria-label="下一年"
                    :disabled="view.getFullYear() === 9999"
                    @click="changeMonth(12)"
                >
                    <ChevronsRight :size="15" aria-hidden="true" />
                </button>
            </div>
            <table class="date-picker-grid" role="grid" :aria-label="monthTitle">
                <thead>
                    <tr>
                        <th v-for="day in weekdays" :key="day" scope="col">{{ day }}</th>
                    </tr>
                </thead>
                <tbody>
                    <tr v-for="(row, index) in rows" :key="index">
                        <td v-for="day in row" :key="day.value">
                            <button
                                type="button"
                                class="date-picker-day"
                                :class="{
                                    outside: day.outside,
                                    selected: day.value === modelValue,
                                    today: day.value === today,
                                }"
                                :disabled="day.disabled"
                                :aria-label="day.label"
                                :aria-pressed="day.value === modelValue"
                                :tabindex="day.value === activeDate ? 0 : -1"
                                :data-date="day.value"
                                @click="choose(day.value)"
                            >
                                {{ day.day }}
                            </button>
                        </td>
                    </tr>
                </tbody>
            </table>
            <div class="date-picker-footer">
                <button type="button" class="button secondary small" @click="choose('')">清空</button>
                <button type="button" class="button secondary small" @click="choose(dateValue(new Date()))">
                    今天
                </button>
            </div>
        </div>
    </div>
</template>
