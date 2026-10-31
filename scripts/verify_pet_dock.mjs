/**
 * 固定屏幕坐标下的贴边重排回归：模拟原生窗口左右/顶部对齐后的DOM移位。
 * 原有固定页面原点的验收无法覆盖“新探头落到静止鼠标下面”。本脚本真实移动鼠标，
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
const output = process.env.PET_QA_SCREENSHOT_DIR || resolve(tmpdir(), 'model-iq-radar-dock-reflow');
mkdirSync(output, { recursive: true });
const server = await createServer({
    root,
    configFile: false,
    logLevel: 'error',
    plugins: [
        {
            name: 'dock-qa',
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
        viewport: { width: 820, height: 920 },
        deviceScaleFactor: 2,
        recordVideo: { dir: resolve(output, 'video'), size: { width: 820, height: 920 } },
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
    await page.goto(origin);
    await page.locator('.pet-actor').waitFor();
    await page.locator('.pet-result-cloud').click();
    await page.locator('.widget:visible').waitFor();
    await page.evaluate(async () => (await import('/scripts/pet-preview-bridge.ts')).setQaRun('passed'));
    await page.mouse.move(400, 700);
    await page.evaluate(async () => {
        const b = await import('/scripts/pet-preview-bridge.ts');
        b.setQaAppearance(144, true);
        b.setQaDock('right', false);
        const align = (scene) => {
            const stage = document.querySelector('.pet-stage');
            stage.style.position = 'fixed';
            stage.style.left = `${820 - scene.actorBounds[0] - scene.actorBounds[2]}px`;
            stage.style.top = '130px';
        };
        b.observeQaScene(align);
        align(b.scenes.at(-1));
        window.dockTrace = [];
        new MutationObserver(() =>
            window.dockTrace.push({
                at: performance.now(),
                tucked: document.querySelector('.pet-stage')?.dataset.docked,
            }),
        ).observe(document.querySelector('.pet-stage'), {
            attributes: true,
            attributeFilter: ['data-docked'],
        });
    });
    await page.locator('.pet-stage[data-edge="right"]').waitFor();
    // 完整角色按钮顶端是134；静止鼠标在132处透明空白，新探头却覆盖130—226。
    // 收纳后的窗口/元素移动会产生pointerenter，不代表用户主动进入探头。
    await page.mouse.move(815, 132);
    await page.waitForTimeout(1300);
    const trace = await page.evaluate(() => window.dockTrace);
    writeFileSync(resolve(output, 'stationary-pointer-trace.json'), JSON.stringify(trace, null, 2));
    await page.screenshot({ path: resolve(output, 'stationary-right.png') });
    assert.equal(
        await page.locator('.pet-stage').getAttribute('data-docked'),
        'true',
        '新探头移到静止鼠标下触发恢复，无法稳定收起',
    );
    assert(trace.filter((item) => item.tucked === 'false').length === 0, '静止指针引起探头自动恢复');
    const matrix = [];
    for (const side of ['right', 'left', 'top']) {
        for (const animation of [true, false]) {
            await page.mouse.move(400, 850);
            await page.evaluate(
                async ({ side, animation }) => {
                    const b = await import('/scripts/pet-preview-bridge.ts');
                    b.observeQaScene();
                    b.setQaAnimation(animation);
                    b.setQaDock(side, false);
                    const align = (scene) => {
                        const stage = document.querySelector('.pet-stage');
                        stage.style.left = `${side === 'right' ? 820 - scene.actorBounds[0] - scene.actorBounds[2] : side === 'left' ? -scene.actorBounds[0] : 330}px`;
                        stage.style.top = `${side === 'top' ? -scene.actorBounds[1] : 130}px`;
                    };
                    b.observeQaScene(align);
                    align(b.scenes.at(-1));
                },
                { side, animation },
            );
            await page.locator('.pet-edge-peek').waitFor();
            await page.locator('.pet-stage[data-dock-motion=""]').waitFor();
            for (let cycle = 0; cycle < 3; cycle++) {
                const box = await page.locator('.pet-edge-peek').boundingBox();
                // 单次定位移入后静止；额外resize/滑入不会冒充第二次真实移动。
                await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
                await page.waitForTimeout(350);
                assert.equal(
                    await page.locator('.pet-stage').getAttribute('data-docked'),
                    'true',
                    `${side}/${animation}静止进入后自动恢复`,
                );
                await page.mouse.move(box.x + box.width / 2 + 3, box.y + box.height / 2, { steps: 3 });
                await page.locator('.pet-stage[data-docked="false"]').waitFor();
                await page.locator('.widget:visible').waitFor();
                await page.mouse.move(400, 850);
                await page.locator('.pet-edge-peek').waitFor();
                await page.locator('.pet-stage[data-dock-motion=""]').waitFor();
            }
            // 直接点击刚进入的探头，不先悬停恢复；默认鼠标focus不得提前移除按钮吞掉click。
            await page.locator('.pet-edge-peek').focus();
            await page.keyboard.press('Enter');
            await page.getByRole('button', { name: '收起为桌宠', exact: true }).click();
            await page.mouse.move(400, 850);
            await page.locator('.pet-edge-peek').waitFor();
            await page.locator('.pet-stage[data-dock-motion=""]').waitFor();
            const clickBox = await page.locator('.pet-edge-peek').boundingBox();
            await page.mouse.move(clickBox.x + clickBox.width / 2, clickBox.y + clickBox.height / 2);
            await page.mouse.down();
            await page.mouse.up();
            await page.locator('.widget:visible').waitFor();
            await page.mouse.move(400, 850);
            await page.locator('.pet-edge-peek').waitFor();
            // 贴边状态的云朵也必须可拖；按下恢复造成节点位移，pointer capture仍应保留手势。
            await page.locator('.pet-stage[data-dock-motion=""]').waitFor();
            const tuckedCloud = await page.locator('.pet-result-cloud').boundingBox();
            const beforeDrag = await page.evaluate(
                async () => (await import('/scripts/pet-preview-bridge.ts')).dragRequests.length,
            );
            await page.mouse.move(
                tuckedCloud.x + tuckedCloud.width / 2,
                tuckedCloud.y + tuckedCloud.height / 2,
            );
            await page.mouse.down();
            await page.mouse.move(
                tuckedCloud.x + tuckedCloud.width / 2 + 22,
                tuckedCloud.y + tuckedCloud.height / 2 + 12,
                { steps: 4 },
            );
            await page.waitForFunction(
                async (count) =>
                    (await import('/scripts/pet-preview-bridge.ts')).dragRequests.length === count + 1,
                beforeDrag,
            );
            await page.mouse.up();
            await page.evaluate(async () => (await import('/scripts/pet-preview-bridge.ts')).finishQaDrag());
            await page.locator('.widget:visible').waitFor();
            assert.equal(await page.locator('.pet-actor').getAttribute('data-action'), 'seated');
            await page.mouse.move(400, 850);
            await page.locator('.pet-edge-peek').waitFor();
            await page.screenshot({ path: resolve(output, `stable-${side}-${animation}.png`) });
            matrix.push({ side, animation, cycles: 3, directClick: true, tuckedCloudDrag: true });
        }
    }
    // 结果气泡真实鼠标短按/小抖动/长按拖动；测试结论与面板可见性保持独立。
    await page.evaluate(async () => {
        const b = await import('/scripts/pet-preview-bridge.ts');
        b.observeQaScene();
        b.setQaDock(null);
        b.setQaAnimation(true);
        const stage = document.querySelector('.pet-stage');
        stage.style.position = 'relative';
        stage.style.left = '0';
        stage.style.top = '0';
    });
    await page.locator('.pet-stage[data-docked="false"]').waitFor();
    const cloud = page.locator('.pet-result-cloud');
    const status = await cloud.getAttribute('data-status');
    const dragCount = () =>
        page.evaluate(async () => (await import('/scripts/pet-preview-bridge.ts')).dragRequests.length);
    for (const open of [true, false]) {
        if ((await page.locator('.widget').isVisible()) !== open) {
            if (open) {
                await page.locator('.pet-character').focus();
                await page.keyboard.press('Enter');
            } else await page.getByRole('button', { name: '收起为桌宠', exact: true }).click();
        }
        const box = await cloud.boundingBox();
        const before = await dragCount();
        await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
        await page.mouse.down();
        await page.mouse.move(box.x + box.width / 2 + 3, box.y + box.height / 2, { steps: 3 });
        assert.equal(await dragCount(), before, '小抖动误启动拖动');
        await page.waitForTimeout(600);
        assert.equal(await page.locator('.pet-context-menu').count(), 0, '气泡长按抢成菜单');
        await page.mouse.move(box.x + box.width / 2 + 23, box.y + box.height / 2 + 12, { steps: 4 });
        await page.waitForFunction(
            async (before) =>
                (await import('/scripts/pet-preview-bridge.ts')).dragRequests.length === before + 1,
            before,
        );
        await page.mouse.up();
        await page.evaluate(async () => (await import('/scripts/pet-preview-bridge.ts')).finishQaDrag());
        await page.waitForTimeout(250);
        assert.equal(await page.locator('.widget').isVisible(), open, '气泡拖后误切面板');
        assert.equal(await cloud.getAttribute('data-status'), status, '气泡拖动改写测试结论');
    }
    await page.screenshot({ path: resolve(output, 'cloud-drag-release.png') });
    const video = await page.video().path();
    await context.close();
    assert.deepEqual(errors, []);
    writeFileSync(
        resolve(output, 'result.json'),
        JSON.stringify({ stationaryRightStable: true, matrix, cloudDrag: true, errors, video }, null, 2),
    );
    console.log(JSON.stringify({ stationaryRightStable: true, video }));
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
