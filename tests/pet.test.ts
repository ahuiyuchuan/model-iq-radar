/** 桌宠业务隔离、资源边界与布局回归；全部使用合成数据，不调用模型或原生窗口。@author ahui */
import { afterEach, expect, it, vi } from 'vitest';
import approved from '../public/skins/blue-whale-maid/skin.json';
import { ACTIONS, type SkinManifest } from '../src/pet/types';
import {
    chooseAction,
    FEEDBACK_TEXT,
    petResult,
    resolveIdleAction,
    ResultFeedback,
} from '../src/pet/controller';
import { contextMenuLayout, peekLayout, rightSceneLayout, sceneLayout } from '../src/pet/layout';
import { loadSkin, validateManifest, validatePack } from '../src/pet/skinLoader';
import type { AppSnapshot, TestRecord } from '../src/types';
vi.mock('../src/bridge', () => ({ desktop: false, call: vi.fn() }));

const manifest = approved as SkinManifest;

it('全部皮肤动作可作默认外观，缺资源回退且不会覆盖拖拽、面板或真实结果', () => {
    expect(resolveIdleAction('seated', ['idle', 'seated'])).toBe('seated');
    for (const kind of ACTIONS) expect(resolveIdleAction(kind, [...ACTIONS])).toBe(kind);
    for (const invalid of [undefined, 'missing', 'bow'])
        expect(resolveIdleAction(invalid, ['idle', 'seated', 'success', 'dragging'])).toBe('idle');
    expect(chooseAction(false, null, false, false, null, 'seated')).toBe('seated');
    expect(chooseAction(true, null, false, false, null, 'seated')).toBe('dragging');
    expect(chooseAction(false, null, false, true, null, 'wave')).toBe('seated');
    expect(chooseAction(false, null, true, false, null, 'wave')).toBe('working');
    expect(chooseAction(false, null, false, false, 'error', 'seated')).toBe('error');
    expect(chooseAction(false, null, false, false, null, 'success')).toBe('success');
    expect(chooseAction(false, null, true, false, null, 'success')).toBe('working');
    expect(chooseAction(false, null, false, false, 'failed', 'success')).toBe('failed');
    expect(petResult(false, null, false).key).toBe('idle');
});

it.each([96, 144, 192])('%i尺寸菜单在底边与左右角落避开完整角色和结果云朵', (size) => {
    for (const right of [false, true]) {
        const base = right
            ? rightSceneLayout(sceneLayout(size, false, 176, true, false))
            : sceneLayout(size, false, 176, true, false);
        for (const bounds of [
            [-1000, -800, 1400, 240],
            [-1000, -800, base.width, 240],
            [0, -800, 1400, 240],
            [0, 0, 1400, 900],
        ] as [number, number, number, number][]) {
            const { layout } = contextMenuLayout(base, bounds);
            const dx = layout.pet[0] - base.pet[0],
                dy = layout.pet[1] - base.pet[1];
            for (const r of [layout.menu, layout.actions]) {
                expect(r[0] - dx).toBeGreaterThanOrEqual(bounds[0]);
                expect(r[1] - dy).toBeGreaterThanOrEqual(bounds[1]);
                expect(r[0] - dx + r[2]).toBeLessThanOrEqual(bounds[2]);
                expect(r[1] - dy + r[3]).toBeLessThanOrEqual(bounds[3]);
                for (const p of [layout.pet, layout.bubble])
                    expect(
                        r[0] + r[2] <= p[0] ||
                            p[0] + p[2] <= r[0] ||
                            r[1] + r[3] <= p[1] ||
                            p[1] + p[3] <= r[1],
                    ).toBe(true);
            }
        }
    }
});

it.each([false, true])('菜单不跟随人物方位硬编码，人物在右侧=%s时优先向右、空间不足向左', (right) => {
    const normal = sceneLayout(144, false, 176, true, false);
    const base = right ? rightSceneLayout(normal) : normal;
    const enough = contextMenuLayout(base, [-1000, -500, 1400, 1000]);
    expect(enough.left).toBe(false);
    expect(enough.layout.actions[0]).toBe(enough.layout.menu[0] + enough.layout.menu[2] - 1);
    const tight = contextMenuLayout(base, [-1000, -500, base.menu[0] + base.menu[2] + 30, 1000]);
    expect(tight.left).toBe(true);
    expect(tight.layout.actions[0] + tight.layout.actions[2] - 1).toBe(tight.layout.menu[0]);
    // 整体平移不能改变人物相对于卡片和云朵的关系，所有可见菜单均在宿主内。
    expect(tight.layout.pet[0] - tight.layout.bubble[0]).toBe(base.pet[0] - base.bubble[0]);
    for (const item of [tight.layout.menu, tight.layout.actions]) {
        expect(item[0]).toBeGreaterThanOrEqual(0);
        expect(item[0] + item[2]).toBeLessThanOrEqual(tight.layout.width);
    }
});

it.each(['left', 'right', 'top'] as const)(
    '探头%s的像素和热区使用相同等比变换，云朵独立正向且不越界',
    (side) => {
        const clip = manifest.docking!.left;
        const layout = peekLayout(clip, side);
        expect(layout.peek[2] < 96 || layout.peek[3] < 96).toBe(true);
        for (const [x, y, w, h] of [...layout.regions, layout.bubble]) {
            expect(x).toBeGreaterThanOrEqual(0);
            expect(y).toBeGreaterThanOrEqual(0);
            expect(x + w).toBeLessThanOrEqual(layout.width);
            expect(y + h).toBeLessThanOrEqual(layout.height);
        }
        const [x, y, w, h] = layout.bubble;
        const [px, py, pw, ph] = layout.peek;
        expect(x + w <= px || px + pw <= x || y + h <= py || py + ph <= y).toBe(true);
        expect(w).toBe(132);
        expect(h).toBe(50);
        if (side === 'top') {
            // 非对称样例能区分朝左/朝右，不能只检查边界内有矩形就判通过。
            const top = peekLayout({ ...clip, hitBands: [[20, 50, 30, 40]] }, side);
            expect(top.transform).toBe('scaleX(-1) rotate(90deg)');
            expect(top.regions).toEqual([[15, 6, 12, 9]]);
        }
    },
);

it('右侧完整场景只调整几何，浮层仍跟随卡片原点且角色尺度不变', () => {
    const original = sceneLayout(192, true, 176, true, false, [[10, 160, 220, 150]]);
    const right = rightSceneLayout(original);
    expect(right.pet[2]).toBe(192);
    expect(right.pet[0]).toBe(original.width - 192);
    expect(right.bubble[0]).toBe(original.width - original.bubble[0] - original.bubble[2]);
    expect(right.floating[0]![0] - right.panel[0]).toBe(10);
});
const snapshot = (id: string | null) => ({ running: id ? { id } : null }) as AppSnapshot;
const record = (id: string, status: TestRecord['status'] = 'passed') => ({ id, status }) as TestRecord;
afterEach(() => vi.unstubAllGlobals());

it('只对本进程观察到的同一任务结算反馈；恢复、重复、条件切换和迟到记录均不庆祝', () => {
    const feedback = new ResultFeedback();
    expect(feedback.update(snapshot(null), record('old'))).toBeNull();
    expect(feedback.update(snapshot('one'), record('old'))).toBeNull();
    expect(feedback.update(snapshot(null), record('one'))).toBe('passed');
    expect(feedback.update(snapshot(null), record('one'))).toBeNull();
    feedback.update(snapshot('two'), null);
    expect(feedback.update(snapshot(null), null)).toBeNull();
    expect(feedback.update(snapshot(null), record('two'))).toBeNull();
    feedback.update(snapshot('three'), null);
    expect(feedback.update(snapshot(null), record('other'))).toBeNull();
});

it.each(['html', 'failed', 'error', 'cancelled'] as const)('%s 保留真实状态，不映射成通过', (status) => {
    const feedback = new ResultFeedback();
    feedback.update(snapshot('one'), null);
    expect(feedback.update(snapshot(null), record('one', status))).toBe(status);
    expect(FEEDBACK_TEXT[status]).not.toContain('通过');
});

it('面板固定侧坐；独立桌宠的拖动优先于临时反馈和测试', () => {
    expect(chooseAction(true, 'success', true, true)).toBe('seated');
    expect(chooseAction(false, 'petting', true, true)).toBe('seated');
    expect(chooseAction(true, 'success', true, false)).toBe('dragging');
    expect(chooseAction(false, 'petting', true, false)).toBe('petting');
    expect(chooseAction(false, null, true, true)).toBe('seated');
    expect(chooseAction(false, null, true, false)).toBe('working');
    expect(chooseAction(false, null, false, true)).toBe('seated');
    expect(chooseAction(false, null, false, false)).toBe('idle');
});

it('已提供动作资源满足解码预算，缺少可选动作仍允许退回待机', () => {
    expect(validateManifest(manifest).id).toBe('blue-whale-maid');
    const minimal = structuredClone(manifest);
    minimal.actions = { idle: minimal.actions.idle };
    expect(validateManifest(minimal).actions.petting).toBeUndefined();
});

it('边饰预设兼容旧皮肤，拒绝外部代码、空值和未知样式', () => {
    for (const decoration of [undefined, 'none', 'ocean', 'stars']) {
        const valid = structuredClone(manifest);
        if (decoration === undefined) delete valid.decorations;
        else Object.assign(valid, { decorations: decoration });
        expect(validateManifest(valid).decorations).toBe(decoration);
    }
    for (const decoration of [null, '<svg onload="alert(1)">', {}, 'url(remote)']) {
        const invalid = structuredClone(manifest);
        Object.assign(invalid, { decorations: decoration });
        expect(() => validateManifest(invalid)).toThrow('装饰预设');
    }
});

it('探头为可选独立静态资源，旧包兼容且拒绝未知方向、动画和越界轮廓', () => {
    const legacy = structuredClone(manifest);
    delete legacy.docking;
    expect(validateManifest(legacy).docking).toBeUndefined();
    expect(validateManifest(manifest).docking!.left.frames).toBe(1);
    const left = manifest.docking!.left;
    for (const docking of [
        null,
        [],
        {},
        { right: left },
        { left, top: left },
        { left: { ...left, frames: 2 } },
        { left: { ...left, columns: 2 } },
        { left: { ...left, atlas: '../edge.png' } },
        { left: { ...left, frameSize: 513 } },
        { left: { ...left, hitBands: [[0, 0, 999, 999]] } },
    ]) {
        const invalid = structuredClone(manifest);
        Object.assign(invalid, { docking });
        expect(() => validateManifest(invalid)).toThrow();
    }
    // 探头和动作不能复用同一文件名却宣称不同尺寸，否则解码预算及实际显示失真。
    const conflict = structuredClone(manifest);
    Object.assign(conflict.docking!.left, {
        atlas: conflict.actions.idle.poster,
        poster: conflict.actions.idle.poster,
        frameSize: 512,
    });
    expect(() => validateManifest(conflict)).toThrow('不同尺寸');
});

/** 仅构造有正确尺寸的 PNG 头；完整像素有效性由下方 Image 替身模拟，避免依赖浏览器解码器。 */
function pngHeader(size: number): string {
    const header = new Uint8Array(33);
    header.set([137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82]);
    new DataView(header.buffer).setUint32(16, size);
    new DataView(header.buffer).setUint32(20, size);
    return `data:image/png;base64,${btoa(String.fromCharCode(...header))}`;
}

/** 小尺寸合成包不读取本机皮肤或模型凭据；共享图名也能验证按实际文件计费。 */
function dockingPack() {
    const data = pngHeader(32);
    const clip = {
        atlas: 'idle.png',
        poster: 'idle.png',
        frameSize: 32,
        frames: 1,
        columns: 1,
        fps: 1,
        loop: true,
        hitBands: [[0, 0, 32, 32]] as [number, number, number, number][],
    };
    return {
        manifest: {
            ...structuredClone(manifest),
            id: 'test-edge',
            actions: { idle: clip },
            docking: { left: { ...clip, atlas: 'edge.png', poster: 'edge.png' } },
        },
        assets: { 'idle.png': data, 'edge.png': data },
    };
}

it('探头引用参与导入尺寸、安全路径及资源完整性校验', () => {
    const pack = dockingPack();
    expect(validatePack(pack).manifest.docking!.left.poster).toBe('edge.png');
    expect(() => validatePack({ ...pack, assets: { 'idle.png': pack.assets['idle.png'] } })).toThrow('PNG');
    expect(() => validatePack({ ...pack, assets: { ...pack.assets, 'edge.png': pngHeader(64) } })).toThrow(
        '尺寸',
    );
});

it('探头计入与动作共享的64MiB实际解码预算，不能借可选字段超量', () => {
    const bounded = dockingPack().manifest as SkinManifest;
    // 7行×8列512px图集为56MiB，加poster1MiB及六个独立单帧刚好63MiB。
    const idle = {
        ...bounded.actions.idle,
        frameSize: 512,
        frames: 56,
        columns: 8,
        atlas: 'large.png',
        poster: 'large-poster.png',
    };
    bounded.actions = { idle };
    for (const action of ['petting', 'dragging', 'working', 'success', 'seated', 'wave'] as const)
        bounded.actions[action] = {
            ...idle,
            frames: 1,
            columns: 1,
            atlas: `${action}.png`,
            poster: `${action}.png`,
        };
    bounded.docking!.left = { ...idle, frames: 1, columns: 1, atlas: 'edge.png', poster: 'edge.png' };
    expect(validateManifest(bounded)).toBe(bounded); // 单张探头增加1MiB，总计64MiB。
    bounded.docking!.left.poster = 'second-edge.png';
    expect(() => validateManifest(bounded)).toThrow('64 MiB');
});

it('启用前实际解码探头，坏图失败释放句柄；旧包不要求额外资源', async () => {
    const loaded: string[] = [];
    let corrupt = false;
    vi.stubGlobal(
        'Image',
        class {
            width = 32;
            height = 32;
            onload: (() => void) | null = null;
            onerror: (() => void) | null = null;
            set src(value: string) {
                if (!value) return;
                loaded.push(value);
                queueMicrotask(() => (corrupt && loaded.length === 2 ? this.onerror?.() : this.onload?.()));
            }
        },
    );
    const pack = dockingPack();
    const skin = await loadSkin('test-edge', pack);
    expect(skin.dockPoster).toBe(pack.assets['edge.png']);
    expect(loaded).toHaveLength(2);
    skin.dispose();
    await expect(skin.image('idle')).rejects.toThrow('释放');
    loaded.length = 0;
    corrupt = true;
    await expect(loadSkin('test-edge', pack)).rejects.toThrow('加载失败');
    loaded.length = 0;
    corrupt = false;
    delete (pack.manifest as SkinManifest).docking;
    delete (pack.assets as Record<string, string>)['edge.png'];
    const legacy = await loadSkin('test-edge', pack);
    expect(legacy.dockPoster).toBeUndefined();
    expect(loaded).toHaveLength(1);
    legacy.dispose();
});

it('眨眼段限定在待机循环内部，拒绝越界、空值与错误的等待区间', () => {
    const valid = structuredClone(manifest);
    valid.actions.idle.frames = 8;
    valid.actions.idle.columns = 4;
    valid.actions.idle.blink = { start: 4, frames: 4, fps: 16, minDelayMs: 3000, maxDelayMs: 6000 };
    expect(validateManifest(valid).actions.idle.blink?.frames).toBe(4);
    for (const blink of [
        null,
        { ...valid.actions.idle.blink!, start: 0 },
        { ...valid.actions.idle.blink!, frames: 100 },
        { ...valid.actions.idle.blink!, minDelayMs: 8000, maxDelayMs: 3500 },
    ]) {
        const invalid = structuredClone(manifest);
        Object.assign(invalid.actions.idle, { blink });
        expect(() => validateManifest(invalid)).toThrow();
    }
    const invalid = structuredClone(manifest);
    invalid.actions.petting!.blink = valid.actions.idle.blink;
    expect(() => validateManifest(invalid)).toThrow();
});

it.each([
    (m: typeof manifest) => {
        m.id = '../escape';
    },
    (m: typeof manifest) => {
        m.actions.idle.atlas = 'https://remote/a.png';
    },
    (m: typeof manifest) => {
        m.tokens.primary = 'url(remote)';
    },
    (m: typeof manifest) => {
        m.actions.idle.hitBands = [[0, 0, 999, 999]];
    },
    (m: typeof manifest) => {
        m.actions.idle.fps = 0;
    },
    (m: typeof manifest) => {
        m.actions.idle.frames = 64;
        m.actions.idle.frameSize = 512;
    },
])('拒绝路径、CSS、越界轮廓、非法帧率和过量解码', (mutate) => {
    const invalid = structuredClone(manifest);
    mutate(invalid);
    expect(() => validateManifest(invalid)).toThrow();
});

it('导入包拒绝内置覆盖、遗漏图片及非 PNG 资源', () => {
    expect(() => validatePack({ manifest, assets: {} })).toThrow('内置');
    const external = structuredClone(manifest);
    external.id = 'custom';
    expect(() => validatePack({ manifest: external, assets: {} })).toThrow('PNG');
    const assets: Record<string, string> = {};
    for (const clip of Object.values(external.actions) as { atlas: string; poster: string }[])
        for (const file of [clip.atlas, clip.poster]) assets[file] = 'data:image/svg+xml;base64,AAAA';
    expect(() => validatePack({ manifest: external, assets })).toThrow('PNG');
});

it.each([96, 144, 192])('%i px 场景包含全部操作区，角色搭接卡片而不悬在左侧', (size) => {
    for (const open of [false, true]) {
        const l = sceneLayout(size, open, 596, true, true);
        for (const r of [l.pet, l.bubble, l.menu, ...(open ? [l.panel] : [])]) {
            expect(r[0]).toBeGreaterThanOrEqual(0);
            expect(r[1]).toBeGreaterThanOrEqual(0);
            expect(r[0] + r[2]).toBeLessThanOrEqual(l.width);
            expect(r[1] + r[3]).toBeLessThanOrEqual(l.height);
        }
        if (open) {
            expect(l.panel[0]).toBeLessThan(l.pet[2] * 0.5);
            expect(l.panel[1]).toBe(4 + Math.round(size * 0.76));
            expect(l.panel[2]).toBe(360);
        }
    }
});

it('四种业务状态使用独立颜色和准确结论，HTML/取消/条件过期不显示通过', () => {
    expect(petResult(true, record('old'), false)).toMatchObject({ label: '测试中', tone: 'blue' });
    expect(petResult(false, record('one'), false)).toMatchObject({ label: '不降智', tone: 'green' });
    expect(petResult(false, record('one', 'failed'), false)).toMatchObject({ label: '降智', tone: 'amber' });
    expect(petResult(false, record('one', 'error'), false)).toMatchObject({ label: '测试失败', tone: 'red' });
    for (const status of ['html', 'cancelled'] as const) {
        expect(petResult(false, record('one', status), false).action).toBeNull();
        expect(petResult(false, record('one', status), false).label).not.toContain('不降智');
    }
    expect(petResult(false, record('old'), true)).toMatchObject({ key: 'stale', action: null });
    expect(petResult(false, null, false)).toMatchObject({ key: 'idle', action: null });
});

it('选择器浮层只扩场景和命中区域，不拉长卡片，关闭后恢复紧凑范围', () => {
    const base = sceneLayout(144, true, 176, true, false);
    const opened = sceneLayout(144, true, 176, true, false, [[20, 165, 250, 210]]);
    expect(opened.panel).toEqual(base.panel);
    expect(opened.height).toBeGreaterThan(base.height);
    expect(opened.floating).toEqual([[base.panel[0] + 20, base.panel[1] + 165, 250, 210]]);
    expect(sceneLayout(144, true, 176, true, false, []).height).toBe(base.height);
    expect(sceneLayout(144, false, 176, true, false, [[20, 165, 250, 210]]).floating).toEqual([]);
});
