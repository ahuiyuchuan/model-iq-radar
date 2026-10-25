/**
 * 规则表单、试匹配与历史展示的共同契约；正式结果始终由后端判定。
 * 不按换行拆分旧 keyword，不修剪有效空白，不将答案加入模型请求。
 * @author ahui
 */
import type { TestRule } from './types';

/**
 * @param rule 已保存规则或当前草稿；缺少替代项时按单答案处理。
 * @returns 含首项的独立数组，保留原文顺序与空白，不修改传入对象。
 */
export function ruleAnswers(rule: Pick<TestRule, 'keyword' | 'alternativeKeywords'>): string[] {
    return [rule.keyword, ...(rule.alternativeKeywords ?? [])];
}

/**
 * @param rule 当前草稿；题目和答案长度按 Unicode 字符计算，与 Rust 一致。
 * @returns 首个可读校验错误；空串表示题目与全部答案均有效。
 */
export function ruleError(rule: TestRule): string {
    if (!rule.prompt.trim() || [...rule.prompt].length > 20000)
        return '题目必须为 1—20000 个字符，不能只有空白。';
    const answers = ruleAnswers(rule);
    if (answers.length > 20) return '每个题目最多设置 20 个判定答案。';
    if (answers.some((answer) => !answer.trim() || [...answer].length > 500))
        return '每个判定答案必须为 1—500 个字符，不能只有空白。';
    if (new Set(answers).size !== answers.length) return '判定答案不能完全重复。';
    return '';
}
