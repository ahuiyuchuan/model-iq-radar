/** 图片预览仅使用本地Mock；验证按需读取、防重、分页及迟到结果隔离，不访问真实服务。@author ahui */
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, expect, it, vi } from 'vitest';
import PromptImagePreview from '../src/components/PromptImagePreview.vue';

const fake = vi.hoisted(() => ({ call: vi.fn() }));
vi.mock('../src/bridge', () => ({ call: fake.call }));
const png = 'data:image/png;base64,aGVsbG8=';
beforeEach(() => {
    fake.call.mockReset();
});

it.each([{ prompt: '短题目' }, { recordId: 'single-image-record' }])(
    '单张图片隐藏分页但保留尺寸切换：%j',
    async (props) => {
        fake.call.mockResolvedValue([png]);
        const wrapper = mount(PromptImagePreview, {
            props: { prompt: props.prompt, recordId: props.recordId },
        });
        await wrapper.get('button').trigger('click');
        await flushPromises();
        expect(wrapper.text()).not.toContain('上一页');
        expect(wrapper.text()).not.toContain('下一页');
        expect(wrapper.text()).not.toContain('第 1 / 1 页');
        expect(wrapper.get('img').attributes('src')).toBe(png);
        await wrapper
            .findAll('button')
            .find((item) => item.text() === '原始尺寸')!
            .trigger('click');
        expect(wrapper.get('img').classes()).not.toContain('fit-width');
        wrapper.unmount();
    },
);

it('草稿预览按需防重、保留完整题目、分页和尺寸切换，不保存或测试', async () => {
    let finish!: (value: string[]) => void;
    fake.call.mockImplementation(() => new Promise<string[]>((resolve) => (finish = resolve)));
    const wrapper = mount(PromptImagePreview, { props: { prompt: ' 原文\n题目 ' } });
    expect(fake.call).not.toHaveBeenCalled();
    await Promise.all([wrapper.get('button').trigger('click'), wrapper.get('button').trigger('click')]);
    expect(fake.call).toHaveBeenCalledTimes(1);
    expect(fake.call).toHaveBeenCalledWith('preview_prompt_images', { prompt: ' 原文\n题目 ' });
    finish([png, 'data:image/png;base64,d29ybGQ=']);
    await flushPromises();
    expect(wrapper.get('img').attributes('src')).toBe(png);
    await wrapper
        .findAll('button')
        .find((item) => item.text() === '下一页')!
        .trigger('click');
    expect(wrapper.text()).toContain('第 2 / 2 页');
    await wrapper
        .findAll('button')
        .find((item) => item.text() === '原始尺寸')!
        .trigger('click');
    expect(wrapper.get('img').classes()).not.toContain('fit-width');
    await wrapper.setProps({ prompt: '换题后不能展示旧图' });
    expect(wrapper.find('img').exists()).toBe(false);
    wrapper.unmount();
});

it('历史只读取稳定记录原图，旧记录无图不重新生成，失败可重试', async () => {
    fake.call
        .mockResolvedValueOnce([])
        .mockRejectedValueOnce(new Error('已清理'))
        .mockResolvedValueOnce([png]);
    const wrapper = mount(PromptImagePreview, { props: { recordId: 'saved-record' } });
    await wrapper.get('button').trigger('click');
    await flushPromises();
    expect(wrapper.text()).toContain('未保存请求原图');
    await wrapper.get('button').trigger('click');
    await flushPromises();
    expect(wrapper.text()).toContain('已清理');
    await wrapper.get('button').trigger('click');
    await flushPromises();
    expect(wrapper.find('img').exists()).toBe(true);
    expect(fake.call.mock.calls.every(([command]) => command === 'get_prompt_images')).toBe(true);
    expect(fake.call).toHaveBeenCalledWith('get_prompt_images', { recordId: 'saved-record' });
    await wrapper.get('img').trigger('error');
    expect(wrapper.text()).toContain('图片无法显示');
    wrapper.unmount();
});

it('多页末页解码失败后重试单张图片，从首页显示而不沿用失效页码', async () => {
    fake.call.mockResolvedValueOnce([png, 'data:image/png;base64,d29ybGQ=']).mockResolvedValueOnce([png]);
    const wrapper = mount(PromptImagePreview, { props: { recordId: 'retry-record' } });
    await wrapper.get('button').trigger('click');
    await flushPromises();
    await wrapper
        .findAll('button')
        .find((item) => item.text() === '下一页')!
        .trigger('click');
    await wrapper.get('img').trigger('error');
    expect(wrapper.text()).toContain('图片无法显示');
    await wrapper.get('button').trigger('click');
    await flushPromises();
    expect(wrapper.get('img').attributes('src')).toBe(png);
    expect(wrapper.get('img').attributes('alt')).toBe('题目图片，第1页，共1页');
    expect(wrapper.text()).not.toContain('下一页');
    expect(wrapper.text()).not.toContain('图片无法显示');
    wrapper.unmount();
});

it('换题、收起或卸载丢弃迟到图片，拒绝外链图片', async () => {
    let finish!: (value: string[]) => void;
    fake.call.mockImplementationOnce(() => new Promise<string[]>((resolve) => (finish = resolve)));
    const wrapper = mount(PromptImagePreview, { props: { prompt: '第一题' } });
    await wrapper.get('button').trigger('click');
    await wrapper.setProps({ prompt: '第二题' });
    finish([png]);
    await flushPromises();
    expect(wrapper.find('img').exists()).toBe(false);
    fake.call.mockResolvedValue(['https://example.com/tracker.png']);
    await wrapper.get('button').trigger('click');
    await flushPromises();
    expect(wrapper.text()).toContain('图片数据无效');
    expect(wrapper.find('img').exists()).toBe(false);
    wrapper.unmount();
});
