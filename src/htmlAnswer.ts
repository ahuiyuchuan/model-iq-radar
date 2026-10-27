/**
 * 回答自动识别与候选提取；与后端 html-v1 规则和公共回归样例保持一致。
 * 代码块优先，多个块保持独立；旧记录只临时解析展示，不改变保存的结论。
 * @author ahui
 */
import type { TestRecord } from './types';
export const HTML_PARSING_VERSION = 'html-v1';
/** 单个可独立运行的 HTML/SVG 源码，不拼接其他代码块；issue 非空时只能查看源码。 */
export interface HtmlCandidate {
    source: string;
    label: string;
    /** 未闭合代码块/文档不执行；源码仍可查看和复制。 */
    issue: string | null;
}
/** 分类只决定显示及是否跳过关键词判定，不代表内容质量，也不产生人工判定任务。 */
export interface AnswerClassification {
    kind: 'text' | 'html';
    candidates: HtmlCandidate[];
}
const roots =
    'html|head|body|main|section|article|div|style|script|svg|canvas|table|ul|ol|header|footer|form|p|span|h[1-6]|button|pre|code|blockquote|figure|details|dialog|label|textarea|select|video|audio';

/** 仅检查明确的截断边界；浏览器可修复普通 HTML 容错，不尝试评价动画内容质量。 */
function candidate(source: string, closed: boolean, index: number): HtmlCandidate {
    let issue = closed ? null : 'HTML 代码块未闭合';
    const root = source.match(/^\s*<(?:!doctype\s+html[^>]*>\s*<)?([a-z][a-z0-9]*)\b/i)?.[1]?.toLowerCase();
    if (!source.trim() || !/<[a-z][^>]*>/i.test(source)) issue = '没有可运行的 HTML';
    for (const tag of [
        'script',
        'style',
        ...(root &&
        ['html', 'svg', 'div', 'main', 'section', 'article', 'canvas', 'table', 'ul', 'ol', 'form'].includes(
            root,
        )
            ? [root]
            : []),
    ]) {
        const opens = source.match(new RegExp('<' + tag + '\\b[^>]*>', 'gi')) ?? [];
        const ends = source.match(new RegExp('</' + tag + '\\s*>', 'gi')) ?? [];
        if (opens.filter((value) => !value.endsWith('/>')).length > ends.length)
            issue = 'HTML 文档或标签未闭合';
    }
    return { source, label: `片段 ${index + 1}`, issue };
}

/**
 * 明确标注 html/htm/svg 的代码围栏直接识别；没有围栏时仅接受独立文档/片段。
 * 行内标签示例、普通单词 HTML/svg 和没有布局结构的单个 img 不会触发执行。
 * 不完整 HTML 仍识别为 HTML，用于禁止关键词判定和部分回答失败邮件。
 */
export function classifyAnswer(answer: string): AnswerClassification {
    const candidates: HtmlCandidate[] = [];
    const lines = answer.split(/\r?\n/);
    for (let i = 0; i < lines.length; i++) {
        if (!/^ {0,3}(?:`{3,}|~{3,})[ \t]*(?:html|htm|svg)[ \t]*$/i.test(lines[i])) continue;
        const fence = lines[i].trimStart().match(/^(?:`+|~+)/)![0];
        const start = ++i;
        const close = new RegExp('^ {0,3}' + fence[0] + '{' + fence.length + ',}[ \t]*$');
        while (i < lines.length && !close.test(lines[i])) i++;
        candidates.push(
            candidate(lines.slice(start, i).join('\n').trim(), i < lines.length, candidates.length),
        );
    }
    if (candidates.length) return { kind: 'html', candidates };
    let source = answer.trim().replace(/^<\?xml[^>]*>\s*/i, '');
    // 完整文档可带说明前缀；仅独立一行起始的文档标记才截取，防止执行行内示例。
    const doc = source.match(/(?:^|\n)[ \t]*(?=<!doctype\s+html\b|<html\b)/i);
    if (doc?.index != null) source = source.slice(doc.index).trim();
    if (new RegExp('^<(?:!doctype\\s+html\\b|' + roots + ')(?:[\\s>/]|$)', 'i').test(source)) {
        const end = /^<!doctype|^<html\b/i.test(source) ? source.toLowerCase().indexOf('</html>') : -1;
        if (end >= 0) source = source.slice(0, end + 7);
        candidates.push(candidate(source, true, 0));
    }
    return { kind: candidates.length ? 'html' : 'text', candidates };
}

/** 保存过识别类型的新记录以当时类型为准；旧历史临时识别，保留原状态。 */
export function isHtmlRecord(record: TestRecord): boolean {
    return record.answerType ? record.answerType === 'html' : classifyAnswer(record.answer).kind === 'html';
}

/** 请求失败/取消的部分源码只能看原文，不能运行，即使结构恰好完整。 */
export function previewAllowed(record: TestRecord): boolean {
    return record.status !== 'error' && record.status !== 'cancelled';
}
