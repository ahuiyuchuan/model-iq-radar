/** 回答安全展示、原文复制和悬浮状态回归；界面故障与模型未通过保持独立。@author ahui */
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import Widget from '../src/components/Widget.vue';
import type { AppSnapshot } from '../src/types';
import { formatTime, formatRelativeTime } from '../src/useProbe';

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
        conditionFingerprints: { one: 'fixture-v1' },
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
    fake.snapshot.config.rule.keyword = '高市早苗';
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
        conditionFingerprint: 'fixture-v1',
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

it('悬浮窗刷新只请求当前模型最近记录，避免自动测试传输完整历史', async () => {
    const wrapper = mount(Widget);
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith('get_snapshot', { includeHistory: false });
    wrapper.unmount();
});

it('两态显示真实规则/强度，菜单保留模型ID；快捷换规则去重且不提交整个配置', async () => {
    fake.snapshot.config.defaultRuleName = '日本首相测试';
    fake.snapshot.config.rulePresets = [
        { id: 'candy', name: '糖果测试', prompt: '糖果题目', keyword: '21颗' },
    ];
    fake.snapshot.config.profiles[0].reasoningEffort = 'xhigh';
    let finish: (() => void) | undefined;
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'set_profile_rule') {
            await new Promise<void>((resolve) => {
                finish = resolve;
            });
            fake.snapshot.config.profiles[0].rulePresetId = 'candy';
        }
        return structuredClone(fake.snapshot);
    });
    const wrapper = mount(Widget);
    await flushPromises();
    expect(wrapper.get('[aria-label="当前测试规则"]').text()).toBe('日本首相测试');
    expect(wrapper.get('[aria-label="当前思考等级"]').text()).toBe('xhigh');
    await wrapper.get('[aria-label="当前模型配置"]').trigger('click');
    await flushPromises();
    expect(wrapper.get('[role="option"]').text()).toContain('本地模型 : fixture');
    expect(wrapper.get('[role="option"]').text()).toContain('日本首相测试');
    await wrapper.get('[aria-label="当前模型配置"]').trigger('keydown', { key: 'Escape' });
    await flushPromises();
    await wrapper.get('[aria-label="当前测试规则"]').trigger('click');
    await flushPromises();
    await wrapper.findAll('[role="option"]')[1].trigger('click');
    await flushPromises();
    expect(wrapper.get('[aria-label="当前测试规则"]').attributes('disabled')).toBeDefined();
    expect(fake.call.mock.calls.filter(([command]) => command === 'set_profile_rule')).toHaveLength(1);
    expect(fake.call).toHaveBeenCalledWith('set_profile_rule', { profileId: 'one', rulePresetId: 'candy' });
    expect(fake.call.mock.calls.some(([command]) => ['start_test', 'save_profile'].includes(command))).toBe(
        false,
    );
    finish?.();
    await flushPromises();
    await wrapper.get('[aria-label="展开测试详情"]').trigger('click');
    await flushPromises();
    expect(wrapper.get('[aria-label="当前测试规则"]').text()).toBe('糖果测试');
    expect(wrapper.get('[aria-label="当前思考等级"]').text()).toBe('xhigh');
    wrapper.unmount();
});

it('未指定强度和失效规则明确展示，失败的快捷切换保留原绑定并解锁', async () => {
    fake.snapshot.config.profiles[0].reasoningEffort = null;
    fake.snapshot.config.profiles[0].rulePresetId = 'deleted';
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'set_profile_rule') throw new Error('写入失败');
        return structuredClone(fake.snapshot);
    });
    const wrapper = mount(Widget);
    await flushPromises();
    expect(wrapper.get('[aria-label="当前测试规则"]').text()).toBe('规则已失效');
    expect(wrapper.get('[aria-label="当前思考等级"]').text()).toBe('不指定');
    await wrapper.get('[aria-label="当前测试规则"]').trigger('click');
    await flushPromises();
    await wrapper.get('[role="option"]').trigger('click');
    await flushPromises();
    expect(wrapper.get('.inline-error').text()).toContain('写入失败');
    expect(wrapper.get('[aria-label="当前测试规则"]').attributes('disabled')).toBeUndefined();
    expect(wrapper.get('[aria-label="当前测试规则"]').text()).toBe('规则已失效');
    wrapper.unmount();
});

it('模型、规则和思考菜单在辅助功能点击与键盘路径也始终互斥', async () => {
    const wrapper = mount(Widget);
    await flushPromises();
    for (const label of ['当前模型配置', '当前测试规则', '当前思考等级', '当前模型配置']) {
        await wrapper.get(`[aria-label="${label}"]`).trigger('click');
        await flushPromises();
        expect(wrapper.findAll('[role="listbox"]')).toHaveLength(1);
        expect(wrapper.get('[role="listbox"]').attributes('aria-label')).toBe(`${label}列表`);
    }
    await wrapper.get('[aria-label="当前思考等级"]').trigger('keydown', { key: 'ArrowDown' });
    await flushPromises();
    expect(wrapper.findAll('[role="listbox"]')).toHaveLength(1);
    expect(wrapper.get('[role="listbox"]').attributes('aria-label')).toBe('当前思考等级列表');
    await wrapper.get('[aria-label="展开测试详情"]').trigger('click');
    await flushPromises();
    expect(wrapper.find('[role="listbox"]').exists()).toBe(false);
    wrapper.unmount();
});

it('未测试不显示横线耗时，默认规则不显示重复副标题', async () => {
    const wrapper = mount(Widget);
    await flushPromises();
    expect(wrapper.get('.compact-timing').text()).toContain('尚未测试');
    expect(wrapper.get('.compact-timing').text()).not.toContain('耗时');
    expect(wrapper.get('.compact-timing').text()).not.toContain('—');
    await wrapper.get('[aria-label="当前测试规则"]').trigger('click');
    await flushPromises();
    expect(wrapper.get('[role="option"]').find('small').exists()).toBe(false);
    await wrapper.get('[aria-label="展开测试详情"]').trigger('click');
    await flushPromises();
    expect(wrapper.get('.metadata').text()).toContain('请求耗时暂无数据');
    wrapper.unmount();
});

it('快捷强度切换去重、只写单字段，读取快照后解锁且更新两态', async () => {
    let finish: (() => void) | undefined;
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'set_profile_effort') {
            await new Promise<void>((resolve) => {
                finish = resolve;
            });
            fake.snapshot.config.profiles[0].reasoningEffort = 'medium';
        }
        return structuredClone(fake.snapshot);
    });
    const wrapper = mount(Widget);
    await flushPromises();
    const effort = wrapper.get('[aria-label="当前思考等级"]');
    expect(effort.text()).toBe('low');
    expect(effort.attributes('title')).toContain('不支持时选“不指定”');
    await effort.trigger('click');
    await flushPromises();
    const medium = wrapper.findAll('[role="option"]').find((item) => item.text() === 'medium')!;
    await medium.trigger('click');
    await medium.trigger('click');
    await flushPromises();
    expect(fake.call.mock.calls.filter(([command]) => command === 'set_profile_effort')).toHaveLength(1);
    expect(fake.call).toHaveBeenCalledWith('set_profile_effort', {
        profileId: 'one',
        reasoningEffort: 'medium',
    });
    expect(effort.attributes('disabled')).toBeDefined();
    expect(
        fake.call.mock.calls.some(([command]) =>
            ['start_test', 'save_profile', 'save_preferences'].includes(command),
        ),
    ).toBe(false);
    finish?.();
    await flushPromises();
    expect(effort.text()).toBe('medium');
    expect(effort.attributes('disabled')).toBeUndefined();
    await wrapper.get('[aria-label="展开测试详情"]').trigger('click');
    await flushPromises();
    expect(wrapper.get('[aria-label="当前思考等级"]').text()).toBe('medium');
    wrapper.unmount();
});

it('强度失败保留原值，明确选择不指定发送 null，已知模型仅展示支持档位', async () => {
    fake.snapshot.config.profiles[0].model = 'gpt-5.1';
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'set_profile_effort') throw new Error('写入失败');
        return structuredClone(fake.snapshot);
    });
    const wrapper = mount(Widget);
    await flushPromises();
    const effort = wrapper.get('[aria-label="当前思考等级"]');
    await effort.trigger('click');
    await flushPromises();
    expect(wrapper.findAll('[role="option"]').map((item) => item.text())).toEqual([
        '不指定',
        'low',
        'medium',
        'high',
    ]);
    await wrapper.findAll('[role="option"]')[0].trigger('click');
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith('set_profile_effort', { profileId: 'one', reasoningEffort: null });
    expect(effort.text()).toBe('low');
    expect(effort.attributes('disabled')).toBeUndefined();
    expect(wrapper.get('.inline-error').text()).toContain('写入失败');
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
        conditionFingerprint: 'fixture-v1',
    });
    const wrapper = mount(Widget);
    await flushPromises();
    expect(wrapper.get('[role="combobox"]').text()).toContain('本地模型 : fixture');
    expect(wrapper.get('.compact-primary').text()).toBe('重新测试');
    expect(wrapper.get('.compact-timing').attributes('title')).toContain('测试时间：');
    // 可见标签和完整时间必须同在第一行，不能只靠 title 通过回归检查。
    expect(wrapper.get('.compact-test-time').text()).toBe(`测试时间 ${formatTime('2026-10-23T00:00:00Z')}`);
    expect(wrapper.get('.compact-timing').text()).toMatch(/\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}/);
    expect(wrapper.get('.compact-timing').text()).toContain('4.64s');
    expect(wrapper.get('.auto-countdown').text()).toContain('03:00');
    await vi.advanceTimersByTimeAsync(2000);
    expect(wrapper.get('.auto-countdown').text()).toContain('02:58');
    await wrapper.get('[aria-label="展开测试详情"]').trigger('click');
    await flushPromises();
    expect(wrapper.get('.expanded-actions .button:last-child').text()).toBe('重新测试');
    expect(wrapper.get('.metadata').text()).toContain('4.64s');
    expect(wrapper.get('.test-timestamp').text()).toContain('测试时间');
    expect(wrapper.get('.test-timestamp').text()).toContain(formatTime('2026-10-23T00:00:00Z'));
    expect(wrapper.get('.metadata .auto-countdown').text()).toContain('02:58');
    expect(wrapper.find('.record-model').exists()).toBe(false);
    fake.snapshot.config.preferences.autoTestEnabled = false;
    fake.subscribe.mock.calls[0][0]();
    await flushPromises();
    expect(wrapper.find('.auto-countdown').exists()).toBe(false);
    await wrapper.get('[aria-label="收起结果"]').trigger('click');
    await flushPromises();
    expect(wrapper.find('.auto-countdown').exists()).toBe(false);
    expect(wrapper.get('.compact-test-time').text()).toBe(`测试时间 ${formatTime('2026-10-23T00:00:00Z')}`);
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
    expect(fake.resize).toHaveBeenCalledWith(false, 90);
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
        conditionFingerprint: 'fixture-v1',
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
        conditionFingerprint: 'fixture-v1',
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
        conditionFingerprint: 'fixture-v1',
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

it('条件指纹变化后不沿用旧绿色，重测期间优先显示测试中，重命名仍保留有效结果', async () => {
    fake.snapshot.history.push({
        id: 'condition',
        profileId: 'one',
        profileName: '旧名称',
        model: 'fixture',
        startedAt: '2026-10-23T00:00:00Z',
        durationMs: 20,
        status: 'passed',
        answer: '答案',
        error: null,
        prompt: '问题',
        keyword: '答案',
        ruleVersion: 'literal-contains-v1',
        conditionFingerprint: 'fixture-v1',
    });
    const wrapper = mount(Widget);
    await flushPromises();
    expect(wrapper.get('.compact-status').text()).toBe('不降智');
    fake.snapshot.config.profiles[0].name = '新名称';
    fake.subscribe.mock.calls[0][0]();
    await flushPromises();
    expect(wrapper.get('.compact-status').text()).toBe('不降智');
    fake.snapshot.conditionFingerprints = { one: 'changed-url-or-effort' };
    fake.subscribe.mock.calls[0][0]();
    await flushPromises();
    expect(wrapper.get('.compact-status').text()).toBe('待重新测试');
    expect(wrapper.get('.widget').classes()).toContain('state-idle');
    expect(wrapper.get('.compact-test-time').text()).toBe(`测试时间 ${formatTime('2026-10-23T00:00:00Z')}`);
    expect(wrapper.get('.compact-timing').text()).toContain(formatTime('2026-10-23T00:00:00Z'));
    expect(wrapper.get('.compact-timing').text()).not.toContain('尚未测试');
    fake.snapshot.running = {
        id: 'run',
        profileName: '新名称',
        model: 'fixture',
        startedAt: new Date().toISOString(),
    };
    fake.subscribe.mock.calls[0][0]();
    await flushPromises();
    expect(wrapper.get('.compact-status').text()).toBe('测试中');
    wrapper.unmount();
});

it('自动测试开关阻止重复提交并等待快照刷新，边缘图标没有悬停提示卡', async () => {
    let release: (() => void) | undefined;
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'set_auto_test_enabled') {
            await new Promise<void>((resolve) => {
                release = resolve;
            });
            fake.snapshot.config.preferences.autoTestEnabled = false;
        }
        return structuredClone(fake.snapshot);
    });
    const wrapper = mount(Widget);
    await flushPromises();
    const toggle = wrapper.get('[aria-label="切换自动测试"]');
    await toggle.trigger('click');
    await toggle.trigger('click');
    expect(fake.call.mock.calls.filter(([command]) => command === 'set_auto_test_enabled')).toHaveLength(1);
    expect(toggle.attributes('disabled')).toBeDefined();
    release?.();
    await flushPromises();
    expect(toggle.attributes('aria-pressed')).toBe('false');
    fake.subscribeDock.mock.calls[0][0]({ side: 'right', tucked: true });
    await flushPromises();
    expect(wrapper.get('.edge-tab').attributes('title')).toBeUndefined();
    wrapper.unmount();
});

it('完整测试时间固定本机年月日和24小时秒数，午夜与各部分补零且不受区域格式影响', () => {
    const midnight = new Date(2026, 0, 2, 0, 3, 4);
    expect(formatTime(midnight.toISOString())).toBe('2026-01-02 00:03:04');
    const evening = new Date(2026, 9, 25, 23, 59, 58);
    expect(formatTime(evening.toISOString())).toBe('2026-10-25 23:59:58');
    expect(formatTime(undefined)).toBe('尚未测试');
    expect(formatTime('')).toBe('尚未测试');
    expect(formatTime('invalid')).toBe('时间不可用');
});

it('相对时间覆盖分钟、天数、无效日期和系统时钟回拨', () => {
    const time = Date.parse('2026-10-25T12:00:00Z');
    expect(formatRelativeTime('2026-10-25T12:00:30Z', time)).toBe('刚刚测试');
    expect(formatRelativeTime('2026-10-25T11:55:00Z', time)).toBe('5 分钟前');
    expect(formatRelativeTime('2026-10-24T12:00:00Z', time)).toBe('1 天前');
    expect(formatRelativeTime('invalid', time)).toBe('时间不可用');
    expect(formatRelativeTime(undefined, time)).toBe('尚未测试');
});

it('旧刷新失败不能覆盖较新快照或错误地展开悬浮窗', async () => {
    const wrapper = mount(Widget);
    await flushPromises();
    let rejectOld: ((reason: Error) => void) | undefined;
    fake.call.mockImplementationOnce(
        () =>
            new Promise((_resolve, reject) => {
                rejectOld = reject;
            }),
    );
    const refresh = fake.subscribe.mock.calls[0][0];
    refresh();
    fake.snapshot.config.profiles[0].name = '最新快照';
    refresh();
    await flushPromises();
    rejectOld?.(new Error('过期的读取失败'));
    await flushPromises();
    expect(wrapper.get('[role="combobox"]').text()).toContain('最新快照');
    expect(wrapper.find('.result-content').exists()).toBe(false);
    expect(wrapper.find('.inline-error').exists()).toBe(false);
    wrapper.unmount();
});

it('取消命令失败后允许再次取消，不将操作按钮永久锁定', async () => {
    fake.snapshot.running = {
        id: 'active',
        profileName: '本地模型',
        model: 'fixture',
        startedAt: new Date().toISOString(),
    };
    let attempts = 0;
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'cancel_test' && ++attempts === 1) throw new Error('本机取消命令失败');
        return structuredClone(fake.snapshot);
    });
    const wrapper = mount(Widget);
    await flushPromises();
    await wrapper.get('.compact-primary').trigger('click');
    await flushPromises();
    const retry = wrapper.get('.expanded-actions .button:last-child');
    expect(retry.attributes('disabled')).toBeUndefined();
    expect(retry.text()).toBe('取消测试');
    await retry.trigger('click');
    await flushPromises();
    expect(attempts).toBe(2);
    expect(retry.attributes('disabled')).toBeDefined();
    wrapper.unmount();
});

it('开始请求尚未同步运行令牌时禁止提前取消和重复开始', async () => {
    let finish: (() => void) | undefined;
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'start_test')
            await new Promise<void>((resolve) => {
                finish = resolve;
            });
        return structuredClone(fake.snapshot);
    });
    const wrapper = mount(Widget);
    await flushPromises();
    await wrapper.get('.compact-primary').trigger('click');
    await flushPromises();
    expect(wrapper.get('.compact-primary').attributes('disabled')).toBeDefined();
    await wrapper.get('.compact-primary').trigger('click');
    expect(fake.call.mock.calls.filter(([command]) => command === 'start_test')).toHaveLength(1);
    expect(fake.call.mock.calls.some(([command]) => command === 'cancel_test')).toBe(false);
    finish?.();
    await flushPromises();
    expect(wrapper.get('.compact-primary').attributes('disabled')).toBeUndefined();
    wrapper.unmount();
});
