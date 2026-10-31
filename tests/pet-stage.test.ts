/** 桌宠场景真实输入/生命周期回归，播放器使用无位图 stub，不依赖绘图环境或模型服务。@author ahui */
import { mount, flushPromises } from '@vue/test-utils';
import { h } from 'vue';
import { beforeEach, afterEach, expect, it, vi } from 'vitest';
import manifest from '../public/skins/blue-whale-maid/skin.json';
import PetStage from '../src/components/PetStage.vue';
import type { AppSnapshot, TestRecord } from '../src/types';
import { ACTIONS, type SkinManifest } from '../src/pet/types';
const fake = vi.hoisted(() => ({
    load: vi.fn(),
    call: vi.fn(),
    release: vi.fn(),
    dispose: vi.fn(),
    drag: vi.fn(),
    dock: vi.fn(),
    dockCleanup: vi.fn(),
    dockSubscription: vi.fn(),
}));
vi.mock('../src/bridge', () => ({
    desktop: true,
    call: fake.call,
    dragPetWindow: fake.drag,
    openSettings: vi.fn(),
    windowAction: vi.fn(),
    registerSceneLayout: vi.fn(() => () => {}),
    subscribeDragEnd: async (handler: () => void) => {
        fake.release.mockImplementation(handler);
        return () => {};
    },
    subscribeDock: async (handler: (dock: { side: string | null; tucked: boolean }) => void) => {
        fake.dockSubscription();
        fake.dock.mockImplementation(handler);
        return fake.dockCleanup;
    },
}));
vi.mock('../src/pet/skinLoader', () => ({ loadSkin: fake.load }));
const state = (running = false) =>
    ({
        config: {
            preferences: { skin: { id: 'blue-whale-maid', size: 144, animation: true, bubbles: true } },
        },
        running: running ? { id: 'task' } : null,
    }) as AppSnapshot;
const props = () => ({
    snapshot: state(),
    latest: null as TestRecord | null,
    busy: false,
    previewExpanded: false,
});
const actor = { props: ['action'], template: '<div class="actor-stub" :data-action="action" />' };
/** 等待最终布局内的160ms滑入；没有退场等待或透明空窗。 */
async function finishDockMotion(): Promise<void> {
    await flushPromises();
    await vi.advanceTimersByTimeAsync(161);
    await flushPromises();
}
/** 模拟真实鼠标进入后继续移动；单纯DOM移到静止指针下不代表用户想恢复。 */
async function moveIntoPeek(wrapper: ReturnType<typeof mount>): Promise<void> {
    const peek = wrapper.get('.pet-edge-peek');
    peek.element.dispatchEvent(new MouseEvent('pointerenter', { screenX: 100, screenY: 100 }));
    peek.element.dispatchEvent(new MouseEvent('pointermove', { screenX: 103, screenY: 100, bubbles: true }));
    await flushPromises();
}
// 单帧姿态也由场景定时驻留；测试不依赖制作流水线是否给旧包补齐放下动作。
const stageManifest = structuredClone(manifest);
Object.assign(stageManifest.actions, { landing: stageManifest.actions.idle });

it.each(ACTIONS)('默认外观%s只改变角色，不启动真实拖动或修改尚未测试状态', async (kind) => {
    const initial = props();
    initial.snapshot.config.preferences.skin!.idleAction = kind;
    initial.snapshot.config.preferences.skin!.autonomous = false;
    const wrapper = mount(PetStage, { props: initial, global: { stubs: { PetActor: actor } } });
    await flushPromises();
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe(kind);
    expect(wrapper.get('.pet-result-cloud').attributes('data-status')).toBe('idle');
    expect(wrapper.attributes('data-docked')).toBe('false');
    expect(fake.drag).not.toHaveBeenCalled();
    expect(fake.call.mock.calls.map(([command]) => command)).not.toContain('start_test');
    wrapper.unmount();
});

it('保存的常态在互动结束和气泡确认后恢复，换肤缺资源时回退普通待机', async () => {
    const initial = props();
    initial.snapshot.config.preferences.skin!.idleAction = 'seated';
    initial.snapshot.config.preferences.skin!.autonomous = false;
    const wrapper = mount(PetStage, { props: initial, global: { stubs: { PetActor: actor } } });
    await flushPromises();
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('seated');
    await wrapper.get('.pet-character').trigger('dblclick');
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('wave');
    await vi.advanceTimersByTimeAsync(3000);
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('seated');
    await wrapper.setProps({ latest: { id: 'done', status: 'failed' } as TestRecord });
    await wrapper.get('.pet-result-cloud').trigger('click');
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('seated');
    const next = structuredClone(initial.snapshot);
    next.config.preferences.skin!.idleAction = 'bow';
    const limited = structuredClone(stageManifest) as SkinManifest;
    delete limited.actions.bow;
    fake.load.mockResolvedValue({ manifest: limited, dispose: fake.dispose });
    fake.call.mockImplementation(async (command) =>
        command === 'get_widget_dock' ? { side: null, tucked: false } : undefined,
    );
    next.config.preferences.skin!.id = 'limited';
    await wrapper.setProps({ snapshot: next, latest: null });
    await flushPromises();
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('idle');
    wrapper.unmount();
});
beforeEach(() => {
    fake.call.mockReset().mockResolvedValue(undefined);
    fake.load.mockReset().mockResolvedValue({
        manifest: stageManifest,
        dispose: fake.dispose,
        dockPoster: '/skins/blue-whale-maid/edge-left.png',
    });
    fake.dispose.mockClear();
    fake.drag.mockReset().mockResolvedValue(undefined);
    fake.release.mockReset();
    fake.dock.mockReset();
    fake.dockCleanup.mockReset();
    fake.dockSubscription.mockReset();
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
});

it('首次场景提交完成后才订阅读取贴边，避免重启被经典窗口几何抢先夹紧', async () => {
    let finishInitialLayout!: () => void;
    fake.call.mockImplementation((command) =>
        command === 'set_pet_scene_layout'
            ? new Promise<void>((resolve) => {
                  finishInitialLayout = resolve;
              })
            : Promise.resolve(),
    );
    const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith('set_pet_scene_layout', expect.any(Object));
    expect(fake.dockSubscription).not.toHaveBeenCalled();
    fake.call.mockResolvedValue(undefined);
    finishInitialLayout();
    await flushPromises();
    expect(fake.dockSubscription).toHaveBeenCalledOnce();
    wrapper.unmount();
});

it('右键动作收纳到二级菜单，支持分层键盘返回并独立扩展命中掩码', async () => {
    const wrapper = mount(PetStage, {
        props: props(),
        attachTo: document.body,
        global: { stubs: { PetActor: actor } },
    });
    await flushPromises();
    await wrapper.get('.pet-character').trigger('contextmenu');
    const root = wrapper.get('[role="menu"][aria-label="桌宠操作"]');
    expect(root.findAll(':scope > button')).toHaveLength(4);
    expect(wrapper.find('.pet-action-submenu').exists()).toBe(false);
    const trigger = wrapper.get('[aria-haspopup="menu"]');
    await trigger.trigger('keydown', { key: 'ArrowRight' });
    await flushPromises();
    const actions = wrapper.get('.pet-action-submenu');
    expect(actions.findAll('[role="menuitem"]')).toHaveLength(Object.keys(stageManifest.actions).length);
    const scene = fake.call.mock.calls.filter(([c]) => c === 'set_pet_scene_layout').at(-1)![1].scene;
    const rootStyle = (root.element as HTMLElement).style;
    const childStyle = (actions.element as HTMLElement).style;
    expect(scene.regions).toContainEqual([
        parseFloat(rootStyle.left) + 1 + parseFloat(childStyle.left),
        parseFloat(rootStyle.top) + 1 + parseFloat(childStyle.top),
        184,
        326,
    ]);
    expect(document.activeElement).toBe(actions.findAll('button')[0]!.element);
    await actions.findAll('button')[0]!.trigger('keydown', { key: 'End' });
    expect(document.activeElement).toBe(actions.findAll('button').at(-1)!.element);
    await actions.findAll('button').at(-1)!.trigger('keydown', { key: 'Escape' });
    expect(wrapper.find('.pet-action-submenu').exists()).toBe(false);
    expect(document.activeElement).toBe(trigger.element);
    await trigger.trigger('keydown', { key: 'Escape' });
    expect(wrapper.find('.pet-context-menu').exists()).toBe(false);
    wrapper.unmount();
});

it('斜向经过其他一级菜单项不会收起动作菜单，选择后才释放菜单区域', async () => {
    const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    await wrapper.get('.pet-character').trigger('contextmenu');
    await wrapper.get('[aria-haspopup="menu"]').trigger('pointerenter');
    await wrapper.get('.pet-action-group').trigger('pointerleave');
    for (const item of wrapper.findAll('.pet-context-menu > button')) {
        await item.trigger('pointerenter');
        expect(wrapper.find('.pet-action-submenu').exists()).toBe(true);
    }
    await wrapper.get('.pet-action-submenu button').trigger('click');
    expect(wrapper.find('.pet-context-menu').exists()).toBe(false);
    wrapper.unmount();
});

it('触边检测包含可见卡片但不包含临时菜单，角色对齐轮廓保持独立', async () => {
    const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    await wrapper.get('.pet-result-cloud').trigger('click');
    await flushPromises();
    const scene = () => fake.call.mock.calls.filter(([c]) => c === 'set_pet_scene_layout').at(-1)![1].scene;
    const before = scene();
    expect(before.dockBounds[0] + before.dockBounds[2]).toBeGreaterThan(
        before.actorBounds[0] + before.actorBounds[2],
    );
    await wrapper.get('.pet-character').trigger('contextmenu');
    await wrapper.get('[aria-haspopup="menu"]').trigger('pointerenter');
    await flushPromises();
    expect(scene().dockBounds).toEqual(before.dockBounds);
    expect(scene().actorBounds).toEqual(before.actorBounds);
    wrapper.unmount();
});

it('动作点播只使用当前皮肤资源，收起卡片后呈现且不改测试结论', async () => {
    const initial = props();
    initial.latest = { id: 'existing', status: 'error' } as TestRecord;
    const wrapper = mount(PetStage, { props: initial, global: { stubs: { PetActor: actor } } });
    await flushPromises();
    await wrapper.get('.pet-result-cloud').trigger('click');
    await wrapper.get('.pet-character').trigger('contextmenu');
    await wrapper.get('[aria-haspopup="menu"]').trigger('pointerenter');
    await wrapper.get('[aria-haspopup="menu"]').trigger('click');
    expect(wrapper.find('.pet-action-submenu').exists()).toBe(true);
    const stretch = wrapper
        .findAll('.pet-action-submenu button')
        .find((button) => button.text() === '伸个懒腰')!;
    await stretch.trigger('click');
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('stretch');
    expect(wrapper.get('.pet-result-cloud').attributes('data-status')).toBe('error');
    expect(wrapper.find('.pet-context-menu').exists()).toBe(false);
    expect(
        fake.call.mock.calls.some(([c]) => ['start_test', 'cancel_test', 'send_test_email'].includes(c)),
    ).toBe(false);
    wrapper.unmount();
});

it('旧皮肤缺少的动作不出现在二级菜单，关闭后不保留菜单输入区域', async () => {
    const legacy = structuredClone(stageManifest) as SkinManifest;
    legacy.actions = { idle: legacy.actions.idle, wave: legacy.actions.wave };
    fake.load.mockResolvedValue({ manifest: legacy, dispose: fake.dispose });
    const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    await wrapper.get('.pet-character').trigger('contextmenu');
    await wrapper.get('[aria-haspopup="menu"]').trigger('pointerenter');
    expect(wrapper.findAll('.pet-action-submenu button').map((button) => button.text())).toEqual([
        '待机',
        '招招手',
    ]);
    await wrapper.get('[aria-haspopup="menu"]').trigger('keydown', { key: 'Escape' });
    await flushPromises();
    expect(wrapper.find('.pet-context-menu').exists()).toBe(false);
    const scene = fake.call.mock.calls.filter(([c]) => c === 'set_pet_scene_layout').at(-1)![1].scene;
    expect(scene.regions.some((r: number[]) => r[2] === 184 && [208, 326].includes(r[3]!))).toBe(false);
    wrapper.unmount();
});

it('右侧反射只改变当前锚点，保存所用默认布局锚点保持一致', async () => {
    const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    const original = fake.call.mock.calls.filter(([c]) => c === 'set_pet_scene_layout').at(-1)![1].scene;
    fake.dock({ side: 'right', tucked: false });
    await flushPromises();
    const reflected = fake.call.mock.calls.filter(([c]) => c === 'set_pet_scene_layout').at(-1)![1].scene;
    expect(reflected.canonicalAnchor).toEqual(original.canonicalAnchor);
    expect(original.canonicalAnchor).toEqual(original.anchor);
    expect(reflected.anchor).not.toEqual(reflected.canonicalAnchor);
    wrapper.unmount();
});

it('左侧贴边延迟收为已批准探头，恢复和业务结果不互相改写', async () => {
    const initial = props();
    initial.latest = { id: 'existing', status: 'failed' } as TestRecord;
    const wrapper = mount(PetStage, { props: initial, global: { stubs: { PetActor: actor } } });
    await flushPromises();
    fake.dock({ side: 'left', tucked: false });
    await vi.advanceTimersByTimeAsync(179);
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(false);
    await vi.advanceTimersByTimeAsync(2);
    expect(wrapper.attributes('data-dock-motion')).toBe('enter');
    await finishDockMotion();
    await flushPromises();
    const peek = wrapper.get('.pet-edge-peek');
    expect(peek.attributes('aria-label')).toContain('降智');
    expect(peek.get('img').attributes('src')).toContain('edge-left.png');
    const scene = fake.call.mock.calls.filter(([c]) => c === 'set_pet_scene_layout').at(-1)![1].scene;
    expect(scene.docking).toBe('left');
    expect(scene.width).toBeGreaterThan(132);
    expect(scene.height).toBe(98);
    expect(wrapper.get('.pet-result-cloud').isVisible()).toBe(true);
    expect(wrapper.get('.pet-result-cloud').text()).toContain('降智');
    await peek.trigger('click');
    await finishDockMotion();
    await flushPromises();
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(false);
    expect(wrapper.get('.pet-result-cloud').attributes('data-status')).toBe('failed');
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('seated');
    await vi.advanceTimersByTimeAsync(100);
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(false);
    expect(
        fake.call.mock.calls.some(([c]) => ['start_test', 'cancel_test', 'send_test_email'].includes(c)),
    ).toBe(false);
    wrapper.unmount();
    expect(fake.dockCleanup).toHaveBeenCalledOnce();
});

it('拖动和菜单阻止收起；旧皮肤缺少探头保持原角色', async () => {
    const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    wrapper
        .get('.pet-character')
        .element.dispatchEvent(
            new MouseEvent('pointerdown', { button: 0, screenX: 10, screenY: 10, bubbles: true }),
        );
    wrapper
        .get('.pet-character')
        .element.dispatchEvent(new MouseEvent('pointermove', { screenX: 30, screenY: 10, bubbles: true }));
    await flushPromises();
    fake.dock({ side: 'left', tucked: false });
    await vi.advanceTimersByTimeAsync(1000);
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(false);
    fake.release();
    await vi.advanceTimersByTimeAsync(501);
    await finishDockMotion();
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(true);
    await moveIntoPeek(wrapper);
    await finishDockMotion();
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(false);
    fake.dock({ side: null, tucked: false });
    await vi.advanceTimersByTimeAsync(1000);
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(false);
    wrapper.unmount();
    const old = structuredClone(stageManifest) as SkinManifest;
    delete old.docking;
    fake.load.mockResolvedValue({ manifest: old, dispose: fake.dispose });
    const legacy = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    fake.dock({ side: 'left', tucked: false });
    await vi.advanceTimersByTimeAsync(1000);
    expect(legacy.find('.pet-edge-peek').exists()).toBe(false);
    legacy.unmount();
});

it('拖动中结算后贴边直接接管，不排队放下和庆祝；自主动作不能重启', async () => {
    const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    await wrapper.setProps({ snapshot: state(true), busy: true });
    const character = wrapper.get('.pet-character').element;
    character.dispatchEvent(
        new MouseEvent('pointerdown', { button: 0, screenX: 10, screenY: 10, bubbles: true }),
    );
    character.dispatchEvent(new MouseEvent('pointermove', { screenX: 40, screenY: 10, bubbles: true }));
    await flushPromises();
    fake.dock({ side: 'top', tucked: false });
    await wrapper.setProps({
        snapshot: state(),
        busy: false,
        latest: { id: 'task', status: 'passed' } as TestRecord,
    });
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('dragging');
    fake.release();
    await flushPromises();
    expect(wrapper.get('.actor-stub').attributes('data-action')).not.toBe('landing');
    // 通过可直接映射静态success表情，但不得驻留1800ms庆祝后才开始贴边。
    await vi.advanceTimersByTimeAsync(181);
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(true);
    expect(wrapper.get('.pet-result-cloud').text()).toContain('不降智');
    await vi.advanceTimersByTimeAsync(20000);
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(true);
    wrapper.unmount();
});

it.each(['left', 'right', 'top'] as const)('%s贴边保留真实状态云朵，关闭临时气泡不隐藏结论', async (side) => {
    const initial = props();
    initial.latest = { id: 'existing', status: 'error' } as TestRecord;
    initial.snapshot.config.preferences.skin!.bubbles = false;
    const wrapper = mount(PetStage, { props: initial, global: { stubs: { PetActor: actor } } });
    await flushPromises();
    fake.dock({ side, tucked: false });
    await vi.advanceTimersByTimeAsync(501);
    await finishDockMotion();
    await flushPromises();
    expect(wrapper.get('.pet-edge-peek').attributes('data-side')).toBe(side);
    const image = wrapper.get('.pet-edge-peek img').element as HTMLElement;
    expect(image.style.transform).toBe(
        side === 'right' ? 'scaleX(-1)' : side === 'top' ? 'scaleX(-1) rotate(90deg)' : 'none',
    );
    const cloud = wrapper.get('.pet-result-cloud');
    expect(cloud.isVisible()).toBe(true);
    expect(cloud.attributes('data-status')).toBe('error');
    expect(cloud.text()).toBe('测试失败');
    const scene = fake.call.mock.calls.filter(([c]) => c === 'set_pet_scene_layout').at(-1)![1].scene;
    expect(scene.docking).toBe(side);
    const style = (cloud.element as HTMLElement).style;
    expect(scene.regions).toContainEqual([parseFloat(style.left), parseFloat(style.top), 132, 50]);
    fake.dock({ side: side === 'right' ? 'top' : 'right', tucked: true });
    await flushPromises();
    expect(wrapper.get('.pet-edge-peek').attributes('data-side')).not.toBe(side);
    await cloud.trigger('click');
    await finishDockMotion();
    await flushPromises();
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(false);
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('seated');
    wrapper.unmount();
});

it.each(['left', 'right', 'top'] as const)(
    '%s回答中确认云朵打开面板后收纳，探头仍显示进度及新结果',
    async (side) => {
        const initial = props();
        initial.snapshot = state(true);
        initial.busy = true;
        initial.snapshot.config.preferences.skin!.bubbles = false;
        const wrapper = mount(PetStage, {
            props: initial,
            global: { stubs: { PetActor: actor } },
        });
        await flushPromises();
        await wrapper.get('.pet-result-cloud').trigger('click');
        expect(wrapper.get('.pet-result-cloud').attributes('style')).toContain('display: none');
        fake.dock({ side, tucked: false });
        await vi.advanceTimersByTimeAsync(181);
        await finishDockMotion();
        expect(wrapper.find('.pet-edge-peek').exists()).toBe(true);
        expect(wrapper.get('.pet-result-cloud').attributes('style')).not.toContain('display: none');
        expect(wrapper.get('.pet-result-cloud').attributes('data-status')).toBe('running');
        const scene = fake.call.mock.calls
            .filter(([command]) => command === 'set_pet_scene_layout')
            .at(-1)![1].scene;
        const style = (wrapper.get('.pet-result-cloud').element as HTMLElement).style;
        expect(scene.regions).toContainEqual([parseFloat(style.left), parseFloat(style.top), 132, 50]);
        await moveIntoPeek(wrapper);
        await finishDockMotion();
        expect(wrapper.get('.pet-result-cloud').attributes('style')).toContain('display: none');
        expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('seated');
        await wrapper.setProps({
            snapshot: state(),
            busy: false,
            latest: { id: 'task', status: 'failed' } as TestRecord,
        });
        expect(wrapper.get('.pet-result-cloud').attributes('style')).not.toContain('display: none');
        expect(wrapper.get('.pet-result-cloud').attributes('data-status')).toBe('failed');
        wrapper.unmount();
    },
);

it.each(['left', 'right', 'top'] as const)('%s贴边可收纳已打开的面板，恢复保留卡片和结果', async (side) => {
    const initial = props();
    initial.latest = { id: 'existing', status: 'failed' } as TestRecord;
    const wrapper = mount(PetStage, {
        props: initial,
        slots: { default: '<input value="保留的输入" />' },
        global: { stubs: { PetActor: actor } },
    });
    await flushPromises();
    await wrapper.get('.pet-result-cloud').trigger('click');
    const input = wrapper.get('input').element;
    fake.dock({ side, tucked: false });
    await vi.advanceTimersByTimeAsync(181);
    expect(wrapper.attributes('data-dock-motion')).toBe('enter');
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(true);
    await finishDockMotion();
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(true);
    expect(wrapper.get('.pet-panel').isVisible()).toBe(false);
    expect(wrapper.emitted('visible')!.at(-1)).toEqual([false]);
    expect(wrapper.get('.pet-result-cloud').text()).toContain('降智');
    expect(wrapper.get('.pet-result-cloud').attributes('aria-label')).toContain('展开测智面板');
    await moveIntoPeek(wrapper);
    await finishDockMotion();
    expect(wrapper.get('.pet-panel').attributes('style')).not.toContain('display: none');
    expect(wrapper.emitted('visible')!.at(-1)).toEqual([true]);
    expect(wrapper.get('.pet-result-cloud').attributes('aria-label')).toContain('收起为桌宠');
    expect(wrapper.get('input').element).toBe(input);
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('seated');
    const final = fake.call.mock.calls.filter(([c]) => c === 'set_pet_scene_layout').at(-1)![1].scene;
    expect(final.regions).not.toContainEqual([0, 0, final.width, final.height]);
    wrapper.unmount();
});

it('面板显示时测试、摸头、菜单动作和标题栏拖动均保持侧坐', async () => {
    const wrapper = mount(PetStage, {
        props: props(),
        slots: { default: '<div class="drag-area">标题栏</div>' },
        global: { stubs: { PetActor: actor } },
    });
    await flushPromises();
    await wrapper.get('.pet-result-cloud').trigger('click');
    await wrapper.get('.pet-character').trigger('dblclick');
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('seated');
    await wrapper.setProps({ snapshot: state(true), busy: true });
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('seated');
    const header = wrapper.get('.drag-area').element;
    header.dispatchEvent(
        new MouseEvent('pointerdown', { button: 0, screenX: 10, screenY: 10, bubbles: true }),
    );
    header.dispatchEvent(new MouseEvent('pointermove', { screenX: 40, screenY: 10, bubbles: true }));
    await flushPromises();
    expect(fake.drag).toHaveBeenCalled();
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('seated');
    await wrapper.setProps({
        snapshot: state(),
        busy: false,
        latest: { id: 'task', status: 'passed' } as TestRecord,
    });
    fake.release();
    await vi.advanceTimersByTimeAsync(500);
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('seated');
    wrapper.unmount();
});

it('返回鼠标立即逆转目标；关闭动画、换肤和卸载不会复活过期收纳', async () => {
    const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    fake.dock({ side: 'left', tucked: false });
    await vi.advanceTimersByTimeAsync(181);
    expect(wrapper.attributes('data-dock-motion')).toBe('enter');
    await moveIntoPeek(wrapper);
    await finishDockMotion();
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(false);
    expect(wrapper.attributes('data-dock-motion')).toBe('');
    await vi.advanceTimersByTimeAsync(550);
    const paused = state();
    paused.config.preferences.skin!.animation = false;
    await wrapper.setProps({ snapshot: paused });
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(true);
    expect(wrapper.attributes('data-dock-motion')).toBe('');
    await moveIntoPeek(wrapper);
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(false);
    const classic = state();
    classic.config.preferences.skin!.id = 'classic';
    await wrapper.setProps({ snapshot: classic });
    await flushPromises();
    await vi.advanceTimersByTimeAsync(1500);
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(false);
    wrapper.unmount();
});

it('探头悬停恢复后立即操作卡片按钮，不被补接点击吞掉', async () => {
    const pressed = vi.fn();
    const wrapper = mount(PetStage, {
        props: props(),
        slots: { default: () => h('button', { class: 'real-control', onClick: pressed }, '详情开关') },
        global: { stubs: { PetActor: actor } },
    });
    await flushPromises();
    await wrapper.get('.pet-result-cloud').trigger('click');
    fake.dock({ side: 'right', tucked: false });
    await vi.advanceTimersByTimeAsync(181);
    await moveIntoPeek(wrapper);
    await wrapper.get('.real-control').trigger('click');
    expect(pressed).toHaveBeenCalledOnce();
    wrapper.unmount();
});

it('选择器浮层、HTML放大和面板按住期间不自动收纳', async () => {
    const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    await wrapper.get('.pet-result-cloud').trigger('click');
    fake.dock({ side: 'left', tucked: false });
    await wrapper.setProps({ floatingBounds: [[20, 120, 180, 90]] });
    await vi.advanceTimersByTimeAsync(1500);
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(false);
    await wrapper.setProps({ floatingBounds: [], previewExpanded: true });
    await vi.advanceTimersByTimeAsync(1500);
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(false);
    await wrapper.setProps({ previewExpanded: false });
    wrapper.element.dispatchEvent(new MouseEvent('pointerdown', { button: 0, bubbles: true }));
    await vi.advanceTimersByTimeAsync(1500);
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(false);
    document.dispatchEvent(new Event('pointerup'));
    await vi.advanceTimersByTimeAsync(501);
    await finishDockMotion();
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(true);
    wrapper.unmount();
});

it.each(['left', 'right', 'top'] as const)('%s探头重排进入静止鼠标下不恢复，真实移动才恢复', async (side) => {
    const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    fake.dock({ side, tucked: false });
    await vi.advanceTimersByTimeAsync(181);
    const peek = wrapper.get('.pet-edge-peek');
    peek.element.dispatchEvent(new MouseEvent('pointerenter', { screenX: 815, screenY: 132 }));
    for (let i = 0; i < 5; i++) {
        peek.element.dispatchEvent(
            new MouseEvent('pointermove', { screenX: 815, screenY: 132, clientX: i * 40, bubbles: true }),
        );
        await vi.advanceTimersByTimeAsync(250);
        expect(wrapper.find('.pet-edge-peek').exists()).toBe(true);
    }
    peek.element.dispatchEvent(new MouseEvent('pointermove', { screenX: 812, screenY: 132, bubbles: true }));
    await flushPromises();
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(false);
    wrapper.unmount();
});

it.each(['left', 'right', 'top'] as const)(
    '%s探头鼠标按下不先触发焦点恢复，完整点击打开卡片',
    async (side) => {
        const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
        await flushPromises();
        fake.dock({ side, tucked: false });
        await vi.advanceTimersByTimeAsync(501);
        const peek = wrapper.get('.pet-edge-peek');
        const down = new MouseEvent('pointerdown', { button: 0, bubbles: true, cancelable: true });
        peek.element.dispatchEvent(down);
        expect(down.defaultPrevented).toBe(true);
        await flushPromises();
        expect(wrapper.find('.pet-edge-peek').exists()).toBe(true);
        await peek.trigger('pointerup');
        await peek.trigger('click');
        expect(wrapper.find('.pet-edge-peek').exists()).toBe(false);
        expect(wrapper.get('.pet-result-cloud').attributes('aria-label')).toContain('收起为桌宠');
        wrapper.unmount();
    },
);

it.each([false, true])('气泡短按与拖动分离，面板已展开=%s时拖后不误切面板或改结论', async (open) => {
    const initial = props();
    initial.latest = { id: 'real-result', status: 'failed' } as TestRecord;
    const wrapper = mount(PetStage, { props: initial, global: { stubs: { PetActor: actor } } });
    await flushPromises();
    const cloud = wrapper.get('.pet-result-cloud');
    if (open) await cloud.trigger('click');
    cloud.element.dispatchEvent(
        new MouseEvent('pointerdown', { button: 0, screenX: 100, screenY: 100, bubbles: true }),
    );
    cloud.element.dispatchEvent(new MouseEvent('pointermove', { screenX: 103, screenY: 100, bubbles: true }));
    await flushPromises();
    expect(fake.drag).not.toHaveBeenCalled();
    // 长按云朵不会抢成菜单；之后仍可超过阈值开始同一拖动。
    await vi.advanceTimersByTimeAsync(600);
    expect(wrapper.find('.pet-context-menu').exists()).toBe(false);
    cloud.element.dispatchEvent(new MouseEvent('pointermove', { screenX: 120, screenY: 110, bubbles: true }));
    await flushPromises();
    expect(fake.drag).toHaveBeenCalledExactlyOnceWith([20, 10]);
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe(open ? 'seated' : 'dragging');
    // 原生拖动尚未松手时，键盘激活同样不能改布局；松手后则可立即使用键盘。
    cloud.element.dispatchEvent(new MouseEvent('click', { detail: 0, bubbles: true }));
    await flushPromises();
    expect(cloud.attributes('aria-label')).toContain(open ? '收起为桌宠' : '展开测智面板');
    fake.release();
    await flushPromises();
    cloud.element.dispatchEvent(new MouseEvent('click', { detail: 1, bubbles: true }));
    await flushPromises();
    expect(cloud.attributes('aria-label')).toContain(open ? '收起为桌宠' : '展开测智面板');
    expect(cloud.attributes('data-status')).toBe('failed');
    expect(
        fake.call.mock.calls.some(([c]) => ['start_test', 'cancel_test', 'send_test_email'].includes(c)),
    ).toBe(false);
    // 键盘激活不被拖后鼠标click保护误吞。
    cloud.element.dispatchEvent(new MouseEvent('click', { detail: 0, bubbles: true }));
    await flushPromises();
    expect(cloud.attributes('aria-label')).toContain(open ? '展开测智面板' : '收起为桌宠');
    wrapper.unmount();
});

it('气泡按下期间隐藏后恢复，普通移动不启动过期拖动', async () => {
    const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    const cloud = wrapper.get('.pet-result-cloud');
    cloud.element.dispatchEvent(
        new MouseEvent('pointerdown', { button: 0, screenX: 100, screenY: 100, bubbles: true }),
    );
    vi.spyOn(document, 'hidden', 'get').mockReturnValue(true);
    document.dispatchEvent(new Event('visibilitychange'));
    await flushPromises();
    vi.spyOn(document, 'hidden', 'get').mockReturnValue(false);
    document.dispatchEvent(new Event('visibilitychange'));
    cloud.element.dispatchEvent(new MouseEvent('pointermove', { screenX: 150, screenY: 150, bubbles: true }));
    await flushPromises();
    expect(fake.drag).not.toHaveBeenCalled();
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('idle');
    wrapper.unmount();
});

it('探头云朵按下恢复后可拖动，短按仍打开已有卡片而不是误关', async () => {
    const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    await wrapper.get('.pet-result-cloud').trigger('click');
    fake.dock({ side: 'right', tucked: false });
    await vi.advanceTimersByTimeAsync(501);
    const cloud = wrapper.get('.pet-result-cloud');
    cloud.element.dispatchEvent(
        new MouseEvent('pointerdown', { button: 0, screenX: 100, screenY: 100, bubbles: true }),
    );
    await flushPromises();
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(false);
    cloud.element.dispatchEvent(new MouseEvent('pointerup', { button: 0, bubbles: true }));
    cloud.element.dispatchEvent(new MouseEvent('click', { detail: 1, bubbles: true }));
    await flushPromises();
    expect(cloud.attributes('aria-label')).toContain('收起为桌宠');
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('seated');
    wrapper.unmount();
});

it('悬停恢复后缺少离开事件也能重收起，关闭吸附立即恢复完整角色', async () => {
    const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    fake.dock({ side: 'left', tucked: false });
    await vi.advanceTimersByTimeAsync(501);
    await finishDockMotion();
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(true);
    await moveIntoPeek(wrapper);
    await finishDockMotion();
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(false);
    // jsdom没有真实hover；不派发pointerleave模拟原生resize吞掉该事件。
    await vi.advanceTimersByTimeAsync(501);
    await finishDockMotion();
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(true);
    const disabled = state();
    disabled.config.preferences.snapToEdge = false;
    await wrapper.setProps({ snapshot: disabled });
    await vi.advanceTimersByTimeAsync(2000);
    expect(wrapper.find('.pet-edge-peek').exists()).toBe(false);
    expect(
        fake.call.mock.calls.filter(([c]) => c === 'set_pet_scene_layout').at(-1)![1].scene.docking,
    ).toBeNull();
    wrapper.unmount();
});

it('拖动中换肤先取消原生循环，迟到换肤不会恢复已退出的场景', async () => {
    const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    wrapper
        .get('.pet-character')
        .element.dispatchEvent(
            new MouseEvent('pointerdown', { button: 0, screenX: 10, screenY: 10, bubbles: true }),
        );
    wrapper
        .get('.pet-character')
        .element.dispatchEvent(new MouseEvent('pointermove', { screenX: 30, screenY: 10, bubbles: true }));
    await flushPromises();
    let finishCancel!: () => void;
    fake.call.mockImplementation((command) =>
        command === 'cancel_pet_drag'
            ? new Promise<void>((resolve) => {
                  finishCancel = resolve;
              })
            : Promise.resolve(),
    );
    const classic = state();
    classic.config.preferences.skin!.id = 'classic';
    await wrapper.setProps({ snapshot: classic });
    expect(fake.call).toHaveBeenCalledWith('cancel_pet_drag');
    expect(wrapper.find('.pet-character').exists()).toBe(true);
    finishCancel();
    await flushPromises();
    expect(wrapper.find('.pet-character').exists()).toBe(false);
    fake.release();
    await vi.advanceTimersByTimeAsync(1000);
    expect(wrapper.find('.pet-character').exists()).toBe(false);
    wrapper.unmount();
});
it('已松手后的换肤仍先撤销原生保存意图，不把旧位置落到新皮肤', async () => {
    const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    fake.dock({ side: 'right', tucked: false });
    await flushPromises();
    let finishCancel!: () => void;
    fake.call.mockImplementation((command) =>
        command === 'cancel_pet_drag'
            ? new Promise<void>((resolve) => {
                  finishCancel = resolve;
              })
            : Promise.resolve(),
    );
    const classic = state();
    classic.config.preferences.skin!.id = 'classic';
    await wrapper.setProps({ snapshot: classic });
    expect(fake.call).toHaveBeenCalledWith('cancel_pet_drag');
    expect(wrapper.find('.pet-character').exists()).toBe(true);
    finishCancel();
    await flushPromises();
    expect(wrapper.find('.pet-character').exists()).toBe(false);
    wrapper.unmount();
});

afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
});

it('空闲自主活动不连抽同动作；测试、拖动和关闭自主开关立即让位', async () => {
    vi.spyOn(Math, 'random').mockReturnValue(0);
    const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    await vi.advanceTimersByTimeAsync(7001);
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('wave');
    await wrapper.setProps({ snapshot: state(true), busy: true });
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('working');
    await vi.advanceTimersByTimeAsync(20000);
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('working');
    await wrapper.setProps({ snapshot: state(), busy: false });
    await vi.advanceTimersByTimeAsync(7001);
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('stretch');
    const disabled = state();
    disabled.config.preferences.skin!.autonomous = false;
    await wrapper.setProps({ snapshot: disabled });
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('idle');
    await vi.advanceTimersByTimeAsync(40000);
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('idle');
    wrapper.unmount();
});

it('摸头后展开立即侧坐，窗口 Region 保留真实卡片且布局不额外发模型请求', async () => {
    const wrapper = mount(PetStage, {
        props: props(),
        slots: {
            default: ({ togglePanel }) =>
                h('button', { class: 'slot-collapse', onClick: togglePanel }, '收起为桌宠'),
        },
        global: { stubs: { PetActor: actor } },
    });
    await flushPromises();
    await wrapper.get('.pet-character').trigger('click');
    await vi.advanceTimersByTimeAsync(221);
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('petting');
    await wrapper.get('.pet-result-cloud').trigger('click');
    await flushPromises();
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('seated');
    const scene = fake.call.mock.calls.filter(([c]) => c === 'set_pet_scene_layout').at(-1)![1].scene;
    const cloud = wrapper.get('.pet-result-cloud').element as HTMLElement;
    expect(wrapper.get('.pet-result-cloud').isVisible()).toBe(false);
    expect(scene.regions).not.toContainEqual([
        parseFloat(cloud.style.left),
        parseFloat(cloud.style.top),
        132,
        50,
    ]);
    expect(wrapper.find('.pet-radar').exists()).toBe(false);
    expect(wrapper.find('.pet-hologram').exists()).toBe(false);
    expect(wrapper.find('.pet-bubble').exists()).toBe(false);
    await wrapper.get('.slot-collapse').trigger('click');
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('idle');
    expect(fake.call.mock.calls.some(([c]) => c === 'start_test')).toBe(false);
    wrapper.unmount();
    expect(fake.dispose).toHaveBeenCalled();
});

it('拖动保持悬空，测试通过在松开后庆祝，拖后 click 不误摸头', async () => {
    const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    await wrapper.setProps({ snapshot: state(true), busy: true });
    wrapper
        .get('.pet-character')
        .element.dispatchEvent(
            new MouseEvent('pointerdown', { button: 0, screenX: 10, screenY: 10, bubbles: true }),
        );
    wrapper
        .get('.pet-character')
        .element.dispatchEvent(new MouseEvent('pointermove', { screenX: 30, screenY: 10, bubbles: true }));
    await flushPromises();
    expect(fake.drag).toHaveBeenCalledOnce();
    expect(fake.drag).toHaveBeenCalledWith([20, 0]);
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('dragging');
    const during = fake.call.mock.calls.filter(([c]) => c === 'set_pet_scene_layout').at(-1)![1].scene;
    expect(during.regions).toContainEqual([0, 0, during.width, during.height]);
    await wrapper.setProps({
        snapshot: state(),
        busy: false,
        latest: { id: 'task', status: 'passed' } as TestRecord,
    });
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('dragging');
    fake.release();
    await flushPromises();
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('landing');
    await vi.advanceTimersByTimeAsync(321);
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('success');
    const released = fake.call.mock.calls.filter(([c]) => c === 'set_pet_scene_layout').at(-1)![1].scene;
    expect(released.regions).not.toContainEqual([0, 0, released.width, released.height]);
    await wrapper.get('.pet-character').trigger('click');
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('success');
    await vi.advanceTimersByTimeAsync(1900);
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('success');
    wrapper.unmount();
});

it('迟到资源在卸载后释放；坏资源回退经典面板', async () => {
    let complete!: (skin: unknown) => void;
    fake.load.mockImplementationOnce(
        () =>
            new Promise((resolve) => {
                complete = resolve;
            }),
    );
    const first = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    first.unmount();
    complete({ manifest, dispose: fake.dispose });
    await flushPromises();
    expect(fake.dispose).toHaveBeenCalledOnce();
    fake.load.mockRejectedValueOnce(new Error('损坏'));
    let finishRestore!: () => void;
    fake.call.mockImplementation((command) =>
        command === 'clear_pet_scene'
            ? new Promise<void>((resolve) => {
                  finishRestore = resolve;
              })
            : Promise.resolve(),
    );
    const second = mount(PetStage, {
        props: props(),
        slots: { default: '原有功能' },
        global: { stubs: { PetActor: actor } },
    });
    await flushPromises();
    expect(second.find('.pet-character').exists()).toBe(false);
    expect(second.text()).toContain('原有功能');
    // 启动时尚未激活过桌宠，active 不发生 true→false；仍需恢复原生尺寸、Region 和吸附。
    expect(fake.call.mock.calls.filter(([command]) => command === 'clear_pet_scene')).toHaveLength(1);
    expect(fake.call.mock.calls.some(([command]) => command === 'set_pet_scene_layout')).toBe(false);
    // 宿主接到错误后会展开详情；原生恢复未完成之前不可让它发出新尺寸。
    expect(second.emitted('error')).toBeUndefined();
    finishRestore();
    await flushPromises();
    expect(String(second.emitted('error'))).toContain('恢复经典');
    second.unmount();
});

it('运行中渲染失败先恢复窗口，再通知宿主展开错误，避免被经典收起尺寸覆盖', async () => {
    const wrapper = mount(PetStage, {
        props: props(),
        slots: { default: '原有功能' },
        global: { stubs: { PetActor: actor } },
    });
    await flushPromises();
    let finishRestore!: () => void;
    fake.call.mockImplementation((command) =>
        command === 'clear_pet_scene'
            ? new Promise<void>((resolve) => {
                  finishRestore = resolve;
              })
            : Promise.resolve(),
    );
    wrapper.findComponent(actor).vm.$emit('error', '渲染故障');
    await flushPromises();
    expect(wrapper.find('.pet-character').exists()).toBe(false);
    expect(wrapper.text()).toContain('原有功能');
    expect(wrapper.emitted('error')).toBeUndefined();
    expect(fake.call.mock.calls.filter(([command]) => command === 'clear_pet_scene')).toHaveLength(1);
    finishRestore();
    await flushPromises();
    expect(String(wrapper.emitted('error'))).toContain('渲染故障');
    wrapper.unmount();
});

it('关闭互动气泡不隐藏真实结果，摸头也只更改同一云朵的次级文案', async () => {
    const initial = props();
    initial.latest = { id: 'existing', status: 'passed' } as TestRecord;
    initial.snapshot.config.preferences.skin!.bubbles = false;
    const wrapper = mount(PetStage, { props: initial, global: { stubs: { PetActor: actor } } });
    await flushPromises();
    expect(wrapper.get('.pet-result-cloud').attributes('data-status')).toBe('passed');
    expect(wrapper.get('.pet-result-cloud').text()).toBe('不降智');
    await wrapper.get('.pet-character').trigger('click');
    await vi.advanceTimersByTimeAsync(221);
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('petting');
    expect(wrapper.get('.pet-result-cloud').text()).toBe('不降智');
    expect(wrapper.find('.pet-cloud-interaction').exists()).toBe(false);
    await wrapper.setProps({ stale: true });
    expect(wrapper.get('.pet-result-cloud').attributes('data-status')).toBe('stale');
    expect(wrapper.get('.pet-result-cloud').text()).toBe('待重新测试');
    wrapper.unmount();
});

it.each([
    ['passed', '不降智', 'green'],
    ['failed', '降智', 'amber'],
    ['error', '测试失败', 'red'],
    ['html', 'HTML 回答', 'blue'],
    ['cancelled', '已取消', 'neutral'],
] as const)('常驻结果 %s 使用准确文案和色彩，不依赖临时计时器', async (status, label, tone) => {
    const initial = props();
    initial.latest = { id: 'existing', status } as TestRecord;
    const wrapper = mount(PetStage, { props: initial, global: { stubs: { PetActor: actor } } });
    await flushPromises();
    await vi.advanceTimersByTimeAsync(4000);
    expect(wrapper.get('.pet-result-cloud').classes()).toContain(`pet-result-${tone}`);
    expect(wrapper.get('.pet-result-cloud').text()).toContain(label);
    expect(wrapper.findAll('.pet-result-cloud')).toHaveLength(1);
    wrapper.unmount();
});

it('双击直接招手，不先摸头；单帧动作完成事件不会立即清除静态姿态', async () => {
    const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    await wrapper.get('.pet-character').trigger('click');
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('idle');
    await wrapper.get('.pet-character').trigger('dblclick');
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('wave');
    wrapper.findComponent(actor).vm.$emit('ended');
    await vi.advanceTimersByTimeAsync(221);
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('wave');
    await vi.advanceTimersByTimeAsync(2500);
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('idle');
    wrapper.unmount();
});

it('浮层包含在原生输入区域，关闭时恢复主体高度且不改动业务卡片尺寸', async () => {
    const wrapper = mount(PetStage, {
        props: { ...props(), contentHeight: 176 },
        global: { stubs: { PetActor: actor } },
    });
    await flushPromises();
    await wrapper.get('.pet-result-cloud').trigger('click');
    await flushPromises();
    const before = fake.call.mock.calls.filter(([c]) => c === 'set_pet_scene_layout').at(-1)![1].scene;
    await wrapper.setProps({ floatingBounds: [[20, 165, 250, 210]] });
    await flushPromises();
    const opened = fake.call.mock.calls.filter(([c]) => c === 'set_pet_scene_layout').at(-1)![1].scene;
    expect(opened.height).toBeGreaterThan(before.height);
    expect(opened.regions).toContainEqual([75, 278, 250, 210]);
    expect(opened.regions).toContainEqual([55, 113, 360, 176]);
    await wrapper.setProps({ floatingBounds: [] });
    await flushPromises();
    const closed = fake.call.mock.calls.filter(([c]) => c === 'set_pet_scene_layout').at(-1)![1].scene;
    expect(closed.height).toBe(before.height);
    wrapper.unmount();
});

it('拖动中结算后条件过期，松手不恢复旧庆祝或绿色通过', async () => {
    const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    await wrapper.setProps({ snapshot: state(true), busy: true });
    wrapper
        .get('.pet-character')
        .element.dispatchEvent(
            new MouseEvent('pointerdown', { button: 0, screenX: 10, screenY: 10, bubbles: true }),
        );
    wrapper
        .get('.pet-character')
        .element.dispatchEvent(new MouseEvent('pointermove', { screenX: 40, screenY: 10, bubbles: true }));
    await flushPromises();
    await wrapper.setProps({
        snapshot: state(),
        busy: false,
        latest: { id: 'task', status: 'passed' } as TestRecord,
    });
    await wrapper.setProps({ stale: true, latest: null });
    fake.release();
    await vi.advanceTimersByTimeAsync(2200);
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('idle');
    expect(wrapper.get('.pet-result-cloud').attributes('data-status')).toBe('stale');
    wrapper.unmount();
});

it('同一记录从通过变为错误时立即结束旧庆祝，角色与云朵保持同一真实状态', async () => {
    const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    await wrapper.setProps({ snapshot: state(true), busy: true });
    await wrapper.setProps({
        snapshot: state(),
        busy: false,
        latest: { id: 'task', status: 'passed' } as TestRecord,
    });
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('success');
    await wrapper.setProps({ latest: { id: 'task', status: 'error' } as TestRecord });
    expect(wrapper.get('.pet-result-cloud').attributes('data-status')).toBe('error');
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('error');
    await vi.advanceTimersByTimeAsync(1900);
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('error');
    wrapper.unmount();
});

it.each(['拖动中', '放下中'])('%s 同记录通过被错误替换后，松手回调不能复活旧庆祝', async (phase) => {
    const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    await wrapper.setProps({ snapshot: state(true), busy: true });
    wrapper
        .get('.pet-character')
        .element.dispatchEvent(
            new MouseEvent('pointerdown', { button: 0, screenX: 10, screenY: 10, bubbles: true }),
        );
    wrapper
        .get('.pet-character')
        .element.dispatchEvent(new MouseEvent('pointermove', { screenX: 40, screenY: 10, bubbles: true }));
    await flushPromises();
    await wrapper.setProps({
        snapshot: state(),
        busy: false,
        latest: { id: 'task', status: 'passed' } as TestRecord,
    });
    if (phase === '放下中') {
        fake.release();
        await flushPromises();
        expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('landing');
    }
    await wrapper.setProps({ latest: { id: 'task', status: 'error' } as TestRecord });
    if (phase === '拖动中') fake.release();
    await vi.advanceTimersByTimeAsync(321);
    expect(wrapper.get('.pet-result-cloud').attributes('data-status')).toBe('error');
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('error');
    wrapper.unmount();
});

it.each([
    ['passed', 'success'],
    ['failed', 'failed'],
    ['error', 'error'],
    ['running', 'working'],
] as const)('确认%s气泡后结束结果姿态，轮询和主动互动不能复活已确认表情', async (status, pose) => {
    const initial = props();
    initial.snapshot = state(status === 'running');
    initial.busy = status === 'running';
    initial.latest = status === 'running' ? null : ({ id: 'result-a', status } as TestRecord);
    const wrapper = mount(PetStage, {
        props: initial,
        attachTo: document.body,
        global: { stubs: { PetActor: actor } },
    });
    await flushPromises();
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe(pose);
    await wrapper.get('.pet-result-cloud').trigger('click');
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('seated');
    await wrapper.get('.pet-character').trigger('keydown', { key: 'Enter' });
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('idle');
    await wrapper.setProps({
        latest: initial.latest ? { ...initial.latest } : null,
        snapshot: state(status === 'running'),
    });
    await wrapper.get('.pet-character').trigger('click');
    await vi.advanceTimersByTimeAsync(221);
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('petting');
    await vi.advanceTimersByTimeAsync(1501);
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('idle');
    expect(wrapper.get('.pet-result-cloud').isVisible()).toBe(false);
    const character = wrapper.get('.pet-character');
    character.element.dispatchEvent(
        new MouseEvent('pointerdown', { button: 0, screenX: 10, screenY: 10, bubbles: true }),
    );
    character.element.dispatchEvent(
        new MouseEvent('pointermove', { screenX: 40, screenY: 10, bubbles: true }),
    );
    await flushPromises();
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('dragging');
    fake.release();
    await vi.advanceTimersByTimeAsync(321);
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('idle');
    // 确认仅影响本次呈现，下一次同结论记录（或任务）仍恢复真实业务姿态。
    const next = state(status === 'running');
    if (next.running) next.running.id = 'task-next';
    await wrapper.setProps({
        snapshot: next,
        latest: status === 'running' ? null : ({ id: 'result-b', status } as TestRecord),
    });
    await flushPromises();
    expect(wrapper.get('.pet-result-cloud').isVisible()).toBe(true);
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe(pose);
    wrapper.unmount();
});

it('离开动作区域延迟收起二级，进入子菜单取消收起，关闭父菜单不遗留计时器', async () => {
    const wrapper = mount(PetStage, {
        props: props(),
        attachTo: document.body,
        global: { stubs: { PetActor: actor } },
    });
    await flushPromises();
    await wrapper.get('.pet-character').trigger('contextmenu');
    await flushPromises();
    const group = wrapper.get('.pet-action-group');
    const trigger = wrapper.get('[aria-haspopup="menu"]');
    await trigger.trigger('pointerenter');
    await group.trigger('pointerleave');
    await vi.advanceTimersByTimeAsync(250);
    expect(wrapper.find('.pet-action-submenu').exists()).toBe(true);
    await wrapper.get('.pet-action-submenu').trigger('pointerenter');
    await vi.advanceTimersByTimeAsync(700);
    expect(wrapper.find('.pet-action-submenu').exists()).toBe(true);
    await group.trigger('pointerleave');
    await vi.advanceTimersByTimeAsync(650);
    expect(wrapper.find('.pet-action-submenu').exists()).toBe(false);
    expect(wrapper.find('[aria-label="桌宠操作"]').exists()).toBe(true);
    await trigger.trigger('pointerenter');
    await wrapper.get('[aria-label="桌宠操作"]').trigger('pointerleave');
    await vi.advanceTimersByTimeAsync(650);
    expect(wrapper.find('.pet-action-submenu').exists()).toBe(false);
    wrapper.unmount();
});

it.each(['left', 'right', 'top'])('%s侧手动动作播完才恢复贴边收纳，不被180ms计时抢占', async (side) => {
    const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    fake.dock({ side, tucked: false });
    await wrapper.get('.pet-character').trigger('contextmenu');
    await flushPromises();
    await wrapper.get('[aria-haspopup="menu"]').trigger('pointerenter');
    const wave = wrapper.findAll('.pet-action-submenu button').find((button) => button.text() === '招招手')!;
    await wave.trigger('click');
    await vi.advanceTimersByTimeAsync(600);
    expect(wrapper.attributes('data-docked')).toBe('false');
    expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('wave');
    await vi.advanceTimersByTimeAsync(2100);
    expect(wrapper.attributes('data-docked')).toBe('false');
    await vi.advanceTimersByTimeAsync(181);
    expect(wrapper.attributes('data-docked')).toBe('true');
    wrapper.unmount();
});

it.each([
    ['left', 176],
    ['right', 176],
    ['left', 596],
    ['right', 596],
] as const)('测试中%s侧菜单打开%i高卡片，退场期间主体轮廓必须在场景内', async (side, height) => {
    const rejected: unknown[] = [];
    fake.call.mockImplementation((command, args) => {
        if (command === 'get_pet_menu_bounds') return Promise.resolve([-1000, -200, 1400, 1000]);
        if (command === 'set_pet_scene_layout') {
            const s = args.scene;
            for (const r of [s.actorBounds, s.dockBounds, ...s.regions]) {
                if (
                    r.some((n: number) => !Number.isFinite(n) || n < 0) ||
                    r[0] + r[2] > s.width + 1 ||
                    r[1] + r[3] > s.height + 1
                ) {
                    rejected.push(s);
                    return Promise.reject(new Error('桌宠角色或吸附轮廓超出场景。'));
                }
            }
        }
        return Promise.resolve();
    });
    const wrapper = mount(PetStage, {
        props: {
            ...props(),
            busy: true,
            snapshot: state(true),
            contentExpanded: height === 596,
            contentHeight: height,
        },
        global: { stubs: { PetActor: actor } },
    });
    await flushPromises();
    fake.dock({ side, tucked: false });
    await wrapper.get('.pet-character').trigger('contextmenu');
    await flushPromises();
    await wrapper.get('[aria-label="桌宠操作"] > button').trigger('click');
    await flushPromises();
    expect(rejected).toEqual([]);
    expect(wrapper.find('.pet-character').exists()).toBe(true);
    expect(wrapper.emitted('error')).toBeUndefined();
    wrapper.unmount();
});

it('菜单等待宿主就绪才显示，二级展开关闭不改变窗口几何，取消不迟到重开', async () => {
    const wrapper = mount(PetStage, {
        props: props(),
        attachTo: document.body,
        global: { stubs: { PetActor: actor } },
    });
    await flushPromises();
    let finish!: () => void;
    fake.call.mockImplementation((command) =>
        command === 'get_pet_menu_bounds'
            ? Promise.resolve([-1000, -200, 1200, 900])
            : new Promise<void>((resolve) => {
                  finish = resolve;
              }),
    );
    await wrapper.get('.pet-character').trigger('contextmenu');
    await flushPromises();
    expect(wrapper.get('[aria-label="桌宠操作"]').isVisible()).toBe(false);
    const scene = fake.call.mock.calls.filter(([name]) => name === 'set_pet_scene_layout').at(-1)![1].scene;
    fake.call.mockResolvedValue(undefined);
    finish();
    await flushPromises();
    expect(wrapper.get('[aria-label="桌宠操作"]').isVisible()).toBe(true);
    await wrapper.get('[aria-haspopup="menu"]').trigger('pointerenter');
    await flushPromises();
    const next = fake.call.mock.calls.filter(([name]) => name === 'set_pet_scene_layout').at(-1)![1].scene;
    expect([next.width, next.height, next.anchor]).toEqual([scene.width, scene.height, scene.anchor]);
    await wrapper.get('[aria-label="桌宠动作"]').trigger('keydown', { key: 'Escape' });
    await flushPromises();
    expect(wrapper.find('[aria-label="桌宠动作"]').exists()).toBe(false);
    expect(wrapper.get('[aria-label="桌宠操作"]').attributes('data-submenu-side')).toBe('right');
    await wrapper.get('[aria-label="桌宠操作"]').trigger('keydown', { key: 'Escape' });
    let completeBounds!: (value: number[]) => void;
    fake.call.mockImplementation((command) =>
        command === 'get_pet_menu_bounds'
            ? new Promise<number[]>((resolve) => {
                  completeBounds = resolve;
              })
            : Promise.resolve(),
    );
    await wrapper.get('.pet-character').trigger('contextmenu');
    await wrapper.get('[aria-label="桌宠操作"]').trigger('keydown', { key: 'Escape' });
    completeBounds([-1000, -200, 1200, 900]);
    await flushPromises();
    expect(wrapper.find('[aria-label="桌宠操作"]').exists()).toBe(false);
    wrapper.unmount();
});

it('云朵点击确认隐藏、轮询不重现，新结果或新任务重新显示', async () => {
    const initial = props();
    initial.latest = { id: 'result-a', status: 'passed' } as TestRecord;
    const wrapper = mount(PetStage, {
        props: initial,
        attachTo: document.body,
        global: { stubs: { PetActor: actor } },
    });
    await flushPromises();
    const cloud = wrapper.get('.pet-result-cloud');
    expect(cloud.isVisible()).toBe(true);
    await cloud.trigger('click');
    await flushPromises();
    expect(cloud.isVisible()).toBe(false);
    await wrapper.setProps({ latest: { ...initial.latest! }, snapshot: state() });
    await flushPromises();
    expect(cloud.isVisible()).toBe(false);
    await wrapper.setProps({ latest: { id: 'result-b', status: 'passed' } as TestRecord });
    await flushPromises();
    expect(cloud.isVisible()).toBe(true);
    await cloud.trigger('click');
    await flushPromises();
    expect(cloud.isVisible()).toBe(false);
    await wrapper.setProps({ busy: true, snapshot: state(true) });
    await flushPromises();
    expect(cloud.isVisible()).toBe(true);
    expect(cloud.attributes('data-status')).toBe('running');
    wrapper.unmount();
});
it.each(['left', 'right'] as const)('角色在卡片%s侧时拖离边缘、到顶部均保留方向', async (side) => {
    const wrapper = mount(PetStage, { props: props(), global: { stubs: { PetActor: actor } } });
    await flushPromises();
    await wrapper.get('.pet-result-cloud').trigger('click');
    fake.dock({ side, tucked: false });
    await flushPromises();
    const expected = side === 'right';
    expect(wrapper.classes().includes('pet-edge-right')).toBe(expected);
    const character = wrapper.get('.pet-character');
    character.element.dispatchEvent(
        new MouseEvent('pointerdown', { button: 0, screenX: 10, screenY: 10, bubbles: true }),
    );
    character.element.dispatchEvent(
        new MouseEvent('pointermove', { screenX: 40, screenY: 10, bubbles: true }),
    );
    await flushPromises();
    fake.dock({ side: null, tucked: false });
    await flushPromises();
    fake.release();
    await flushPromises();
    expect(wrapper.classes().includes('pet-edge-right')).toBe(expected);
    fake.dock({ side: 'top', tucked: false });
    await flushPromises();
    expect(wrapper.classes().includes('pet-edge-right')).toBe(expected);
    // 只检查方向class会漏掉收纳往返；完整跑两次恢复并核对提交给原生的角色/卡片几何。
    const fullScene = () =>
        fake.call.mock.calls.filter(([command]) => command === 'set_pet_scene_layout').at(-1)![1].scene;
    const before = fullScene();
    for (let cycle = 0; cycle < 2; cycle++) {
        await vi.advanceTimersByTimeAsync(900);
        await finishDockMotion();
        expect(fullScene().docking).toBe('top');
        await moveIntoPeek(wrapper);
        await finishDockMotion();
        const restored = fullScene();
        expect(restored.docking).toBeNull();
        expect([restored.anchor, restored.actorBounds, restored.dockBounds]).toEqual([
            before.anchor,
            before.actorBounds,
            before.dockBounds,
        ]);
        expect(wrapper.classes().includes('pet-edge-right')).toBe(expected);
        expect(wrapper.get('.actor-stub').attributes('data-action')).toBe('seated');
        await wrapper.trigger('pointerleave');
    }
    wrapper.unmount();
});
it.each([false, true])('退出桌宠恢复经典窗口的真实展开状态=%s', async (expanded) => {
    const wrapper = mount(PetStage, {
        props: { ...props(), contentExpanded: expanded },
        global: { stubs: { PetActor: actor } },
    });
    await flushPromises();
    const next = state();
    next.config.preferences.skin = { ...next.config.preferences.skin!, id: 'classic' };
    await wrapper.setProps({ snapshot: next });
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith('clear_pet_scene', { expanded });
    wrapper.unmount();
});
