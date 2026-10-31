/**
 * 小视口选择器首帧验收：延迟模拟窗口扩高，检查页面不滚动、菜单就绪后才出画。
 * 本地内存替身，不接触生产数据／模型；浏览器不能验证原生合成。@author ahui
 */
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { createServer } from 'vite';
import vue from '@vitejs/plugin-vue';
const root = process.cwd();
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PET_PLAYWRIGHT_MODULE || 'playwright');
const output = process.env.PET_QA_SCREENSHOT_DIR;
mkdirSync(output, { recursive: true });
const server = await createServer({
    root,
    configFile: false,
    logLevel: 'error',
    plugins: [
        {
            name: 'picker-qa',
            enforce: 'pre',
            load(id) {
                const path = id.replaceAll('\\', '/');
                if (path === resolve(root, 'src/bridge.ts').replaceAll('\\', '/'))
                    return `export * from '/scripts/pet-preview-bridge.ts';`;
                if (path === resolve(root, 'scripts/readme-preview-bridge.ts').replaceAll('\\', '/'))
                    return readFileSync(id, 'utf8').replace(
                        "{ id: 'demo-model', label: 'demo-model' },",
                        Array.from(
                            { length: 8 },
                            (_, i) => `{id:'qa-model-${i}',label:'qa-model-${i}'},`,
                        ).join('\n'),
                    );
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
    browser = await chromium.launch({ headless: true, executablePath: process.env.PET_CHROMIUM_EXECUTABLE });
    const context = await browser.newContext({
        viewport: { width: 820, height: 920 },
        recordVideo: { dir: resolve(output, 'video'), size: { width: 820, height: 920 } },
    });
    await context.route('**/*', async (route) => {
        const u = new URL(route.request().url());
        if (u.origin === origin || ['blob:', 'data:'].includes(u.protocol)) await route.continue();
        else {
            errors.push('remote:' + u.origin);
            await route.abort();
        }
    });
    page = await context.newPage();
    page.on('pageerror', (e) => errors.push(e.message));
    let lastSize = '';
    let hostQueue = Promise.resolve();
    await page.exposeFunction('resizeQaHost', (scene) => {
        hostQueue = hostQueue.then(async () => {
            const key = `${scene.width}/${scene.height}`;
            if (key !== lastSize) {
                lastSize = key;
                await new Promise((done) => setTimeout(done, 120));
                await page.setViewportSize({ width: scene.width, height: scene.height });
            }
        });
        return hostQueue;
    });
    await page.goto(origin);
    await page.locator('.pet-actor').waitFor();
    await page.evaluate(async () => {
        const b = await import('/scripts/pet-preview-bridge.ts');
        b.setQaAppearance(144, true);
        b.setQaDock(null);
        const s = await b.call('get_snapshot');
        await b.call('set_profile_model', { profileId: s.config.selectedProfileId, model: 'qa-model-7' });
    });
    await page.locator('.pet-result-cloud').click();
    await page.locator('.widget:visible').waitFor();
    await page.evaluate(async () => {
        const b = await import('/scripts/pet-preview-bridge.ts');
        b.observeQaScene(window.resizeQaHost);
        b.observeQaClassic((expanded) => window.resizeQaHost({ width: 360, height: expanded ? 572 : 156 }));
        await window.resizeQaHost(b.scenes.at(-1));
    });
    const rows = [];
    for (const size of [96, 144, 192])
        for (const side of ['left', 'right']) {
            await page.evaluate(
                async ({ size, side }) => {
                    const b = await import('/scripts/pet-preview-bridge.ts');
                    b.setQaAppearance(size, true);
                    b.setQaDock(side);
                },
                { size, side },
            );
            await page.getByRole('combobox', { name: '当前模型配置', exact: true }).focus();
            await page.keyboard.press('Tab');
            await page.waitForTimeout(250);
            for (const label of ['当前分组模型', '当前测试规则', '当前思考等级', '当前模型配置']) {
                await page.evaluate(() => {
                    window.pickerFrames = [];
                    window.pickerSampling = true;
                    const sample = () => {
                        if (!window.pickerSampling) return;
                        const menu = document.querySelector('.model-menu');
                        const r = menu?.getBoundingClientRect();
                        window.pickerFrames.push({
                            scroll: window.scrollY,
                            rootScroll: document.documentElement.scrollTop,
                            card: document.querySelector('.widget').getBoundingClientRect().top,
                            visible: !!menu && getComputedStyle(menu).visibility !== 'hidden',
                            bottom: r?.bottom,
                            height: innerHeight,
                        });
                        requestAnimationFrame(sample);
                    };
                    sample();
                });
                await page.getByRole('combobox', { name: label, exact: true }).click();
                await page.getByRole('listbox').waitFor();
                assert.equal(
                    await page.getByRole('listbox').evaluate((node) => getComputedStyle(node).boxShadow),
                    'none',
                    '下拉菜单不应再绘制阴影',
                );
                await page.waitForTimeout(350);
                const frames = await page.evaluate(() => {
                    window.pickerSampling = false;
                    return window.pickerFrames;
                });
                rows.push({ size, side, label, frames });
                await page.screenshot({ path: resolve(output, `picker-${rows.length}.png`) });
                writeFileSync(resolve(output, 'picker-frames.json'), JSON.stringify(rows, null, 2));
                assert(
                    frames.every((f) => f.scroll === 0 && f.rootScroll === 0),
                    `${label}滚动了整个页面`,
                );
                assert(
                    frames.filter((f) => f.visible).every((f) => f.bottom <= f.height + 1),
                    `${label}在窗口扩高前出画被裁切`,
                );
                const tops = frames.map((f) => f.card);
                assert(Math.max(...tops) - Math.min(...tops) < 1, `${label}打开时卡片跳位`);
                await page.keyboard.press('Escape');
                await page.waitForTimeout(220);
            }
        }
    // 真实按键展开，已确认云朵不能因轮询或面板收放重新出现。
    await page.evaluate(async () => {
        const b = await import('/scripts/pet-preview-bridge.ts');
        b.setQaDock(null);
        b.setQaRun('passed');
    });
    const cloud = page.locator('.pet-result-cloud');
    await cloud.waitFor();
    await cloud.click();
    await page.waitForTimeout(220);
    assert.equal(await cloud.isVisible(), false);
    await page.waitForTimeout(1200);
    assert.equal(await cloud.isVisible(), false);
    // 确认文案与表情属于同一次呈现：面板侧坐，收起待机，不能恢复旧业务表情。
    for (const [status, pose] of [
        ['passed', 'success'],
        ['failed', 'failed'],
        ['error', 'error'],
        ['running', 'working'],
    ]) {
        if (await page.locator('.widget').isVisible()) {
            await page.locator('.pet-character').focus();
            await page.keyboard.press('Enter');
        }
        await page.evaluate(async (status) => {
            (await import('/scripts/pet-preview-bridge.ts')).setQaRun(status);
        }, status);
        await page.locator(`.pet-actor[data-action="${pose}"][data-transitioning="false"]`).waitFor();
        await cloud.click();
        await page.locator('.pet-actor[data-action="seated"][data-transitioning="false"]').waitFor();
        assert.equal(await cloud.isVisible(), false);
        await page.locator('.pet-character').focus();
        await page.keyboard.press('Enter');
        await page.locator('.pet-actor[data-action="idle"][data-transitioning="false"]').waitFor();
        // 超过旧庆祝反馈驻留和快照轮询周期，仍应保持已确认的待机。
        await page.waitForTimeout(2000);
        assert.equal(await page.locator('.pet-actor').getAttribute('data-action'), 'idle');
        assert.equal(await cloud.isVisible(), false);
        await page.screenshot({ path: resolve(output, `acknowledged-${status}.png`) });
    }
    await page.evaluate(async () => (await import('/scripts/pet-preview-bridge.ts')).setQaRun('passed'));
    await cloud.waitFor();
    const menuSamples = [];
    for (const [name, bounds, expected] of [
        ['room-right', [-1000, -200, 1400, 1200], 'right'],
        ['tight-right', [-1000, -200, 250, 1200], 'left'],
        ['bottom', [-1000, -40, 1400, 340], 'right'],
        ['bottom-right', [-1000, -800, 280, 240], null],
        ['bottom-left', [0, -800, 1400, 240], null],
    ]) {
        await page.evaluate(async (bounds) => {
            const b = await import('/scripts/pet-preview-bridge.ts');
            b.setQaMenuBounds(bounds);
            b.setQaDock(null);
            window.menuSamples = [];
            window.menuSampling = true;
            const sample = () => {
                if (!window.menuSampling) return;
                for (const el of document.querySelectorAll('.pet-context-menu')) {
                    if (getComputedStyle(el).visibility === 'hidden') continue;
                    const r = el.getBoundingClientRect();
                    window.menuSamples.push({
                        left: r.left,
                        top: r.top,
                        right: r.right,
                        bottom: r.bottom,
                        w: innerWidth,
                        h: innerHeight,
                    });
                }
                requestAnimationFrame(sample);
            };
            requestAnimationFrame(sample);
        }, bounds);
        await page.locator('.pet-character').click({ button: 'right' });
        const rootMenu = page.getByRole('menu', { name: '桌宠操作', exact: true });
        await rootMenu.waitFor();
        const actualSide = await rootMenu.getAttribute('data-submenu-side');
        if (expected) assert.equal(actualSide, expected);
        const parentSize = page.viewportSize();
        await page.getByRole('menuitem', { name: '动作', exact: true }).hover();
        await page.getByRole('menu', { name: '桌宠动作', exact: true }).waitFor();
        await page.waitForTimeout(200);
        assert.deepEqual(page.viewportSize(), parentSize, '子菜单展开改变宿主尺寸');
        const parent = await rootMenu.boundingBox();
        const child = await page.locator('.pet-action-submenu').boundingBox();
        assert(
            actualSide === 'right'
                ? child.x >= parent.x + parent.width - 2
                : child.x + child.width <= parent.x + 2,
        );
        const safe = await page.evaluate(() => {
            const protectedRects = [...document.querySelectorAll('.pet-character,.pet-result-cloud')]
                .filter((el) => getComputedStyle(el).display !== 'none')
                .map((el) => el.getBoundingClientRect());
            return [...document.querySelectorAll('.pet-context-menu')].every((el) => {
                const r = el.getBoundingClientRect();
                return protectedRects.every(
                    (p) => r.right <= p.left || p.right <= r.left || r.bottom <= p.top || p.bottom <= r.top,
                );
            });
        });
        assert(safe, name + '菜单遮挡角色或结果云朵');
        await page.screenshot({ path: resolve(output, `menu-${name}.png`) });
        // 鼠标离开延迟收二级；键盘进入二级后不因鼠标停在外部而丢失焦点。
        const trigger = page.getByRole('menuitem', { name: '动作', exact: true });
        await rootMenu.locator(':scope > button').first().hover();
        await page.waitForTimeout(750);
        assert.equal(await page.locator('.pet-action-submenu').count(), 0, '停在其他一级项后二级不收起');
        assert.deepEqual(page.viewportSize(), parentSize);
        await trigger.hover();
        await page.locator('.pet-action-submenu').waitFor();
        // 菜单现在可能向上避让，固定(10,10)可能落在菜单内；移到明确受保护的角色区域。
        const outsideMenu = await page.locator('.pet-character').boundingBox();
        await page.mouse.move(outsideMenu.x + outsideMenu.width / 2, outsideMenu.y + outsideMenu.height / 2);
        await page.waitForTimeout(750);
        assert.equal(await page.locator('.pet-action-submenu').count(), 0, '离开整棵菜单后二级不收起');
        await trigger.focus();
        await page.keyboard.press('ArrowRight');
        await page.mouse.move(
            outsideMenu.x + outsideMenu.width / 2 + 2,
            outsideMenu.y + outsideMenu.height / 2,
        );
        await page.waitForTimeout(750);
        assert.equal(await page.locator('.pet-action-submenu').count(), 1, '鼠标离开打断键盘子菜单操作');
        await page.keyboard.press('Escape');
        assert.deepEqual(page.viewportSize(), parentSize, '子菜单关闭改变宿主尺寸');
        await page.keyboard.press('Escape');
        await page.waitForTimeout(300);
        const samples = await page.evaluate(() => {
            window.menuSampling = false;
            return window.menuSamples;
        });
        assert(samples.length > 0);
        assert(
            samples.every((r) => r.left >= -1 && r.top >= -1 && r.right <= r.w + 1 && r.bottom <= r.h + 1),
            '菜单出画或销毁存在裁切帧',
        );
        menuSamples.push({ name, expected, samples });
    }
    writeFileSync(resolve(output, 'menu-frames.json'), JSON.stringify(menuSamples, null, 2));
    await page.evaluate(async () => (await import('/scripts/pet-preview-bridge.ts')).setQaMenuBounds(null));
    assert.equal(
        await page.locator('.pet-character').evaluate((el) => getComputedStyle(el).outlineStyle),
        'none',
        '透明画布残留矩形焦点框',
    );
    for (const side of ['left', 'right', 'top']) {
        await page.evaluate(async (side) => {
            const b = await import('/scripts/pet-preview-bridge.ts');
            document.activeElement?.blur();
            b.setQaDock(side);
        }, side);
        await page.mouse.move(790, 880);
        await page.locator('.pet-edge-peek').waitFor();
        const peekBox = await page.locator('.pet-edge-peek').boundingBox();
        await page.mouse.move(peekBox.x + peekBox.width / 2, peekBox.y + peekBox.height / 2);
        await page.mouse.move(peekBox.x + peekBox.width / 2 + 3, peekBox.y + peekBox.height / 2, {
            steps: 3,
        });
        await page.locator('.pet-character:visible').click({ button: 'right' });
        await page.getByRole('menuitem', { name: '动作', exact: true }).hover();
        await page.getByRole('menuitem', { name: '招招手', exact: true }).click();
        await page.mouse.move(790, 880);
        await page.evaluate(() => document.activeElement?.blur());
        await page.waitForTimeout(700);
        assert.equal(
            await page.locator('.pet-stage').getAttribute('data-docked'),
            'false',
            '手动动作被贴边抢占',
        );
        assert.equal(await page.locator('.pet-actor').getAttribute('data-action'), 'wave');
        await page.evaluate(async () => (await import('/scripts/pet-preview-bridge.ts')).setQaRun('passed'));
        await page.waitForTimeout(500);
        assert.equal(
            await page.locator('.pet-actor').getAttribute('data-action'),
            'wave',
            '结算打断手动动作',
        );
        await page.screenshot({ path: resolve(output, `manual-edge-${side}.png`) });
        await page.locator('.pet-edge-peek').waitFor({ timeout: 4000 });
    }
    await page.evaluate(async () => (await import('/scripts/pet-preview-bridge.ts')).setQaDock(null));
    // 复现测试中从菜单重新打开详情；桥接逐次执行原生同等矩形拒绝规则。
    for (let i = 0; i < 3; i++) {
        await page.evaluate(async () => (await import('/scripts/pet-preview-bridge.ts')).setQaRun('running'));
        if (!(await page.locator('.widget').isVisible())) {
            await page.locator('.pet-character').focus();
            await page.keyboard.press('Enter');
        }
        if (!(await page.locator('.widget').evaluate((el) => el.classList.contains('expanded'))))
            await page.getByRole('button', { name: '展开测试详情', exact: true }).click();
        await page.locator('.pet-character').focus();
        await page.keyboard.press('Enter');
        await page.locator('.pet-character').click({ button: 'right' });
        await page.getByRole('menuitem', { name: '打开测智面板', exact: true }).click();
        await page.locator('.widget.expanded:visible').waitFor();
        await page.waitForTimeout(250);
        assert.equal(await page.locator('.classic-stage').count(), 0, '轮廓越界导致意外经典回退');
    }
    assert.deepEqual(
        await page.evaluate(async () => (await import('/scripts/pet-preview-bridge.ts')).rejectedScenes),
        [],
    );
    // 页面实际几何验证：左右布局拖离边缘不重置，顶部云朵正文按倒转轮廓重新居中。
    for (const side of ['left', 'right']) {
        await page.evaluate(async (side) => {
            const b = await import('/scripts/pet-preview-bridge.ts');
            b.setQaDock(side);
        }, side);
        await page.locator('.pet-character:visible').focus();
        await page.keyboard.press('Enter');
        if (!(await page.locator('.widget').isVisible())) {
            await page.locator('.pet-character').focus();
            await page.keyboard.press('Enter');
        }
        await page.mouse.move(30, 120);
        await page.waitForTimeout(170);
        const start = await page.locator('.pet-character:visible').boundingBox();
        await page.mouse.move(start.x + start.width / 2, start.y + start.height / 2);
        await page.mouse.down();
        await page.mouse.move(start.x + start.width / 2 + 25, start.y + start.height / 2 + 12, { steps: 4 });
        await page.evaluate(async () => {
            const b = await import('/scripts/pet-preview-bridge.ts');
            b.setQaDock(null);
            b.finishQaDrag();
        });
        await page.mouse.up();
        assert.equal(
            await page.locator('.pet-stage').evaluate((el) => el.classList.contains('pet-edge-right')),
            side === 'right',
        );
    }
    for (const side of ['left', 'right', 'top']) {
        await page.evaluate(async (side) => {
            const b = await import('/scripts/pet-preview-bridge.ts');
            b.setQaRun('html');
            b.setQaDock(side);
        }, side);
        await page.evaluate(() => document.activeElement?.blur());
        await page.mouse.move(790, 880);
        await page.locator('.pet-edge-peek').waitFor();
        await page.locator('.pet-stage[data-dock-motion=""]').waitFor();
        // 方向切换已收纳时没有滑入计时，必须等待异步宿主尺寸真正到位再检查裁切。
        await page.waitForFunction(() => {
            const b = document.querySelector('.pet-stage').getBoundingClientRect();
            return b.right <= innerWidth + 1 && b.bottom <= innerHeight + 1;
        });
        const center = await cloud.evaluate((el) => {
            const b = el.getBoundingClientRect();
            const t = el.querySelector('.pet-cloud-content').getBoundingClientRect();
            return (t.top + t.height / 2 - b.top) / b.height;
        });
        assert(Math.abs(center - (side === 'top' ? 38 / 68 : 30 / 68)) < 0.06, side + '云朵文字偏离正文中心');
        await page.screenshot({ path: resolve(output, `cloud-${side}-center.png`) });
    }
    // 同一页面反复切皮肤，使用clear_pet_scene真实请求参数调整浏览器视口。
    await page.evaluate(async () => (await import('/scripts/pet-preview-bridge.ts')).setQaDock(null));
    for (const expanded of [false, true, false, true]) {
        if (!(await page.locator('.widget').isVisible())) {
            await page.locator('.pet-character').focus();
            await page.keyboard.press('Enter');
        }
        if ((await page.locator('.widget').evaluate((el) => el.classList.contains('expanded'))) !== expanded)
            await page
                .getByRole('button', { name: expanded ? '展开测试详情' : '收起结果', exact: true })
                .click();
        await page.evaluate(async () => {
            const b = await import('/scripts/pet-preview-bridge.ts');
            const s = await b.call('get_snapshot');
            s.config.preferences.skin.id = 'classic';
            await b.call('save_preferences', { preferences: s.config.preferences });
        });
        await page.locator('.classic-stage').waitFor();
        await page.waitForFunction((height) => innerHeight === height, expanded ? 572 : 156);
        const box = await page.locator('.widget').boundingBox();
        assert(box.y + box.height <= page.viewportSize().height + 1, '经典卡片被截断');
        assert.equal(page.viewportSize().height, expanded ? 572 : 156);
        await page.screenshot({ path: resolve(output, `classic-${expanded}.png`) });
        await page.evaluate(async () => {
            const b = await import('/scripts/pet-preview-bridge.ts');
            const s = await b.call('get_snapshot');
            s.config.preferences.skin.id = 'blue-whale-maid';
            await b.call('save_preferences', { preferences: s.config.preferences });
        });
        await page.locator('.pet-character').waitFor();
    }
    assert.deepEqual(errors, []);
    const video = await page.video().path();
    await context.close();
    writeFileSync(
        resolve(output, 'result.json'),
        JSON.stringify(
            {
                pickers: rows.length,
                cloudDismiss: true,
                acknowledgedPoses: 4,
                adaptiveMenus: 5,
                submenuAutoClose: true,
                manualEdgeActions: 3,
                runningMenuDetails: 3,
                rejectedScenes: 0,
                stableSides: true,
                cloudCenters: true,
                classicRoundTrips: 4,
                errors,
                video,
            },
            null,
            2,
        ),
    );
    console.log(JSON.stringify({ pickers: rows.length, video }));
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
