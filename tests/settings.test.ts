/**
 * 设置规则的交互回归：区分草稿/保存、保留有效空白、空规则拦截与密钥边界。
 * 所有调用为本地 Mock，不使用真实密钥和第三方模型服务。@author ahui
 */
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import Settings from '../src/components/Settings.vue';
import type { AppSnapshot } from '../src/types';

const fake = vi.hoisted(() => ({
    call: vi.fn(),
    subscribe: vi.fn(),
    resize: vi.fn(),
    snapshot: {} as AppSnapshot,
}));
vi.mock('../src/bridge', () => ({
    desktop: true,
    DEFAULT_PROMPT: '默认问题',
    call: fake.call,
    subscribe: fake.subscribe,
    dragWindow: vi.fn(),
    windowAction: vi.fn(),
    resizeWidget: fake.resize,
}));

beforeEach(() => {
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
                shortcutEnabled: true,
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
    fake.resize.mockReset().mockResolvedValue(undefined);
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
    await wrapper.get('.history-item').trigger('click');
    expect(wrapper.get('.history-answer strong').text()).toBe('高市');
    expect(wrapper.get('.result-pill').text()).toBe('降智');
    await wrapper.findAll('.settings-actions button')[1].trigger('click');
    expect(writeText.mock.calls[0][0]).toContain(`回答：\n${source}`);
    expect(fake.snapshot.history[0].answer).toBe(source);
    wrapper.unmount();
});

it('关于页只手动检查更新，检查中去重，无发布和错误不能误报最新', async () => {
    let resolveUpdate: (value: unknown) => void = () => {};
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'get_snapshot') return structuredClone(fake.snapshot);
        if (command === 'check_updates')
            return new Promise((resolve) => {
                resolveUpdate = resolve;
            });
    });
    const wrapper = mount(Settings);
    await flushPromises();
    await wrapper.findAll('nav button')[4].trigger('click');
    expect(wrapper.text()).toContain('免安装版');
    expect(wrapper.text()).toContain('v1.0.0');
    expect(fake.call.mock.calls.some(([command]) => command === 'check_updates')).toBe(false);
    const check = wrapper.get('.settings-actions .button');
    await check.trigger('click');
    await check.trigger('click');
    expect(fake.call.mock.calls.filter(([command]) => command === 'check_updates')).toHaveLength(1);
    resolveUpdate({
        status: 'no-release',
        currentVersion: '1.0.0',
        latestVersion: null,
        releaseUrl: 'https://github.com/ahuiyuchuan/model-iq-radar/releases',
        notes: '',
    });
    await flushPromises();
    expect(wrapper.text()).toContain('尚无正式发布版本');
    expect(wrapper.text()).not.toContain('当前已是最新');
    fake.call.mockRejectedValue(new Error('检查失败'));
    await check.trigger('click');
    await flushPromises();
    expect(wrapper.get('.about-update [role="alert"]').text()).toContain('检查失败');
    wrapper.unmount();
});

it('拉取目录只更新草稿模型，连接测试不保存配置或产生测智请求', async () => {
    fake.call.mockImplementation(async (command: string) => {
        if (command === 'get_snapshot') return structuredClone(fake.snapshot);
        if (command === 'fetch_models')
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
    const catalog = wrapper.get('[aria-label="拉取到的模型"]');
    await catalog.trigger('click');
    await wrapper.findAll('[role="option"]')[1].trigger('click');
    await flushPromises();
    expect(wrapper.get('#model-name').element).toHaveProperty('value', 'remote-two');
    await wrapper.findAll('.connection-actions button')[1].trigger('click');
    await flushPromises();
    expect(wrapper.get('.diagnostic-message').text()).toContain('连接成功');
    expect(fake.call.mock.calls.some(([cmd]) => ['save_profile', 'start_test'].includes(cmd))).toBe(false);
    await wrapper.get('#base-url').setValue('https://changed.example/v1');
    expect(wrapper.find('[aria-label="拉取到的模型"]').exists()).toBe(false);
    wrapper.unmount();
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
    resolveModels([{ id: 'stale', ownedBy: null }]);
    await flushPromises();
    expect(wrapper.find('[aria-label="拉取到的模型"]').exists()).toBe(false);
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
        expect(wrapper.findAll('[role="option"]')).toHaveLength(2);
        await trigger.trigger('keydown', { key: 'Home' });
        await trigger.trigger('keydown', { key: 'Enter' });
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
            rule: { prompt: '新问题\n请只按记忆回答。', keyword: ' Answer\nNext ' },
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
