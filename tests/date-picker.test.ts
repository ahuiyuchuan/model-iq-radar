/**
 * 日期筛选的独立回归：应用内弹层、输入/清空、键盘、年界与生命周期。
 * 使用本机日历日和固定时钟；不联网，不触碰真实历史或配置。@author ahui
 */
import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import DatePicker from '../src/components/DatePicker.vue';

enableAutoUnmount(afterEach);
beforeEach(() => {
    vi.useFakeTimers({ toFake: ['Date'] });
    vi.setSystemTime(new Date(2026, 9, 25, 12));
});
afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
});

it.each(['Enter', ' ', 'F4', 'Alt+ArrowDown'])(
    '%s 打开应用内日历并聚焦当前日，Escape 关闭恢复输入焦点',
    async (key) => {
        const wrapper = mount(DatePicker, {
            props: { modelValue: '2026-10-25', label: '日期' },
            attachTo: document.body,
        });
        const input = wrapper.get<HTMLInputElement>('input');
        input.element.focus();
        expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
        await input.trigger('keydown', {
            key: key === 'Alt+ArrowDown' ? 'ArrowDown' : key,
            altKey: key === 'Alt+ArrowDown',
        });
        await flushPromises();
        expect(wrapper.get('[role="dialog"]').classes()).toContain('model-menu');
        expect(document.activeElement?.getAttribute('data-date')).toBe('2026-10-25');
        await wrapper.get('[data-date="2026-10-25"]').trigger('keydown', { key: 'Escape' });
        expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
        expect(document.activeElement).toBe(input.element);
        expect(wrapper.emitted('update:modelValue')).toBeUndefined();
    },
);

it('手动输入和清空仍可用，超出四位年范围的输入不发出筛选更新', async () => {
    const wrapper = mount(DatePicker, { props: { modelValue: '', label: '日期' } });
    const input = wrapper.get<HTMLInputElement>('input');
    await input.setValue('2024-02-29');
    await input.setValue('10000-01-01');
    await input.setValue('');
    expect(wrapper.emitted('update:modelValue')).toEqual([['2024-02-29'], ['']]);
});

it('不截获 Alt+F4/Ctrl+F4 等宿主窗口快捷键', async () => {
    const wrapper = mount(DatePicker, { props: { modelValue: '', label: '日期' } });
    const input = wrapper.get('input');
    for (const modifiers of [{ altKey: true }, { ctrlKey: true }, { metaKey: true }]) {
        const event = new KeyboardEvent('keydown', {
            key: 'F4',
            bubbles: true,
            cancelable: true,
            ...modifiers,
        });
        input.element.dispatchEvent(event);
        expect(event.defaultPrevented).toBe(false);
    }
    expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
});

it('点击选日只提交一个本机 ISO 日期，清空与今天不进行 UTC 转换', async () => {
    const wrapper = mount(DatePicker, { props: { modelValue: '2026-10-25', label: '日期' } });
    const input = wrapper.get('input');
    await input.trigger('click');
    await wrapper.get('[data-date="2026-11-01"]').trigger('click');
    expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
    await input.trigger('click');
    await wrapper.findAll('.date-picker-footer button')[0].trigger('click');
    await input.trigger('click');
    await wrapper.findAll('.date-picker-footer button')[1].trigger('click');
    expect(wrapper.emitted('update:modelValue')).toEqual([['2026-11-01'], [''], ['2026-10-25']]);
});

it('方向键跨月、翻月保留日期且月底夹取，Shift 翻年正确处理闰日', async () => {
    const wrapper = mount(DatePicker, {
        props: { modelValue: '2024-01-31', label: '日期' },
        attachTo: document.body,
    });
    await wrapper.get('input').trigger('keydown', { key: 'Enter' });
    await wrapper.get('[data-date="2024-01-31"]').trigger('keydown', { key: 'PageDown' });
    expect(document.activeElement?.getAttribute('data-date')).toBe('2024-02-29');
    await wrapper.get('[data-date="2024-02-29"]').trigger('keydown', {
        key: 'PageDown',
        shiftKey: true,
    });
    expect(document.activeElement?.getAttribute('data-date')).toBe('2025-02-28');
    await wrapper.get('[data-date="2025-02-28"]').trigger('keydown', { key: 'ArrowRight' });
    expect(document.activeElement?.getAttribute('data-date')).toBe('2025-03-01');
    expect(wrapper.emitted('update:modelValue')).toBeUndefined();
});

it.each(['0001-01-01', '9999-12-31'])(
    '%s 年界禁用越界翻页与日期，键盘不会选择范围外的日期',
    async (value) => {
        const wrapper = mount(DatePicker, { props: { modelValue: value, label: '日期' } });
        await wrapper.get('input').trigger('click');
        const earliest = value.startsWith('0001');
        const prefix = earliest ? '上一' : '下一';
        expect(wrapper.get(`[aria-label="${prefix}年"]`).attributes('disabled')).toBeDefined();
        expect(wrapper.get(`[aria-label="${prefix}月"]`).attributes('disabled')).toBeDefined();
        expect(wrapper.findAll('.date-picker-day:disabled').length).toBeGreaterThan(0);
        await wrapper.get(`[data-date="${value}"]`).trigger('keydown', {
            key: earliest ? 'ArrowLeft' : 'ArrowRight',
        });
        expect(wrapper.get('[role="grid"]').attributes('aria-label')).toBe(
            earliest ? '1年1月' : '9999年12月',
        );
        expect(wrapper.emitted('update:modelValue')).toBeUndefined();
    },
);

it('外部点击、离焦、窗口缩放和外层滚动关闭，内部滚动不关闭；禁用后不能打开', async () => {
    const wrapper = mount(DatePicker, { props: { modelValue: '', label: '日期' } });
    const input = wrapper.get('input');
    for (const event of ['pointerdown', 'blur', 'resize', 'scroll']) {
        await input.trigger('click');
        await wrapper.get('[role="dialog"]').trigger('scroll');
        expect(wrapper.find('[role="dialog"]').exists()).toBe(true);
        if (event === 'blur' || event === 'resize') window.dispatchEvent(new Event(event));
        else document.dispatchEvent(new Event(event));
        await flushPromises();
        expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
    }
    await input.trigger('click');
    await input.trigger('focusout', { relatedTarget: document.body });
    expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
    await input.trigger('click');
    await wrapper.setProps({ disabled: true });
    expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
    await input.trigger('click');
    expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
});

it('翻到年份边界时把焦点交给日期，导航按钮禁用不会误关闭日历', async () => {
    const wrapper = mount(DatePicker, {
        props: { modelValue: '0002-01-01', label: '日期' },
        attachTo: document.body,
    });
    await wrapper.get('input').trigger('click');
    const previousYear = wrapper.get<HTMLButtonElement>('[aria-label="上一年"]');
    previousYear.element.focus();
    await previousYear.trigger('click');
    await flushPromises();
    expect(wrapper.find('[role="dialog"]').exists()).toBe(true);
    expect(previousYear.element.disabled).toBe(true);
    expect(document.activeElement?.getAttribute('data-date')).toBe('0001-01-01');
});

it('上下两侧空间都不足时在滚动主体内移位，完整日历不越过固定底栏', async () => {
    const host = document.createElement('div');
    host.className = 'settings-scroll';
    document.body.append(host);
    const rectangle = (left: number, top: number, width: number, height: number): DOMRect => ({
        left,
        top,
        width,
        height,
        right: left + width,
        bottom: top + height,
        x: left,
        y: top,
        toJSON: () => ({}),
    });
    vi.spyOn(host, 'getBoundingClientRect').mockReturnValue(rectangle(150, 44, 550, 406));
    const wrapper = mount(DatePicker, { props: { modelValue: '', label: '日期' }, attachTo: host });
    vi.spyOn(wrapper.element, 'getBoundingClientRect').mockReturnValue(rectangle(190, 230, 156, 32));
    await wrapper.get('input').trigger('click');
    const style = wrapper.get<HTMLElement>('[role="dialog"]').element.style;
    const top = 230 + parseFloat(style.top);
    const height = parseFloat(style.maxHeight);
    expect(height).toBe(304);
    expect(top).toBeGreaterThanOrEqual(44 + 8);
    expect(top + height).toBeLessThanOrEqual(450 - 8);
    wrapper.unmount();
    host.remove();
});

it('右侧和底部空间不足时弹层向左/上避让，卸载移除全部全局监听', async () => {
    const wrapper = mount(DatePicker, { props: { modelValue: '', label: '日期' } });
    vi.spyOn(wrapper.element, 'getBoundingClientRect').mockReturnValue({
        left: 900,
        right: 1000,
        top: 700,
        bottom: 732,
        width: 100,
        height: 32,
        x: 900,
        y: 700,
        toJSON: () => ({}),
    });
    const removeDocument = vi.spyOn(document, 'removeEventListener');
    const removeWindow = vi.spyOn(window, 'removeEventListener');
    await wrapper.get('input').trigger('click');
    const popup = wrapper.get<HTMLElement>('[role="dialog"]');
    expect(parseFloat(popup.element.style.top)).toBeLessThan(0);
    expect(parseFloat(popup.element.style.left)).toBeLessThan(0);
    expect(parseFloat(popup.element.style.maxHeight)).toBeGreaterThan(0);
    wrapper.unmount();
    for (const event of ['pointerdown', 'scroll'])
        expect(removeDocument.mock.calls.some(([name]) => name === event)).toBe(true);
    for (const event of ['blur', 'resize'])
        expect(removeWindow.mock.calls.some(([name]) => name === event)).toBe(true);
});
