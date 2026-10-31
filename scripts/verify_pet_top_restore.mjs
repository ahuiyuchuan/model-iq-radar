/**
 * 顶部收纳往返回归：真实组件与鼠标，模拟原生锚点原点换算及可见区域夹紧。
 * 固定页面原点无法覆盖卡片右侧角色的横向跳位；生产数学逻辑另由Rust回归验证。本脚本
 * 使用本页内存桥接，拒绝远程请求；不控制系统窗口，不代验Win32穿透或物理DPI。
 * 运行方式与verify_pet_ui.mjs相同；PET_QA_SCREENSHOT_DIR指定独立证据目录。@author ahui
 */
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { createServer } from 'vite';
import vue from '@vitejs/plugin-vue';
const root = fileURLToPath(new URL('../', import.meta.url));
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PET_PLAYWRIGHT_MODULE || 'playwright');
const bridgePath = resolve(root, 'src/bridge.ts').replaceAll('\\', '/');
const output = process.env.PET_QA_SCREENSHOT_DIR || resolve(tmpdir(), 'model-iq-radar-top-anchor');
mkdirSync(output, { recursive: true });
const server = await createServer({
    root,
    configFile: false,
    logLevel: 'error',
    plugins: [
        {
            name: 'top-anchor-qa',
            enforce: 'pre',
            load(id) {
                if (id.replaceAll('\\', '/') === bridgePath)
                    return `export * from '/scripts/pet-preview-bridge.ts';`;
            },
        },
        vue(),
    ],
    server: { host: '127.0.0.1', port: 0, hmr: false, watch: null },
});
let browser, page;
const errors = [];
try {
    await server.listen();
    const origin = `http://127.0.0.1:${server.httpServer.address().port}`;
    browser = await chromium.launch({
        headless: true,
        ...(process.env.PET_CHROMIUM_EXECUTABLE
            ? { executablePath: process.env.PET_CHROMIUM_EXECUTABLE }
            : {}),
    });
    const context = await browser.newContext({
        viewport: { width: 1200, height: 1000 },
        deviceScaleFactor: 2,
        recordVideo: { dir: resolve(output, 'video'), size: { width: 1200, height: 1000 } },
    });
    await context.route('**/*', async (route) => {
        const url = new URL(route.request().url());
        if (url.origin === origin || ['data:', 'blob:'].includes(url.protocol)) await route.continue();
        else {
            errors.push(`remote:${url.origin}`);
            await route.abort();
        }
    });
    page = await context.newPage();
    page.on('pageerror', (e) => errors.push(e.message));
    const matrix = [];
    for (const side of ['right', 'left']) {
        for (const size of [96, 144, 192]) {
            for (const corner of [false, true]) {
                await page.goto(origin);
                await page.locator('.pet-actor').waitFor();
                await page.locator('.pet-result-cloud').click();
                if (size === 192)
                    await page.getByRole('button', { name: '展开测试详情', exact: true }).click();
                await page.mouse.move(600, 950);
                await page.evaluate(
                    async ({ side, size }) => {
                        const b = await import('/scripts/pet-preview-bridge.ts');
                        b.setQaAppearance(size, true);
                        // 用running保留探头云朵，确保角落测试包括云朵占据的实际宽度。
                        b.setQaRun('running');
                        window.qaPosition = [400, 80];
                        window.qaPrevious = b.scenes.at(-1);
                        window.qaNormal = null;
                        window.qaDockSide = side;
                        window.qaDragging = false;
                        window.qaTransitions = [];
                        window.qaPlace = () => {
                            const stage = document.querySelector('.pet-stage');
                            stage.style.position = 'fixed';
                            stage.style.left = `${window.qaPosition[0]}px`;
                            stage.style.top = `${window.qaPosition[1]}px`;
                        };
                        // 这里只提供虚拟窗口，不调用OS。与Rust同一原点/锚点单位（此矩阵scale=1）；
                        // 多DPI与负工作区由生产函数的Rust测试覆盖，浏览器截图不冒充物理多屏验收。
                        b.observeQaScene((scene) => {
                            const previous = window.qaPrevious;
                            const old = [...window.qaPosition];
                            let [x, y] = old;
                            if (previous?.docking && !scene.docking) {
                                const saved = window.qaNormal;
                                [x, y] = saved?.position ?? old;
                                if (saved) {
                                    x += Math.round(saved.anchor[0] - scene.anchor[0]);
                                    y += Math.round(saved.anchor[1] - scene.anchor[1]);
                                }
                                if (previous.docking === 'top')
                                    x = old[0] + Math.round(previous.anchor[0] - scene.anchor[0]);
                            } else if (scene.docking === 'top' && !previous?.docking) {
                                x += Math.round(previous.anchor[0] - scene.anchor[0]);
                            } else if (previous && !scene.docking) {
                                x += Math.round(previous.anchor[0] - scene.anchor[0]);
                                y += Math.round(previous.anchor[1] - scene.anchor[1]);
                            }
                            if (window.qaDragging) [x, y] = old;
                            else {
                                const left = Math.min(...scene.regions.map((r) => r[0]));
                                const top = Math.min(...scene.regions.map((r) => r[1]));
                                const right = Math.max(...scene.regions.map((r) => r[0] + r[2]));
                                const bottom = Math.max(...scene.regions.map((r) => r[1] + r[3]));
                                const clamp = (v, min, max) => Math.max(min, Math.min(v, Math.max(min, max)));
                                x = clamp(x, -Math.floor(left), 1200 - Math.ceil(right));
                                y = clamp(y, -Math.floor(top), 1000 - Math.ceil(bottom));
                                const edge = scene.docking ?? window.qaDockSide;
                                if (edge === 'left') x = -Math.floor(scene.actorBounds[0]);
                                if (edge === 'right')
                                    x = 1200 - Math.ceil(scene.actorBounds[0] + scene.actorBounds[2]);
                                if (edge === 'top') y = -Math.floor(scene.actorBounds[1]);
                                x = clamp(x, -Math.floor(left), 1200 - Math.ceil(right));
                                y = clamp(y, -Math.floor(top), 1000 - Math.ceil(bottom));
                            }
                            if (scene.docking && !previous?.docking)
                                window.qaNormal = { position: old, anchor: previous.anchor };
                            if (!scene.docking) window.qaNormal = null;
                            if (scene.docking !== previous?.docking)
                                window.qaTransitions.push({
                                    from: previous?.docking ?? null,
                                    to: scene.docking ?? null,
                                    beforeX: old[0] + previous.anchor[0],
                                    afterX: x + scene.anchor[0],
                                    position: [x, y],
                                });
                            window.qaPosition = [x, y];
                            window.qaPrevious = scene;
                            window.qaPlace();
                        });
                        window.qaPlace();
                        b.setQaDock(side, false);
                    },
                    { side, size },
                );
                // 先hover已打开的卡片，防止设置通知后进入180ms收纳；真实拖动从卡片标题开始。
                await page.locator('.widget-header').hover();
                const header = await page.locator('.widget-header').boundingBox();
                const count = await page.evaluate(
                    async () => (await import('/scripts/pet-preview-bridge.ts')).dragRequests.length,
                );
                await page.mouse.move(header.x + header.width / 2, header.y + header.height / 2);
                await page.mouse.down();
                await page.mouse.move(header.x + header.width / 2 - 25, header.y + header.height / 2 + 20, {
                    steps: 4,
                });
                await page.waitForFunction(
                    async (count) =>
                        (await import('/scripts/pet-preview-bridge.ts')).dragRequests.length > count,
                    count,
                );
                await page.evaluate(async (corner) => {
                    const b = await import('/scripts/pet-preview-bridge.ts');
                    const scene = b.scenes.at(-1);
                    window.qaDragging = true;
                    window.qaPosition = [(corner ? 1180 : 600) - scene.anchor[0], -scene.actorBounds[1]];
                    window.qaPlace();
                    window.qaDockSide = 'top';
                    b.setQaWindowPosition(...window.qaPosition, 1200);
                }, corner);
                await page.mouse.up();
                await page.evaluate(async () => {
                    window.qaDragging = false;
                    (await import('/scripts/pet-preview-bridge.ts')).finishQaDrag();
                });
                await page.mouse.move(600, 950);
                const restoredXs = [];
                for (let cycle = 0; cycle < 3; cycle++) {
                    await page.locator('.pet-stage[data-docked="true"][data-dock-motion=""]').waitFor();
                    const peek = await page.locator('.pet-edge-peek').boundingBox();
                    assert(peek && peek.x >= -1 && peek.x + peek.width <= 1201);
                    if (cycle === 0)
                        await page.screenshot({
                            path: resolve(output, `${side}-${size}-${corner}-tucked.png`),
                        });
                    await page.mouse.move(peek.x + peek.width / 2, peek.y + peek.height / 2);
                    await page.mouse.move(peek.x + peek.width / 2 + 3, peek.y + peek.height / 2, {
                        steps: 3,
                    });
                    await page.locator('.widget:visible').waitFor();
                    await page.locator('.widget-header').hover();
                    await page.locator('.pet-stage[data-docked="false"][data-dock-motion=""]').waitFor();
                    const measured = await page.evaluate(() => ({
                        x: window.qaPosition[0] + window.qaPrevious.anchor[0],
                        side: document.querySelector('.pet-stage').classList.contains('pet-edge-right'),
                        transitions: window.qaTransitions,
                    }));
                    assert.equal(measured.side, side === 'right', '顶部恢复改变角色在卡片的左右方位');
                    const restored = measured.transitions.at(-1);
                    assert.equal(restored.from, 'top');
                    // 右侧恢复有足够空间，应在可见探头原位出现；左侧角落允许完整卡片向内夹紧。
                    if (side === 'right' || !corner)
                        assert(Math.abs(restored.afterX - restored.beforeX) <= 1, '顶部恢复横向跳位');
                    restoredXs.push(measured.x);
                    if (cycle === 0)
                        await page.screenshot({
                            path: resolve(output, `${side}-${size}-${corner}-restored.png`),
                        });
                    await page.mouse.move(600, 950);
                }
                assert(Math.max(...restoredXs) - Math.min(...restoredXs) <= 1, '反复收纳逐次漂移');
                const record = await page.evaluate(async () => {
                    const b = await import('/scripts/pet-preview-bridge.ts');
                    return {
                        transitions: window.qaTransitions,
                        rejected: b.rejectedScenes,
                        requests: b.requests,
                    };
                });
                assert.deepEqual(record.rejected, []);
                assert(
                    !record.requests.some((name) =>
                        ['start_test', 'cancel_test', 'send_test_email'].includes(name),
                    ),
                );
                // 中间位置不受边界夹紧，进入和恢复均必须保持锚点；这会在旧原点逻辑下失败。
                if (!corner)
                    for (const t of record.transitions.filter((t) => t.to === 'top'))
                        assert(Math.abs(t.afterX - t.beforeX) <= 1, '顶部收起沿用了卡片原点');
                matrix.push({ side, size, corner, cycles: 3, restoredXs, transitions: record.transitions });
            }
        }
    }
    const video = await page.video().path();
    await context.close();
    assert.deepEqual(errors, []);
    writeFileSync(resolve(output, 'result.json'), JSON.stringify({ matrix, errors, video }, null, 2));
    console.log(JSON.stringify({ cases: matrix.length, cycles: matrix.length * 3, video }));
} catch (error) {
    if (page && !page.isClosed()) await page.screenshot({ path: resolve(output, 'failure.png') });
    writeFileSync(
        resolve(output, 'failure.json'),
        JSON.stringify({ message: String(error), errors }, null, 2),
    );
    throw error;
} finally {
    await browser?.close();
    await server.close();
}
