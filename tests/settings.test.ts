/**
 * 设置规则的交互回归：区分草稿/保存、保留有效空白、空规则拦截与密钥边界。
 * 所有调用为本地 Mock，不使用真实密钥和第三方模型服务。@author ahui
 */
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import Settings from '../src/components/Settings.vue';
import HistoryPanel from '../src/components/HistoryPanel.vue';
import MailSettings from '../src/components/MailSettings.vue';
import type { AppSnapshot } from '../src/types';
import { version } from '../package.json';

const fake = vi.hoisted(() => ({
    call: vi.fn(),
    subscribe: vi.fn(),
    updates: vi.fn(),
    resize: vi.fn(),
    nativeClose: vi.fn(),
    closeSettings: vi.fn(),
    snapshot: {} as AppSnapshot,
}));

/** 五类选择、二次确认与提交等待必须属于同一流程；两个入口始终具有相同语义。 */
it.each(['恢复所有设置', '清除所有数据'])('%s 空选禁用、取消保留、双击和提交中去重', async (label) => {
    let finishReset!: () => void;
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'get_snapshot') return structuredClone(fake.snapshot);
        if (command === 'get_autostart') return false;
        if (command === 'reset_selected_data')
            return new Promise<void>((resolve) => {
                finishReset = resolve;
            });
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[2].trigger('click');
    await flushPromises();
    const entry = wrapper.findAll('button').find((button) => button.text().trim() === label)!;
    await Promise.all([entry.trigger('click'), entry.trigger('click')]);
    await flushPromises();
    expect(wrapper.findAll('[role="dialog"]')).toHaveLength(1);
    expect(wrapper.findAll('[role="dialog"] input:checked')).toHaveLength(0);
    const next = () => wrapper.findAll('[role="dialog"] button').find((button) => button.text() === '继续')!;
    expect(next().attributes('disabled')).toBeDefined();
    await wrapper.findAll('.reset-option input')[1].setValue(true);
    await next().trigger('click');
    await flushPromises();
    expect(wrapper.get('[role="dialog"]').text()).toContain('测试规则');
    expect(wrapper.get('[role="dialog"]').text()).toContain('糖果测试');
    const confirm = wrapper
        .findAll('[role="dialog"] button')
        .find((button) => button.text() === '确认清空并恢复初始状态')!;
    await Promise.all([confirm.trigger('click'), confirm.trigger('click')]);
    await flushPromises();
    expect(fake.call.mock.calls.filter(([command]) => command === 'reset_selected_data')).toHaveLength(1);
    expect(fake.call).toHaveBeenCalledWith('reset_selected_data', {
        selection: { api: false, rules: true, widget: false, history: false, mail: false },
    });
    expect(entry.attributes('disabled')).toBeDefined();
    finishReset();
    await flushPromises();
    expect(wrapper.get('.success-message').text()).toContain('测试规则');
    await entry.trigger('click');
    await flushPromises();
    await wrapper.get('.reset-select-all input').setValue(true);
    expect(wrapper.findAll('.reset-option input:checked')).toHaveLength(5);
    await wrapper.findAll('[role="dialog"] button')[0].trigger('click');
    expect(fake.call.mock.calls.filter(([command]) => command === 'reset_selected_data')).toHaveLength(1);
    wrapper.unmount();
});

/** 移除全局快捷键后，两入口只重置桌面偏好；不要求释放系统组合键或操作其他分类。 */
it.each(['恢复所有设置', '清除所有数据'])('%s 可单独清空悬浮设置且不再显示快捷键选项', async (label) => {
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[2].trigger('click');
    await flushPromises();
    expect(wrapper.find('#shortcut').exists()).toBe(false);
    expect(wrapper.text()).not.toContain('Ctrl + Alt + P');
    await wrapper
        .findAll('button')
        .find((button) => button.text().trim() === label)!
        .trigger('click');
    await flushPromises();
    await wrapper.findAll('.reset-option input')[2].setValue(true);
    await wrapper
        .findAll('[role="dialog"] button')
        .find((button) => button.text() === '继续')!
        .trigger('click');
    await flushPromises();
    await wrapper
        .findAll('[role="dialog"] button')
        .find((button) => button.text() === '确认清空并恢复初始状态')!
        .trigger('click');
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith('reset_selected_data', {
        selection: { api: false, rules: false, widget: true, history: false, mail: false },
    });
    expect(wrapper.get('.success-message').text()).toContain('悬浮设置');
    expect(wrapper.find('.inline-error').exists()).toBe(false);
    wrapper.unmount();
});

it('新建默认选择现存糖果，已有绑定与被删除预设保持原意', async () => {
    fake.snapshot.config.rulePresets = [
        { id: 'builtin-candy-test', name: '糖果测试', prompt: '糖果题', keyword: '21颗' },
    ];
    const wrapper = mount(Settings);
    await flushPromises();
    expect(wrapper.get('[aria-label="绑定规则预设"]').text()).not.toBe('糖果测试');
    await wrapper.get('[aria-label="新增接口配置"]').trigger('click');
    expect(wrapper.get('[aria-label="绑定规则预设"]').text()).toBe('糖果测试');
    await wrapper.findAll('nav button')[1].trigger('click');
    await flushPromises();
    expect(wrapper.get('[aria-label="编辑测试规则"]').text()).toBe('糖果测试');
    expect(wrapper.get<HTMLTextAreaElement>('#test-prompt').element.value).toBe('糖果题');
    wrapper.unmount();
    fake.snapshot.config.rulePresets = [];
    const deleted = mount(Settings);
    await flushPromises();
    await deleted.get('[aria-label="新增接口配置"]').trigger('click');
    expect(deleted.get('[aria-label="绑定规则预设"]').text()).not.toBe('糖果测试');
    deleted.unmount();
});

it('历史四项筛选按 AND 组合，保留已删项并把缺失强度与不指定分开', async () => {
    const base = {
        id: 'a',
        profileId: 'deleted',
        profileName: '已删除分组',
        model: 'model-a',
        ruleName: '规则甲',
        status: 'passed' as const,
        reasoningEffort: 'high' as const,
        startedAt: '2026-10-27T00:00:00Z',
        durationMs: 12,
        answer: '正文',
        error: null,
        prompt: '题',
        keyword: '正文',
        ruleVersion: 'literal-contains-v1',
    };
    const history = [
        base,
        { ...base, id: 'b', model: 'model-b' },
        { ...base, id: 'c', ruleName: '规则乙' },
        { ...base, id: 'd', reasoningEffort: null },
        { ...base, id: 'e', reasoningEffort: null, conditionFingerprint: 'saved' },
    ];
    const wrapper = mount(HistoryPanel, { props: { history, profiles: [], modelValue: null } });
    await choose(wrapper, '按分组筛选历史', '已删除分组5 条记录');
    await choose(wrapper, '按模型筛选历史', 'model-a');
    await choose(wrapper, '按规则预设筛选历史', '规则甲');
    await choose(wrapper, '按思考强度筛选历史', 'high');
    expect(wrapper.findAll('.history-item')).toHaveLength(1);
    expect(wrapper.emitted('filtered')?.at(-1)?.[0]).toEqual([base]);
    await choose(wrapper, '按思考强度筛选历史', '未记录');
    expect(wrapper.emitted('filtered')?.at(-1)?.[0]).toEqual([history[3]]);
    await choose(wrapper, '按思考强度筛选历史', '不指定');
    expect(wrapper.emitted('filtered')?.at(-1)?.[0]).toEqual([history[4]]);
    wrapper.unmount();
});

it.each([false, true])('详情复制迟到结果在返回并重进同记录后不回写：失败=%s', async (failure) => {
    fake.snapshot.history = [
        {
            id: 'copy',
            profileId: 'one',
            profileName: '测试模型',
            model: 'model',
            status: 'passed',
            startedAt: '2026-10-27T00:00:00Z',
            durationMs: 12,
            answer: '原文',
            error: null,
            prompt: '题',
            keyword: '原文',
            ruleVersion: 'literal-contains-v1',
        },
    ];
    let complete!: () => void;
    const writeText = vi.fn(
        () =>
            new Promise<void>((resolve, reject) => {
                complete = () => (failure ? reject(new Error('fixture')) : resolve());
            }),
    );
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[3].trigger('click');
    await wrapper.get('.history-record-info').trigger('click');
    await wrapper
        .findAll('.settings-actions button')
        .find((button) => button.text().includes('复制测试记录'))!
        .trigger('click');
    await wrapper
        .findAll('.settings-actions button')
        .find((button) => button.text().includes('返回记录'))!
        .trigger('click');
    await wrapper.get('.history-record-info').trigger('click');
    complete();
    await flushPromises();
    expect(wrapper.text()).not.toContain('已复制本条测试记录');
    expect(wrapper.text()).not.toContain('复制失败');
    wrapper.unmount();
});

it('默认规则可改名，恢复内置题答保留名称与单答案，并不立即保存', async () => {
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[1].trigger('click');
    await wrapper.get('#preset-name').setValue('日本首相测试');
    await wrapper
        .findAll('button')
        .find((button) => button.text() === '添加答案')!
        .trigger('click');
    await wrapper.get('#answer-2').setValue('替代答案');
    await wrapper
        .findAll('button')
        .find((button) => button.text() === '恢复默认')!
        .trigger('click');
    expect(wrapper.get<HTMLInputElement>('#preset-name').element.value).toBe('日本首相测试');
    expect(wrapper.find('#answer-2').exists()).toBe(false);
    expect(fake.call.mock.calls.some(([command]) => command === 'save_rule')).toBe(false);
    await wrapper.get('#rule-settings').trigger('submit');
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith('save_rule', {
        name: '日本首相测试',
        rule: { prompt: '默认问题', keyword: '高市早苗', timeoutSeconds: null },
    });
    wrapper.unmount();
});

it('旧配置缺省 Chat 且回退关闭，显式开启随模型保存，连接诊断说明实际协议', async () => {
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'get_snapshot') return structuredClone(fake.snapshot);
        if (command === 'test_connection')
            return {
                durationMs: 10,
                interfaceMode: 'responses',
                attemptedModes: ['chat_completions', 'responses'],
            };
    });
    const wrapper = mount(Settings);
    await flushPromises();
    expect(wrapper.get('[aria-label="接口模式"]').text()).toBe('Chat Completions');
    const toggle = wrapper.get<HTMLInputElement>('#protocol-fallback');
    expect(toggle.element.checked).toBe(false);
    expect(wrapper.text()).toContain('可能多产生一次 API 调用');
    await toggle.setValue(true);
    await wrapper
        .findAll('button')
        .find((button) => button.text() === '测试连接')!
        .trigger('click');
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith(
        'test_connection',
        expect.objectContaining({ input: expect.objectContaining({ protocolFallbackEnabled: true }) }),
    );
    expect(wrapper.text()).toContain('Responses（已切换协议）');
    await wrapper.get('#profile-settings').trigger('submit');
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith(
        'save_profile',
        expect.objectContaining({ input: expect.objectContaining({ protocolFallbackEnabled: true }) }),
    );
    expect(fake.call.mock.calls.some(([command]) => command === 'start_test')).toBe(false);
    wrapper.unmount();
});

it.each([
    { mode: 'responses' as const, enabled: false, label: 'Responses' },
    { mode: 'chat_completions' as const, enabled: true, label: 'Chat Completions' },
])('已有配置保留 $mode / 回退 $enabled，不被新建默认值覆盖', async ({ mode, enabled, label }) => {
    fake.snapshot.config.profiles[0].interfaceMode = mode;
    fake.snapshot.config.profiles[0].protocolFallbackEnabled = enabled;
    const wrapper = mount(Settings);
    await flushPromises();
    expect(wrapper.get('[aria-label="接口模式"]').text()).toBe(label);
    expect(wrapper.get<HTMLInputElement>('#protocol-fallback').element.checked).toBe(enabled);
    expect(fake.call.mock.calls.some(([command]) => command === 'save_profile')).toBe(false);
    wrapper.unmount();
});

it('新增接口默认 Responses 并开启回退，连接与保存传入相同策略，仍允许手动关闭', async () => {
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.get('[aria-label="新增接口配置"]').trigger('click');
    expect(wrapper.get('[aria-label="接口模式"]').text()).toBe('Responses');
    expect(wrapper.get<HTMLInputElement>('#protocol-fallback').element.checked).toBe(true);
    await wrapper.get('#profile-name').setValue('新接口');
    await wrapper.get('#base-url').setValue('https://api.example.com/v1');
    await wrapper.get('#model-name').setValue('gpt-6.1-sol');
    await wrapper
        .findAll('button')
        .find((button) => button.text() === '测试连接')!
        .trigger('click');
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith(
        'test_connection',
        expect.objectContaining({
            input: expect.objectContaining({ interfaceMode: 'responses', protocolFallbackEnabled: true }),
        }),
    );
    await wrapper.get('#profile-settings').trigger('submit');
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith(
        'save_profile',
        expect.objectContaining({
            input: expect.objectContaining({ interfaceMode: 'responses', protocolFallbackEnabled: true }),
        }),
    );
    // 保存后的刷新由 Mock 保留旧配置；重新新建后仍可主动选择 Chat 并关闭回退。
    await wrapper.get('[aria-label="新增接口配置"]').trigger('click');
    await choose(wrapper, '接口模式', 'Chat Completions');
    await wrapper.get('#protocol-fallback').setValue(false);
    expect(wrapper.get('[aria-label="接口模式"]').text()).toBe('Chat Completions');
    expect(wrapper.get<HTMLInputElement>('#protocol-fallback').element.checked).toBe(false);
    wrapper.unmount();
});

it('绑定默认规则仅显示名称，保存仍归一为 null 表示沿用默认', async () => {
    fake.snapshot.config.defaultRuleName = '日本首相测试';
    fake.snapshot.config.rulePresets = [{ id: 'candy', name: '糖果测试', prompt: '题', keyword: '21颗' }];
    fake.snapshot.config.profiles[0].rulePresetId = 'candy';
    const wrapper = mount(Settings);
    await flushPromises();
    await choose(wrapper, '绑定规则预设', '日本首相测试');
    expect(wrapper.get('[aria-label="绑定规则预设"]').text()).toBe('日本首相测试');
    expect(wrapper.text()).not.toContain('沿用默认：');
    await wrapper.get('#profile-settings').trigger('submit');
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith(
        'save_profile',
        expect.objectContaining({ input: expect.objectContaining({ rulePresetId: null }) }),
    );
    wrapper.unmount();
});

it('自定义预设仅还原自身，切换单答案规则时不残留额外答案', async () => {
    fake.snapshot.config.rulePresets = [
        {
            id: 'candy',
            name: '糖果测试',
            prompt: '糖果问题',
            keyword: '21颗',
            alternativeKeywords: ['二十一颗'],
        },
    ];
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[1].trigger('click');
    await choose(wrapper, '编辑测试规则', '糖果测试');
    await wrapper.get('#keyword').setValue('草稿答案');
    await wrapper.get('#preset-name').setValue('草稿名');
    await wrapper
        .findAll('button')
        .find((button) => button.text() === '还原已保存')!
        .trigger('click');
    expect(wrapper.get<HTMLTextAreaElement>('#test-prompt').element.value).toBe('糖果问题');
    expect(wrapper.get<HTMLInputElement>('#preset-name').element.value).toBe('糖果测试');
    expect(wrapper.get<HTMLTextAreaElement>('#answer-2').element.value).toBe('二十一颗');
    await choose(wrapper, '编辑测试规则', '默认规则');
    expect(wrapper.find('#answer-2').exists()).toBe(false);
    expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
    wrapper.unmount();
});

it('多答案任一命中才通过本地试匹配，空项/重复项阻止保存，删除首项提升下一项', async () => {
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[1].trigger('click');
    await wrapper
        .findAll('button')
        .find((button) => button.text() === '添加答案')!
        .trigger('click');
    await wrapper.get('#rule-settings').trigger('submit');
    expect(wrapper.get('[role="alert"]').text()).toContain('空白');
    await wrapper.get('#answer-2').setValue('高市早苗');
    await wrapper.get('#rule-settings').trigger('submit');
    expect(wrapper.get('[role="alert"]').text()).toContain('重复');
    await wrapper.get('#answer-2').setValue('21颗');
    await wrapper.get('#sample-answer').setValue('答案是21颗');
    expect(wrapper.get('.rule-sandbox p').text()).toBe('命中 → 不降智');
    await wrapper.get('[aria-label="删除答案 1"]').trigger('click');
    expect(wrapper.get<HTMLTextAreaElement>('#keyword').element.value).toBe('21颗');
    expect(wrapper.find('#answer-2').exists()).toBe(false);
    wrapper.unmount();
});

it('悬浮设置DOM按常用测试、外观交互、自启、历史顺序排列', async () => {
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[2].trigger('click');
    expect(wrapper.findAll('.preference-row label').map((label) => label.attributes('for'))).toEqual([
        'auto-test',
        'auto-interval',
        'timeout',
        'always-top',
        'snap-edge',
        'opacity',
        'autostart',
        'history-max-records',
        'history-retention-days',
    ]);
    wrapper.unmount();
});

it('卡片外部切规则只合并未编辑的绑定字段，不覆盖接口名称草稿', async () => {
    fake.snapshot.config.rulePresets = [{ id: 'candy', name: '糖果测试', prompt: '题', keyword: '21颗' }];
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.get('#profile-name').setValue('未保存名称');
    fake.snapshot.config.profiles[0].rulePresetId = 'candy';
    fake.subscribe.mock.calls[0][0]();
    await flushPromises();
    expect(wrapper.get('[aria-label="绑定规则预设"]').text()).toBe('糖果测试');
    expect(wrapper.get<HTMLInputElement>('#profile-name').element.value).toBe('未保存名称');
    wrapper.unmount();
});

it('晚返回的初始缓存不能覆盖已完成的模型刷新', async () => {
    fake.snapshot.config.profiles[0].catalogToken = 'persisted-list';
    let resolveCatalog: (models: object[]) => void = () => {};
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'get_snapshot') return structuredClone(fake.snapshot);
        if (command === 'get_model_catalog')
            return new Promise<object[]>((resolve) => {
                resolveCatalog = resolve;
            });
        if (command === 'fetch_models')
            return { token: 'persisted-list', models: [{ id: 'fresh-model', ownedBy: null }] };
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper
        .findAll('button')
        .find((button) => button.text() === '拉取模型')!
        .trigger('click');
    await flushPromises();
    resolveCatalog([{ id: 'stale-model', ownedBy: null }]);
    await flushPromises();
    await wrapper.get('[aria-label="展开模型列表"]').trigger('click');
    expect(wrapper.get('[role="listbox"]').text()).toContain('fresh-model');
    expect(wrapper.get('[role="listbox"]').text()).not.toContain('stale-model');
    wrapper.unmount();
});

it('已保存列表无需重新拉取，更新失败仍能搜索选择缓存模型', async () => {
    fake.snapshot.config.profiles[0].catalogToken = 'persisted-list';
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'get_snapshot') return structuredClone(fake.snapshot);
        if (command === 'get_model_catalog') return [{ id: 'cached-model', ownedBy: null }];
        if (command === 'fetch_models') throw new Error('离线');
    });
    const wrapper = mount(Settings);
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith('get_model_catalog', {
        token: 'persisted-list',
        baseUrl: 'https://api.example.com/v1',
    });
    expect(wrapper.find('[aria-label="拉取到的模型"]').exists()).toBe(false);
    await wrapper.findAll('.connection-actions button')[0].trigger('click');
    await flushPromises();
    expect(wrapper.get('.diagnostic-error').text()).toContain('离线');
    await wrapper.get('#model-name').setValue('cached');
    await wrapper.get('[aria-label="展开模型列表"]').trigger('click');
    await flushPromises();
    await wrapper.get('[role="option"]').trigger('click');
    await flushPromises();
    expect(wrapper.get<HTMLInputElement>('#model-name').element.value).toBe('cached-model');
    wrapper.unmount();
});
vi.mock('../src/bridge', () => ({
    desktop: true,
    CANDY_RULE_ID: 'builtin-candy-test',
    DEFAULT_PROMPT: '默认问题',
    call: fake.call,
    subscribe: fake.subscribe,
    subscribeUpdateStatus: fake.updates,
    dragWindow: vi.fn(),
    windowAction: vi.fn(),
    resizeWidget: fake.resize,
    protectWindowClose: fake.nativeClose,
    closeSettingsWindow: fake.closeSettings,
}));

beforeEach(() => {
    fake.nativeClose.mockReset().mockResolvedValue(() => {});
    fake.closeSettings.mockReset().mockResolvedValue(undefined);
    fake.snapshot = {
        config: {
            profiles: [
                {
                    id: 'one',
                    name: '测试模型',
                    baseUrl: 'https://api.example.com/v1',
                    model: 'test-model',
                    hasKey: true,
                },
            ],
            selectedProfileId: 'one',
            rule: { prompt: '原来的问题', keyword: '高市早苗' },
            preferences: {
                alwaysOnTop: true,
                snapToEdge: true,
                opacity: 97,
                timeoutSeconds: 60,
                autoTestEnabled: true,
                autoTestIntervalMinutes: 3,
            },
        },
        history: [],
        running: null,
    };
    fake.call.mockReset().mockImplementation(async (command: string) => {
        if (command === 'get_snapshot') return structuredClone(fake.snapshot);
        if (command === 'get_autostart') return false;
    });
    fake.subscribe.mockReset().mockResolvedValue(() => {});
    fake.updates.mockReset().mockResolvedValue(() => {});
    fake.resize.mockReset().mockResolvedValue(undefined);
});

it('设置中的全部二值选项共用滑动开关及可访问名称，不残留方形复选框', async () => {
    fake.snapshot.config.emailNotifications = {
        enabled: false,
        smtpHost: '',
        smtpPort: 465,
        security: 'tls',
        sender: '',
        username: '',
        recipients: [],
        onDegraded: true,
        onRecovered: true,
        onFailure: true,
        hasPassword: true,
    };
    const wrapper = mount(Settings);
    await flushPromises();
    let total = 0;
    for (let page = 0; page < 6; page++) {
        await wrapper.findAll('nav button')[page].trigger('click');
        await flushPromises();
        for (const input of wrapper.findAll<HTMLInputElement>('input[type="checkbox"]')) {
            total++;
            expect(input.classes()).toContain('switch');
            expect(input.attributes('role')).toBe('switch');
            expect(input.element.labels?.length).toBeGreaterThan(0);
        }
    }
    expect(total).toBe(11);
    expect(fake.call.mock.calls.some(([command]) => command.startsWith('save_'))).toBe(false);
    wrapper.unmount();
});

it('邮件滑动开关仅编辑草稿，提交期间禁用且不会重复保存，失败保留选择', async () => {
    let rejectSave: (reason: Error) => void = () => {};
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'save_email_settings')
            return new Promise((_resolve, reject) => {
                rejectSave = reject;
            });
    });
    const wrapper = mount(MailSettings, { props: { disabled: false, afterSave: async () => true } });
    const toggles = wrapper.findAll<HTMLInputElement>('[role="switch"]');
    await toggles[0].setValue(true);
    await toggles[1].setValue(false);
    await toggles[3].setValue(false);
    expect(fake.call).not.toHaveBeenCalled();
    await wrapper.get('form').trigger('submit');
    await wrapper.get('form').trigger('submit');
    expect(toggles.every((input) => input.element.matches(':disabled'))).toBe(true);
    expect(fake.call).toHaveBeenCalledTimes(1);
    expect(fake.call).toHaveBeenCalledWith('save_email_settings', {
        input: expect.objectContaining({
            settings: expect.objectContaining({
                enabled: true,
                onDegraded: false,
                onRecovered: true,
                onFailure: false,
            }),
        }),
    });
    rejectSave(new Error('模拟保存失败'));
    await flushPromises();
    expect(toggles.every((input) => !input.element.matches(':disabled'))).toBe(true);
    expect(toggles.map((input) => input.element.checked)).toEqual([true, false, true, false]);
    expect(wrapper.get('[role="alert"]').text()).toContain('模拟保存失败');
    wrapper.unmount();
});

it('自启读取系统状态，切换立即生效且去重，不保存配置或重置测试周期', async () => {
    let enabled = true;
    let finish: (value: boolean) => void = () => {};
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'get_snapshot') return structuredClone(fake.snapshot);
        if (command === 'get_autostart') return enabled;
        if (command === 'set_autostart')
            return new Promise<boolean>((resolve) => {
                finish = resolve;
            });
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[2].trigger('click');
    await flushPromises();
    const toggle = wrapper.get<HTMLInputElement>('#autostart');
    expect(toggle.element.checked).toBe(true);
    await toggle.setValue(false);
    expect(toggle.element.disabled).toBe(true);
    expect(toggle.element.checked).toBe(true);
    await toggle.trigger('change');
    expect(fake.call.mock.calls.filter(([command]) => command === 'set_autostart')).toHaveLength(1);
    expect(fake.call).toHaveBeenCalledWith('set_autostart', { enabled: false });
    finish(false);
    await flushPromises();
    expect(toggle.element.checked).toBe(false);
    enabled = true;
    window.dispatchEvent(new Event('focus'));
    await flushPromises();
    expect(toggle.element.checked).toBe(true);
    expect(
        fake.call.mock.calls.some(([command]) => ['save_preferences', 'start_test'].includes(command)),
    ).toBe(false);
    wrapper.unmount();
    const before = fake.call.mock.calls.length;
    window.dispatchEvent(new Event('focus'));
    await flushPromises();
    expect(fake.call.mock.calls.length).toBe(before);
});

it('设置页显式请求完整历史，悬浮窗快照优化不影响筛选、导出和详情数据源', async () => {
    const wrapper = mount(Settings);
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith('get_snapshot', { includeHistory: true });
    wrapper.unmount();
});

it('侧栏沿用首版顺序，新增功能位于关于之前，关于固定为最后一项', async () => {
    const wrapper = mount(Settings);
    await flushPromises();
    expect(wrapper.findAll('nav button').map((button) => button.text())).toEqual([
        '接口配置',
        '测试规则',
        '悬浮设置',
        '测试历史',
        '邮箱通知',
        '关于',
    ]);
    await wrapper.findAll('nav button').at(-1)!.trigger('click');
    expect(wrapper.get('.page-intro h1').text()).toBe('关于');
    wrapper.unmount();
});

it.each(['历史开始日期', '历史结束日期'])(
    '%s：整框打开应用内日历，选择后更新对应筛选，Tab 聚焦不自动弹出',
    async (label) => {
        const wrapper = mount(HistoryPanel, { props: { history: [], profiles: [], modelValue: null } });
        const input = wrapper.get<HTMLInputElement>(`input[aria-label="${label}"]`);
        await input.trigger('focus');
        expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
        await input.trigger('click');
        expect(wrapper.get('[role="dialog"]').attributes('aria-label')).toBe(`${label}选择器`);
        const day = wrapper.get('.date-picker-day:not(.outside):not(:disabled)');
        const value = day.attributes('data-date');
        await day.trigger('click');
        expect(input.element.value).toBe(value);
        expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
        await input.setValue('');
        expect(input.element.value).toBe('');
        expect(wrapper.find('[role="alert"]').exists()).toBe(false);
        wrapper.unmount();
    },
);

it('自启读取或修改失败显示错误并恢复系统状态，允许重试', async () => {
    let failRead = true;
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'get_snapshot') return structuredClone(fake.snapshot);
        if (command === 'get_autostart') {
            if (failRead) throw new Error('读取失败');
            return false;
        }
        if (command === 'set_autostart') throw new Error('权限不足');
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[2].trigger('click');
    await flushPromises();
    const toggle = wrapper.get<HTMLInputElement>('#autostart');
    expect(toggle.element.disabled).toBe(true);
    expect(wrapper.get('.preference-row [role="alert"]').text()).toContain('读取失败');
    failRead = false;
    await wrapper.get('.preference-row [role="alert"] button').trigger('click');
    await flushPromises();
    expect(toggle.element.disabled).toBe(false);
    await toggle.setValue(true);
    await flushPromises();
    expect(toggle.element.checked).toBe(false);
    expect(wrapper.get('.preference-row [role="alert"]').text()).toContain('权限不足');
    wrapper.unmount();
});

it('历史分组与关键词组合筛选，删除的配置仍可筛选，复制包含原始 Markdown', async () => {
    const source = '**高市**早苗\n\n```js\nconst n = 1;\n```';
    fake.snapshot.history = [
        {
            id: 'one',
            profileId: 'one',
            profileName: '测试模型',
            model: 'test-model',
            startedAt: '2026-10-23T00:00:00Z',
            durationMs: 10,
            status: 'failed',
            answer: source,
            error: null,
            prompt: '原问题',
            keyword: '高市早苗',
            ruleVersion: 'literal-contains-v1',
        },
        {
            id: 'deleted',
            profileId: 'deleted',
            profileName: '已删除分组',
            model: 'old',
            startedAt: '2026-10-23T00:00:00Z',
            durationMs: 10,
            status: 'passed',
            answer: '旧分组答案',
            error: null,
            prompt: '问题',
            keyword: '答案',
            ruleVersion: 'literal-contains-v1',
        },
    ];
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[3].trigger('click');
    expect(wrapper.findAll('.history-item')).toHaveLength(2);
    const filter = wrapper.get('[aria-label="按分组筛选历史"]');
    await filter.trigger('click');
    await wrapper.findAll('[role="option"]')[2].trigger('click');
    await flushPromises();
    expect(wrapper.findAll('.history-item')).toHaveLength(1);
    expect(wrapper.get('.history-item').text()).toContain('已删除分组');
    await wrapper.get('[aria-label="搜索历史"]').setValue('不存在');
    expect(wrapper.findAll('.history-item')).toHaveLength(0);
    await wrapper.get('[aria-label="搜索历史"]').setValue('');
    await filter.trigger('click');
    await wrapper.findAll('[role="option"]')[1].trigger('click');
    await flushPromises();
    await wrapper.get('.history-record-info').trigger('click');
    expect(wrapper.get('.history-answer strong').text()).toBe('高市');
    expect(wrapper.get('.result-pill').text()).toBe('降智');
    await wrapper.findAll('.settings-actions button')[1].trigger('click');
    expect(writeText.mock.calls[0][0]).toContain(`回答：\n${source}`);
    expect(fake.snapshot.history[0].answer).toBe(source);
    wrapper.unmount();
});

it('更新只读后台缓存，在关于入口显示新版本，切页不检查且卸载清理监听', async () => {
    const cleanup = vi.fn();
    fake.updates.mockImplementation(async (handler) => {
        handler({
            status: 'available',
            latestVersion: '1.1.0',
            releaseUrl: 'https://github.com/ahuiyuchuan/model-iq-radar/releases/tag/v1.1.0',
            notes: '公开发布说明',
        });
        return cleanup;
    });
    const wrapper = mount(Settings);
    await flushPromises();
    expect(wrapper.get('.nav-update-badge').text()).toBe('新版本');
    expect(wrapper.get('.nav-update-badge').attributes('title')).toContain('1.1.0');
    await wrapper
        .findAll('nav button')
        .find((button) => button.text().includes('关于'))!
        .trigger('click');
    expect(wrapper.text()).toContain('免安装版');
    expect(wrapper.text()).toContain(`v${version}`);
    expect(fake.call.mock.calls.some(([command]) => command === 'check_updates')).toBe(false);
    expect(wrapper.find('.about-update [role="alert"]').exists()).toBe(false);
    expect(wrapper.get('.settings-actions').text()).toContain('前往下载 v1.1.0');
    fake.updates.mock.calls[0][0]({
        status: 'no-release',
        currentVersion: '1.0.1',
        latestVersion: null,
        releaseUrl: 'https://github.com/ahuiyuchuan/model-iq-radar/releases',
        notes: '',
    });
    await flushPromises();
    expect(wrapper.find('.nav-update-badge').exists()).toBe(false);
    expect(wrapper.text()).not.toContain('尚无正式发布版本');
    expect(wrapper.text()).not.toContain('当前已是最新');
    expect(fake.updates).toHaveBeenCalledTimes(1);
    wrapper.unmount();
    expect(cleanup).toHaveBeenCalledOnce();
});

it('更新缓存读取失败保持静默，其他设置仍可使用', async () => {
    fake.updates.mockRejectedValue(new Error('网络或 IPC 失败'));
    const wrapper = mount(Settings);
    await flushPromises();
    expect(wrapper.text()).not.toContain('网络或 IPC 失败');
    expect(wrapper.find('.nav-update-badge').exists()).toBe(false);
    expect(wrapper.get('#profile-name').attributes('disabled')).toBeUndefined();
    wrapper.unmount();
});

it.each(['current', 'no-release'] as const)(
    '关于保留手动检查及真实结果，自动缓存不触发网络：%s',
    async (status) => {
        let finishCheck: (value: unknown) => void = () => {};
        fake.call.mockImplementation(async (command: string) => {
            if (command === 'get_snapshot') return structuredClone(fake.snapshot);
            if (command === 'check_updates')
                return new Promise((resolve) => {
                    finishCheck = resolve;
                });
        });
        const wrapper = mount(Settings);
        await flushPromises();
        await wrapper
            .findAll('nav button')
            .find((button) => button.text().includes('关于'))!
            .trigger('click');
        expect(wrapper.text()).not.toMatch(/静默|每小时/);
        expect(fake.call.mock.calls.some(([command]) => command === 'check_updates')).toBe(false);
        const check = wrapper
            .findAll('.settings-actions button')
            .find((button) => button.text() === '检查更新')!;
        await check.trigger('click');
        expect(check.attributes('disabled')).toBeDefined();
        expect(wrapper.get('.about-update [role="status"]').text()).toContain('正在检查');
        await check.trigger('click');
        expect(fake.call.mock.calls.filter(([command]) => command === 'check_updates')).toHaveLength(1);
        finishCheck({
            status,
            currentVersion: '1.0.1',
            latestVersion: null,
            notes: '',
            releaseUrl: 'https://github.com/ahuiyuchuan/model-iq-radar/releases',
        });
        await flushPromises();
        expect(wrapper.get('.about-update [role="status"]').text()).toContain(
            status === 'current' ? '当前已是最新' : '尚无正式发布',
        );
        expect(wrapper.get('.about-update').text()).toContain('上次检查：');
        expect(check.attributes('disabled')).toBeUndefined();
        wrapper.unmount();
    },
);

it('手动检查失败可重试且保留更新入口，迟到响应不覆盖较新的后台状态', async () => {
    let finishCheck: (value: unknown) => void = () => {};
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'get_snapshot') return structuredClone(fake.snapshot);
        if (command === 'check_updates') throw new Error('模拟检查失败');
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper
        .findAll('nav button')
        .find((button) => button.text().includes('关于'))!
        .trigger('click');
    const available = {
        status: 'available',
        currentVersion: '1.0.1',
        latestVersion: '1.2.0',
        releaseUrl: 'https://github.com/ahuiyuchuan/model-iq-radar/releases/tag/v1.2.0',
        notes: '新发布说明',
    };
    fake.updates.mock.calls[0][0](available);
    await flushPromises();
    const check = wrapper.findAll('.settings-actions button').find((button) => button.text() === '检查更新')!;
    await check.trigger('click');
    await flushPromises();
    expect(wrapper.get('.about-update [role="alert"]').text()).toContain('模拟检查失败');
    expect(wrapper.get('.nav-update-badge').attributes('title')).toContain('1.2.0');
    expect(wrapper.get('.settings-actions').text()).toContain('前往下载 v1.2.0');
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'check_updates')
            return new Promise((resolve) => {
                finishCheck = resolve;
            });
    });
    await check.trigger('click');
    expect(wrapper.find('.about-update [role="alert"]').exists()).toBe(false);
    fake.updates.mock.calls[0][0]({ ...available, latestVersion: '1.3.0' });
    finishCheck({ ...available, latestVersion: '1.2.0' });
    await flushPromises();
    expect(wrapper.get('.about-update').text()).toContain('v1.3.0');
    expect(wrapper.get('.nav-update-badge').attributes('title')).toContain('1.3.0');
    wrapper.unmount();
});

it('外部强度只合并未编辑字段，保留名称与用户主动选择的强度草稿', async () => {
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.get('#profile-name').setValue('未保存名称');
    fake.snapshot.config.profiles[0].reasoningEffort = 'medium';
    fake.subscribe.mock.calls[0][0]();
    await flushPromises();
    expect(wrapper.get('[aria-label="思考强度"]').text()).toBe('medium');
    expect(wrapper.get<HTMLInputElement>('#profile-name').element.value).toBe('未保存名称');
    await wrapper.get('[aria-label="思考强度"]').trigger('click');
    await flushPromises();
    await wrapper
        .findAll('[role="option"]')
        .find((option) => option.text() === 'high')!
        .trigger('click');
    await flushPromises();
    fake.snapshot.config.profiles[0].reasoningEffort = null;
    fake.subscribe.mock.calls[0][0]();
    await flushPromises();
    expect(wrapper.get('[aria-label="思考强度"]').text()).toBe('high');
    expect(wrapper.get<HTMLInputElement>('#profile-name').element.value).toBe('未保存名称');
    wrapper.unmount();
});

it('拉取目录只更新草稿模型，连接测试不保存配置或产生测智请求', async () => {
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'get_snapshot') return structuredClone(fake.snapshot);
        if (command === 'fetch_models')
            return {
                token: 'catalog-one',
                models: [
                    { id: 'remote-one', ownedBy: 'fixture' },
                    { id: 'remote-two', ownedBy: null },
                ],
            };
        if (command === 'get_model_catalog')
            return [
                { id: 'remote-one', ownedBy: 'fixture' },
                { id: 'remote-two', ownedBy: null },
            ];
        if (command === 'test_connection') return { durationMs: 123 };
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('.connection-actions button')[0].trigger('click');
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith('fetch_models', {
        input: expect.objectContaining({ id: 'one', apiKey: '' }),
    });
    await wrapper.get('#model-name').setValue('');
    const catalog = wrapper.get('[aria-label="展开模型列表"]');
    await catalog.trigger('click');
    await wrapper.findAll('[role="option"]')[1].trigger('click');
    await flushPromises();
    expect(wrapper.get('#model-name').element).toHaveProperty('value', 'remote-two');
    await wrapper.findAll('.connection-actions button')[1].trigger('click');
    await flushPromises();
    expect(wrapper.get('.diagnostic-message').text()).toContain('连接成功');
    expect(fake.call.mock.calls.some(([cmd]) => ['save_profile', 'start_test'].includes(cmd))).toBe(false);
    await wrapper.get('#base-url').setValue('https://changed.example/v1');
    expect(wrapper.get('[aria-label="展开模型列表"]').attributes('disabled')).toBeDefined();
    wrapper.unmount();
});

it('历史状态和来源筛选只影响导出集合，并保留原始 Markdown', async () => {
    const source = '**原始 Markdown**\n```js\nconst n = 1;\n```';
    fake.snapshot.history = [
        {
            id: 'manual',
            profileId: 'one',
            profileName: '测试模型',
            model: 'test-model',
            startedAt: '2026-10-23T00:00:00Z',
            durationMs: 10,
            status: 'failed',
            source: 'manual',
            answer: source,
            error: null,
            prompt: '问题',
            keyword: '高市早苗',
            ruleVersion: 'literal-contains-v1',
        },
        {
            id: 'automatic',
            profileId: 'one',
            profileName: '测试模型',
            model: 'test-model',
            startedAt: '2026-10-23T00:01:00Z',
            durationMs: 10,
            status: 'passed',
            source: 'automatic',
            answer: '自动答案',
            error: null,
            prompt: '问题',
            keyword: '高市早苗',
            ruleVersion: 'literal-contains-v1',
        },
    ];
    const createObjectURL = vi.fn().mockReturnValue('blob:test');
    const revokeObjectURL = vi.fn();
    Object.defineProperty(URL, 'createObjectURL', { configurable: true, value: createObjectURL });
    Object.defineProperty(URL, 'revokeObjectURL', { configurable: true, value: revokeObjectURL });
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[3].trigger('click');
    expect(wrapper.findAll('.history-item')).toHaveLength(2);
    await wrapper.get('[aria-label="按结果筛选历史"]').trigger('click');
    await wrapper
        .findAll('[role="option"]')
        .find((item) => item.text().includes('降智') && !item.text().includes('不降智'))!
        .trigger('click');
    await wrapper.get('[aria-label="按测试来源筛选历史"]').trigger('click');
    await wrapper
        .findAll('[role="option"]')
        .find((item) => item.text().includes('手动测试'))!
        .trigger('click');
    expect(wrapper.findAll('.history-item')).toHaveLength(1);
    const exportButton = wrapper
        .findAll('.settings-actions button')
        .find((button) => button.text() === '导出');
    await exportButton!.trigger('click');
    expect(createObjectURL).toHaveBeenCalled();
    expect(click).toHaveBeenCalled();
    expect(revokeObjectURL).not.toHaveBeenCalled();
    wrapper.unmount();
    delete (URL as unknown as { createObjectURL?: unknown }).createObjectURL;
    delete (URL as unknown as { revokeObjectURL?: unknown }).revokeObjectURL;
    click.mockRestore();
});

it('修改接口期间到达的旧模型列表被丢弃', async () => {
    let resolveModels: (value: unknown) => void = () => {};
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'get_snapshot') return structuredClone(fake.snapshot);
        if (command === 'fetch_models')
            return new Promise((resolve) => {
                resolveModels = resolve;
            });
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('.connection-actions button')[0].trigger('click');
    await wrapper.get('#base-url').setValue('https://changed.example/v1');
    resolveModels({ token: 'stale-list', models: [{ id: 'stale', ownedBy: null }] });
    await flushPromises();
    expect(wrapper.get('[aria-label="展开模型列表"]').attributes('disabled')).toBeDefined();
    wrapper.unmount();
});

describe('自定义测试规则', () => {
    it('设置页使用统一深色选择器，新建仅清空草稿且不会提交表单或调整悬浮窗', async () => {
        const wrapper = mount(Settings);
        await flushPromises();
        expect(wrapper.find('select').exists()).toBe(false);
        const trigger = wrapper.get('[role="combobox"][aria-label="编辑接口配置"]');
        expect(trigger.text()).toContain('测试模型');
        await wrapper.get('#api-key').setValue('fictional-local-draft');
        await trigger.trigger('click');
        await flushPromises();
        expect(wrapper.findAll('[role="option"]')).toHaveLength(1);
        await trigger.trigger('keydown', { key: 'Escape' });
        await wrapper.get('[aria-label="新增接口配置"]').trigger('click');
        await flushPromises();
        expect(wrapper.find('[role="dialog"]').exists()).toBe(true);
        await wrapper
            .get('[role="dialog"]')
            .findAll('button')
            .find((button) => button.text() === '放弃修改')!
            .trigger('click');
        await flushPromises();
        expect(wrapper.get('#profile-name').element).toHaveProperty('value', '');
        expect(wrapper.get('#api-key').element).toHaveProperty('value', '');
        expect(fake.resize).not.toHaveBeenCalled();
        expect(fake.call.mock.calls.some(([command]) => command === 'save_profile')).toBe(false);
        expect(wrapper.get('.settings-actions button[type="submit"]').attributes('form')).toBe(
            'profile-settings',
        );
        wrapper.unmount();
    });

    it('只在保存时提交，并保留题面和关键词的有效空白', async () => {
        const wrapper = mount(Settings);
        await flushPromises();
        await wrapper.findAll('nav button')[1].trigger('click');
        await wrapper.get('#test-prompt').setValue('新问题\n请只按记忆回答。');
        await wrapper.get('#keyword').setValue(' Answer\nNext ');
        expect(fake.call.mock.calls.some(([command]) => command === 'save_rule')).toBe(false);
        await wrapper.get('form').trigger('submit');
        await flushPromises();
        expect(fake.call).toHaveBeenCalledWith('save_rule', {
            rule: { prompt: '新问题\n请只按记忆回答。', keyword: ' Answer\nNext ', timeoutSeconds: null },
            name: '默认规则',
        });
        expect(wrapper.text()).toContain('已保存');
        wrapper.unmount();
    });

    it('空白规则被拦截，恢复默认仅改变草稿', async () => {
        const wrapper = mount(Settings);
        await flushPromises();
        await wrapper.findAll('nav button')[1].trigger('click');
        await wrapper.get('#keyword').setValue('   ');
        await wrapper.get('form').trigger('submit');
        expect(wrapper.text()).toContain('不能');
        expect(fake.call.mock.calls.some(([command]) => command === 'save_rule')).toBe(false);
        await wrapper
            .findAll('button')
            .find((button) => button.text().includes('恢复默认'))!
            .trigger('click');
        expect((wrapper.get('#test-prompt').element as HTMLTextAreaElement).value).toBe('默认问题');
        expect(fake.call.mock.calls.some(([command]) => command === 'save_rule')).toBe(false);
        wrapper.unmount();
    });

    it('显示模型适用范围，服务异常保留规则草稿', async () => {
        const wrapper = mount(Settings);
        await flushPromises();
        await wrapper.findAll('nav button')[1].trigger('click');
        expect(wrapper.text()).toContain('适用于 GPT 6 及以上系列模型，其他模型无法保证测试准确度。');
        fake.call.mockImplementation(async (command: string) => {
            if (command === 'save_rule') throw new Error('磁盘空间不足');
            return structuredClone(fake.snapshot);
        });
        await wrapper.get('#keyword').setValue('保留这个草稿');
        await wrapper.get('form').trigger('submit');
        await flushPromises();
        expect(wrapper.get('[role="alert"]').text()).toContain('磁盘空间不足');
        expect((wrapper.get('#keyword').element as HTMLTextAreaElement).value).toBe('保留这个草稿');
        wrapper.unmount();
    });
});

/** 通过可见下拉选项操作，覆盖鼠标真实路径而非直接改内部响应式状态。 */
async function choose(wrapper: ReturnType<typeof mount>, label: string, text: string): Promise<void> {
    await wrapper.get(`[role="combobox"][aria-label="${label}"]`).trigger('click');
    await flushPromises();
    await wrapper
        .findAll('[role="option"]')
        .find((option) => option.text().trim() === text)!
        .trigger('click');
    await flushPromises();
}

it('不指定思考强度仍保留，旧接口超时不展示、不提交且不影响诊断超时', async () => {
    fake.snapshot.config.profiles[0].reasoningEffort = null;
    fake.snapshot.config.profiles[0].timeoutSeconds = 30;
    const wrapper = mount(Settings);
    await flushPromises();
    expect(wrapper.get('[aria-label="思考强度"]').text()).toContain('不指定');
    expect(wrapper.find('#profile-timeout').exists()).toBe(false);
    expect(wrapper.get('.connection-actions').element.parentElement?.textContent).toContain('60');
    await wrapper.get('#profile-settings').trigger('submit');
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith('save_profile', {
        input: expect.objectContaining({ reasoningEffort: null }),
    });
    const input = fake.call.mock.calls.find(([command]) => command === 'save_profile')![1].input;
    expect(input).not.toHaveProperty('timeoutSeconds');
    wrapper.unmount();
});

it('原生关闭与切页共用保存/放弃/取消，保存失败保留草稿与原页面', async () => {
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'get_snapshot') return structuredClone(fake.snapshot);
        if (command === 'save_profile') throw new Error('模拟保存失败');
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.get('#profile-name').setValue('未保存名称');
    const close = fake.nativeClose.mock.calls[0][0] as () => Promise<void>;
    void close();
    await flushPromises();
    expect(wrapper.get('[role="dialog"]').text()).toContain('有未保存的修改');
    await wrapper
        .get('[role="dialog"]')
        .findAll('button')
        .find((button) => button.text() === '取消')!
        .trigger('click');
    await flushPromises();
    expect(fake.closeSettings).not.toHaveBeenCalled();
    expect(wrapper.get('#profile-name').element).toHaveProperty('value', '未保存名称');
    await wrapper.findAll('nav button')[1].trigger('click');
    await wrapper
        .get('[role="dialog"]')
        .findAll('button')
        .find((button) => button.text() === '保存并继续')!
        .trigger('click');
    await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toContain('模拟保存失败');
    expect(wrapper.find('#profile-settings').exists()).toBe(true);
    expect(wrapper.get('#profile-name').element).toHaveProperty('value', '未保存名称');
    wrapper.unmount();
});

it('外部自动测试开关只同步对应字段，不覆盖其他偏好草稿', async () => {
    let changed: () => void = () => {};
    fake.subscribe.mockImplementation(async (handler: () => void) => {
        changed = handler;
        return () => {};
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[2].trigger('click');
    await flushPromises();
    await wrapper.get('#opacity').setValue('91');
    fake.snapshot.config.preferences.autoTestEnabled = false;
    changed();
    await flushPromises();
    expect(wrapper.get<HTMLInputElement>('#auto-test').element.checked).toBe(false);
    expect(wrapper.get('#opacity').element).toHaveProperty('value', '91');
    wrapper.unmount();
});

it('修改协议或强度会丢弃正在返回的旧连接成功结果', async () => {
    let resolve: (value: unknown) => void = () => {};
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'get_snapshot') return structuredClone(fake.snapshot);
        if (command === 'test_connection')
            return new Promise((done) => {
                resolve = done;
            });
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('.connection-actions button')[1].trigger('click');
    await choose(wrapper, '接口模式', 'Responses');
    resolve({ durationMs: 15 });
    await flushPromises();
    expect(wrapper.find('.diagnostic-message').exists()).toBe(false);
    expect(wrapper.find('.diagnostic-error').exists()).toBe(false);
    wrapper.unmount();
});

it('接口与规则共用选择器加新增入口，下拉只含已有项，删除操作固定在底栏', async () => {
    fake.snapshot.config.rulePresets = [
        { id: 'preset-a', name: '预设 A', prompt: '旧题目', keyword: '旧答案' },
    ];
    const wrapper = mount(Settings);
    await flushPromises();
    expect(wrapper.get('.profile-toolbar > button').attributes('aria-label')).toBe('新增接口配置');
    await wrapper.get('[aria-label="编辑接口配置"]').trigger('click');
    await flushPromises();
    expect(wrapper.findAll('[role="option"]')).toHaveLength(1);
    expect(wrapper.get('[role="listbox"]').text()).not.toContain('新建');
    await wrapper.get('[aria-label="编辑接口配置"]').trigger('keydown', { key: 'Escape' });
    await wrapper.findAll('nav button')[1].trigger('click');
    expect(wrapper.get('.profile-toolbar > button').attributes('aria-label')).toBe('新增规则预设');
    await wrapper.get('[aria-label="编辑测试规则"]').trigger('click');
    await flushPromises();
    expect(wrapper.findAll('[role="option"]').map((option) => option.text())).toEqual(['默认规则', '预设 A']);
    await wrapper.findAll('[role="option"]')[1].trigger('click');
    await flushPromises();
    expect(wrapper.get('.profile-toolbar').text()).not.toContain('删除');
    expect(wrapper.get('.settings-actions').text()).toContain('删除预设');
    expect(fake.call.mock.calls.some(([command]) => command.startsWith('save_'))).toBe(false);
    wrapper.unmount();
});

it('没有接口配置时禁用空选择器，独立新增按钮仍可编辑草稿', async () => {
    fake.snapshot.config.profiles = [];
    fake.snapshot.config.selectedProfileId = null;
    const wrapper = mount(Settings);
    await flushPromises();
    expect(wrapper.get<HTMLButtonElement>('[aria-label="编辑接口配置"]').element.disabled).toBe(true);
    expect(wrapper.get('[aria-label="编辑接口配置"]').text()).toBe('新建接口配置（未保存）');
    expect(wrapper.get<HTMLButtonElement>('[aria-label="新增接口配置"]').element.disabled).toBe(false);
    expect(wrapper.get('[aria-label="接口模式"]').text()).toBe('Responses');
    expect(wrapper.get<HTMLInputElement>('#protocol-fallback').element.checked).toBe(true);
    await wrapper.get('[aria-label="新增接口配置"]').trigger('click');
    await wrapper.get('#profile-name').setValue('新的接口草稿');
    expect(wrapper.get<HTMLInputElement>('#profile-name').element.value).toBe('新的接口草稿');
    expect(fake.call.mock.calls.some(([command]) => command === 'save_profile')).toBe(false);
    wrapper.unmount();
});

it.each(['取消', '放弃修改', '保存并继续'] as const)(
    '点击新增规则前保护现有草稿：%s；重复点击只出现一次决策',
    async (choice) => {
        fake.call.mockImplementation(
            async (command: string, args?: { rule: { prompt: string; keyword: string } }) => {
                if (command === 'get_snapshot') return structuredClone(fake.snapshot);
                if (command === 'save_rule') fake.snapshot.config.rule = structuredClone(args!.rule);
            },
        );
        const wrapper = mount(Settings);
        await flushPromises();
        await wrapper.findAll('nav button')[1].trigger('click');
        await wrapper.get('#keyword').setValue('未保存字符串');
        await wrapper.get('[aria-label="新增规则预设"]').trigger('click');
        await wrapper.get('[aria-label="新增规则预设"]').trigger('click');
        expect(wrapper.findAll('[role="dialog"]')).toHaveLength(1);
        await wrapper
            .get('[role="dialog"]')
            .findAll('button')
            .find((button) => button.text() === choice)!
            .trigger('click');
        await flushPromises();
        expect(wrapper.get<HTMLInputElement>('#preset-name').element.value).toBe(
            choice === '取消' ? '默认规则' : '',
        );
        if (choice === '取消') {
            expect(wrapper.get<HTMLTextAreaElement>('#keyword').element.value).toBe('未保存字符串');
            expect(wrapper.get('[aria-label="编辑测试规则"]').text()).toBe('默认规则');
        } else {
            expect(wrapper.get('[aria-label="编辑测试规则"]').text()).toBe('新建规则预设（未保存）');
            expect(wrapper.get<HTMLTextAreaElement>('#keyword').element.value).toBe('高市早苗');
        }
        expect(fake.call.mock.calls.filter(([command]) => command === 'save_rule')).toHaveLength(
            choice === '保存并继续' ? 1 : 0,
        );
        expect(fake.call.mock.calls.some(([command]) => command === 'save_rule_preset')).toBe(false);
        wrapper.unmount();
    },
);

it('新增规则只创建草稿，保存期间禁用新增与选择器，提交一次后显示真实预设', async () => {
    type Preset = NonNullable<AppSnapshot['config']['rulePresets']>[number];
    let finishSaving: () => void = () => {};
    fake.call.mockImplementation(async (command: string, args?: { preset: Preset }) => {
        if (command === 'get_snapshot') return structuredClone(fake.snapshot);
        if (command === 'save_rule_preset') {
            await new Promise<void>((resolve) => {
                finishSaving = resolve;
            });
            (fake.snapshot.config.rulePresets ??= []).push(structuredClone(args!.preset));
        }
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[1].trigger('click');
    await wrapper.get('[aria-label="新增规则预设"]').trigger('click');
    await flushPromises();
    expect(wrapper.get('[aria-label="编辑测试规则"]').text()).toBe('新建规则预设（未保存）');
    expect(fake.call.mock.calls.some(([command]) => command === 'save_rule_preset')).toBe(false);
    await wrapper.get('#preset-name').setValue('新增预设');
    await wrapper.get('#test-prompt').setValue('新的测试题目');
    await wrapper.get('#keyword').setValue('新的判定字符串');
    await wrapper.get('#rule-settings').trigger('submit');
    await wrapper.get('#rule-settings').trigger('submit');
    expect(wrapper.get<HTMLButtonElement>('[aria-label="新增规则预设"]').element.disabled).toBe(true);
    expect(wrapper.get<HTMLButtonElement>('[aria-label="编辑测试规则"]').element.disabled).toBe(true);
    expect(fake.call.mock.calls.filter(([command]) => command === 'save_rule_preset')).toHaveLength(1);
    finishSaving();
    await flushPromises();
    expect(wrapper.get('[aria-label="编辑测试规则"]').text()).toBe('新增预设');
    expect(fake.snapshot.config.rulePresets).toHaveLength(1);
    expect(fake.snapshot.config.rule).toEqual({ prompt: '原来的问题', keyword: '高市早苗' });
    expect(wrapper.get('.settings-actions').text()).toContain('删除预设');
    wrapper.unmount();
});

it('新增规则前保存旧草稿失败时保留原编辑对象与输入，不提前进入新建状态', async () => {
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'get_snapshot') return structuredClone(fake.snapshot);
        if (command === 'save_rule') throw new Error('模拟保存失败');
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[1].trigger('click');
    await wrapper.get('#keyword').setValue('需要保留的草稿');
    await wrapper.get('[aria-label="新增规则预设"]').trigger('click');
    await wrapper
        .get('[role="dialog"]')
        .findAll('button')
        .find((button) => button.text() === '保存并继续')!
        .trigger('click');
    await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toContain('模拟保存失败');
    expect(wrapper.get<HTMLInputElement>('#preset-name').element.value).toBe('默认规则');
    expect(wrapper.get('[aria-label="编辑测试规则"]').text()).toBe('默认规则');
    expect(wrapper.get<HTMLTextAreaElement>('#keyword').element.value).toBe('需要保留的草稿');
    wrapper.unmount();
});

it('已有规则预设保存保持 ID，引用规则删除必须明确确认原子解绑', async () => {
    fake.snapshot.config.rulePresets = [
        { id: 'preset-a', name: '预设 A', prompt: '旧题目', keyword: '旧答案' },
    ];
    fake.snapshot.config.profiles[0].rulePresetId = 'preset-a';
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[1].trigger('click');
    await choose(wrapper, '编辑测试规则', '预设 A');
    await wrapper.get('#test-prompt').setValue('新题目');
    await wrapper.get('#rule-settings').trigger('submit');
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith('save_rule_preset', {
        preset: { id: 'preset-a', name: '预设 A', prompt: '新题目', keyword: '旧答案', timeoutSeconds: null },
    });
    await wrapper
        .findAll('button')
        .find((button) => button.text().includes('删除预设'))!
        .trigger('click');
    expect(wrapper.get('[role="dialog"]').text()).toContain('1 个模型');
    expect(fake.call.mock.calls.some(([command]) => command === 'delete_rule_preset')).toBe(false);
    await wrapper
        .get('[role="dialog"]')
        .findAll('button')
        .find((button) => button.text() === '解绑并删除')!
        .trigger('click');
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith('delete_rule_preset', { id: 'preset-a', unbindProfiles: true });
    wrapper.unmount();
});

it('缩短历史范围先预览清理数量，取消不保存，确认才携带清理许可', async () => {
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'get_snapshot') return structuredClone(fake.snapshot);
        if (command === 'get_autostart') return false;
        if (command === 'preview_history_retention') return { removedCount: 7, remainingCount: 3 };
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[2].trigger('click');
    await wrapper.get('#history-max-records').setValue(3);
    await wrapper.get('#widget-settings').trigger('submit');
    await flushPromises();
    expect(wrapper.get('[role="dialog"]').text()).toContain('7 条历史');
    await wrapper
        .get('[role="dialog"]')
        .findAll('button')
        .find((button) => button.text() === '取消')!
        .trigger('click');
    await flushPromises();
    expect(fake.call.mock.calls.some(([command]) => command === 'save_preferences')).toBe(false);
    await wrapper.get('#widget-settings').trigger('submit');
    await flushPromises();
    await wrapper
        .get('[role="dialog"]')
        .findAll('button')
        .find((button) => button.text() === '确认清理并保存')!
        .trigger('click');
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith('save_preferences', {
        preferences: expect.objectContaining({ historyMaxRecords: 3 }),
        confirmCleanup: true,
        preserveAutoTest: true,
    });
    wrapper.unmount();
});

it('邮件试发使用草稿且不保存，失败保留授权码并保护离开页面', async () => {
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'get_snapshot') return structuredClone(fake.snapshot);
        if (command === 'send_test_email') throw new Error('模拟 SMTP 失败');
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper
        .findAll('nav button')
        .find((button) => button.text().includes('邮箱通知'))!
        .trigger('click');
    await wrapper.get('#smtp-host').setValue('smtp.example.com');
    await wrapper.get('#smtp-password').setValue('fictional-test-credential');
    await wrapper.get('#mail-recipients').setValue('one@example.com; two@example.com');
    await wrapper
        .findAll('button')
        .find((button) => button.text() === '发送测试邮件')!
        .trigger('click');
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith('send_test_email', {
        input: expect.objectContaining({
            password: 'fictional-test-credential',
            settings: expect.objectContaining({ recipients: ['one@example.com', 'two@example.com'] }),
        }),
    });
    expect(fake.call.mock.calls.some(([command]) => command === 'save_email_settings')).toBe(false);
    expect(wrapper.get('#smtp-password').element).toHaveProperty('value', 'fictional-test-credential');
    expect(wrapper.get('[role="alert"]').text()).toContain('SMTP 失败');
    await wrapper.findAll('nav button')[0].trigger('click');
    expect(wrapper.find('[role="dialog"]').exists()).toBe(true);
    wrapper.unmount();
});

it.each([null, '配置已保存，但悬浮窗退出吸边失败：窗口操作失败，请重新打开应用。'])(
    '配置导入先预览重复项，已提交的窗口警告不会变成导入失败：%s',
    async (warning) => {
        const backup = {
            version: 1,
            exportedAt: '2026-10-25T00:00:00Z',
            config: structuredClone(fake.snapshot.config),
            note: '不含密钥',
        };
        fake.call.mockImplementation(async (command: string) => {
            if (command === 'get_snapshot') return structuredClone(fake.snapshot);
            if (command === 'get_autostart') return false;
            if (command === 'preview_config_import')
                return {
                    profilesCount: 1,
                    presetsCount: 0,
                    duplicateProfiles: 1,
                    duplicatePresets: 0,
                    profilesMissingKeys: 1,
                    warnings: [],
                };
            if (command === 'preview_history_retention') return { removedCount: 0, remainingCount: 0 };
            if (command === 'import_config_backup') return { warning };
        });
        const wrapper = mount(Settings);
        await flushPromises();
        await wrapper.findAll('nav button')[2].trigger('click');
        const file = new File([JSON.stringify(backup)], 'backup.json', { type: 'application/json' });
        Object.defineProperty(file, 'text', { value: async () => JSON.stringify(backup) });
        const input = wrapper.get<HTMLInputElement>('[aria-label="选择配置备份"]');
        Object.defineProperty(input.element, 'files', { configurable: true, value: [file] });
        await input.trigger('change');
        await flushPromises();
        expect(wrapper.get('[role="dialog"]').text()).toContain('重复 1 个');
        expect(fake.call.mock.calls.some(([command]) => command === 'import_config_backup')).toBe(false);
        await choose(wrapper, '重复配置处理方式', '作为副本导入（不含密钥）');
        await wrapper
            .get('[role="dialog"]')
            .findAll('button')
            .find((button) => button.text() === '确认导入')!
            .trigger('click');
        await flushPromises();
        expect(fake.call).toHaveBeenCalledWith('import_config_backup', {
            backup,
            duplicateStrategy: 'copy',
            confirmCleanup: false,
        });
        expect(fake.call.mock.calls.filter(([command]) => command === 'import_config_backup')).toHaveLength(
            1,
        );
        expect(
            fake.call.mock.calls.some(([command]) =>
                ['save_profile', 'save_rule', 'save_preferences', 'save_rule_preset'].includes(command),
            ),
        ).toBe(false);
        expect(wrapper.text()).toContain('补填 API Key 和邮箱授权码');
        if (warning) expect(wrapper.get('.save-warning').text()).toContain(warning);
        expect(wrapper.find('.snapshot-recovery').exists()).toBe(false);
        wrapper.unmount();
    },
);

it('偏好已提交但原生窗口恢复失败时仍同步保存基线，离页不要求重复保存', async () => {
    const warning = '配置已保存，但悬浮窗退出吸边失败：窗口操作失败，请重新打开应用。';
    fake.call.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
        if (command === 'get_snapshot') return structuredClone(fake.snapshot);
        if (command === 'get_autostart') return false;
        if (command === 'preview_history_retention') return { removedCount: 0, remainingCount: 0 };
        if (command === 'save_preferences') {
            fake.snapshot.config.preferences = structuredClone(
                args?.preferences,
            ) as AppSnapshot['config']['preferences'];
            return { warning };
        }
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[2].trigger('click');
    await wrapper.get('#snap-edge').setValue(false);
    await wrapper.get('#widget-settings').trigger('submit');
    await flushPromises();
    expect(wrapper.get('.save-warning').text()).toContain(warning);
    expect(wrapper.get<HTMLInputElement>('#snap-edge').element.checked).toBe(false);
    await wrapper.findAll('nav button')[3].trigger('click');
    await flushPromises();
    expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
    expect(wrapper.get('.save-warning').text()).toContain(warning);
    expect(fake.call.mock.calls.filter(([command]) => command === 'save_preferences')).toHaveLength(1);
    wrapper.unmount();
});

it('旧历史 null 强度显示未记录，新协议快照的 null 才显示不指定', async () => {
    const legacy = {
        id: 'old',
        profileId: 'one',
        profileName: '测试模型',
        model: 'old-model',
        startedAt: '2026-10-25T00:00:00Z',
        durationMs: 10,
        status: 'passed' as const,
        answer: '高市早苗',
        error: null,
        prompt: '题目',
        keyword: '高市早苗',
        ruleVersion: 'literal-contains-v1',
        reasoningEffort: null,
    };
    fake.snapshot.history = [
        legacy,
        {
            ...legacy,
            id: 'new',
            model: 'new-model',
            interfaceMode: 'responses',
            source: 'manual',
            conditionFingerprint: 'sha256:fixture',
        },
    ];
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[3].trigger('click');
    await wrapper.findAll('.history-record-info')[0].trigger('click');
    expect(wrapper.get('.history-detail').text()).not.toContain('不指定');
    expect(wrapper.get('.history-detail').text()).toContain('未记录');
    await wrapper
        .findAll('.settings-actions button')
        .find((button) => button.text().includes('返回记录'))!
        .trigger('click');
    await wrapper.findAll('.history-record-info')[1].trigger('click');
    expect(wrapper.get('.history-detail').text()).toContain('不指定');
    wrapper.unmount();
});

it('邮件发送期间禁用底栏保存且禁止切页，失败后恢复按钮并保留草稿', async () => {
    let rejectSend: (reason: Error) => void = () => {};
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'get_snapshot') return structuredClone(fake.snapshot);
        if (command === 'send_test_email')
            return new Promise((_resolve, reject) => {
                rejectSend = reject;
            });
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper
        .findAll('nav button')
        .find((button) => button.text().includes('邮箱通知'))!
        .trigger('click');
    await wrapper.get('#smtp-password').setValue('fictional-in-flight-password');
    await wrapper
        .findAll('button')
        .find((button) => button.text() === '发送测试邮件')!
        .trigger('click');
    const saveButton = wrapper.get<HTMLButtonElement>('.settings-actions [form="mail-settings"]');
    expect(saveButton.element.disabled).toBe(true);
    expect(saveButton.text()).toContain('正在处理');
    await saveButton.trigger('click');
    expect(fake.call.mock.calls.some(([command]) => command === 'save_email_settings')).toBe(false);
    await wrapper.findAll('nav button')[0].trigger('click');
    expect(wrapper.find('#mail-settings').exists()).toBe(true);
    expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
    expect(wrapper.text()).toContain('邮件操作正在进行');
    rejectSend(new Error('模拟连接失败'));
    await flushPromises();
    expect(saveButton.element.disabled).toBe(false);
    expect(saveButton.text()).toContain('保存邮件设置');
    expect(wrapper.get('#smtp-password').element).toHaveProperty('value', 'fictional-in-flight-password');
    await wrapper.findAll('nav button')[0].trigger('click');
    expect(wrapper.find('[role="dialog"]').exists()).toBe(true);
    wrapper.unmount();
});

it('历史清理预览期间收到托盘开关变更，保存不会撤销较新的开关状态', async () => {
    let changed: () => void = () => {};
    let finishPreview: (value: unknown) => void = () => {};
    fake.subscribe.mockImplementation(async (handler: () => void) => {
        changed = handler;
        return () => {};
    });
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'get_snapshot') return structuredClone(fake.snapshot);
        if (command === 'get_autostart') return false;
        if (command === 'preview_history_retention')
            return new Promise((resolve) => {
                finishPreview = resolve;
            });
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[2].trigger('click');
    await wrapper.get('#opacity').setValue('93');
    await wrapper.get('#widget-settings').trigger('submit');
    expect(wrapper.get<HTMLInputElement>('#auto-interval').element.disabled).toBe(true);
    fake.snapshot.config.preferences.autoTestEnabled = false;
    changed();
    await flushPromises();
    finishPreview({ removedCount: 0, remainingCount: 0 });
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith('save_preferences', {
        preferences: expect.objectContaining({ opacity: 93, autoTestEnabled: false }),
        confirmCleanup: false,
        preserveAutoTest: true,
    });
    wrapper.unmount();
});

it.each([false, true])('保存偏好仅在用户明确编辑开关时覆盖权威定时状态：edited=%s', async (edited) => {
    fake.call.mockImplementation(
        async (
            command: string,
            args?: { preferences: Record<string, unknown>; preserveAutoTest: boolean },
        ) => {
            if (command === 'get_snapshot') return structuredClone(fake.snapshot);
            if (command === 'get_autostart') return false;
            if (command === 'preview_history_retention') return { removedCount: 0, remainingCount: 0 };
            if (command === 'save_preferences') {
                // 模拟托盘已写入而事件尚未送达。后端须在锁内决定是否保留，不能依赖旧快照。
                fake.snapshot.config.preferences.autoTestEnabled = args!.preserveAutoTest
                    ? false
                    : Boolean(args!.preferences.autoTestEnabled);
            }
        },
    );
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[2].trigger('click');
    await wrapper.get('#opacity').setValue('94');
    if (edited) await wrapper.get('#auto-test').setValue(false);
    await wrapper.get('#widget-settings').trigger('submit');
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith('save_preferences', {
        preferences: expect.objectContaining({ opacity: 94, autoTestEnabled: !edited }),
        confirmCleanup: false,
        preserveAutoTest: !edited,
    });
    expect(wrapper.get<HTMLInputElement>('#auto-test').element.checked).toBe(false);
    wrapper.unmount();
});

it('历史无搜索词时不读取长回答，输入搜索后仍按原文匹配', async () => {
    let reads = 0;
    const record = {
        id: 'large',
        profileId: 'one',
        profileName: '测试模型',
        model: 'fixture',
        startedAt: '2026-10-25T00:00:00Z',
        durationMs: 1,
        status: 'passed' as const,
        prompt: '题目',
        keyword: '目标',
        answer: '',
        error: null,
        ruleVersion: 'literal-contains-v1',
    };
    Object.defineProperty(record, 'answer', {
        enumerable: true,
        get: () => {
            reads++;
            return 'x'.repeat(1024 * 1024) + 'TARGET';
        },
    });
    const wrapper = mount(HistoryPanel, { props: { history: [record], profiles: [], modelValue: null } });
    await flushPromises();
    expect(wrapper.findAll('.history-item')).toHaveLength(1);
    // Vue Test Utils 初始化 props 可读取 getter；只观察实际日期过滤导致的正文访问。
    reads = 0;
    await wrapper.get('[aria-label="历史开始日期"]').setValue('2026-10-25');
    expect(reads).toBe(0);
    await wrapper.get('[aria-label="搜索历史"]').setValue('target');
    expect(wrapper.findAll('.history-item')).toHaveLength(1);
    expect(reads).toBeGreaterThan(0);
    wrapper.unmount();
});

it('原生关闭保护注册失败显示真实边界，不产生未处理拒绝', async () => {
    fake.nativeClose.mockRejectedValueOnce(new Error('模拟监听权限失败'));
    const wrapper = mount(Settings);
    await flushPromises();
    expect(wrapper.get('.inline-error').text()).toContain('未能启用原生关闭保护');
    expect(wrapper.get('.inline-error').text()).toContain('请先保存');
    wrapper.unmount();
});

it('保存配置期间锁住已拉取目录，防止选择结果被保存基线吞掉', async () => {
    let finishSaving: () => void = () => {};
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'get_snapshot') return structuredClone(fake.snapshot);
        if (command === 'fetch_models')
            return { token: 'catalog-one', models: [{ id: 'remote-one', ownedBy: null }] };
        if (command === 'get_model_catalog') return [{ id: 'remote-one', ownedBy: null }];
        if (command === 'save_profile')
            await new Promise<void>((resolve) => {
                finishSaving = resolve;
            });
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('.connection-actions button')[0].trigger('click');
    await flushPromises();
    await wrapper.get('#profile-settings').trigger('submit');
    expect(wrapper.get<HTMLButtonElement>('[aria-label="展开模型列表"]').element.disabled).toBe(true);
    await wrapper.get('[aria-label="展开模型列表"]').trigger('click');
    expect(wrapper.find('[role="listbox"]').exists()).toBe(false);
    finishSaving();
    await flushPromises();
    wrapper.unmount();
});

it('新建配置写入成功但快照失败时禁止重复创建，只重读状态后恢复编辑', async () => {
    let failSnapshot = false;
    fake.call.mockImplementation(async (command: string, args?: { input: Record<string, unknown> }) => {
        if (command === 'get_snapshot') {
            if (failSnapshot) throw new Error('模拟读取失败');
            return structuredClone(fake.snapshot);
        }
        if (command === 'save_profile') {
            // 与真实快照一样只暴露非敏感配置，不能把 apiKey 草稿混进测试快照。
            fake.snapshot.config.profiles.push({
                id: 'created',
                name: String(args!.input.name),
                baseUrl: String(args!.input.baseUrl),
                model: String(args!.input.model),
                hasKey: false,
            });
            fake.snapshot.config.selectedProfileId = 'created';
            failSnapshot = true;
        }
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper
        .findAll('.profile-toolbar button')
        .find((button) => button.text().includes('新增'))!
        .trigger('click');
    await wrapper.get('#profile-name').setValue('新配置');
    await wrapper.get('#base-url').setValue('https://example.com/v1');
    await wrapper.get('#model-name').setValue('new-model');
    await wrapper.get('#profile-settings').trigger('submit');
    await flushPromises();
    expect(wrapper.get('.snapshot-recovery').text()).toContain('更改已保存');
    expect(wrapper.findAll('[role="alert"]')).toHaveLength(1);
    expect(wrapper.get('.settings-scroll').attributes('inert')).toBeDefined();
    await wrapper.get('#profile-settings').trigger('submit');
    expect(fake.call.mock.calls.filter(([command]) => command === 'save_profile')).toHaveLength(1);
    failSnapshot = false;
    await wrapper.get('.snapshot-recovery button').trigger('click');
    await flushPromises();
    expect(wrapper.find('.snapshot-recovery').exists()).toBe(false);
    expect(wrapper.find('.inline-error').exists()).toBe(false);
    expect(wrapper.get<HTMLInputElement>('#profile-name').element.value).toBe('新配置');
    expect(wrapper.get('[aria-label="编辑接口配置"]').text()).toContain('新配置');
    wrapper.unmount();
});

it('原生关闭去重，销毁失败在窗口显示错误并允许再次关闭', async () => {
    let rejectClose: (reason: Error) => void = () => {};
    fake.closeSettings.mockImplementationOnce(
        () =>
            new Promise((_resolve, reject) => {
                rejectClose = reject;
            }),
    );
    const wrapper = mount(Settings);
    await flushPromises();
    const onClose = fake.nativeClose.mock.calls[0][0] as () => Promise<void>;
    const pending = onClose();
    await onClose();
    expect(fake.closeSettings).toHaveBeenCalledTimes(1);
    rejectClose(new Error('模拟原生窗口失败'));
    await pending;
    await flushPromises();
    expect(wrapper.get('.inline-error').text()).toContain('关闭设置失败');
    await onClose();
    expect(fake.closeSettings).toHaveBeenCalledTimes(2);
    wrapper.unmount();
});

it('导入已提交但快照失败时不误报导入失败，不重复导入且仍可关闭窗口', async () => {
    const backup = { version: 1, config: structuredClone(fake.snapshot.config) };
    let committed = false;
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'get_snapshot') {
            if (committed) throw new Error('模拟导入后的读取失败');
            return structuredClone(fake.snapshot);
        }
        if (command === 'get_autostart') return false;
        if (command === 'preview_config_import')
            return {
                profilesCount: 1,
                presetsCount: 0,
                duplicateProfiles: 1,
                duplicatePresets: 0,
                profilesMissingKeys: 1,
                warnings: [],
            };
        if (command === 'preview_history_retention') return { removedCount: 0, remainingCount: 0 };
        if (command === 'import_config_backup') committed = true;
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[2].trigger('click');
    const file = new File([JSON.stringify(backup)], 'backup.json', { type: 'application/json' });
    Object.defineProperty(file, 'text', { value: async () => JSON.stringify(backup) });
    const input = wrapper.get<HTMLInputElement>('[aria-label="选择配置备份"]');
    Object.defineProperty(input.element, 'files', { configurable: true, value: [file] });
    await input.trigger('change');
    await flushPromises();
    await wrapper
        .get('[role="dialog"]')
        .findAll('button')
        .find((button) => button.text() === '确认导入')!
        .trigger('click');
    await flushPromises();
    expect(wrapper.get('.snapshot-recovery').text()).toContain('配置已导入');
    expect(wrapper.get('.snapshot-recovery').text()).not.toContain('导入失败');
    expect(wrapper.findAll('[role="alert"]')).toHaveLength(1);
    await input.trigger('change');
    await flushPromises();
    expect(fake.call.mock.calls.filter(([command]) => command === 'import_config_backup')).toHaveLength(1);
    await (fake.nativeClose.mock.calls[0][0] as () => Promise<void>)();
    expect(fake.closeSettings).toHaveBeenCalledTimes(1);
    wrapper.unmount();
});

it('邮件保存并继续等待权威快照，重进页面使用保存结果且没有重复提交', async () => {
    let persisted = false;
    let finishSnapshot: () => void = () => {};
    fake.call.mockImplementation(async (command: string, args?: { input: { settings: object } }) => {
        if (command === 'get_snapshot') {
            if (persisted)
                await new Promise<void>((resolve) => {
                    finishSnapshot = resolve;
                });
            return structuredClone(fake.snapshot);
        }
        if (command === 'save_email_settings') {
            fake.snapshot.config.emailNotifications = {
                ...args!.input.settings,
                hasPassword: true,
            } as NonNullable<AppSnapshot['config']['emailNotifications']>;
            persisted = true;
        }
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper
        .findAll('nav button')
        .find((button) => button.text() === '邮箱通知')!
        .trigger('click');
    await wrapper.get('#smtp-host').setValue('smtp.saved.example');
    await wrapper.get('#smtp-password').setValue('fictional-saved-password');
    await wrapper.findAll('nav button')[0].trigger('click');
    await wrapper
        .get('[role="dialog"]')
        .findAll('button')
        .find((button) => button.text() === '保存并继续')!
        .trigger('click');
    await flushPromises();
    expect(wrapper.find('#mail-settings').exists()).toBe(true);
    expect(wrapper.get<HTMLInputElement>('#smtp-password').element.value).toBe('');
    await wrapper.get('#mail-settings').trigger('submit');
    expect(fake.call.mock.calls.filter(([command]) => command === 'save_email_settings')).toHaveLength(1);
    finishSnapshot();
    await flushPromises();
    expect(wrapper.find('#profile-settings').exists()).toBe(true);
    await wrapper
        .findAll('nav button')
        .find((button) => button.text() === '邮箱通知')!
        .trigger('click');
    expect(wrapper.get<HTMLInputElement>('#smtp-host').element.value).toBe('smtp.saved.example');
    expect(wrapper.get<HTMLInputElement>('#smtp-password').attributes('placeholder')).toContain('已安全保存');
    wrapper.unmount();
});

it('邮件已保存但读取失败进入只重读恢复态，不能离页回填旧设置或重复保存', async () => {
    let persisted = false;
    let failSnapshot = true;
    fake.call.mockImplementation(async (command: string, args?: { input: { settings: object } }) => {
        if (command === 'get_snapshot') {
            if (persisted && failSnapshot) throw new Error('模拟邮件保存后的读取失败');
            return structuredClone(fake.snapshot);
        }
        if (command === 'save_email_settings') {
            fake.snapshot.config.emailNotifications = {
                ...args!.input.settings,
                hasPassword: true,
            } as NonNullable<AppSnapshot['config']['emailNotifications']>;
            persisted = true;
        }
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper
        .findAll('nav button')
        .find((button) => button.text() === '邮箱通知')!
        .trigger('click');
    await wrapper.get('#smtp-host').setValue('smtp.saved.example');
    await wrapper.get('#smtp-password').setValue('fictional-saved-password');
    await wrapper.get('#mail-settings').trigger('submit');
    await flushPromises();
    expect(wrapper.get('.snapshot-recovery').text()).toContain('邮件设置已保存');
    expect(wrapper.get('.settings-scroll').attributes('inert')).toBeDefined();
    expect(wrapper.findAll('[role="alert"]')).toHaveLength(1);
    await wrapper.get('#mail-settings').trigger('submit');
    await wrapper.findAll('nav button')[0].trigger('click');
    await flushPromises();
    expect(wrapper.find('#mail-settings').exists()).toBe(true);
    expect(fake.call.mock.calls.filter(([command]) => command === 'save_email_settings')).toHaveLength(1);
    failSnapshot = false;
    await wrapper.get('.snapshot-recovery button').trigger('click');
    await flushPromises();
    expect(wrapper.find('.snapshot-recovery').exists()).toBe(false);
    expect(wrapper.get<HTMLInputElement>('#smtp-host').element.value).toBe('smtp.saved.example');
    expect(wrapper.get<HTMLInputElement>('#smtp-password').element.value).toBe('');
    wrapper.unmount();
});

/** 规则超时是可空的独立草稿，保存/还原/切换必须清除旧题残值。 */
it('规则超时按题保存，清空继承且切换题目不残留覆盖值', async () => {
    fake.snapshot.config.rule.timeoutSeconds = 80;
    fake.snapshot.config.rulePresets = [
        { id: 'timed', name: '限时题', prompt: '题', keyword: '答', timeoutSeconds: 120 },
    ];
    fake.call.mockImplementation(async (command: string, args?: Record<string, any>) => {
        if (command === 'get_snapshot') return structuredClone(fake.snapshot);
        if (command === 'save_rule') fake.snapshot.config.rule = structuredClone(args!.rule);
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[1].trigger('click');
    expect(wrapper.get<HTMLInputElement>('#rule-timeout').element.value).toBe('80');
    expect(wrapper.get('#rule-timeout').attributes('min')).toBe('1');
    expect(wrapper.get('#rule-timeout').attributes('max')).toBeUndefined();
    for (const invalid of ['0', '-1', '1.5', '9007199254740992']) {
        await wrapper.get('#rule-timeout').setValue(invalid);
        await wrapper.get('#rule-settings').trigger('submit');
        expect(fake.call.mock.calls.some(([command]) => command === 'save_rule')).toBe(false);
        expect(wrapper.text()).toContain('正整数秒');
    }
    for (const timeout of [1, 9, 600, 3600]) {
        await wrapper.get('#rule-timeout').setValue(String(timeout));
        await wrapper.get('#rule-settings').trigger('submit');
        await flushPromises();
        const saved = fake.call.mock.calls.filter(([command]) => command === 'save_rule');
        expect(saved.at(-1)![1].rule.timeoutSeconds).toBe(timeout);
    }
    await wrapper.get('#rule-timeout').setValue('');
    await wrapper.get('#rule-settings').trigger('submit');
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith('save_rule', {
        rule: expect.objectContaining({ timeoutSeconds: null }),
        name: '默认规则',
    });
    await choose(wrapper, '编辑测试规则', '限时题');
    expect(wrapper.get<HTMLInputElement>('#rule-timeout').element.value).toBe('120');
    await choose(wrapper, '编辑测试规则', '默认规则');
    expect(wrapper.get<HTMLInputElement>('#rule-timeout').element.value).toBe('');
    expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
    wrapper.unmount();
});

/** 全局值同样不能沿用旧上限，且空输入不具有规则超时的继承语义。 */
it('全局超时接受正整数秒，拒绝零、空值、小数和不安全整数', async () => {
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[2].trigger('click');
    expect(wrapper.get('#timeout').attributes('min')).toBe('1');
    expect(wrapper.get('#timeout').attributes('max')).toBeUndefined();
    for (const invalid of ['', '0', '-1', '1.5', '9007199254740992']) {
        await wrapper.get('#timeout').setValue(invalid);
        await wrapper.get('#widget-settings').trigger('submit');
        expect(fake.call.mock.calls.some(([command]) => command === 'save_preferences')).toBe(false);
    }
    for (const timeout of [1, 600, 3600]) {
        await wrapper.get('#timeout').setValue(String(timeout));
        await wrapper.get('#widget-settings').trigger('submit');
        await flushPromises();
        const saved = fake.call.mock.calls.filter(([command]) => command === 'save_preferences');
        expect(saved.at(-1)![1].preferences.timeoutSeconds).toBe(timeout);
    }
    wrapper.unmount();
});

it('连接诊断取草稿绑定的已保存规则超时，卡片换模型只合并未编辑模型字段', async () => {
    fake.snapshot.config.rule.timeoutSeconds = 90;
    fake.snapshot.config.rulePresets = [
        { id: 'timed', name: '限时题', prompt: '题', keyword: '答', timeoutSeconds: 120 },
    ];
    const wrapper = mount(Settings);
    await flushPromises();
    expect(wrapper.text()).toContain('90');
    await wrapper.get('#profile-name').setValue('名称草稿');
    fake.snapshot.config.profiles[0].model = 'from-widget';
    fake.subscribe.mock.calls[0][0]();
    await flushPromises();
    expect(wrapper.get<HTMLInputElement>('#model-name').element.value).toBe('from-widget');
    expect(wrapper.get<HTMLInputElement>('#profile-name').element.value).toBe('名称草稿');
    await choose(wrapper, '绑定规则预设', '限时题');
    expect(wrapper.text()).toContain('120');
    await wrapper.get('#model-name').setValue('model-draft');
    fake.snapshot.config.profiles[0].model = 'later-widget';
    fake.subscribe.mock.calls[0][0]();
    await flushPromises();
    expect(wrapper.get<HTMLInputElement>('#model-name').element.value).toBe('model-draft');
    wrapper.unmount();
});
