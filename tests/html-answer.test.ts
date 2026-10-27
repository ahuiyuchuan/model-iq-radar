/** 自动识别、沙箱生命周期、加载及统一放大交互回归；不调用真实模型。@author ahui */
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import cases from './fixtures/html-answers.json';
import { classifyAnswer, isHtmlRecord, previewAllowed } from '../src/htmlAnswer';
import HtmlPreview from '../src/components/HtmlPreview.vue';
import AnswerContent from '../src/components/AnswerContent.vue';
import HistoryAnswerPreview from '../src/components/HistoryAnswerPreview.vue';
import { preparePreview, previewMessage } from '../src/htmlSandbox';
import type { TestRecord } from '../src/types';

const fake = vi.hoisted(() => ({ call: vi.fn() }));
vi.mock('../src/bridge', () => ({ call: fake.call }));
beforeEach(() => {
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
        callback(performance.now());
        return 1;
    });
    fake.call
        .mockReset()
        .mockImplementation((cmd: string) =>
            Promise.resolve(
                cmd === 'prepare_html_preview'
                    ? `http://html-preview.localhost/${crypto.randomUUID()}`
                    : undefined,
            ),
        );
    HTMLDialogElement.prototype.showModal = function () {
        this.open = true;
    };
    HTMLDialogElement.prototype.close = function () {
        this.open = false;
    };
});
afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
    document.body.innerHTML = '';
});
it.each(cases)('$name 识别为 $kind', ({ answer, kind }) => {
    expect(classifyAnswer(answer).kind).toBe(kind);
});
it('代码块独立，不把外部说明或嵌套 SVG 再加入候选', () => {
    const result = classifyAnswer('```html\n<div>A</div>\n```\n```svg\n<svg></svg>\n```');
    expect(result.candidates.map((c) => c.source)).toEqual(['<div>A</div>', '<svg></svg>']);
    expect(classifyAnswer('```html\n<html><body>').candidates[0].issue).toContain('闭合');
});
it('旧历史仅展示不重判，错误/取消不执行部分 HTML', () => {
    const record = { status: 'passed', answer: '<div>ok</div>' } as TestRecord;
    expect(isHtmlRecord(record)).toBe(true);
    expect(record.status).toBe('passed');
    expect(isHtmlRecord({ ...record, answerType: 'text' })).toBe(false);
    expect(previewAllowed({ ...record, status: 'error' })).toBe(false);
    expect(previewAllowed({ ...record, status: 'cancelled' })).toBe(false);
});
it('iframe 只开放脚本，小预览点击放大且没有额外工具栏，卸载释放文档', async () => {
    const wrapper = mount(HtmlPreview, {
        props: { candidates: classifyAnswer('<div>demo</div>').candidates },
    });
    await flushPromises();
    const iframe = wrapper.get('iframe');
    expect(iframe.attributes('sandbox')).toBe('allow-scripts');
    expect(iframe.attributes('scrolling')).toBe('no');
    expect(iframe.attributes('style')).toContain('pointer-events: none');
    expect(wrapper.find('div').html()).not.toContain('<div>demo</div>');
    const url = iframe.attributes('src');
    expect(wrapper.find('[role="tablist"]').exists()).toBe(false);
    expect(wrapper.find('.html-preview-stage[role="button"]').exists()).toBe(true);
    expect(wrapper.text()).not.toContain('预览已停止');
    expect(fake.call.mock.calls.some(([cmd]) => /start_test|start_html_preview/.test(cmd))).toBe(false);
    wrapper.unmount();
    expect(fake.call).toHaveBeenCalledWith('release_html_preview', { url });
});
it('不完整 HTML 和失败响应只保留源码，禁止运行', async () => {
    const invalid = mount(HtmlPreview, {
        props: { candidates: classifyAnswer('```html\n<div>bad').candidates },
    });
    await flushPromises();
    expect(invalid.get('pre').text()).toBe('<div>bad');
    expect(invalid.emitted('failure')?.at(-1)?.[0]).toContain('未闭合');
    expect(fake.call).not.toHaveBeenCalled();
    invalid.unmount();
    const partial = mount(AnswerContent, {
        props: { source: '<div>partial</div>', answerType: 'html', executable: false },
    });
    expect(partial.find('pre').exists()).toBe(true);
    expect(fake.call).not.toHaveBeenCalled();
    partial.unmount();
});
it('保存为 HTML 但没有候选时仍显示原始源码，历史详情默认运行', async () => {
    const invalid = mount(AnswerContent, { props: { source: '历史迁移保留原文', answerType: 'html' } });
    expect(invalid.get('pre').text()).toBe('历史迁移保留原文');
    expect(fake.call).not.toHaveBeenCalled();
    invalid.unmount();
    const wrapper = mount(AnswerContent, { props: { source: '<div>history</div>', answerType: 'html' } });
    await flushPromises();
    expect(wrapper.find('iframe').exists()).toBe(true);
    wrapper.unmount();
});
it('切换记录后迟到文档立即释放，不能覆盖新记录', async () => {
    let settle!: (value: string) => void;
    fake.call.mockImplementation((cmd: string) =>
        cmd === 'prepare_html_preview'
            ? new Promise<string>((resolve) => {
                  settle = resolve;
              })
            : Promise.resolve(),
    );
    const wrapper = mount(HtmlPreview, {
        props: { candidates: classifyAnswer('<div>old</div>').candidates },
    });
    await flushPromises();
    const finish = settle;
    await wrapper.setProps({ candidates: classifyAnswer('<div>new</div>').candidates });
    finish('http://html-preview.localhost/old');
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith('release_html_preview', {
        url: 'http://html-preview.localhost/old',
    });
    expect(wrapper.find('iframe').exists()).toBe(false);
    wrapper.unmount();
    settle('http://html-preview.localhost/new');
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith('release_html_preview', {
        url: 'http://html-preview.localhost/new',
    });
});
it('放大复用宿主并发送交互锁；dialog 取消后恢复，卸载也恢复', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'performance'] });
    const wrapper = mount(HtmlPreview, {
        attachTo: document.body,
        props: { candidates: classifyAnswer('<div>demo</div>').candidates },
    });
    await flushPromises();
    const initialDoc = fake.call.mock.calls.find(([cmd]) => cmd === 'prepare_html_preview')![1].document;
    const token = initialDoc.match(/const token = "([^"]+)"/)![1];
    const ready = () =>
        window.dispatchEvent(
            new MessageEvent('message', {
                source: document.querySelector('iframe')?.contentWindow,
                data: { token, type: 'ready' },
            }),
        );
    ready();
    await vi.advanceTimersByTimeAsync(300);
    await flushPromises();
    expect(wrapper.find('.html-preview-spinner').exists()).toBe(false);
    await wrapper.get('.html-preview-stage').trigger('click');
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith('set_html_preview_expanded', { expanded: true });
    expect(wrapper.emitted('expandedChange')?.at(-1)).toEqual([true]);
    expect(document.querySelector('dialog')?.open).toBe(true);
    expect(document.querySelector('iframe')?.style.pointerEvents).toBe('auto');
    // 移动后即使尚未收到新的 load 事件，也不能在已完成两帧布局后提前隐藏等待反馈。
    expect(document.querySelector('.html-preview-spinner')).not.toBeNull();
    ready();
    await vi.advanceTimersByTimeAsync(300);
    await flushPromises();
    expect(document.querySelector('.html-preview-spinner')).toBeNull();
    await wrapper.get('dialog').trigger('cancel');
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith('set_html_preview_expanded', { expanded: false });
    expect(wrapper.emitted('expandedChange')?.at(-1)).toEqual([false]);
    wrapper.unmount();
});
it('消息必须同时匹配真实 frame 和会话令牌，不接受旧页面和伪造来源', () => {
    const frame = document.createElement('iframe');
    document.body.append(frame);
    expect(
        previewMessage(
            new MessageEvent('message', { source: frame.contentWindow, data: { token: 'current' } }),
            frame,
            'current',
        ),
    ).toBe(true);
    expect(
        previewMessage(
            new MessageEvent('message', { source: window, data: { token: 'current' } }),
            frame,
            'current',
        ),
    ).toBe(false);
    expect(
        previewMessage(
            new MessageEvent('message', { source: frame.contentWindow, data: { token: 'old' } }),
            frame,
            'current',
        ),
    ).toBe(false);
});
it('文档大小不截断，准备失败可重载，成功 ready 后清除旧错误', async () => {
    await expect(preparePreview('中'.repeat(400000))).rejects.toThrow('1 MiB');
    fake.call.mockRejectedValueOnce(new Error('准备失败'));
    const wrapper = mount(HtmlPreview, {
        attachTo: document.body,
        props: { candidates: classifyAnswer('<div>demo</div>').candidates },
    });
    await flushPromises();
    expect(wrapper.emitted('failure')?.at(-1)?.[0]).toContain('准备失败');
    await wrapper.get('.html-preview-error button').trigger('click');
    await flushPromises();
    const doc = fake.call.mock.calls.filter(([cmd]) => cmd === 'prepare_html_preview').at(-1)![1]
        .document as string;
    const token = doc.match(/const token = "([^"]+)"/)![1];
    window.dispatchEvent(
        new MessageEvent('message', {
            source: wrapper.get('iframe').element.contentWindow,
            data: { token, type: 'ready' },
        }),
    );
    await flushPromises();
    expect(wrapper.emitted('failure')?.at(-1)?.[0]).toBe('');
    wrapper.unmount();
});
it('加载在 ready 前保持动画，合法尺寸整体缩放，伪造尺寸和背景不能注入样式', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'performance'] });
    const wrapper = mount(HtmlPreview, {
        attachTo: document.body,
        props: { candidates: classifyAnswer('<div>demo</div>').candidates },
    });
    await flushPromises();
    const frame = wrapper.get('iframe').element;
    const doc = fake.call.mock.calls.find(([cmd]) => cmd === 'prepare_html_preview')![1].document;
    const token = doc.match(/const token = "([^"]+)"/)![1];
    const send = (type: string, value?: unknown) =>
        window.dispatchEvent(
            new MessageEvent('message', {
                source: frame.contentWindow,
                data: { token, type, value },
            }),
        );
    await wrapper.get('iframe').trigger('load');
    expect(wrapper.find('.html-preview-spinner').exists()).toBe(true);
    send('dimensions', { width: 1800, height: 1400 });
    await flushPromises();
    expect(frame.style.width).toBe('1800px');
    send('dimensions', { width: 2000.5, height: 1500 });
    send('background', 'url(https://example.invalid)');
    await flushPromises();
    expect(frame.style.width).toBe('1800px');
    expect(wrapper.get('.html-preview').attributes('style') ?? '').not.toContain('example.invalid');
    send('ready', 'rgb(240, 240, 240)');
    await vi.advanceTimersByTimeAsync(300);
    await flushPromises();
    expect(wrapper.find('.html-preview-spinner').exists()).toBe(false);
    expect(wrapper.get('.html-preview').attributes('style')).toContain('rgb(240, 240, 240)');
    send('dimensions', { width: 5000, height: 1400 });
    await flushPromises();
    expect(wrapper.find('iframe').exists()).toBe(false);
    expect(wrapper.text()).toContain('超过预览上限');
    wrapper.unmount();
});
it.each([0, 100, 500])('页面在 %dms 就绪时，动画至少 300ms 且慢加载不另加延时', async (readyAt) => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'performance'] });
    const wrapper = mount(HtmlPreview, {
        attachTo: document.body,
        props: { candidates: classifyAnswer('<div>timed</div>').candidates },
    });
    await flushPromises();
    const token = fake.call.mock.calls
        .find(([cmd]) => cmd === 'prepare_html_preview')![1]
        .document.match(/const token = "([^"]+)"/)![1];
    await vi.advanceTimersByTimeAsync(readyAt);
    expect(wrapper.find('.html-preview-spinner').exists()).toBe(true);
    window.dispatchEvent(
        new MessageEvent('message', {
            source: wrapper.get('iframe').element.contentWindow,
            data: { token, type: 'ready' },
        }),
    );
    await flushPromises();
    const remainder = Math.max(0, 300 - readyAt);
    if (remainder) {
        await vi.advanceTimersByTimeAsync(remainder - 1);
        expect(wrapper.find('.html-preview-spinner').exists()).toBe(true);
        await vi.advanceTimersByTimeAsync(1);
    }
    await flushPromises();
    expect(wrapper.find('.html-preview-spinner').exists()).toBe(false);
    wrapper.unmount();
    expect(vi.getTimerCount()).toBe(0);
});

it.each(['load', 'record'])('等待最短显示期间发生 %s，旧完成计时不能结束新加载', async (change) => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'performance'] });
    const wrapper = mount(HtmlPreview, {
        attachTo: document.body,
        props: { candidates: classifyAnswer('<div>old</div>').candidates },
    });
    await flushPromises();
    const token = fake.call.mock.calls
        .find(([cmd]) => cmd === 'prepare_html_preview')![1]
        .document.match(/const token = "([^"]+)"/)![1];
    window.dispatchEvent(
        new MessageEvent('message', {
            source: wrapper.get('iframe').element.contentWindow,
            data: { token, type: 'ready' },
        }),
    );
    await vi.advanceTimersByTimeAsync(100);
    if (change === 'load') await wrapper.get('iframe').trigger('load');
    else await wrapper.setProps({ candidates: classifyAnswer('<div>new</div>').candidates });
    await flushPromises();
    await vi.advanceTimersByTimeAsync(400);
    expect(wrapper.find('.html-preview-spinner').exists()).toBe(true);
    wrapper.unmount();
    expect(vi.getTimerCount()).toBe(0);
});

it('首帧等待超时只释放本次文档并提供重载，不改变测试结论或通知', async () => {
    vi.useFakeTimers();
    const wrapper = mount(HtmlPreview, {
        props: { candidates: classifyAnswer('<div>slow</div>').candidates },
    });
    await flushPromises();
    await vi.advanceTimersByTimeAsync(12000);
    expect(wrapper.find('iframe').exists()).toBe(false);
    expect(wrapper.text()).toContain('HTML 加载超时');
    expect(wrapper.find('.html-preview-error button').exists()).toBe(true);
    expect(fake.call.mock.calls.every(([cmd]) => /^(prepare|release)_html_preview$/.test(cmd))).toBe(true);
    wrapper.unmount();
});

it('历史仅可见行运行，放大期间滚出视口仍保留会话，关闭后释放', async () => {
    let visibility!: (entries: Array<{ isIntersecting: boolean }>) => void;
    vi.stubGlobal(
        'IntersectionObserver',
        class {
            constructor(callback: typeof visibility) {
                visibility = callback;
            }
            observe() {}
            disconnect() {}
        },
    );
    const record = {
        id: 'row',
        status: 'html',
        answer: '<div>history</div>',
        answerType: 'html',
    } as TestRecord;
    const wrapper = mount(HistoryAnswerPreview, { attachTo: document.body, props: { record } });
    await flushPromises();
    expect(fake.call).not.toHaveBeenCalled();
    visibility([{ isIntersecting: true }]);
    await flushPromises();
    await wrapper.get('.html-preview-stage').trigger('click');
    await flushPromises();
    visibility([{ isIntersecting: false }]);
    await flushPromises();
    expect(document.querySelector('dialog[open] iframe')).not.toBeNull();
    expect(wrapper.emitted('details')).toBeUndefined();
    await wrapper.get('dialog').trigger('cancel');
    await flushPromises();
    expect(document.querySelector('iframe')).toBeNull();
    expect(fake.call.mock.calls.filter(([cmd]) => cmd === 'release_html_preview')).toHaveLength(1);
    wrapper.unmount();
});

it('列表加载失败保留与卡片相同的重载入口，不需要先进入详情', async () => {
    let visibility!: (entries: Array<{ isIntersecting: boolean }>) => void;
    vi.stubGlobal(
        'IntersectionObserver',
        class {
            constructor(callback: typeof visibility) {
                visibility = callback;
            }
            observe() {}
            disconnect() {}
        },
    );
    fake.call.mockRejectedValueOnce(new Error('fixture load failure'));
    const record = {
        id: 'row',
        status: 'html',
        answer: '<div>history</div>',
        answerType: 'html',
    } as TestRecord;
    const wrapper = mount(HistoryAnswerPreview, { props: { record } });
    visibility([{ isIntersecting: true }]);
    await flushPromises();
    expect(wrapper.text()).toContain('fixture load failure');
    await wrapper.get('.html-preview-error button').trigger('click');
    await flushPromises();
    expect(wrapper.find('iframe').exists()).toBe(true);
    expect(wrapper.emitted('details')).toBeUndefined();
    wrapper.unmount();
});
