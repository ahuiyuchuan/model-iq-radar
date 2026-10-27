/**
 * 页面内 iframe 的文档包装和消息校验。
 * 宿主只接收有界状态，不读取模型 DOM、不生成截图；网络限制由原生响应头 CSP 执行。
 * @author ahui
 */
import { call } from './bridge';

// 使用足够容纳桌面作品的基准视口；实际页面更高/更宽时在首帧后有界扩展，再整体适配。
export const HTML_VIEWPORT = { width: 1440, height: 1000 } as const;
export const HTML_MAX_VIEWPORT = { width: 4096, height: 8192 } as const;
/** 在原文档坐标中的可见内容边界；仅裁去外围页面留白，不改变 SVG viewBox 或作品内部布局。 */
export interface PreviewContentBounds {
    x: number;
    y: number;
    width: number;
    height: number;
}

/**
 * 在沙箱自己的文档中测量一级可见内容，函数源码注入 iframe，不跨源读取模型 DOM。
 * 保留全部并列内容及 16px 阴影余量；全屏作品不裁剪，复杂/空页面返回 null 后沿用完整画布。
 * @returns 文档内有界矩形；null 表示无法可靠确定内容范围，不能据此隐藏或猜测作品内容。
 */
export function measurePreviewContent(): PreviewContentBounds | null {
    if (!document.body || document.body.childNodes.length > 256) return null;
    const rects: DOMRect[] = [];
    for (const node of document.body.childNodes) {
        if (node instanceof Element) {
            if (/^(SCRIPT|STYLE|META|LINK|TITLE|TEMPLATE|NOSCRIPT)$/.test(node.tagName)) continue;
            const style = getComputedStyle(node);
            if (style.display === 'none' || style.visibility === 'hidden') continue;
            const rect = node.getBoundingClientRect();
            if (rect.width > 0 && rect.height > 0) rects.push(rect);
        } else if (node.nodeType === Node.TEXT_NODE && node.textContent?.trim()) {
            const range = document.createRange();
            range.selectNode(node);
            if (typeof range.getBoundingClientRect !== 'function') return null;
            const rect = range.getBoundingClientRect();
            if (rect.width > 0 && rect.height > 0) rects.push(rect);
        }
    }
    if (!rects.length) return null;
    const width = document.documentElement.clientWidth;
    const height = document.documentElement.clientHeight;
    const x = Math.max(0, Math.floor(Math.min(...rects.map((rect) => rect.left)) - 16));
    const y = Math.max(0, Math.floor(Math.min(...rects.map((rect) => rect.top)) - 16));
    const right = Math.min(width, Math.ceil(Math.max(...rects.map((rect) => rect.right)) + 16));
    const bottom = Math.min(height, Math.ceil(Math.max(...rects.map((rect) => rect.bottom)) + 16));
    return right > x && bottom > y ? { x, y, width: right - x, height: bottom - y } : null;
}
export interface PreparedPreview {
    url: string;
    token: string;
    /** 仅释放内存文档；调用者必须同时移除 iframe 才会停止脚本。 */
    dispose(): void;
}

/**
 * 包装原始 HTML/SVG；不改写原始历史，也不加载截图或其他第三方运行库。
 * @param source 完整单个候选，UTF-8 不超过 1 MiB；无效内容拒绝，不截断。
 * @returns 随机文档 URL/消息令牌，调用方负责幂等释放。
 */
export async function preparePreview(source: string): Promise<PreparedPreview> {
    if (!source.trim() || new TextEncoder().encode(source).length > 1024 * 1024)
        throw new Error('HTML 为空或超过 1 MiB 预览上限，原始回答仍完整保留。');
    const token = crypto.randomUUID();
    // 可信引导先于模型代码执行；序列化只用于生成脚本文本，不把源码插入宿主 DOM。
    const bootstrap = `(() => {
        const token = ${JSON.stringify(token)};
        const send = (type, value = '') => parent.postMessage({ token, type, value }, '*');
        const measureContent = ${measurePreviewContent.toString()};
        // 不跨源读取 DOM；只回传 body/html 的计算背景色，让固定画布两侧沿用作品背景。
        const background = () => {
            for (const node of [document.body, document.documentElement]) {
                if (!node) continue;
                const color = getComputedStyle(node).backgroundColor;
                if (color && color !== 'transparent' && !/rgba\\([^)]*,\\s*0(?:\\.0+)?\\s*\\)/.test(color)) return color;
            }
            return '';
        };
        addEventListener('contextmenu', event => event.preventDefault(), true);
        addEventListener('keydown', event => { if (event.key === 'Escape') send('escape'); });
        addEventListener('resize', () => send('background', background()));
        addEventListener('load', () => {
            send('background', background());
            // 在完整页面布局后测量一次，不随动画逐帧追逐尺寸，避免 vw/vh 与扩展视口反馈循环。
            // 仅回传数字，宿主不读取跨源 DOM；原始正文、CSS 与作品自身的居中布局均保留。
            const measure = () => requestAnimationFrame(() => requestAnimationFrame(() => {
                const root = document.documentElement;
                const body = document.body;
                send('dimensions', {
                    width: Math.ceil(Math.max(${HTML_VIEWPORT.width}, root.scrollWidth, body?.scrollWidth || 0)),
                    height: Math.ceil(Math.max(${HTML_VIEWPORT.height}, root.scrollHeight, body?.scrollHeight || 0))
                });
                // 先让宿主更新有界视口再测内容位置，避免居中布局在扩展前后产生坐标偏移。
                requestAnimationFrame(() => requestAnimationFrame(() => {
                    send('content', measureContent());
                    requestAnimationFrame(() => requestAnimationFrame(() => send('ready', background())));
                }));
            }));
            if (document.fonts?.ready) document.fonts.ready.then(measure, measure);
            else measure();
        }, { once: true });
    })();`;
    const script = (value: string) => `<script>${value.replace(/<\/script/gi, '<\\/script')}</script>`;
    const document = `<!doctype html><html><head><meta charset="utf-8"><style>html,body{margin:0;min-height:100%;overflow:hidden!important}html{width:100%;height:100%}body{box-sizing:border-box}::-webkit-scrollbar{display:none!important}</style>${script(bootstrap)}</head><body>${source}</body></html>`;
    const url = await call<string>('prepare_html_preview', { document, source });
    let released = false;
    return {
        url,
        token,
        dispose() {
            if (released) return;
            released = true;
            void call('release_html_preview', { url }).catch(() => {});
        },
    };
}

/** 验证真实 iframe 来源及每次会话令牌；其他页面/旧任务消息不能改变当前预览。 */
export function previewMessage(
    event: MessageEvent,
    frame: HTMLIFrameElement | undefined,
    token: string,
): boolean {
    return Boolean(
        frame?.contentWindow && event.source === frame.contentWindow && event.data?.token === token,
    );
}
