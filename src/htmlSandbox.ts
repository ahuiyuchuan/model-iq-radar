/**
 * 页面内 iframe 的文档包装和消息校验。
 * 宿主只接收有界状态，不读取模型 DOM、不生成截图；网络限制由原生响应头 CSP 执行。
 * @author ahui
 */
import { call } from './bridge';

// 使用足够容纳桌面作品的基准视口；实际页面更高/更宽时在首帧后有界扩展，再整体适配。
export const HTML_VIEWPORT = { width: 1440, height: 1000 } as const;
export const HTML_MAX_VIEWPORT = { width: 4096, height: 8192 } as const;
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
                // 宿主接收尺寸后还需更新缩放；等待绘制再就绪，三处预览共用同一加载结束边界。
                requestAnimationFrame(() => requestAnimationFrame(() => send('ready', background())));
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
