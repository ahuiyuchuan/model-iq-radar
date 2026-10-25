/** 多答案契约回归，严格保留旧单答案空白，不借助真实模型服务。@author ahui */
import { expect, it } from 'vitest';
import { ruleAnswers, ruleError } from '../src/rules';

it('旧多行答案保持一项，替代答案按原文区分大小写', () => {
    expect(ruleAnswers({ keyword: ' A\nB ' })).toEqual([' A\nB ']);
    expect(ruleError({ prompt: '题', keyword: ' A\nB ', alternativeKeywords: ['a'] })).toBe('');
});
it('拒绝空白、精确重复、超长和超过20项，Unicode计数与后端一致', () => {
    expect(ruleError({ prompt: '题', keyword: 'A', alternativeKeywords: ['\n'] })).toContain('空白');
    expect(ruleError({ prompt: '题', keyword: 'A', alternativeKeywords: ['A'] })).toContain('重复');
    expect(ruleError({ prompt: '题', keyword: '😀'.repeat(500) })).toBe('');
    expect(ruleError({ prompt: '题', keyword: '😀'.repeat(501) })).toContain('500');
    expect(
        ruleError({
            prompt: '题',
            keyword: 'A',
            alternativeKeywords: Array.from({ length: 20 }, (_, i) => String(i)),
        }),
    ).toContain('20');
});
