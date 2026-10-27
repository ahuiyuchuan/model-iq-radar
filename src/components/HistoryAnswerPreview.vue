<!-- 历史右侧复用卡片/详情的回答入口；可见行懒加载，放大期间保留会话，不生成截图。@author ahui -->
<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { isHtmlRecord, previewAllowed } from '../htmlAnswer';
import type { TestRecord } from '../types';
import AnswerContent from './AnswerContent.vue';

const props = defineProps<{ record: TestRecord }>();
const root = ref<HTMLElement>();
const visible = ref(false);
const expanded = ref(false);
// 页内弹层可能让原列表行离开视口；关闭之前不能卸载 iframe，否则会中断放大操作。
const active = computed(() => visible.value || expanded.value);
const html = ref(false);
const summary = ref('点击查看回答');
defineEmits<{ details: [] }>();
let observer: IntersectionObserver | undefined;

/** 只解析可见或正在放大的回答；普通屏外行释放 iframe，不保留渲染或截图缓存。 */
function load(): void {
    if (!active.value) return;
    const record = props.record;
    html.value = isHtmlRecord(record);
    if (!previewAllowed(record)) {
        summary.value = record.status === 'cancelled' ? '已取消' : record.error || '测试失败';
        return;
    }
    summary.value = record.answer.slice(0, 180).replace(/\s+/gu, ' ').trim() || '无正文';
}
watch([() => props.record.id, active], load);
onMounted(() => {
    // 仅实际可见项目才运行；不支持观察器时保留占位，不隐式加载整页 HTML。
    if (typeof IntersectionObserver !== 'undefined' && root.value) {
        observer = new IntersectionObserver((entries) => {
            visible.value = entries.some((item) => item.isIntersecting);
        });
        observer.observe(root.value);
    }
});
onBeforeUnmount(() => observer?.disconnect());
</script>
<template>
    <div ref="root" class="history-answer-preview" :class="{ 'is-html': html }">
        <!-- 与另外两处共用候选切换、失败重载和源码边界，不因列表缩略尺寸另设交互分支。 -->
        <AnswerContent
            v-if="active && html"
            :source="record.answer"
            :answer-type="record.answerType"
            :executable="previewAllowed(record)"
            @expanded-change="expanded = $event"
        />
        <button v-else type="button" class="history-answer-summary" @click="$emit('details')">
            {{ summary }}
        </button>
    </div>
</template>
