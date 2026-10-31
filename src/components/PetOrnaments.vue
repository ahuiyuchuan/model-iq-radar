<!--
    已确认海洋卡片的内缘装饰：海浪、金铃和贝壳随真实卡片高度排列。
    仅绘制宿主可信矢量，不接受皮肤脚本/外链；完全不接收输入，不扩大窗口或命中区域。
    @author ahui
-->
<script setup lang="ts">
defineProps<{ preset: 'ocean' | 'stars'; animate: boolean }>();
</script>
<template>
    <div class="pet-ornaments" :class="[preset, { animated: animate }]" aria-hidden="true">
        <template v-if="preset === 'ocean'">
            <svg class="pet-wave-ribbon" viewBox="0 0 460 26" preserveAspectRatio="none" fill="none">
                <path
                    d="M0 15c8-1 8-13 13-11 5 3-2 10 4 12 7 3 8-10 15-9 8 1 2 12 13 13 10 0 9-6 17-5 9 0 10 7 22 6 12-2 18-5 31-1 15 4 23-1 35 2H0Z"
                    fill="#4fb4ef"
                    fill-opacity=".78"
                />
                <path
                    d="M0 15c5 1 9-1 12-6-1 8 5 12 11 9 6-2 5-8 10-8-4 8 3 13 11 11 10-3 13-3 21 0 10 4 18-1 27 1"
                    stroke="#b9eeff"
                    stroke-width="1.3"
                />
                <path
                    d="M329 23c11-5 18 1 27-3 7-3 8-9 13-7 4 2-4 5-1 8 4 4 8-5 15-3 6 2 9 6 17 3 9-4 14-3 20-1 7 3 11 1 16-3 5-4 12-3 24-2v11H329Z"
                    fill="#55b9f1"
                    fill-opacity=".66"
                />
                <path
                    d="M352 22c8 1 9-8 15-8m11 8c5-6 9-4 13-1m15 1c10-5 14 1 23-3m8-1c8-5 12-2 21-1"
                    stroke="#bceeff"
                    stroke-opacity=".8"
                    stroke-width="1"
                />
                <path
                    d="m24 4 2 2m18 6 2-1m15 5 2 1m327-5 2-3m14 4 2 1m32-6 2-3"
                    stroke="#6acaff"
                    stroke-width="1.2"
                    stroke-linecap="round"
                />
            </svg>
            <svg class="pet-gold-bell" viewBox="0 0 24 32" fill="none">
                <path d="M12 1v7m-3-4c0-4 6-4 6 0v3" stroke="#a9782e" stroke-width="1.3" />
                <path
                    d="M5 19c0-5 1-11 7-11s7 6 7 11l2 3H3Z"
                    fill="#e1b155"
                    stroke="#8e632d"
                    stroke-width="1.2"
                />
                <path
                    d="M5 17q7-5 14 0M5 20h14M10 11h3"
                    stroke="#fff0b4"
                    stroke-width="1.1"
                    stroke-linecap="round"
                />
                <path d="M10 24a2 2 0 0 0 4 0" stroke="#9b6c2e" stroke-width="1.8" />
                <path d="M12 26v3" stroke="#cf9f4d" stroke-width="1.2" />
            </svg>
            <svg class="pet-pearl-shell" viewBox="0 0 44 40" fill="none">
                <path
                    d="M22 31C13 29 2 20 5 13 6 9 10 10 12 7c3-5 7-3 10-2 4-3 8-2 10 3 6-1 9 5 7 10-1 7-10 12-17 13Z"
                    fill="#fff4ee"
                    stroke="#a0bad8"
                    stroke-width="1.2"
                />
                <path
                    d="M22 30 12 9m10 21L20 6m2 24L29 8m-7 22 13-15M8 15c2 7 8 12 14 15"
                    stroke="#edc8c0"
                    stroke-width="1.4"
                    stroke-linecap="round"
                />
                <path d="m15 30 7-2 7 3-7 5Z" fill="#e8bd6d" stroke="#ad8548" stroke-width=".9" />
                <circle cx="22" cy="31" r="3" fill="#fff5d7" stroke="#d2ad63" />
                <path
                    d="m35 7 2-4m4 10 2-1M3 25l-2 2"
                    stroke="#6fc5f7"
                    stroke-width="1.6"
                    stroke-linecap="round"
                />
            </svg>
        </template>
        <template v-else>
            <svg class="pet-star-corner top-right" viewBox="0 0 40 40" fill="none">
                <path d="m18 8 3 8 8 3-8 3-3 8-3-8-8-3 8-3ZM31 6v6m-3-3h6" />
            </svg>
            <svg class="pet-star-corner bottom-left" viewBox="0 0 40 40" fill="none">
                <path d="m18 8 3 8 8 3-8 3-3 8-3-8-8-3 8-3ZM31 6v6m-3-3h6" />
            </svg>
        </template>
    </div>
</template>
<style scoped>
/* 主体和浮层分开：inset:0只跟随.pet-panel实际内容高，不依赖菜单或窗口外框。 */
.pet-ornaments {
    position: absolute;
    inset: 0;
    z-index: 2;
    overflow: hidden;
    border-radius: 14px;
    user-select: none;
}
.pet-ornaments,
.pet-ornaments * {
    pointer-events: none;
}
.pet-ornaments svg {
    position: absolute;
}
.pet-wave-ribbon {
    left: 2px;
    bottom: 1px;
    width: calc(100% - 4px);
    height: 19px;
}
.pet-gold-bell {
    width: 17px;
    height: 23px;
    left: 1px;
    bottom: 5px;
}
.pet-pearl-shell {
    width: 27px;
    height: 25px;
    right: 0;
    bottom: 0;
}
.pet-star-corner {
    width: 27px;
    height: 27px;
    stroke: var(--pet-border);
    stroke-width: 1.2;
    opacity: 0.75;
}
.top-right {
    right: 3px;
    top: 3px;
}
.bottom-left {
    left: 3px;
    bottom: 3px;
}
</style>
