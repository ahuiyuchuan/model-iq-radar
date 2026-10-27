/**
 * 使用当前 Vue 组件、虚构配置和用户提供的 HTML 生成 README 图片；不启动桌面 EXE 或调用外部服务。
 * 先安装项目锁定的 npm 依赖；另需 Playwright（或 README_PLAYWRIGHT_MODULE 指定其模块路径），
 * 可通过 README_CHROMIUM_EXECUTABLE 指定已有 Chrome/Chromium。运行：node scripts/generate_readme_screenshots.mjs。
 * 文档渲染只监听回环随机端口，拦截所有远程请求；独立临时浏览器上下文不读取用户会话或凭据。
 * @author ahui
 */
import { readFileSync, mkdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
import { createServer } from 'vite';
import vue from '@vitejs/plugin-vue';

const root = fileURLToPath(new URL('../', import.meta.url));
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.README_PLAYWRIGHT_MODULE || 'playwright');
const bridgePath = resolve(root, 'src/bridge.ts').replaceAll('\\', '/');
const output = resolve(root, 'docs/screenshots');
mkdirSync(output, { recursive: true });
const server = await createServer({
    root,
    configFile: false,
    logLevel: 'error',
    plugins: [
        {
            name: 'readme-only-memory-bridge',
            enforce: 'pre',
            load(id) {
                // 原桥接只以不同 query 导入内置题目常量；生产文件始终不改写。
                if (id.replaceAll('\\', '/') === bridgePath)
                    return readFileSync(new URL('./readme-preview-bridge.ts', import.meta.url), 'utf8');
            },
        },
        vue(),
    ],
    server: { host: '127.0.0.1', port: 0, strictPort: true, hmr: false, watch: null },
});
let browser;
const remoteRequests = [];
const errors = [];
try {
    await server.listen();
    const origin = `http://127.0.0.1:${server.httpServer.address().port}`;
    browser = await chromium.launch({
        headless: true,
        ...(process.env.README_CHROMIUM_EXECUTABLE
            ? { executablePath: process.env.README_CHROMIUM_EXECUTABLE }
            : {}),
    });
    const context = await browser.newContext({
        viewport: { width: 1080, height: 820 },
        deviceScaleFactor: 2,
        locale: 'zh-CN',
        timezoneId: 'Asia/Shanghai',
    });
    // 包括 iframe 在内只允许本次本机服务；所有潜在外部请求都拒绝并最终报告。
    await context.route('**/*', async (route) => {
        const url = new URL(route.request().url());
        if (url.origin === origin || ['data:', 'blob:'].includes(url.protocol)) await route.continue();
        else {
            remoteRequests.push(url.origin);
            await route.abort();
        }
    });
    const page = await context.newPage();
    page.on('pageerror', (error) => errors.push(error.message));

    /**
     * 等待真实组件及字体布局后保存，不修改组件样式、DOM 或回答内容。
     * @param {import('playwright').Locator} locator 当前组件的可见根节点。
     * @param {string} name 本脚本固定指定的 PNG 文件名，保存到 docs/screenshots。
     * @returns {Promise<void>} 截图文件成功保存后完成；浏览器或文件写入异常向外传播。
     */
    async function capture(locator, name) {
        await page.evaluate(() => document.fonts.ready);
        await locator.screenshot({
            path: resolve(output, name),
            animations: 'disabled',
            omitBackground: true,
        });
        console.log(name);
    }
    await page.setViewportSize({ width: 360, height: 572 });
    await page.goto(origin);
    await page.getByText('不降智', { exact: true }).waitFor();
    await capture(page.locator('.widget'), 'widget-compact.png');
    await page.getByRole('button', { name: '展开测试详情' }).click();
    await page.locator('.answer-box').getByText('21颗', { exact: true }).waitFor();
    await capture(page.locator('.widget'), 'widget-expanded.png');
    await page.goto(`${origin}/?example=html`);
    await page.getByRole('button', { name: '展开测试详情' }).click();
    await page.locator('.html-preview[aria-label]').waitFor();
    await page.waitForFunction(() => !document.querySelector('.html-preview-loading'));
    await capture(page.locator('.widget'), 'widget-html.png');

    // 模拟放大后的宿主可用空间，画面仍由原组件决定，不裁改用户 HTML 或重画其内容。
    await page.setViewportSize({ width: 1080, height: 820 });
    await page.getByRole('button', { name: '放大 HTML 预览', exact: true }).click();
    await page.locator('dialog[open]').waitFor();
    await page.waitForFunction(() => !document.querySelector('.html-preview-loading'));
    await capture(page.locator('dialog[open]'), 'html-expanded.png');

    await page.setViewportSize({ width: 1080, height: 1000 });
    await page.goto(`${origin}/?view=settings`);
    await page.locator('#profile-name').waitFor();
    await capture(page.locator('.settings'), 'api.png');
    for (const [label, name] of [
        ['测试规则', 'rules.png'],
        ['测试历史', 'history.png'],
        ['悬浮设置', 'settings.png'],
        ['关于', 'about.png'],
    ]) {
        if (label === '测试历史') await page.setViewportSize({ width: 1080, height: 820 });
        await page
            .getByRole('navigation', { name: '设置分类' })
            .getByRole('button', { name: label, exact: true })
            .click();
        if (label === '测试历史')
            await page.waitForFunction(() => document.querySelectorAll('.html-preview-loading').length === 0);
        await capture(page.locator('.settings'), name);
    }
    if (remoteRequests.length || errors.length) throw new Error(JSON.stringify({ remoteRequests, errors }));
    console.log(JSON.stringify({ generated: 9, remoteRequests: 0, pageErrors: 0 }));
} finally {
    await browser?.close();
    await server.close();
}
