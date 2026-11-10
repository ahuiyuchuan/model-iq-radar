<!--
    题目图片按需查看：草稿使用正式排版器，历史只读取当次保存的PNG，不重建旧题冒充原图。
    只接受内嵌PNG，不加载外链；单次只展示一页，关闭/换题/卸载释放图片引用。
    @author ahui
-->
<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { Image, LoaderCircle } from 'lucide-vue-next';
import { call } from '../bridge';

const props = defineProps<{
    /** 规则页当前未保存的题目；不包含判定答案，不触发配置写入或模型请求。 */
    prompt?: string;
    /** 历史稳定ID；存在时仅读取该记录原图，缺图明确说明，不使用prompt重新生成。 */
    recordId?: string;
    disabled?: boolean;
}>();
const pages = ref<string[]>([]);
const page = ref(0);
const pending = ref(false);
const expanded = ref(false);
const error = ref('');
const fit = ref(true);
const root = ref<HTMLElement>();
let generation = 0;
let disposed = false;
const label = computed(() => (props.recordId ? '查看当次请求图片' : '预览题目图片'));

/** 关闭仅释放本地显示引用；已在执行的系统排版不能强制中断，迟到结果按世代丢弃。 */
function close(): void {
    generation++;
    pages.value = [];
    expanded.value = false;
    page.value = 0;
    error.value = '';
    fit.value = true;
}

/**
 * 重复点击只读取/生成一次；错误可重试，旧记录无图不伪造，失败不改变题目开关。
 * @returns 读取或排版结束后完成；异常转换为本组件提示，不影响模型测试结果。
 */
async function open(): Promise<void> {
    if (pending.value || props.disabled) return;
    if (expanded.value && pages.value.length) {
        root.value?.scrollIntoView?.({ block: 'nearest' });
        return;
    }
    const turn = ++generation;
    pending.value = true;
    expanded.value = true;
    error.value = '';
    try {
        const result = props.recordId
            ? await call<string[]>('get_prompt_images', { recordId: props.recordId })
            : await call<string[]>('preview_prompt_images', { prompt: props.prompt ?? '' });
        if (disposed || turn !== generation) return;
        // 历史内容也是不可信输入；只接收PNG data URL，禁止借图片预览访问任意网络地址。
        if (
            !Array.isArray(result) ||
            result.some((url) => !/^data:image\/png;base64,[A-Za-z0-9+/]+={0,2}$/u.test(url))
        )
            throw new Error('图片数据无效，请重新读取或预览。');
        // 每次读取都是一份新集合；重试可能比原集合更短，不能沿用失败前的页码。
        page.value = 0;
        pages.value = result;
    } catch (cause) {
        if (!disposed && turn === generation) error.value = String(cause);
    } finally {
        if (!disposed) pending.value = false;
    }
}

/** 图片解码失败也要可见、可重试；忽略换页或换题后旧节点的迟到错误。 */
function imageFailed(event: Event): void {
    if ((event.currentTarget as HTMLImageElement).getAttribute('src') !== pages.value[page.value]) return;
    pages.value = [];
    error.value = '图片无法显示，请重新读取或预览。';
}

watch(() => [props.prompt, props.recordId], close);
onBeforeUnmount(() => {
    disposed = true;
    close();
});
defineExpose({ open });
</script>

<template>
    <section ref="root" class="prompt-image-preview" aria-label="题目图片预览">
        <div class="prompt-image-toolbar">
            <button
                type="button"
                class="button secondary small"
                :disabled="disabled || pending"
                :aria-busy="pending"
                @click="open"
            >
                <LoaderCircle v-if="pending" :size="15" class="spin" aria-hidden="true" />
                <Image v-else :size="15" aria-hidden="true" />{{ pending ? '正在读取图片…' : label }}
            </button>
            <span class="muted small-text">{{
                recordId ? '当次请求原图，不代表服务端已收到' : '预览当前草稿，无需保存，不调用模型'
            }}</span>
            <button v-if="expanded" type="button" class="button subtle small" @click="close">收起图片</button>
        </div>
        <template v-if="expanded">
            <p v-if="error" class="inline-error" role="alert">{{ error }}</p>
            <p v-else-if="!pending && !pages.length" class="muted" role="status">
                这条记录未保存请求原图，可能来自旧版或在生成图片前失败。不会重新生成图片冒充当次原图。
            </p>
            <template v-if="pages.length">
                <div class="prompt-image-toolbar">
                    <!-- 单张图片没有翻页操作；仅长题生成多张图片时显示页码与导航。 -->
                    <template v-if="pages.length > 1">
                        <button
                            type="button"
                            class="button subtle small"
                            :disabled="page === 0"
                            @click="page--"
                        >
                            上一页
                        </button>
                        <span role="status">第 {{ page + 1 }} / {{ pages.length }} 页</span>
                        <button
                            type="button"
                            class="button subtle small"
                            :disabled="page + 1 === pages.length"
                            @click="page++"
                        >
                            下一页
                        </button>
                    </template>
                    <button
                        type="button"
                        class="button subtle small"
                        :aria-pressed="!fit"
                        @click="fit = !fit"
                    >
                        {{ fit ? '原始尺寸' : '适应宽度' }}
                    </button>
                </div>
                <div class="prompt-image-viewport" tabindex="0" aria-label="题目图片，可滚动查看">
                    <img
                        :key="page"
                        @error="imageFailed"
                        :src="pages[page]"
                        :class="{ 'fit-width': fit }"
                        :alt="`题目图片，第${page + 1}页，共${pages.length}页`"
                    />
                </div>
            </template>
        </template>
    </section>
</template>

<style scoped>
.prompt-image-preview {
    min-width: 0;
    margin: 10px 0;
}
.prompt-image-toolbar {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 8px;
}
.prompt-image-viewport {
    max-height: 420px;
    max-width: 100%;
    overflow: auto;
    background: #fff;
    border: 1px solid var(--line);
    border-radius: 6px;
}
.prompt-image-viewport img {
    display: block;
    max-width: none;
}
.prompt-image-viewport img.fit-width {
    width: 100%;
    height: auto;
}
</style>
