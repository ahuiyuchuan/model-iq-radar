<!--
    关于信息参考 CC Switch 的品牌/版本、便携模式、更新、项目链接和近期变更元素。
    只展示本项目真实信息，不复制第三方品牌、官网或自动安装能力。@author ahui
-->
<script setup lang="ts">
import { Target, Github, ExternalLink, MessageSquare, ShieldCheck, Sparkles } from 'lucide-vue-next';
import { version } from '../../package.json';
import type { UpdateResult } from '../types';

defineProps<{
    result: UpdateResult | null;
    checking: boolean;
    error: string;
    checkedAt: string;
}>();
defineEmits<{ open: [url: string] }>();
const repository = 'https://github.com/ahuiyuchuan/model-iq-radar';
</script>

<template>
    <div class="about-page">
        <section class="about-card about-identity" aria-label="软件信息">
            <span class="about-logo"><Target :size="36" aria-hidden="true" /></span>
            <div>
                <h2>测智雷达</h2>
                <p>用你的问题，观察模型的每一次回答。</p>
                <div class="about-badges">
                    <span>v{{ version }}</span
                    ><span>免安装版 · Windows x64</span>
                </div>
            </div>
        </section>
        <section class="about-card about-update" aria-label="版本更新">
            <h3>版本更新</h3>
            <p v-if="checking" role="status">正在检查更新…</p>
            <p v-else-if="error" class="text-failed" role="alert">{{ error }}</p>
            <p v-else-if="result?.status === 'available'" class="text-passed" role="status">
                发现新版本 v{{ result.latestVersion }}
            </p>
            <p v-else-if="checkedAt && result?.status === 'current'" role="status">当前已是最新正式版本。</p>
            <p v-else-if="checkedAt && result?.status === 'no-release'" role="status">
                项目尚无正式发布版本。
            </p>
            <p v-else>点击下方“检查更新”，查看最新版本。</p>
            <p v-if="result?.status === 'available' && result.notes" class="about-release-notes">
                {{ result.notes }}
            </p>
            <p v-if="checkedAt" class="muted small-text">上次检查：{{ checkedAt }}</p>
        </section>
        <section class="about-card" aria-label="项目链接">
            <h3>项目与反馈</h3>
            <div class="about-links">
                <button type="button" class="button secondary small" @click="$emit('open', repository)">
                    <Github :size="14" />GitHub 项目
                </button>
                <button
                    type="button"
                    class="button secondary small"
                    @click="$emit('open', `${repository}/releases`)"
                >
                    <ExternalLink :size="14" />发布记录
                </button>
                <button
                    type="button"
                    class="button secondary small"
                    @click="$emit('open', `${repository}/issues`)"
                >
                    <MessageSquare :size="14" />问题反馈
                </button>
            </div>
            <p class="muted small-text">作者 ahui · 喜欢这个工具，可以在 GitHub 上点亮 Star。</p>
        </section>
        <section class="about-card" aria-label="本版更新">
            <h3><Sparkles :size="15" />本版改进</h3>
            <ul>
                <li>回答支持 Markdown、表格、任务列表及本地代码高亮。</li>
                <li>悬浮回答与历史详情统一显示，复制保留原始 Markdown。</li>
                <li>接口支持拉取模型、测试连接，历史支持分组与关键词组合筛选。</li>
                <li>支持 Chat Completions/Responses、思考强度、模型级超时和规则预设绑定。</li>
                <li>悬浮窗与托盘提供自动测试快捷开关，历史可按状态筛选并导出原始记录。</li>
            </ul>
        </section>
        <p class="about-privacy">
            <ShieldCheck :size="15" /><span
                >配置与历史保存在本机，密钥由 Windows 凭据管理器保存。检查更新仅请求 GitHub
                发布信息，不发送题目、回答或密钥。</span
            >
        </p>
    </div>
</template>
