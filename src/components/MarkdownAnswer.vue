<!--
    悬浮回答与历史详情共用的只读显示层。链接只在明确点击后交给系统浏览器；
    不可信正文的渲染失败局限于本组件，不能影响测试状态、原文保存和复制。@author ahui
-->
<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { call } from '../bridge';
import { externalUrl, renderMarkdown } from '../markdown';

const props = defineProps<{ source: string }>();
const html = computed(() => renderMarkdown(props.source));
const linkError = ref('');
const opening = ref(false);
watch(
    () => props.source,
    () => {
        linkError.value = '';
    },
);

/**
 * 拦截所有锚点导航（含中键），不允许 WebView 导航、弹窗或预取回答中的地址。
 * @param event 用户的鼠标或键盘激活产生的点击事件；右键菜单仍由系统提供。
 */
async function activateLink(event: MouseEvent): Promise<void> {
    const element = event.target instanceof Element ? event.target : null;
    const anchor = element?.closest('a');
    if (!anchor) return;
    event.preventDefault();
    event.stopPropagation();
    const url = externalUrl(anchor.getAttribute('href') ?? '');
    if (!url || opening.value || event.button > 1) return;
    opening.value = true;
    linkError.value = '';
    try {
        await call('open_external_url', { url });
    } catch {
        linkError.value = '无法打开系统浏览器，请稍后重试。';
    } finally {
        opening.value = false;
    }
}
</script>

<template>
    <div class="markdown-answer" @click="activateLink" @auxclick="activateLink">
        <!-- html 只来自禁用 HTML 的解析器及白名单净化器，任何失败直接显示原文。 -->
        <div v-if="html !== null" class="markdown-body" v-html="html"></div>
        <p v-else class="markdown-fallback">{{ source }}</p>
        <p v-if="linkError" class="markdown-link-error" role="alert">{{ linkError }}</p>
    </div>
</template>
