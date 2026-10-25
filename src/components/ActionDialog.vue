<!--
    设置中的明确选择对话框：保留键盘焦点、限制背景交互，避免原生 confirm 只有放弃按钮。
    @author ahui
-->
<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref } from 'vue';

defineProps<{
    title: string;
    message: string;
    actions: { id: string; label: string; danger?: boolean; primary?: boolean }[];
}>();
const emit = defineEmits<{ choose: [id: string] }>();
const root = ref<HTMLElement>();
const previousFocus = document.activeElement as HTMLElement | null;

/** Tab 留在可见对话框内；Escape 等价取消，不能隐式放弃未保存内容。 */
function keydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
        event.preventDefault();
        emit('choose', 'cancel');
    }
    if (event.key !== 'Tab') return;
    const buttons = [...(root.value?.querySelectorAll<HTMLElement>('button, input, [tabindex="0"]') ?? [])];
    const first = buttons[0];
    const last = buttons.at(-1);
    if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last?.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first?.focus();
    }
}
onMounted(async () => {
    await nextTick();
    root.value?.querySelector<HTMLButtonElement>('button')?.focus();
});
onUnmounted(() => previousFocus?.focus());
</script>

<template>
    <div class="dialog-backdrop" @mousedown.self="emit('choose', 'cancel')">
        <section
            ref="root"
            class="action-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="settings-dialog-title"
            aria-describedby="settings-dialog-description"
            @keydown="keydown"
        >
            <h2 id="settings-dialog-title">{{ title }}</h2>
            <div class="dialog-body">
                <p id="settings-dialog-description">{{ message }}</p>
                <slot />
            </div>
            <div class="dialog-actions">
                <button
                    v-for="action in actions"
                    :key="action.id"
                    type="button"
                    class="button"
                    :class="{ danger: action.danger, secondary: !action.primary }"
                    @click="emit('choose', action.id)"
                >
                    {{ action.label }}
                </button>
            </div>
        </section>
    </div>
</template>
