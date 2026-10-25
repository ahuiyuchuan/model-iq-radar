/** 历史导出边界：无损 JSON 之外，CSV 必须阻止不可信文本成为公式。@author ahui */
import { describe, expect, it } from 'vitest';
import { historyCsv, localDate } from '../src/historyExport';
import type { TestRecord } from '../src/types';

describe('历史导出与本机日期', () => {
    const record = (answer: string): TestRecord => ({
        id: 'r',
        profileId: 'p',
        profileName: '分组',
        model: 'model',
        startedAt: '2026-10-25T00:00:00Z',
        durationMs: 10,
        status: 'failed',
        answer,
        error: null,
        prompt: '问题',
        keyword: '答案',
        ruleVersion: 'literal-contains-v1',
    });
    it.each(['=HYPERLINK("https://example.com")', '+cmd', '-1+2', '@SUM(A1)', '  =1+1', '\t=2+2'])(
        'CSV 转义公式前缀 %s，保留双引号与换行且不修改原记录',
        (source) => {
            const original = record(source + '\n"Markdown"');
            const csv = historyCsv([original]);
            expect(csv).toContain(`"'${source.replaceAll('"', '""')}\n""Markdown"""`);
            expect(original.answer).toBe(source + '\n"Markdown"');
            expect(JSON.parse(JSON.stringify(original)).answer).toBe(original.answer);
        },
    );
    it('正常长表格和代码原样包含在完整 CSV 单元格，不截断正文', () => {
        const source = '| 表格 |\n| --- |\n' + '```ts\n' + 'const value = "x";\n'.repeat(3000) + '```';
        expect(historyCsv([record(source)])).toContain(source.replaceAll('"', '""'));
    });
    it('日历日按本机日期字段计算，非法历史日期不匹配有效范围', () => {
        const date = new Date(2026, 9, 25, 0, 5, 0);
        expect(localDate(date.toISOString())).toBe('2026-10-25');
        expect(localDate('invalid')).toBe('');
    });
    it('多答案以JSON数组存进一个CSV单元格，保留换行、逗号和规则名称快照', () => {
        const original = {
            ...record('第二项命中'),
            alternativeKeywords: [' A, B\nC ', '😀'],
            ruleName: '糖果测试',
        };
        const csv = historyCsv([original]);
        expect(csv).toContain('alternativeKeywords,ruleName');
        expect(csv).toContain(JSON.stringify(original.alternativeKeywords).replaceAll('"', '""'));
        expect(csv).toContain('糖果测试');
    });
});
