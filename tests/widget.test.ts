/** 回答安全展示、原文复制和悬浮状态回归；界面故障与模型未通过保持独立。@author ahui */
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import Widget from '../src/components/Widget.vue';
import type { AppSnapshot } from '../src/types';

const fake = vi.hoisted(() => ({
    call: vi.fn(),
    subscribe: vi.fn(),
    resize: vi.fn(),
    subscribeDock: vi.fn(),
    snapshot: {} as AppSnapshot,
}));
vi.mock('../src/bridge', () => ({
    call: fake.call,
    subscribe: fake.subscribe,
    dragWindow: vi.fn(),
    openSettings: vi.fn(),
    resizeWidget: fake.resize,
    subscribeDock: fake.subscribeDock,
}));

beforeEach(() => {
    // 布局用例直接完成浏览器帧；动画时序用例单独换成可控的帧队列。
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
        callback(performance.now());
        return 1;
    });
    fake.snapshot = {
        config: {
            profiles: [
                {
                    id: 'one',
                    name: '本地模型',
                    model: 'fixture',
                    baseUrl: 'http://localhost/v1',
                    hasKey: false,
                },
            ],
            selectedProfileId: 'one',
            rule: { prompt: '问题', keyword: '答案' },
            preferences: {
                alwaysOnTop: true,
                snapToEdge: true,
                opacity: 97,
                timeoutSeconds: 60,
                shortcutEnabled: true,
                autoTestEnabled: true,
                autoTestIntervalMinutes: 3,
            },
        },
        history: [],
        running: null,
        autoTestRemainingMs: 180000,
    };
    fake.call.mockReset().mockImplementation(async () => structuredClone(fake.snapshot));
    fake.subscribe.mockReset().mockResolvedValue(() => {});
    fake.resize.mockReset().mockResolvedValue(undefined);
    fake.subscribeDock.mockReset().mockResolvedValue(() => {});
});

afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
});

it('Markdown 只改变显示，复制仍为原文且状态仍使用后端结论', async () => {
    const source = '**高市**早苗\n\n```js\nconst n = 1;\n```';
    // 渲染后文本可连成关键词，后端 failed 仍必须保持，不在前端重新匹配。
    fake.snapshot.history.push({
        id: 'markdown',
        profileId: 'one',
        profileName: '本地模型',
        model: 'fixture',
        startedAt: '2026-10-23T00:00:00Z',
        durationMs: 20,
        status: 'failed',
        answer: source,
        error: null,
        prompt: '问题',
        keyword: '高市早苗',
        ruleVersion: 'literal-contains-v1',
    });
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } });
    const wrapper = mount(Widget);
    await flushPromises();
    await wrapper.get('[aria-label="展开测试详情"]').trigger('click');
    await flushPromises();
    expect(wrapper.get('h1').text()).toBe('降智');
    expect(wrapper.get('.answer-box strong').text()).toBe('高市');
    await wrapper.get('.expanded-actions .button').trigger('click');
    expect(writeText).toHaveBeenCalledWith(source);
    expect(fake.snapshot.history[0].answer).toBe(source);
    wrapper.unmount();
});

it('顶部启动自动收起，恢复先隐藏图标且不会重复提交窗口调整', async () => {
    vi.useFakeTimers();
    fake.subscribeDock.mockImplementation(async (handler) => {
        handler({ side: 'top', tucked: false });
        return () => {};
    });
    const frames: FrameRequestCallback[] = [];
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
        frames.push(callback);
        return frames.length;
    });
    const wrapper = mount(Widget);
    await flushPromises();
    await vi.advanceTimersByTimeAsync(249);
    expect(wrapper.find('.dock-away').exists()).toBe(false);
    await vi.advanceTimersByTimeAsync(1);
    expect(wrapper.find('.dock-away').exists()).toBe(true);
    await vi.advanceTimersByTimeAsync(180);
    expect(fake.resize).toHaveBeenCalledWith(false, 0, true);
    const icon = wrapper.get('.edge-tab');
    await icon.trigger('mouseenter');
    await icon.trigger('click');
    expect(wrapper.get('.edge-tab').classes()).toContain('edge-restoring');
    expect(fake.resize).toHaveBeenCalledTimes(1);
    frames.shift()?.(performance.now());
    await flushPromises();
    expect(wrapper.get('.widget').classes()).toContain('dock-top');
    expect(wrapper.get('.widget').classes()).toContain('dock-away');
    expect(fake.resize).toHaveBeenCalledTimes(2);
    frames.shift()?.(performance.now());
    frames.shift()?.(performance.now());
    await flushPromises();
    expect(wrapper.get('.widget').classes()).not.toContain('dock-away');
    // 返回面板可取消下一次收起，250ms 等待内快速移出再移入也不能误收。
    await wrapper.get('.widget').trigger('mouseenter');
    await wrapper.get('.widget').trigger('mouseleave');
    await vi.advanceTimersByTimeAsync(200);
    await wrapper.get('.widget').trigger('mouseenter');
    await vi.advanceTimersByTimeAsync(600);
    expect(fake.resize).toHaveBeenCalledTimes(2);
    wrapper.unmount();
    expect(vi.getTimerCount()).toBe(0);
});

it('两态同步模型名、测试时间、耗时、倒计时和主按钮，关闭定时后隐藏倒计时', async () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval', 'performance'] });
    fake.snapshot.history.push({
        id: 'timing',
        profileId: 'one',
        profileName: '本地模型',
        model: 'fixture',
        startedAt: '2026-10-23T00:00:00Z',
        durationMs: 4640,
        status: 'passed',
        answer: '答案',
        error: null,
        prompt: '问题',
        keyword: '答案',
        ruleVersion: 'literal-contains-v1',
    });
    const wrapper = mount(Widget);
    await flushPromises();
    expect(wrapper.get('[role="combobox"]').text()).toContain('本地模型 : fixture');
    expect(wrapper.get('.compact-primary').text()).toBe('重新测试');
    expect(wrapper.get('.compact-timing').text()).toContain('08:00:00');
    expect(wrapper.get('.compact-timing').text()).toContain('4.64s');
    expect(wrapper.get('.auto-countdown').text()).toContain('03:00');
    await vi.advanceTimersByTimeAsync(2000);
    expect(wrapper.get('.auto-countdown').text()).toContain('02:58');
    await wrapper.get('[aria-label="展开测试详情"]').trigger('click');
    await flushPromises();
    expect(wrapper.get('.expanded-actions .button:last-child').text()).toBe('重新测试');
    expect(wrapper.get('.metadata').text()).toContain('4.64s');
    expect(wrapper.get('.metadata .auto-countdown').text()).toContain('02:58');
    expect(wrapper.find('.record-model').exists()).toBe(false);
    fake.snapshot.config.preferences.autoTestEnabled = false;
    fake.subscribe.mock.calls[0][0]();
    await flushPromises();
    expect(wrapper.find('.auto-countdown').exists()).toBe(false);
    await wrapper.get('[aria-label="收起结果"]').trigger('click');
    await flushPromises();
    expect(wrapper.find('.auto-countdown').exists()).toBe(false);
    wrapper.unmount();
});

it('设置和展开收起始终在右上角，箭头随状态变化', async () => {
    const wrapper = mount(Widget);
    await flushPromises();
    const expand = wrapper.get('.widget-tools [aria-label="展开测试详情"]');
    expect(expand.attributes('aria-expanded')).toBe('false');
    expect(wrapper.find('.compact-actions .expand-button').exists()).toBe(false);
    await expand.trigger('click');
    await flushPromises();
    expect(wrapper.find('.widget-tools [aria-label="打开设置"]').exists()).toBe(true);
    expect(wrapper.find('.rule-note button').exists()).toBe(false);
    expect(wrapper.get('.widget-tools [aria-label="收起结果"]').attributes('aria-expanded')).toBe('true');
    wrapper.unmount();
});

it('模型菜单支持键盘选择、Escape 关闭，并为菜单预留窗口高度', async () => {
    fake.snapshot.config.profiles.push({
        id: 'two',
        name: '第二个模型',
        model: 'model-two',
        baseUrl: 'http://localhost/v1',
        hasKey: false,
    });
    const wrapper = mount(Widget);
    await flushPromises();
    const trigger = wrapper.get('[role="combobox"]');
    await trigger.trigger('click');
    await flushPromises();
    expect(fake.resize).toHaveBeenCalledWith(false, 88);
    expect(wrapper.get('[role="option"][aria-selected="true"]').text()).toContain('本地模型');
    await trigger.trigger('keydown', { key: 'ArrowDown' });
    await trigger.trigger('keydown', { key: 'Enter' });
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith('select_profile', { id: 'two' });
    expect(wrapper.find('[role="listbox"]').exists()).toBe(false);
    await trigger.trigger('click');
    await flushPromises();
    await trigger.trigger('keydown', { key: 'Escape' });
    await flushPromises();
    expect(wrapper.find('[role="listbox"]').exists()).toBe(false);
    wrapper.unmount();
});

it.each(['passed', 'failed'] as const)('边缘图标继承 %s 结论，悬停恢复面板', async (status) => {
    let dockHandler: (dock: { side: string | null; tucked: boolean }) => void = () => {};
    fake.subscribeDock.mockImplementation(async (handler) => {
        dockHandler = handler;
        return () => {};
    });
    fake.snapshot.history.push({
        id: 'passed',
        profileId: 'one',
        profileName: '本地模型',
        model: 'fixture',
        startedAt: '2026-10-23T00:00:00Z',
        durationMs: 10,
        status,
        answer: '答案',
        error: null,
        prompt: '问题',
        keyword: '答案',
        ruleVersion: 'literal-contains-v1',
    });
    const wrapper = mount(Widget);
    await flushPromises();
    dockHandler({ side: 'top', tucked: true });
    await flushPromises();
    expect(wrapper.get('.edge-tab').classes()).toContain(`state-${status}`);
    expect(wrapper.get('.edge-tab').classes()).toContain('top');
    await wrapper.get('.edge-tab').trigger('mouseenter');
    await flushPromises();
    expect(fake.resize).toHaveBeenCalledWith(false, 0, false);
    expect(wrapper.find('.widget').exists()).toBe(true);
    wrapper.unmount();
});

it('接口错误显示测试失败，回答中的 HTML 不作为可执行内容', async () => {
    fake.snapshot.history.push({
        id: 'record',
        profileId: 'one',
        profileName: '本地模型',
        model: 'fixture',
        startedAt: '2026-10-23T00:00:00Z',
        durationMs: 20,
        status: 'error',
        answer: '<img src=x onerror=alert(1)>',
        error: '回答被截断，未执行判定。',
        prompt: '问题',
        keyword: '答案',
        ruleVersion: 'literal-contains-v1',
    });
    const wrapper = mount(Widget);
    await flushPromises();
    expect(wrapper.get('.compact-status').text()).toBe('测试失败');
    await wrapper.get('[aria-label="展开测试详情"]').trigger('click');
    await flushPromises();
    expect(wrapper.get('.answer-box').text()).toContain('<img');
    expect(wrapper.find('.answer-box img').exists()).toBe(false);
    expect(wrapper.get('h1').text()).toBe('测试失败');
    wrapper.unmount();
});

it('其他配置的历史不会被显示为当前配置结论', async () => {
    fake.snapshot.history.push({
        id: 'record',
        profileId: 'other',
        profileName: '其他模型',
        model: 'other-model',
        startedAt: '2026-10-23T00:00:00Z',
        durationMs: 20,
        status: 'passed',
        answer: '答案',
        error: null,
        prompt: '问题',
        keyword: '答案',
        ruleVersion: 'literal-contains-v1',
    });
    const wrapper = mount(Widget);
    await flushPromises();
    expect(wrapper.get('.compact-status').text()).toBe('待测试');
    wrapper.unmount();
});

it('运行中只允许取消，不重复提交付费请求', async () => {
    fake.snapshot.running = {
        id: 'active',
        profileName: '本地模型',
        model: 'fixture',
        startedAt: '2026-10-23T00:00:00Z',
    };
    const wrapper = mount(Widget);
    await flushPromises();
    // 忙碌状态缩成边缘图标后，保留可被识别的动态圆环，而不是旋转对称的靶心。
    const dockHandler = fake.subscribeDock.mock.calls[0][0];
    dockHandler({ side: 'left', tucked: true });
    await flushPromises();
    expect(wrapper.get('.edge-tab').classes()).toContain('state-running');
    expect(wrapper.find('.edge-spinner.spin').exists()).toBe(true);
    await wrapper.get('.edge-tab').trigger('mouseenter');
    await flushPromises();
    await wrapper.get('.compact-primary').trigger('click');
    expect(fake.call).toHaveBeenCalledWith('cancel_test');
    expect(fake.call.mock.calls.some(([command]) => command === 'start_test')).toBe(false);
    wrapper.unmount();
});
