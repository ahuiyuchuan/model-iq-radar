<!-- 二维图集播放器：就绪交接、预乘Alpha短过渡及连续帧采样，不变形角色原画。@author ahui -->
<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import type { LoadedSkin, PetAction, Rect, SkinClip } from '../pet/types';
const props = defineProps<{
    skin: LoadedSkin;
    action: PetAction;
    animate: boolean;
    size: number;
    /** 兼容旧宿主输入；未分层的整幅角色不根据目光或拖动速度进行几何变形。 */
    look?: [number, number];
    dragVelocity?: [number, number];
}>();
const emit = defineEmits<{
    ended: [];
    ready: [action: PetAction];
    error: [message: string];
    bounds: [regions: Rect[]];
}>();
const canvas = ref<HTMLCanvasElement>();
const renderedAction = ref<PetAction>();
const transitioning = ref(false);
/** 96ms有界呈现交接，双方权重之和恒为1；不冒充补制的连续肢体动作。 */
const TRANSITION_MS = 96;
interface Playback {
    action: PetAction;
    image: HTMLImageElement;
    clip: SkinClip;
    elapsed: number;
    ended: boolean;
    nextBlink: number;
    blinkingAt: number;
}
let playback: Playback | undefined;
let previous: HTMLCanvasElement | undefined;
let surface: HTMLCanvasElement | undefined;
let frame = 0,
    generation = 0,
    disposed = false,
    last = 0,
    transitionTime = 0;

/** 合并已出画双方的轮廓，只留抗锯齿余量；静态角色不需要旧悬摆的大范围命中膨胀。 */
function publishBounds(clips: SkinClip[]): void {
    const size = clips[clips.length - 1]!.frameSize;
    const regions: Rect[] = [];
    const step = Math.max(4, Math.ceil(size / 80));
    for (let y = 0; y < size; y += step) {
        let left = size,
            right = 0;
        for (const clip of clips)
            for (const [x, top, width, height] of clip.hitBands) {
                const scale = size / clip.frameSize;
                if (top * scale - 2 <= y + step && (top + height) * scale + 2 >= y) {
                    left = Math.min(left, x * scale - 2);
                    right = Math.max(right, (x + width) * scale + 2);
                }
            }
        if (right > left)
            regions.push([
                Math.max(0, left),
                y,
                Math.min(size, right) - Math.max(0, left),
                Math.min(step, size - y),
            ]);
    }
    emit('bounds', regions);
}

/** 仅像素尺寸真正变化时分配画布；切动作不会清空 canvas 或重置设备像素比。 */
function resize(): void {
    const target = canvas.value;
    if (!target) return;
    const pixels = Math.round(props.size * Math.min(devicePixelRatio || 1, 2));
    if (target.width === pixels && target.height === pixels) return;
    const saved = document.createElement('canvas');
    saved.width = target.width;
    saved.height = target.height;
    saved.getContext('2d')?.drawImage(target, 0, 0);
    target.width = target.height = pixels;
    target.getContext('2d')?.drawImage(saved, 0, 0, pixels, pixels);
    if (playback) paint(0);
}

/** 微动帧以预乘 Alpha 加权；重合实色部位不会在插值中变半透明。 */
function drawPose(context: CanvasRenderingContext2D, state: Playback): void {
    const { clip, elapsed } = state;
    let position = (elapsed * clip.fps) / 1000;
    let count = clip.blink?.start ?? clip.frames,
        start = 0;
    if (clip.blink && state.blinkingAt >= 0) {
        position = ((elapsed - state.blinkingAt) * clip.blink.fps) / 1000;
        start = clip.blink.start;
        count = clip.blink.frames;
    }
    const raw = Math.floor(position);
    const index = clip.loop && start === 0 ? raw % count : Math.min(raw, count - 1);
    const next = start || !clip.loop ? Math.min(index + 1, count - 1) : (index + 1) % count;
    const fraction = props.animate ? position - raw : 0;
    const pixels = canvas.value!.width;
    const drawFrame = (index: number, weight: number) => {
        if (weight <= 0) return;
        const at = start + index;
        context.globalAlpha = weight;
        context.drawImage(
            state.image,
            (at % clip.columns) * clip.frameSize,
            Math.floor(at / clip.columns) * clip.frameSize,
            clip.frameSize,
            clip.frameSize,
            0,
            0,
            pixels,
            pixels,
        );
    };
    drawFrame(index, 1 - fraction);
    context.globalCompositeOperation = 'lighter';
    drawFrame(next, fraction);
}

/** 只采样已制作的图集像素；不以整图旋转或缩放代替角色动作。 */
function paint(dt: number): void {
    const target = canvas.value;
    if (!target || !playback) return;
    const context = target.getContext('2d');
    if (!context) throw new Error('无法创建角色画布。');
    transitionTime += dt * 1000;
    const progress = previous ? Math.min(1, transitionTime / TRANSITION_MS) : 1;
    const weight = progress * progress * (3 - 2 * progress);
    context.clearRect(0, 0, target.width, target.height);
    context.save();
    if (previous) {
        context.globalAlpha = 1 - weight;
        context.drawImage(previous, 0, 0, target.width, target.height);
    }
    surface ??= document.createElement('canvas');
    if (surface.width !== target.width || surface.height !== target.height)
        surface.width = surface.height = target.width;
    const ink = surface.getContext('2d');
    if (!ink) throw new Error('无法创建角色缓冲画布。');
    ink.clearRect(0, 0, surface.width, surface.height);
    ink.save();
    drawPose(ink, playback);
    ink.restore();
    // source-over会让重合实色部位中途透明；预乘Alpha相加保证同一轮廓不闪空。
    // 不同轮廓仍存在短暂溶解，这只是静态素材交接，不代表真实姿态补帧。
    context.globalCompositeOperation = previous ? 'lighter' : 'source-over';
    context.globalAlpha = previous ? weight : 1;
    context.drawImage(surface, 0, 0);
    context.restore();
    if (previous && progress === 1) {
        previous = undefined;
        transitioning.value = false;
        publishBounds([playback.clip]);
    }
}

/** 静态姿态无需持续重绘；隐藏/暂停时保留画面，非循环片段仅保留结算所需的时钟。 */
function schedule(): void {
    cancelAnimationFrame(frame);
    last = performance.now();
    if (
        !disposed &&
        props.animate &&
        !document.hidden &&
        playback &&
        (previous ||
            (playback.clip.loop && playback.clip.frames > 1) ||
            (!playback.clip.loop && !playback.ended))
    )
        frame = requestAnimationFrame(tick);
}
function tick(now: number): void {
    if (disposed || !playback || document.hidden || !props.animate) return;
    if (now - last < 1000 / 45) {
        frame = requestAnimationFrame(tick);
        return;
    }
    const dt = Math.min(0.05, Math.max(0, (now - last) / 1000));
    last = now;
    const state = playback;
    state.elapsed += dt * 1000;
    const blink = state.clip.blink;
    if (blink) {
        if (state.blinkingAt < 0 && state.elapsed >= state.nextBlink) state.blinkingAt = state.elapsed;
        if (state.blinkingAt >= 0 && state.elapsed - state.blinkingAt >= (blink.frames / blink.fps) * 1000) {
            state.blinkingAt = -1;
            state.nextBlink =
                state.elapsed + blink.minDelayMs + Math.random() * (blink.maxDelayMs - blink.minDelayMs);
        }
    }
    paint(dt);
    if (!state.clip.loop && state.elapsed >= (state.clip.frames / state.clip.fps) * 1000 && !state.ended) {
        state.ended = true;
        emit('ended');
    }
    if (previous || (state.clip.loop && state.clip.frames > 1) || (!state.clip.loop && !state.ended))
        frame = requestAnimationFrame(tick);
}

/** 图集准备好才交接；加载期间旧动画继续，快速打断从屏幕实际合成帧出发，迟到结果丢弃。 */
async function play(): Promise<void> {
    const current = ++generation;
    const action = props.action;
    const manifest = props.skin.manifest;
    try {
        const image = await props.skin.image(action);
        if (disposed || current !== generation) return;
        await nextTick();
        if (!canvas.value || disposed || current !== generation) return;
        resize();
        const old = playback;
        const clip = manifest.actions[action] ?? manifest.actions.idle;
        // 所有动作统一交接；下一资源就绪后才退旧帧，静态姿态也不再硬切。
        if (old && props.animate) {
            previous = document.createElement('canvas');
            previous.width = canvas.value.width;
            previous.height = canvas.value.height;
            previous.getContext('2d')!.drawImage(canvas.value, 0, 0);
        } else previous = undefined;
        const blink = clip.blink;
        playback = {
            action,
            image,
            clip,
            elapsed: 0,
            ended: false,
            blinkingAt: -1,
            nextBlink: blink
                ? blink.minDelayMs + Math.random() * (blink.maxDelayMs - blink.minDelayMs)
                : Infinity,
        };
        transitionTime = 0;
        transitioning.value = Boolean(previous);
        renderedAction.value = action;
        publishBounds(old && previous ? [old.clip, clip] : [clip]);
        paint(0);
        emit('ready', action);
        schedule();
    } catch (cause) {
        if (!disposed && current === generation) emit('error', String(cause));
    }
}
onMounted(() => {
    void play();
    document.addEventListener('visibilitychange', schedule);
    window.addEventListener('resize', resize);
});
watch(
    () => [props.skin, props.action],
    () => void play(),
);
watch(() => props.size, resize);
watch(
    () => props.animate,
    () => {
        // 暂停保留当前实际合成帧；用户在过渡中暂停也不会跳回动作首帧。
        transitioning.value = props.animate && Boolean(previous);
        schedule();
    },
);
onUnmounted(() => {
    disposed = true;
    generation++;
    cancelAnimationFrame(frame);
    playback = undefined;
    previous = surface = undefined;
    document.removeEventListener('visibilitychange', schedule);
    window.removeEventListener('resize', resize);
});
</script>
<template>
    <canvas
        ref="canvas"
        :data-action="renderedAction"
        :data-transitioning="transitioning"
        data-renderer="atlas"
        class="pet-actor"
        :style="{ width: `${size}px`, height: `${size}px` }"
        aria-hidden="true"
    />
</template>
