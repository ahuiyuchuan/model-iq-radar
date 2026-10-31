/**
 * 在生产 dist 上执行真实 CSP/资源/保存前校验回归，防止仅验证开发服务器遗漏打包策略。
 * 依赖现有 Playwright/Chrome，先 npm run build，再 node scripts/verify_pet_csp.mjs。
 * PET_PLAYWRIGHT_MODULE/PET_CHROMIUM_EXECUTABLE 可指定本机工具。仅监听随机回环端口；
 * 不调用模型/SMTP、不访问用户数据库。进程退出关闭服务；失败退出非零。@author ahui
 */
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { resolve, extname } from 'node:path';
const root = fileURLToPath(new URL('../', import.meta.url));
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PET_PLAYWRIGHT_MODULE || 'playwright');
const config = JSON.parse(readFileSync(resolve(root, 'src-tauri/tauri.conf.json'), 'utf8'));
const policy = config.app.security.csp;
const server = createServer((request, response) => {
    const path = new URL(request.url, 'http://localhost').pathname;
    const file = resolve(root, 'dist', path === '/' ? 'index.html' : `.${path}`);
    if (
        !file.startsWith(
            resolve(root, 'dist') + '/'.replaceAll('/', process.platform === 'win32' ? '\\' : '/'),
        )
    ) {
        response.writeHead(403).end();
        return;
    }
    try {
        response.setHeader('Content-Security-Policy', policy);
        response.setHeader(
            'Content-Type',
            {
                '.html': 'text/html',
                '.js': 'text/javascript',
                '.css': 'text/css',
                '.json': 'application/json',
                '.png': 'image/png',
            }[extname(file)] || 'application/octet-stream',
        );
        response.end(readFileSync(file));
    } catch {
        response.writeHead(404).end();
    }
});
let browser;
try {
    await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
    const origin = `http://127.0.0.1:${server.address().port}`;
    browser = await chromium.launch({
        headless: true,
        ...(process.env.PET_CHROMIUM_EXECUTABLE
            ? { executablePath: process.env.PET_CHROMIUM_EXECUTABLE }
            : {}),
    });
    const context = await browser.newContext({
        viewport: { width: 1080, height: 850 },
        reducedMotion: 'reduce',
    });
    await context.route('**/*', async (route) => {
        if (new URL(route.request().url()).origin === origin) await route.continue();
        else await route.abort();
    });
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', (error) => errors.push(error.message));
    page.on('console', (message) => {
        if (/violates.*content security|refused.*content security/i.test(message.text()))
            errors.push(message.text());
    });
    await page.goto(`${origin}/?view=settings`);
    await page.getByRole('navigation').getByRole('button', { name: '皮肤管理', exact: true }).click();
    await page.getByRole('button', { name: /蓝鲸女仆/ }).click();
    await page
        .locator('.pet-actor[data-action="idle"]')
        .waitFor()
        .catch((cause) => {
            console.error(JSON.stringify({ textureOrCspErrors: errors }));
            throw cause;
        });
    // 真正读取画布两次，确认选中“启用动画”时不是静止贴图；页面模拟系统减少动态效果。
    const frame = async () => (await page.locator('canvas').screenshot()).toString('base64');
    const first = await frame();
    await page.waitForTimeout(650);
    assert.notEqual(await frame(), first, '待机动画没有持续播放');
    for (const [label, action] of [
        ['摸头', 'petting'],
        ['拖动', 'dragging'],
        ['测试中', 'working'],
        ['通过', 'success'],
        ['侧坐', 'seated'],
        ['招手', 'wave'],
        ['伸懒腰', 'stretch'],
        ['放下站稳', 'landing'],
        ['鞠躬', 'bow'],
        ['降智', 'failed'],
        ['测试失败', 'error'],
    ]) {
        await page.getByRole('button', { name: label, exact: true }).click();
        await page.locator(`canvas[data-action="${action}"]`).waitFor();
    }
    assert.equal(await page.getByText('桌宠遵循系统减少动态效果设置。无专用动作时使用待机姿态。').count(), 0);
    await page.getByRole('button', { name: '保存皮肤设置', exact: true }).click();
    await page.getByRole('status').filter({ hasText: '预览已更新' }).waitFor();
    assert.equal(await page.locator('.inline-error').count(), 0);
    // 实际损坏一个后续动作的 PNG：待机能显示也不能跳过整套预检。
    await page.route('**/skins/blue-whale-maid/working-atlas.png', (route) =>
        route.fulfill({ body: Buffer.from('invalid-png'), contentType: 'image/png' }),
    );
    await page.goto(`${origin}/?view=settings`);
    await page.getByRole('navigation').getByRole('button', { name: '皮肤管理', exact: true }).click();
    await page.getByRole('button', { name: /蓝鲸女仆/ }).click();
    await page.locator('canvas[data-action="idle"]').waitFor();
    await page.getByRole('button', { name: '保存皮肤设置', exact: true }).click();
    await page.getByRole('alert').filter({ hasText: '皮肤图片加载失败' }).first().waitFor();
    assert.equal(await page.getByRole('status').filter({ hasText: '预览已更新' }).count(), 0);
    await page.unroute('**/skins/blue-whale-maid/working-atlas.png');
    // 再现原故障：描述被拦时保存不能再出现成功提示，经典仍可恢复。
    await page.route('**/skins/blue-whale-maid/skin.json', (route) => route.abort());
    await page.goto(`${origin}/?view=settings`);
    await page.getByRole('navigation').getByRole('button', { name: '皮肤管理', exact: true }).click();
    await page.getByRole('button', { name: /蓝鲸女仆/ }).click();
    await page.getByText('预览不可用', { exact: true }).waitFor();
    await page.getByRole('button', { name: '保存皮肤设置', exact: true }).click();
    await page.getByRole('alert').filter({ hasText: '内置皮肤资源无法读取' }).first().waitFor();
    assert.equal(await page.getByRole('status').filter({ hasText: '预览已更新' }).count(), 0);
    await page.getByRole('button', { name: /经典雷达/ }).click();
    await page.getByRole('button', { name: '保存皮肤设置', exact: true }).click();
    await page.getByRole('status').filter({ hasText: '预览已更新' }).waitFor();
    assert.deepEqual(errors, []);
    console.log(
        JSON.stringify({
            productionCsp: true,
            decodedActions: 12,
            idleAnimates: true,
            failedLoadBlocksSave: true,
            failedImageBlocksSave: true,
            classicRecovery: true,
            pageErrors: 0,
        }),
    );
} finally {
    await browser?.close();
    server.close();
}
