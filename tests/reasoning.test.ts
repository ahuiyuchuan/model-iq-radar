/** 已核实档位与未知能力边界：选项展示不能静默改变实际请求条件。@author ahui */
import { describe, expect, it } from 'vitest';
import { reasoningSupport } from '../src/reasoning';

describe('模型思考强度支持范围', () => {
    it.each(['chat_completions', 'responses'] as const)(
        '%s 对精确已知模型应用官方档位，并保留不指定选项',
        (mode) => {
            const older = reasoningSupport('gpt-5.1', mode, 'low');
            expect(older.options.map((option) => option.id)).toEqual(['', 'low', 'medium', 'high']);
            expect(older.currentSupported).toBe(true);
            expect(older.recognized).toBe(true);
            expect(reasoningSupport('gpt-5.2', mode, 'xhigh').options.map((option) => option.id)).toEqual([
                '',
                'low',
                'medium',
                'high',
                'xhigh',
            ]);
        },
    );

    it('只认官方列出的快照，不以模型前缀推断 codex/pro/第三方能力', () => {
        expect(reasoningSupport('gpt-5.1-2025-11-13', undefined, 'low').recognized).toBe(true);
        expect(reasoningSupport('gpt-5.2-2025-12-11', undefined, 'low').recognized).toBe(true);
        for (const model of [
            'gpt-5.1-codex',
            'gpt-5.2-pro',
            'gpt-5.1-2099-01-01',
            'provider/gpt-5.1',
            'GPT-5.1',
            'o3',
            'o4-mini',
            'gpt-6',
            'custom-model',
            '__proto__',
            'toString',
        ]) {
            const support = reasoningSupport(model, 'responses', 'xhigh');
            expect(support.recognized).toBe(false);
            expect(support.currentSupported).toBeNull();
            expect(support.notice).toContain('未核实');
            expect(support.options.some((option) => option.id === 'xhigh')).toBe(true);
        }
    });

    it('旧配置的 xhigh 必须可见并明确标记，不能为了更换模型自动改写当前档位', () => {
        const current = 'xhigh';
        const support = reasoningSupport('gpt-5.1', 'responses', current);
        expect(support.currentSupported).toBe(false);
        expect(support.options.at(-1)).toMatchObject({ id: current, name: 'xhigh（当前值，官方不支持）' });
        expect(current).toBe('xhigh');
        expect(support.notice).toContain('不会自动换档');
        expect(support.notice).toContain('请求失败不会换档重试');
    });

    it('不指定永远保留为省略参数，不能作为 API 的 none 发送', () => {
        const support = reasoningSupport('gpt-5.1', undefined, null);
        expect(support.currentSupported).toBe(true);
        expect(support.options[0]).toEqual({ id: '', name: '不指定', model: '' });
        expect(support.options.some((option) => option.id === 'none')).toBe(false);
        expect(support.notice).toContain('不等同于关闭推理');
    });
});
