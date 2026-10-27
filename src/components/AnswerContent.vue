<!-- 文本与 HTML 结果的统一展示入口；复制功能仍由调用方读取原始 answer。 @author ahui -->
<script setup lang="ts">
import { computed } from 'vue';
import MarkdownAnswer from './MarkdownAnswer.vue';
import HtmlPreview from './HtmlPreview.vue';
import { classifyAnswer } from '../htmlAnswer';
import type { AnswerType } from '../types';

const props = withDefaults(
    defineProps<{
        source: string;
        answerType?: AnswerType | null;
        executable?: boolean;
    }>(),
    { executable: true },
);
const classification = computed(() => classifyAnswer(props.source));
const isHtml = computed(() =>
    props.answerType ? props.answerType === 'html' : classification.value.kind === 'html',
);
const candidates = computed(() => {
    if (classification.value.candidates.length) return classification.value.candidates;
    // answerType 可能来自历史迁移或未来识别版本；即使当前提取器没有候选，
    // 也必须保留原始回答供查看/复制，不能用空列表把源码悄悄隐藏。
    return isHtml.value ? [{ source: props.source, label: '完整回答', issue: '没有可运行的 HTML' }] : [];
});
const canExecute = computed(() => props.executable !== false && classification.value.candidates.length > 0);
defineEmits<{ failure: [message: string]; expandedChange: [expanded: boolean] }>();
</script>

<template>
    <HtmlPreview
        v-if="isHtml"
        :candidates="candidates"
        :executable="canExecute"
        @failure="$emit('failure', $event)"
        @expanded-change="$emit('expandedChange', $event)"
    />
    <MarkdownAnswer v-else :source="source" />
</template>
