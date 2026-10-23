/**
 * 不可信回答的单向展示管线：CommonMark + GFM → 本地高亮 → HTML 白名单净化。
 * 仅返回展示副本，不保存、不改写回答原文，也不参与关键词判定或复制。
 * 所有依赖随应用打包；图片只显示说明文字，解析期间不访问网络。@author ahui
 */
import MarkdownIt from 'markdown-it';
import taskLists from 'markdown-it-task-lists';
import DOMPurify from 'dompurify';
import hljs from 'highlight.js/lib/common';

/** 超长回答退回纯文本，避免 2 MiB 响应在同步解析/高亮时长期阻塞桌面交互。 */
const MAX_MARKDOWN_LENGTH = 200_000;
const MAX_HIGHLIGHT_LENGTH = 20_000;

/**
 * 外部链接白名单。只接受绝对 HTTP(S) 地址，拒绝凭据、控制字符及本地/脚本协议。
 * @param value 来自 Markdown 或关于页的 URL，不允许相对地址和协议相对地址。
 * @returns 规范化地址；被拒绝时为 null。此函数不会请求地址。
 */
export function externalUrl(value: string): string | null {
    if (value.length > 8192 || /[\u0000-\u0020\u007f]/.test(value) || !/^https?:\/\//i.test(value)) {
        return null;
    }
    try {
        const url = new URL(value);
        return url.hostname && !url.username && !url.password ? url.href : null;
    } catch {
        return null;
    }
}

const markdown = new MarkdownIt('commonmark', {
    html: false,
    linkify: true,
    typographer: false,
    breaks: false,
    maxNesting: 50,
    highlight(code, language) {
        // 不猜测语言、不动态下载语法；未知语言和超长代码保留转义后的原文。
        if (code.length > MAX_HIGHLIGHT_LENGTH || !language || !hljs.getLanguage(language)) return '';
        return hljs.highlight(code, { language, ignoreIllegals: true }).value;
    },
})
    .enable(['table', 'strikethrough', 'linkify'])
    .use(taskLists, { enabled: false, label: false });

markdown.validateLink = (url) => externalUrl(url) !== null;
// 不生成 img/src/srcset，包含 tracking URL、data URI 或本地路径的图片也不会加载。
markdown.renderer.rules.image = (tokens, index) =>
    `<span class="markdown-image-note">[图片：${markdown.utils.escapeHtml(tokens[index]?.content || '无说明')}，未加载]</span>`;
markdown.renderer.rules.table_open = () =>
    '<div class="markdown-table-scroll" role="region" aria-label="表格，可横向滚动" tabindex="0"><table>';
markdown.renderer.rules.table_close = () => '</table></div>\n';
const defaultFence = markdown.renderer.rules.fence!;
markdown.renderer.rules.fence = (...args) =>
    defaultFence(...args).replace('<pre>', '<pre tabindex="0" aria-label="代码块，可横向滚动">');

/**
 * 安全渲染。解析/高亮/净化抛错或超过预算时返回 null，由组件以 Vue 插值显示完整原文。
 * 净化白名单不能放开 style、事件属性、图片、iframe、SVG 或可编辑表单。
 * @param source 完整原始模型回答；只读，允许空字符串。
 * @returns 净化后的 HTML，或者需要纯文本回退的 null。
 */
export function renderMarkdown(source: string): string | null {
    if (source.length > MAX_MARKDOWN_LENGTH) return null;
    try {
        return DOMPurify.sanitize(markdown.render(source), {
            ALLOWED_TAGS: [
                'p',
                'br',
                'hr',
                'h1',
                'h2',
                'h3',
                'h4',
                'h5',
                'h6',
                'strong',
                'em',
                's',
                'blockquote',
                'ul',
                'ol',
                'li',
                'a',
                'table',
                'thead',
                'tbody',
                'tr',
                'th',
                'td',
                'pre',
                'code',
                'span',
                'div',
                'input',
            ],
            ALLOWED_ATTR: [
                'href',
                'title',
                'class',
                'align',
                'start',
                'type',
                'checked',
                'disabled',
                'role',
                'aria-label',
                'tabindex',
            ],
            ALLOW_DATA_ATTR: false,
            ALLOW_ARIA_ATTR: false,
            // 这些只由解析器生成的非 URL 属性需保留，避免 URI 白名单把 checkbox/对齐值误删。
            ADD_URI_SAFE_ATTR: ['type', 'align', 'start', 'role', 'aria-label', 'tabindex'],
            // markdown-it 先验证，净化器再次拒绝脚本/data/file 等地址；不留默认导航能力。
            ALLOWED_URI_REGEXP: /^https?:\/\//i,
        });
    } catch {
        return null;
    }
}
