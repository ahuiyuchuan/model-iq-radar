<!-- @author ahui：两个桌面窗口共用入口，预览提示仅出现在普通浏览器中。 -->
<script setup lang="ts">
import { desktop } from './bridge';
import Widget from './components/Widget.vue';
import Settings from './components/Settings.vue';

const settingsView = new URLSearchParams(location.search).get('view') === 'settings';
if (!desktop) document.body.classList.add('browser-preview');
// 原生浮窗缩放/平移动画只允许内部回答滚动，禁止瞬时旧内容尺寸撑出 WebView 滚动条。
if (desktop && !settingsView) document.documentElement.classList.add('widget-viewport');
</script>

<template>
    <Settings v-if="settingsView" />
    <Widget v-else />
    <div v-if="!desktop" class="preview-note">
        <span class="preview-dot"></span> 界面预览 · 实际测试请使用桌面应用
    </div>
</template>
