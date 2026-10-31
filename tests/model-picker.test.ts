/**
 * 选择器异步尺寸与列表边界回归。仅 Mock 本机窗口尺寸接口，不创建原生窗口。
 * @author ahui
 */
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, expect, it, vi } from 'vitest';
import ModelPicker from '../src/components/ModelPicker.vue';

const fake = vi.hoisted(() => ({ resize: vi.fn() }));
vi.mock('../src/bridge', () => ({ resizeWidget: fake.resize }));

const profiles = [
    { id: 'one', name: '分组一', model: 'model-one' },
    { id: 'two', name: '分组二', model: 'model-two' },
];
beforeEach(() => {
    fake.resize.mockReset().mockResolvedValue(undefined);
});

it('桌宠场景菜单由宿主测量，开关不再自行调整卡片或经典窗口高度', async () => {
    const wrapper = mount(ModelPicker, {
        props: {
            profiles,
            selectedId: 'one',
            expanded: false,
            disabled: false,
            context: 'scene',
            prefix: '接口',
        },
    });
    expect(wrapper.get('.model-prefix').text()).toBe('接口');
    await wrapper.get('[role="combobox"]').trigger('click');
    await flushPromises();
    expect(wrapper.find('[role="listbox"]').exists()).toBe(true);
    expect(fake.resize).not.toHaveBeenCalled();
    await wrapper.get('[role="combobox"]').trigger('keydown', { key: 'Escape' });
    await flushPromises();
    expect(wrapper.find('[role="listbox"]').exists()).toBe(false);
    expect(fake.resize).not.toHaveBeenCalled();
    expect(wrapper.emitted('openChange')).toEqual([[true], [false]]);
    wrapper.unmount();
});

it('模型菜单左侧保留标题与规则，强度独立列不重复思考前缀，选中图标单独占位', async () => {
    const wrapper = mount(ModelPicker, {
        props: {
            profiles,
            selectedId: 'two',
            expanded: false,
            disabled: false,
            context: 'settings',
            summaries: {
                one: { rule: '日本首相测试', effort: 'low' },
                two: { rule: '糖果测试', effort: 'medium' },
            },
        },
    });
    await wrapper.get('[role="combobox"]').trigger('click');
    await flushPromises();
    const rows = wrapper.findAll('.model-option');
    expect(rows.map((row) => row.get('.option-effort').text())).toEqual(['low', 'medium']);
    expect(rows.map((row) => row.get('.option-copy small').text())).toEqual(['日本首相测试', '糖果测试']);
    expect(rows.every((row) => row.classes().includes('has-summary'))).toBe(true);
    expect(rows[1].get('svg').element.previousElementSibling?.className).toBe('option-effort');
    expect(rows[1].attributes('title')).toContain('思考：medium');
    wrapper.unmount();
});

it('可编辑模型列表保留空格、光标和输入法，Enter 才选择候选且失焦不改值', async () => {
    const wrapper = mount(ModelPicker, {
        props: {
            profiles,
            selectedId: 'custom',
            modelValue: '',
            editable: true,
            context: 'settings',
            expanded: false,
            disabled: false,
        },
    });
    const input = wrapper.get('input');
    await input.trigger('click');
    expect(wrapper.find('[role="listbox"]').exists()).toBe(true);
    for (const event of [
        { key: ' ' },
        { key: 'Home' },
        { key: 'End' },
        { key: 'ArrowDown', shiftKey: true },
        { key: 'Enter', isComposing: true },
    ]) {
        const native = new KeyboardEvent('keydown', { ...event, cancelable: true, bubbles: true });
        input.element.dispatchEvent(native);
        await flushPromises();
        expect(native.defaultPrevented).toBe(false);
        expect(wrapper.emitted('update:modelValue')).toBeUndefined();
    }
    await input.trigger('keydown', { key: 'ArrowDown' });
    await input.trigger('keydown', { key: 'Enter' });
    await flushPromises();
    expect(wrapper.emitted('update:modelValue')).toEqual([['two']]);
    expect(wrapper.emitted('select')).toBeUndefined();
    await wrapper.setProps({ modelValue: 'custom-id' });
    await input.trigger('blur');
    expect(wrapper.emitted('update:modelValue')).toHaveLength(1);
    wrapper.unmount();
});

it('可编辑搜索只筛选展示，不删除缓存，Shift+Tab 关闭候选并保持自定义输入', async () => {
    const wrapper = mount(ModelPicker, {
        props: {
            profiles,
            selectedId: 'custom',
            modelValue: 'two',
            editable: true,
            context: 'settings',
            expanded: false,
            disabled: false,
        },
    });
    // 完整命中已缓存 ID 时展示所有项，便于直接换模型。
    await wrapper.get('input').trigger('click');
    expect(wrapper.findAll('[role="option"]')).toHaveLength(2);
    await wrapper.get('input').setValue('model-o');
    expect(wrapper.findAll('[role="option"]')).toHaveLength(1);
    await wrapper.get('input').setValue('');
    expect(wrapper.findAll('[role="option"]')).toHaveLength(2);
    await wrapper.get('input').trigger('keydown', { key: 'Tab', shiftKey: true });
    await flushPromises();
    expect(wrapper.find('[role="listbox"]').exists()).toBe(false);
    expect(wrapper.emitted('update:modelValue')).toEqual([['model-o'], ['']]);
    wrapper.unmount();
});

it('未保存草稿使用独立提示，鼠标和键盘只会选择真实列表项', async () => {
    const wrapper = mount(ModelPicker, {
        props: {
            profiles,
            selectedId: 'draft',
            placeholder: '新建规则预设（未保存）',
            showModel: false,
            context: 'settings',
            expanded: false,
            disabled: false,
        },
    });
    const trigger = wrapper.get('[role="combobox"]');
    expect(trigger.text()).toBe('新建规则预设（未保存）');
    await trigger.trigger('click');
    expect(wrapper.findAll('[role="option"]').map((option) => option.text())).toEqual([
        '分组一model-one',
        '分组二model-two',
    ]);
    expect(wrapper.findAll('[aria-selected="true"]')).toHaveLength(0);
    await trigger.trigger('keydown', { key: 'Enter' });
    await flushPromises();
    expect(wrapper.emitted('select')).toEqual([['one']]);
    expect(fake.resize).not.toHaveBeenCalled();
    wrapper.unmount();
});

it.each(['disabled', 'Escape', 'blur'] as const)(
    '原生打开尺寸尚未返回时去重，%s 会取消菜单并恢复卡片尺寸',
    async (action) => {
        let finishOpening: () => void = () => {};
        fake.resize.mockImplementation(async (_expanded: boolean, height?: number) => {
            if (height)
                await new Promise<void>((resolve) => {
                    finishOpening = resolve;
                });
        });
        const wrapper = mount(ModelPicker, {
            props: { profiles, selectedId: 'one', expanded: false, disabled: false },
        });
        await wrapper.get('[role="combobox"]').trigger('click');
        await wrapper.get('[role="combobox"]').trigger('click');
        expect(fake.resize).toHaveBeenCalledTimes(1);
        if (action === 'disabled') await wrapper.setProps({ disabled: true });
        else if (action === 'Escape')
            await wrapper.get('[role="combobox"]').trigger('keydown', { key: 'Escape' });
        else window.dispatchEvent(new Event('blur'));
        expect(fake.resize).toHaveBeenLastCalledWith(false);
        finishOpening();
        await flushPromises();
        expect(wrapper.find('[role="listbox"]').exists()).toBe(false);
        wrapper.unmount();
    },
);

it('等待关闭尺寸期间开始测试，不再发出旧配置选择或重复选择', async () => {
    const wrapper = mount(ModelPicker, {
        props: { profiles, selectedId: 'one', expanded: false, disabled: false },
    });
    await wrapper.get('[role="combobox"]').trigger('click');
    await flushPromises();
    let finishClosing: () => void = () => {};
    fake.resize.mockImplementation(
        () =>
            new Promise<void>((resolve) => {
                finishClosing = resolve;
            }),
    );
    await wrapper.findAll('[role="option"]')[1].trigger('click');
    await wrapper.setProps({ disabled: true });
    finishClosing();
    await flushPromises();
    expect(wrapper.emitted('select')).toBeUndefined();
    wrapper.unmount();
});

it('后台缩短列表会约束键盘高亮，空列表关闭菜单且不产生无效选择', async () => {
    const wrapper = mount(ModelPicker, {
        props: { profiles, selectedId: 'one', expanded: false, disabled: false, context: 'settings' },
    });
    const trigger = wrapper.get('[role="combobox"]');
    await trigger.trigger('click');
    await trigger.trigger('keydown', { key: 'End' });
    expect(trigger.attributes('aria-activedescendant')).toMatch(/option-1$/);
    await wrapper.setProps({ profiles: [profiles[0]] });
    expect(trigger.attributes('aria-activedescendant')).toMatch(/option-0$/);
    await wrapper.setProps({ profiles: [] });
    await flushPromises();
    expect(wrapper.find('[role="listbox"]').exists()).toBe(false);
    expect(trigger.attributes('aria-activedescendant')).toBeUndefined();
    wrapper.unmount();
});

/** 异步场景扩窗期间菜单只能隐藏测量，关闭/迟到回调不允许重新出画。 */
it.each([false, true])('场景菜单等待窗口就绪；等待期间取消=%s', async (cancel) => {
    let finish!: () => void;
    const prepare = vi.fn(
        () =>
            new Promise<void>((resolve) => {
                finish = resolve;
            }),
    );
    const wrapper = mount(ModelPicker, {
        props: {
            profiles,
            selectedId: 'one',
            expanded: false,
            disabled: false,
            context: 'scene',
            prepareSceneMenu: prepare,
        },
    });
    await wrapper.get('[role="combobox"]').trigger('click');
    await flushPromises();
    expect(prepare).toHaveBeenCalledOnce();
    expect((wrapper.get('.model-menu').element as HTMLElement).style.visibility).toBe('hidden');
    if (cancel) await wrapper.get('[role="combobox"]').trigger('keydown', { key: 'Escape' });
    finish();
    await flushPromises();
    expect(wrapper.find('.model-menu').exists()).toBe(!cancel);
    if (!cancel) expect((wrapper.get('.model-menu').element as HTMLElement).style.visibility).toBe('');
    wrapper.unmount();
});
it('场景扩窗失败关闭隐藏菜单并交回错误', async () => {
    const wrapper = mount(ModelPicker, {
        props: {
            profiles,
            selectedId: 'one',
            expanded: false,
            disabled: false,
            context: 'scene',
            prepareSceneMenu: async () => {
                throw new Error('扩窗失败');
            },
        },
    });
    await wrapper.get('[role="combobox"]').trigger('click');
    await flushPromises();
    expect(wrapper.find('.model-menu').exists()).toBe(false);
    expect(String(wrapper.emitted('error'))).toContain('扩窗失败');
    wrapper.unmount();
});
