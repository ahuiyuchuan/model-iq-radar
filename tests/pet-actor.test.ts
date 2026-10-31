/**
 * 原画播放器生命周期回归：真实挂载组件，用可控时钟和画布记录验证可见输出交接。
 * 不依赖图片服务；像素效果另由真实浏览器截图与原生窗口验收。
 * @author ahui
 */
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import PetActor from '../src/components/PetActor.vue';
import approved from '../public/skins/blue-whale-maid/skin.json';
import type { LoadedSkin, PetAction, SkinManifest } from '../src/pet/types';

const contexts = new Map<HTMLCanvasElement, ReturnType<typeof makeContext>>();
let now = 0;
let nextFrame = 0;
const frames = new Map<number, FrameRequestCallback>();

/** 每个画布有独立调用记录，能区分可见画布、帧缓冲和过渡快照。 */
function makeContext() {
    const context = {
        weights: [] as number[],
        clearRect: vi.fn(),
        drawImage: vi.fn(),
        save: vi.fn(),
        restore: vi.fn(),
        translate: vi.fn(),
        rotate: vi.fn(),
        globalAlpha: 1,
        globalCompositeOperation: 'source-over',
    };
    context.drawImage.mockImplementation(() => context.weights.push(context.globalAlpha));
    return context;
}

/** 每次只推进一个显示刷新；应用自己负责限帧与长间隔截断。 */
async function step(milliseconds = 50): Promise<void> {
    now += milliseconds;
    const callbacks = [...frames.values()];
    frames.clear();
    callbacks.forEach((callback) => callback(now));
    await flushPromises();
}

function fixture() {
    const manifest = structuredClone(approved) as SkinManifest;
    // 播放生命周期用可控的连续图集，避免随内置素材改为静态而失去异步交接覆盖。
    for (const clip of Object.values(manifest.actions)) {
        clip.frames = 4;
        clip.columns = 2;
        clip.loop = true;
        delete clip.blink;
    }
    const images = Object.fromEntries(
        Object.keys(manifest.actions).map((action) => [action, new Image()]),
    ) as Record<PetAction, HTMLImageElement>;
    const image = vi.fn(async (action: PetAction) => images[action]);
    const skin: LoadedSkin = { manifest, image, poster: '', dispose: vi.fn() };
    return { manifest, images, image, skin };
}

beforeEach(() => {
    now = 0;
    nextFrame = 0;
    frames.clear();
    contexts.clear();
    vi.spyOn(performance, 'now').mockImplementation(() => now);
    vi.spyOn(document, 'hidden', 'get').mockReturnValue(false);
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
        frames.set(++nextFrame, callback);
        return nextFrame;
    });
    vi.stubGlobal('cancelAnimationFrame', (id: number) => frames.delete(id));
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(function (
        this: HTMLCanvasElement,
    ) {
        if (!contexts.has(this)) contexts.set(this, makeContext());
        return contexts.get(this) as unknown as CanvasRenderingContext2D;
    });
});
afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
});

it('后续动作解码期间继续旧动画；就绪后保持同一画布且不重设像素尺寸', async () => {
    const { skin, images, image } = fixture();
    const widthSetter = vi.spyOn(HTMLCanvasElement.prototype, 'width', 'set');
    const wrapper = mount(PetActor, { props: { skin, action: 'idle', animate: true, size: 160 } });
    await flushPromises();
    const canvas = wrapper.get('canvas').element;
    let resolve!: (image: HTMLImageElement) => void;
    image.mockImplementationOnce(() => new Promise((complete) => (resolve = complete)));
    const assignments = widthSetter.mock.calls.length;
    await wrapper.setProps({ action: 'working' });
    const clears = contexts.get(canvas)!.clearRect.mock.calls.length;
    await step();
    expect(contexts.get(canvas)!.clearRect.mock.calls.length).toBeGreaterThan(clears);
    expect(wrapper.attributes('data-action')).toBe('idle');
    resolve(images.working);
    await flushPromises();
    expect(wrapper.get('canvas').element).toBe(canvas);
    // 交接会新建快照/缓冲画布，但可见画布本身不得再设置 width（设置会清空像素）。
    expect(widthSetter.mock.contexts.slice(assignments)).not.toContain(canvas);
    expect(wrapper.attributes('data-action')).toBe('working');
    expect(wrapper.attributes('data-transitioning')).toBe('true');
    await step();
    await step();
    await step();
    await step();
    expect(wrapper.attributes('data-transitioning')).toBe('false');
    wrapper.unmount();
    expect(frames.size).toBe(0);
});

it('快速打断从实际可见合成帧取快照；迟到解码不能覆盖新动作或发送 ready', async () => {
    const { skin, image, images } = fixture();
    const wrapper = mount(PetActor, { props: { skin, action: 'idle', animate: true, size: 160 } });
    await flushPromises();
    const canvas = wrapper.get('canvas').element;
    const initialCanvases = new Set(contexts.keys());
    let finish!: (image: HTMLImageElement) => void;
    image.mockImplementationOnce(() => new Promise((resolve) => (finish = resolve)));
    await wrapper.setProps({ action: 'working' });
    await wrapper.setProps({ action: 'petting' });
    await flushPromises();
    await step();
    await wrapper.setProps({ action: 'dragging' });
    await flushPromises();
    const snapshots = [...contexts].filter(
        ([element, context]) =>
            !initialCanvases.has(element) &&
            context.drawImage.mock.calls.some(([source]) => source === canvas),
    );
    expect(snapshots).toHaveLength(2);
    finish(images.working);
    await flushPromises();
    expect(wrapper.attributes('data-action')).toBe('dragging');
    expect(wrapper.emitted('ready')).toEqual([['idle'], ['petting'], ['dragging']]);
    wrapper.unmount();
});

it('过渡区域包含前后姿态；完成后释放旧区域，并在隐藏/暂停期间保留进度', async () => {
    const { skin, manifest } = fixture();
    manifest.actions.idle.hitBands = [[0, 0, 20, 20]];
    manifest.actions.seated!.hitBands = [[280, 280, 40, 40]];
    const wrapper = mount(PetActor, { props: { skin, action: 'idle', animate: true, size: 160 } });
    await flushPromises();
    await wrapper.setProps({ action: 'seated' });
    await flushPromises();
    const union = wrapper.emitted('bounds')!.at(-1)![0] as number[][];
    expect(union.some(([x, y]) => x === 0 && y === 0)).toBe(true);
    expect(union.some(([x, y]) => x > 200 && y > 200)).toBe(true);
    await step();
    const canvas = wrapper.get('canvas').element;
    await wrapper.setProps({ animate: false });
    const clears = contexts.get(canvas)!.clearRect.mock.calls.length;
    await step(10000);
    expect(contexts.get(canvas)!.clearRect).toHaveBeenCalledTimes(clears);
    await wrapper.setProps({ animate: true });
    await step(25);
    expect(wrapper.attributes('data-transitioning')).toBe('true');
    vi.mocked(Object.getOwnPropertyDescriptor(document, 'hidden')!.get!).mockReturnValue(true);
    document.dispatchEvent(new Event('visibilitychange'));
    const hiddenClears = contexts.get(canvas)!.clearRect.mock.calls.length;
    await step(10000);
    expect(contexts.get(canvas)!.clearRect).toHaveBeenCalledTimes(hiddenClears);
    vi.mocked(Object.getOwnPropertyDescriptor(document, 'hidden')!.get!).mockReturnValue(false);
    document.dispatchEvent(new Event('visibilitychange'));
    await step();
    await step();
    expect(wrapper.attributes('data-transitioning')).toBe('false');
    const current = wrapper.emitted('bounds')!.at(-1)![0] as number[][];
    expect(current.every(([x, y]) => x > 200 && y > 200)).toBe(true);
    wrapper.unmount();
});

it.each(Object.keys(approved.actions) as PetAction[])(
    '%s单帧交接的可见权重恒为1，不产生全透明闪空且及时停止重绘',
    async (action) => {
        const { skin, manifest } = fixture();
        for (const clip of Object.values(manifest.actions)) {
            clip.frames = 1;
            clip.columns = 1;
        }
        const initial = action === 'idle' ? 'seated' : 'idle';
        const wrapper = mount(PetActor, {
            props: { skin, action: initial as PetAction, animate: true, size: 144 },
        });
        await flushPromises();
        const canvas = wrapper.get('canvas').element;
        await wrapper.setProps({ action });
        await flushPromises();
        expect(wrapper.attributes('data-transitioning')).toBe('true');
        for (let i = 0; i < 4; i++) {
            const paint = contexts.get(canvas)!;
            paint.weights.length = 0;
            await step(30);
            if (paint.weights.length) expect(paint.weights.reduce((sum, v) => sum + v, 0)).toBeCloseTo(1);
        }
        expect(wrapper.attributes('data-transitioning')).toBe('false');
        expect(wrapper.attributes('data-action')).toBe(action);
        expect(frames.size).toBe(0);
        wrapper.unmount();
    },
);

it('非循环动作只结算一次；卸载后迟到失败不得触发错误事件', async () => {
    const { skin, manifest, image } = fixture();
    manifest.actions.success!.frames = 3;
    manifest.actions.success!.fps = 10;
    manifest.actions.success!.loop = false;
    const wrapper = mount(PetActor, { props: { skin, action: 'success', animate: true, size: 160 } });
    await flushPromises();
    for (let i = 0; i < 12; i++) await step();
    expect(wrapper.emitted('ended')).toEqual([[]]);
    let reject!: (cause: Error) => void;
    image.mockImplementationOnce(() => new Promise((_, fail) => (reject = fail)));
    await wrapper.setProps({ action: 'working' });
    wrapper.unmount();
    reject(new Error('模拟解码失败'));
    await flushPromises();
    expect(wrapper.emitted('error')).toBeUndefined();
    expect(frames.size).toBe(0);
});

it('静态招手完成统一交接后不持续重绘；拖动和目光输入不能旋转整幅人物', async () => {
    const { skin, manifest } = fixture();
    for (const action of ['wave', 'dragging'] as const) {
        manifest.actions[action]!.frames = 1;
        manifest.actions[action]!.columns = 1;
    }
    const wrapper = mount(PetActor, {
        props: { skin, action: 'idle', animate: true, size: 144, look: [1, 1] },
    });
    await flushPromises();
    await wrapper.setProps({ action: 'wave' });
    await flushPromises();
    expect(wrapper.attributes('data-transitioning')).toBe('true');
    for (let i = 0; i < 4; i++) await step();
    expect(wrapper.attributes('data-transitioning')).toBe('false');
    expect(frames.size).toBe(0);
    const canvas = wrapper.get('canvas').element;
    const calls = contexts.get(canvas)!.drawImage.mock.calls.length;
    await step(1000);
    expect(contexts.get(canvas)!.drawImage).toHaveBeenCalledTimes(calls);
    await wrapper.setProps({ action: 'dragging', dragVelocity: [4000, -4000] });
    await flushPromises();
    await step();
    for (const context of contexts.values()) expect(context.rotate).not.toHaveBeenCalled();
    wrapper.unmount();
});
