/**
 * 已确认整体设计的浏览器验收：真实Vue组件、本地透明PNG与隔离内存状态。
 * 先npm run build再运行；依赖现有Playwright/Chromium。仅监听回环，拒绝远程请求，
 * 不访问真实配置/凭据、不调用模型或邮件。输出截图、交互视频与JSON到临时目录。
 * PET_QA_SCREENSHOT_DIR / PET_PLAYWRIGHT_MODULE / PET_CHROMIUM_EXECUTABLE可覆盖路径。
 * 浏览器几何与渲染通过不代表Windows点击穿透、原生拖动或跨屏验证通过。@author ahui
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
const bridge = resolve(root, 'src/bridge.ts').replaceAll('\\', '/');
const output = resolve(process.env.PET_QA_SCREENSHOT_DIR || resolve(tmpdir(), 'model-iq-radar-pet-qa'));
mkdirSync(output, { recursive: true });
const server = await createServer({
    root,
    configFile: false,
    logLevel: 'error',
    plugins: [
        {
            name: 'pet-qa-memory-bridge',
            enforce: 'pre',
            load(id) {
                if (id.replaceAll('\\', '/') === bridge)
                    return `export * from '/scripts/pet-preview-bridge.ts';`;
            },
        },
        vue(),
    ],
    server: { host: '127.0.0.1', port: 0, strictPort: true, hmr: false, watch: null },
});
let browser;
let page;
const errors = [];
const captures = [];
let topStressFrames = 0;
const dockSides = ['left', 'right', 'top'];
const dockScenes = [];
const dockMotionSamples = [];
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
        locale: 'zh-CN',
        recordVideo: { dir: resolve(output, 'video'), size: { width: 820, height: 920 } },
    });
    await context.route('**/*', async (route) => {
        const url = new URL(route.request().url());
        if (url.origin === origin || ['data:', 'blob:'].includes(url.protocol)) await route.continue();
        else {
            errors.push(`远程请求：${url.origin}`);
            await route.abort();
        }
    });
    page = await context.newPage();
    page.on('pageerror', (error) => errors.push(error.message));
    const qa = (method, value) =>
        page.evaluate(
            async ({ method, value }) => {
                const module = await import('/scripts/pet-preview-bridge.ts');
                return module[method](...(Array.isArray(value) ? value : [value]));
            },
            { method, value },
        );
    /** 真实进入后继续移动3px；跳转一次坐标不能代表悬停意图，布局重排也会派发enter。 */
    const hoverPeek = async () => {
        const box = await page.locator('.pet-edge-peek').boundingBox();
        assert(box);
        await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
        await page.mouse.move(box.x + box.width / 2 + 3, box.y + box.height / 2, { steps: 3 });
    };
    /** 真实斜向路径穿过其他一级菜单项；locator.click会直接跳到目标，不能复现误收起。 */
    const crossActionMenu = async () => {
        const trigger = page.getByRole('menuitem', { name: '动作', exact: true });
        await trigger.hover();
        const start = await trigger.boundingBox();
        const child = page.getByRole('menu', { name: '桌宠动作', exact: true });
        const end = await child.boundingBox();
        assert(start && end);
        await page.mouse.move(start.x + start.width / 2, start.y + start.height / 2);
        // 先向上经过一级菜单，再斜着进入子菜单，模拟用户慢速寻找首项。
        await page.mouse.move(start.x + start.width / 2, end.y + 18, { steps: 12 });
        await page.waitForTimeout(240);
        assert.equal(await child.count(), 1, '经过一级菜单时动作子菜单自动收起');
        await page.mouse.move(end.x + end.width / 2, end.y + 18, { steps: 16 });
        await page.waitForTimeout(240);
        assert.equal(await child.count(), 1, '移入动作子菜单途中自动收起');
        // 反向回来后再次进入，不能依赖某一次碰巧足够快的鼠标移动。
        await page.mouse.move(start.x + start.width / 2, start.y + start.height / 2, { steps: 16 });
        await page.mouse.move(end.x + end.width / 2, end.y + 18, { steps: 16 });
        assert.equal(await child.count(), 1);
    };
    const capture = async (name) => {
        await page.locator('.pet-actor[data-transitioning="false"]').waitFor();
        await page
            .locator('.pet-stage')
            .screenshot({ path: resolve(output, `pet-${name}.png`), omitBackground: true });
        captures.push(name);
    };
    /** 探头状态会暂停完整角色画布；直接截取真实场景，不等待已隐藏播放器。 */
    const capturePeek = async (name) => {
        await page.locator('.pet-edge-peek img').waitFor();
        await page.locator('.pet-stage[data-dock-motion=""]').waitFor();
        await page
            .locator('.pet-stage')
            .screenshot({ path: resolve(output, `pet-${name}.png`), omitBackground: true });
        captures.push(name);
    };
    const expectStatus = async (key) =>
        page.locator(`.pet-result-cloud[data-status="${key}"]`).waitFor({ state: 'attached' });
    /** 三个方向只变换探头；状态云朵保持屏幕正向，原生参数覆盖真实可见范围。 */
    const peekMatches = async (side, status, label) => {
        await page.locator('.pet-edge-peek').waitFor();
        await page.locator('.pet-stage[data-dock-motion=""]').waitFor();
        await expectStatus(status);
        assert.equal(await page.locator('.pet-result-cloud:visible').count(), 1, '贴边隐藏了真实结论');
        await page.waitForFunction(
            (label) => document.querySelector('.pet-edge-peek')?.getAttribute('aria-label')?.includes(label),
            label,
        );
        const peek = await page.locator('.pet-edge-peek').boundingBox();
        assert(peek.width <= 100 && peek.height <= 100, '贴边没有缩为紧凑探头');
        assert(side === 'top' ? peek.width > peek.height : peek.height > peek.width, '探头方向尺寸不符');
        const cloud = await page.locator('.pet-result-cloud').evaluate((el) => {
            const r = el.getBoundingClientRect();
            const stage = el.closest('.pet-stage').getBoundingClientRect();
            return {
                transform: getComputedStyle(el).transform,
                outlineTransform: getComputedStyle(el.querySelector('.pet-cloud-outline')).transform,
                writingMode: getComputedStyle(el).writingMode,
                text: el.textContent,
                rect: [r.x - stage.x, r.y - stage.y, r.width, r.height],
            };
        });
        assert.equal(cloud.transform, 'none', '状态云朵被旋转或镜像');
        assert.equal(
            cloud.outlineTransform,
            side === 'top'
                ? 'matrix(1, 0, 0, -1, 0, 0)'
                : side === 'right'
                  ? 'matrix(-1, 0, 0, 1, 0, 0)'
                  : 'none',
            '云朵尾巴没有朝向角色',
        );
        assert.equal(cloud.writingMode, 'horizontal-tb');
        assert(cloud.text.includes(label), '探头云朵结论文字错误');
        const scene = await page.evaluate(async () => {
            const bridge = await import('/scripts/pet-preview-bridge.ts');
            return bridge.scenes.at(-1);
        });
        assert.equal(scene.docking, side);
        assert(scene.actorBounds?.length === 4 && scene.actorBounds.every(Number.isFinite));
        assert(scene.actorBounds[2] > 0 && scene.actorBounds[3] > 0);
        assert(
            scene.regions.some((r) => r.every((v, i) => Math.abs(v - cloud.rect[i]) < 0.1)),
            '原生区域遗漏状态云朵',
        );
        assert(cloud.rect[0] + cloud.rect[2] <= scene.width && cloud.rect[1] + cloud.rect[3] <= scene.height);
        dockScenes.push({
            side,
            status,
            width: scene.width,
            height: scene.height,
            actorBounds: scene.actorBounds,
            regions: scene.regions,
            cloud: cloud.rect,
        });
        return peek;
    };
    const openPanel = async () => {
        if (!(await page.locator('.widget').isVisible())) {
            if (await page.locator('.pet-result-cloud').isVisible())
                await page.locator('.pet-result-cloud').click();
            else {
                await page.locator('.pet-character').focus();
                await page.keyboard.press('Enter');
            }
        }
    };
    const closePanel = async () => {
        if (await page.locator('.widget').isVisible())
            await page.getByRole('button', { name: '收起为桌宠', exact: true }).click();
    };
    /** 九点命中包含选择器边缘和标题按钮，避免仅中心可点却被透明角色矩形挡住。 */
    const selectorsClear = async () => {
        for (const picker of await page
            .locator('.widget button[role="combobox"], .widget-tools button')
            .all())
            assert(
                await picker.evaluate((el) => {
                    const r = el.getBoundingClientRect();
                    return [0.1, 0.5, 0.9].every((x) =>
                        [0.1, 0.5, 0.9].every((y) =>
                            el.contains(document.elementFromPoint(r.x + r.width * x, r.y + r.height * y)),
                        ),
                    );
                }),
                `控件被角色或装饰遮挡：${await picker.getAttribute('aria-label')}`,
            );
    };
    /** 对照最新截图验证两行独立选择框、宽窄列对齐与纵向正文顺序。 */
    const layoutMatches = async (expanded) => {
        const geometry = await page.locator('.widget').evaluate((card) => {
            const box = (el) => {
                const r = el.getBoundingClientRect();
                return { x: r.x, y: r.y, width: r.width, height: r.height, bottom: r.bottom };
            };
            const controls = [...card.querySelectorAll('button[role="combobox"]')].map(box);
            const sections = [
                '.result-heading',
                '.answer-label',
                '.answer-box',
                '.metadata',
                '.expanded-actions',
                '.rule-note',
                '.scope-note',
            ]
                .map((selector) => card.querySelector(selector))
                .filter(Boolean)
                .map(box);
            return {
                card: box(card),
                controls,
                sections,
                scrollWidth: card.scrollWidth,
                clientWidth: card.clientWidth,
            };
        });
        assert.equal(geometry.card.width, 360);
        assert.equal(geometry.controls.length, 4);
        const [group, model, rule, effort] = geometry.controls;
        assert.equal(group.y, model.y);
        assert.equal(rule.y, effort.y);
        assert.equal(group.x, rule.x);
        assert.equal(model.x, effort.x);
        assert(group.width > model.width * 2, '未恢复左宽右窄的两行布局');
        assert(rule.y >= group.bottom, '两行选择器重叠');
        assert.equal(geometry.scrollWidth, geometry.clientWidth, '卡片横向溢出');
        if (expanded) {
            assert.equal(geometry.sections.length, 7);
            for (let i = 1; i < geometry.sections.length; i++)
                assert(
                    geometry.sections[i].y >= geometry.sections[i - 1].bottom - 1,
                    '详情内容顺序错误或重叠',
                );
            assert.equal(geometry.card.height, 546, '文字详情未采用统一缩短后的高度');
            assert(geometry.sections[2].height >= 100, '缩短后回答区高度不足');
            assert(geometry.sections.at(-1).bottom < geometry.card.bottom - 15, '说明与底部边饰重叠');
        }
    };
    await page.goto(origin);
    await page.locator('.pet-actor').waitFor();
    // 回归用户截图：卡片右侧已到边缘，但角色仍位于左上方；不能只注入一个right事件
    // 就声称贴边判定通过。先检查场景主体矩形，再由隔离替身计算工作区触边通知。
    await openPanel();
    await page.locator('.pet-actor[data-action="seated"]').waitFor();
    await page.waitForTimeout(220);
    const beforeEdge = await page.evaluate(async () =>
        (await import('/scripts/pet-preview-bridge.ts')).scenes.at(-1),
    );
    const cardEdgeX = 820 - (beforeEdge.dockBounds[0] + beforeEdge.dockBounds[2]);
    assert(
        cardEdgeX + beforeEdge.actorBounds[0] + beforeEdge.actorBounds[2] < 803,
        '回归前提不成立：角色本身已经贴边',
    );
    assert.equal(await qa('setQaWindowPosition', [cardEdgeX, 100]), 'right');
    await page.locator('.pet-stage[data-edge="right"]').waitFor();
    // 留在场景矩形内部的透明上方空白；旧stage:hover保护会导致永不收起。
    const blank = await page.locator('.pet-stage').boundingBox();
    await page.mouse.move(blank.x + 8, blank.y + 8);
    await page.locator('.pet-edge-peek').waitFor({ timeout: 2500 });
    await page.screenshot({ path: resolve(output, 'right-card-edge-transparent-space.png') });
    captures.push('right-card-edge-transparent-space');
    await qa('setQaDock', [null, false]);
    await page.locator('.pet-character').waitFor();
    await closePanel();
    await page.mouse.move(790, 880);
    assert.equal(await page.locator('.pet-radar,.pet-hologram,.pet-bubble').count(), 0, '旧球或旧气泡残留');
    const originalCanvas = await page.locator('.pet-actor').elementHandle();
    for (const status of ['running', 'passed', 'failed', 'error', 'cancelled', 'html']) {
        await qa('setQaRun', status);
        await expectStatus(status);
        const action = {
            running: 'working',
            passed: 'success',
            failed: 'failed',
            error: 'error',
            cancelled: 'idle',
            html: 'idle',
        }[status];
        // 云朵先更新而图集仍在异步解码；必须等真实目标姿态出画，不能只验文字。
        await page.locator(`.pet-actor[data-action="${action}"]`).waitFor();
        await capture(`status-${status}`);
    }
    await qa('setQaRun', 'passed');
    await qa('setQaAppearance', [192, false]);
    await page.locator('.pet-character').click();
    await page.locator('.pet-actor[data-action="petting"]').waitFor();
    await expectStatus('passed');
    assert.equal(await page.locator('.pet-cloud-interaction').count(), 0);
    await capture('petting');
    await qa('setQaStale', true);
    await expectStatus('stale');
    await capture('status-stale');
    await qa('setQaStale', false);
    await qa('setQaHistory', true);
    await expectStatus('idle');
    await page.locator('.pet-actor[data-action="idle"]').waitFor();
    await capture('idle');
    // Canvas原始像素采样证明实际待机发生变化；不把鼠标hover样式算成角色动画。
    const pixels = () => page.locator('.pet-actor').evaluate((el) => el.toDataURL());
    const idleFirst = await pixels();
    await page.waitForTimeout(800);
    assert.notEqual(await pixels(), idleFirst, '待机像素未变化');
    await qa('setQaHistory', false);
    await qa('setQaAppearance', [192, true]);
    await openPanel();
    await page.locator('.pet-actor[data-action="seated"]').waitFor();
    await capture('compact');
    await selectorsClear();
    await layoutMatches(false);
    assert.equal(await page.locator('.pet-ornaments').getAttribute('aria-hidden'), 'true');
    assert.equal(
        await page.locator('.pet-ornaments').evaluate((el) => getComputedStyle(el).pointerEvents),
        'none',
    );
    const compact = await page.locator('.widget').boundingBox();
    await page.locator('.widget button[role="combobox"]').nth(2).click();
    await page.locator('.model-menu').waitFor();
    await page.waitForTimeout(150);
    const menuBox = await page.locator('.model-menu').boundingBox();
    const sceneBox = await page.locator('.pet-stage').boundingBox();
    const menuCard = await page.locator('.widget').boundingBox();
    assert.equal(menuCard.height, compact.height, '下拉菜单撑大了卡片');
    assert(
        menuBox.x >= sceneBox.x &&
            menuBox.y >= sceneBox.y &&
            menuBox.x + menuBox.width <= sceneBox.x + sceneBox.width &&
            menuBox.y + menuBox.height <= sceneBox.y + sceneBox.height,
        '菜单越出场景',
    );
    await capture('menu');
    await page.keyboard.press('Escape');
    await page.getByRole('button', { name: '展开测试详情', exact: true }).click();
    await page.locator('.widget.expanded').waitFor();
    await selectorsClear();
    await layoutMatches(true);
    await capture('details');
    const details = await page.locator('.widget').boundingBox();
    assert.equal(details.x, compact.x);
    assert.equal(details.y, compact.y);
    assert.equal(details.width, compact.width);
    await page.getByRole('button', { name: '收起结果', exact: true }).click();
    for (const size of [96, 144, 192]) {
        await qa('setQaAppearance', [size, true]);
        await selectorsClear();
        await layoutMatches(false);
        await capture(`compact-${size}`);
        await page.getByRole('button', { name: '展开测试详情', exact: true }).click();
        await selectorsClear();
        await layoutMatches(true);
        await capture(`details-${size}`);
        await page.getByRole('button', { name: '收起结果', exact: true }).click();
    }
    await closePanel();
    await qa('setQaRun', 'running');
    await page.locator('.pet-actor[data-action="working"]').waitFor();
    const actor = await page.locator('.pet-character').boundingBox();
    await page.mouse.move(actor.x + actor.width / 2, actor.y + actor.height / 2);
    await page.mouse.down();
    await page.mouse.move(actor.x + actor.width / 2 + 20, actor.y + actor.height / 2 + 12, { steps: 4 });
    await page.locator('.pet-actor[data-action="dragging"]').waitFor();
    await qa('setQaRun', 'passed');
    assert.equal(await page.locator('.pet-actor').getAttribute('data-action'), 'dragging');
    await expectStatus('passed');
    await capture('dragging');
    await page.mouse.up();
    await qa('finishQaDrag');
    await page.locator('.pet-actor[data-action="landing"]').waitFor();
    await page.locator('.pet-actor[data-action="success"]').waitFor();
    await capture('success');
    await page.locator('.pet-character').dblclick();
    await page.locator('.pet-actor[data-action="wave"]').waitFor();
    await page.locator('.pet-actor[data-transitioning="false"]').waitFor();
    const wave = await pixels();
    await page.waitForTimeout(350);
    assert.equal(await pixels(), wave, '静态招手像素发生变化');
    await capture('wave');
    await page.locator('.pet-character').click({ button: 'right' });
    await page.getByRole('menuitem', { name: '动作', exact: true }).hover();
    await page.getByRole('menu', { name: '桌宠动作', exact: true }).waitFor();
    assert.equal(await page.locator('.pet-action-submenu [role="menuitem"]').count(), 12);
    await capture('action-submenu');
    await crossActionMenu();
    // 双层菜单键盘返回后重开；悬停和点击同时生效时不能误关子菜单。
    await page.getByRole('menuitem', { name: '动作', exact: true }).focus();
    await page.keyboard.press('ArrowRight');
    await page.keyboard.press('End');
    assert.equal(await page.evaluate(() => document.activeElement?.textContent?.trim()), '失败姿态');
    await page.keyboard.press('Escape');
    assert.equal(await page.locator('.pet-action-submenu').count(), 0);
    await page.keyboard.press('ArrowRight');
    const [actorSamples] = await Promise.all([
        page.evaluate(async () => {
            const samples = [];
            for (let i = 0; i < 32; i++) {
                await new Promise(requestAnimationFrame);
                const canvas = document.querySelector('.pet-actor');
                const pixels = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data;
                let alpha = 0;
                for (let j = 3; j < pixels.length; j += 4) alpha += pixels[j];
                samples.push({
                    action: canvas.dataset.action,
                    transitioning: canvas.dataset.transitioning,
                    alpha,
                });
            }
            return samples;
        }),
        page.getByRole('menuitem', { name: '伸个懒腰', exact: true }).click(),
    ]);
    assert(
        actorSamples.some((sample) => sample.transitioning === 'true'),
        '未采到实际画布交接',
    );
    assert(
        Math.min(...actorSamples.map((sample) => sample.alpha)) >
            Math.max(...actorSamples.map((sample) => sample.alpha)) * 0.55,
        '角色交接出现Alpha塌陷或闪空',
    );
    writeFileSync(resolve(output, 'actor-motion-samples.json'), JSON.stringify(actorSamples, null, 2));
    await page.locator('.pet-actor[data-action="stretch"]').waitFor();
    await page.locator('.pet-actor[data-transitioning="false"]').waitFor();
    const stretch = await pixels();
    await page.waitForTimeout(350);
    assert.equal(await pixels(), stretch, '静态伸懒腰像素发生变化');
    await capture('stretch');
    // 长按打开菜单后松手，不应补发摸头；Escape和外部点击均能正常退出。
    await page.locator('.pet-character').hover();
    await page.mouse.down();
    await page.getByRole('menu', { name: '桌宠操作', exact: true }).waitFor();
    await page.mouse.up();
    await page.waitForTimeout(260);
    assert.equal(await page.locator('.pet-actor').getAttribute('data-action'), 'stretch');
    await page.keyboard.press('Escape');
    assert.equal(await page.locator('.pet-context-menu').count(), 0);
    await page.locator('.pet-character').click({ button: 'right' });
    await page.mouse.click(790, 880);
    assert.equal(await page.locator('.pet-context-menu').count(), 0);
    // 所有现有姿态都通过真实二级菜单点播，滚动到目标后实际点击；不访问setup私有方法。
    for (const [action, label] of Object.entries({
        idle: '待机',
        petting: '摸摸头',
        wave: '招招手',
        stretch: '伸个懒腰',
        bow: '小幅鞠躬',
        seated: '侧坐休息',
        dragging: '悬空姿态',
        landing: '放下站稳',
        working: '测试中姿态',
        success: '不降智姿态',
        failed: '降智姿态',
        error: '失败姿态',
    })) {
        await page.locator('.pet-character').click({ button: 'right' });
        await page.getByRole('menuitem', { name: '动作', exact: true }).click();
        await page.getByRole('menuitem', { name: label, exact: true }).click();
        await page.locator(`.pet-actor[data-action="${action}"]`).waitFor();
        await capture(`menu-action-${action}`);
        assert.equal(await page.locator('.pet-context-menu').count(), 0);
    }
    assert(
        await originalCanvas.evaluate((node) => node === document.querySelector('.pet-actor')),
        '动作切换重建了画布',
    );
    // 以事件替身驱动真实场景状态机；浏览器不伪装原生移动、点击穿透或跨屏结果。
    await qa('setQaHistory', true);
    await qa('setQaAppearance', [144, true]);
    await page.mouse.move(790, 880);
    for (const side of dockSides) {
        // 真实点击会确认隐藏当前云朵。不能只检查仍在DOM中的文案，必须检查收纳后的可见性。
        await qa('setQaDock', [null, false]);
        await qa('setQaRun', 'running');
        await qa('setQaAppearance', [144, false]);
        await page.locator('.pet-character:visible').waitFor();
        await page.locator('.pet-result-cloud:visible').click();
        await page.locator('.widget:visible').waitFor();
        assert.equal(await page.locator('.pet-result-cloud:visible').count(), 0);
        await page.mouse.move(790, 880);
        await qa('setQaDock', [side, false]);
        await page.locator('.pet-edge-peek').waitFor();
        await peekMatches(side, 'running', '测试中');
        await capturePeek(`edge-${side}-acknowledged-running-panel`);
        await qa('setQaAppearance', [144, true]);
        for (const [status, label] of [
            ['running', '测试中'],
            ['passed', '不降智'],
            ['failed', '降智'],
            ['error', '测试失败'],
        ]) {
            await qa('setQaRun', status);
            await peekMatches(side, status, label);
            await capturePeek(`edge-${side}-${status}`);
        }
        // 临时互动文案开关不能隐藏必要结论；三个朝向均显示同一正向云朵。
        await qa('setQaAppearance', [144, false]);
        await peekMatches(side, 'error', '测试失败');
        assert.equal(await page.locator('.pet-cloud-interaction').count(), 0);
        await qa('setQaAppearance', [144, true]);
        // 背景只用于检查透明边缘，不写入用户主题或生产配置。
        for (const [theme, color] of [
            ['dark', '#172132'],
            ['light', '#f3f5f8'],
        ]) {
            await page.evaluate((color) => (document.body.style.background = color), color);
            await page.screenshot({
                path: resolve(output, `pet-edge-${side}-${theme}-background.png`),
                clip: { x: 0, y: 0, width: 330, height: 200 },
            });
            captures.push(`edge-${side}-${theme}-background`);
        }
        await page.evaluate(() => document.body.style.removeProperty('background'));
        await hoverPeek();
        await page.locator('.pet-character:visible').waitFor();
        await expectStatus('error');
        await page.mouse.move(790, 880);
        await page.locator('.pet-edge-peek').waitFor();
        await page.locator('.pet-edge-peek').focus();
        await page.locator('.pet-character:visible').waitFor();
        await expectStatus('error');
        await page.keyboard.press('Enter');
        await page.locator('.widget:visible').waitFor();
        await selectorsClear();
        await closePanel();
        // 键盘焦点仍需保护；主动离开焦点后才能验证鼠标收起路径。
        await page.evaluate(() => document.activeElement?.blur());
        await page.mouse.move(20, 20);
        await page.mouse.move(790, 880);
        await page.locator('.pet-edge-peek').waitFor();
        const peekBox = await page.locator('.pet-edge-peek').boundingBox();
        await page.mouse.move(peekBox.x + peekBox.width / 2, peekBox.y + peekBox.height / 2);
        await page.mouse.down();
        await page.mouse.up();
        await page.locator('.widget:visible').waitFor();
        await expectStatus('error');
        await selectorsClear();
        // 面板打开也应收纳；悬停后恢复同一侧坐卡片，不把旧规则“面板永久阻止”当通过。
        await page.getByRole('button', { name: '展开测试详情', exact: true }).click();
        for (const status of ['running', 'passed', 'failed', 'error']) {
            await qa('setQaRun', status);
            await expectStatus(status);
            assert.equal(
                await page.locator('.pet-actor').getAttribute('data-action'),
                'seated',
                '面板显示时被测试动作替换',
            );
        }
        await page.evaluate(() => document.activeElement?.blur());
        await page.mouse.move(790, 880);
        await page.locator('.pet-edge-peek').waitFor();
        await page.locator('.pet-stage[data-dock-motion=""]').waitFor();
        assert.equal(await page.locator('.widget:visible').count(), 0, '打开的面板没有随贴边隐藏');
        // 真实鼠标悬停和逐帧样式：必须全程不透明，并出现中间滑入位移。
        const [samples] = await Promise.all([
            page.evaluate(async () => {
                const rows = [];
                for (let i = 0; i < 30; i++) {
                    await new Promise(requestAnimationFrame);
                    const stage = document.querySelector('.pet-stage');
                    const element =
                        document.querySelector('.pet-edge-peek') || document.querySelector('.pet-character');
                    const style = getComputedStyle(element);
                    rows.push({
                        phase: stage.dataset.dockMotion,
                        opacity: Number(style.opacity),
                        transform: style.transform,
                        action: document.querySelector('.pet-actor')?.dataset.action,
                        status: document.querySelector('.pet-result-cloud')?.dataset.status,
                    });
                }
                return rows;
            }),
            (async () => {
                await hoverPeek();
                // 恢复后原探头坐标可能落在透明空白；立即跟随真实卡片，避免抽帧期间合理地再次收纳。
                await page.locator('.widget-header').hover();
            })(),
        ]);
        assert(
            samples.every((sample) => sample.opacity === 1),
            `${side}收纳恢复发生透明闪空`,
        );
        assert(
            samples.some(
                (sample) => sample.transform !== 'none' && sample.transform !== 'matrix(1, 0, 0, 1, 0, 0)',
            ),
            `${side}缺少滑入中间帧`,
        );
        assert(
            samples.every((sample) => sample.status === 'error'),
            '过渡改写测试结论',
        );
        dockMotionSamples.push({ side, samples });
        await page.locator('.pet-stage[data-dock-motion=""]').waitFor();
        await page.locator('.widget:visible').waitFor();
        await page.locator('.pet-actor[data-action="seated"]').waitFor();
        assert.equal(await page.locator('.widget.expanded:visible').count(), 1, '收纳恢复丢失详情模式');
        // 右侧恢复后角色移到卡片右上方，旧探头位置可能变成透明空白；截图耗时期间
        // 鼠标真实离开交互对象本应自动收起，先移到云朵，不能要求透明空白锁住窗口。
        await page.locator('.widget-header').hover();
        await capture(`edge-${side}-restored-panel`);
        await page.locator('.pet-character').click({ button: 'right' });
        await page.getByRole('menuitem', { name: '动作', exact: true }).hover();
        const submenu = await page.getByRole('menu', { name: '桌宠动作', exact: true }).boundingBox();
        const menuStage = await page.locator('.pet-stage').boundingBox();
        assert(
            submenu.x >= menuStage.x &&
                submenu.y >= menuStage.y &&
                submenu.x + submenu.width <= menuStage.x + menuStage.width &&
                submenu.y + submenu.height <= menuStage.y + menuStage.height,
            `${side}二级菜单被窗口裁切`,
        );
        const menuScene = await page.evaluate(async () =>
            (await import('/scripts/pet-preview-bridge.ts')).scenes.at(-1),
        );
        assert(
            menuScene.regions.some(
                (r) =>
                    Math.abs(r[0] - (submenu.x - menuStage.x)) < 0.1 &&
                    Math.abs(r[1] - (submenu.y - menuStage.y)) < 0.1 &&
                    r[2] === submenu.width &&
                    r[3] === submenu.height,
            ),
            `${side}二级菜单缺少输入掩码`,
        );
        await capture(`edge-${side}-action-submenu`);
        await crossActionMenu();
        await page.getByRole('menuitem', { name: '动作', exact: true }).focus();
        await page.keyboard.press('ArrowRight');
        await page.keyboard.press('Escape');
        await page.keyboard.press('Escape');
        await page.getByRole('button', { name: '收起结果', exact: true }).click();
        await closePanel();
        await page.mouse.move(790, 880);
        await page.locator('.pet-edge-peek').waitFor();
    }
    // 已收起时切换方向必须保持收起和当前结论，不能绕回完整角色或重启测试。
    for (const side of ['right', 'top', 'left']) {
        await qa('setQaDock', [side, true]);
        await peekMatches(side, 'error', '测试失败');
        await capturePeek(`edge-switch-${side}`);
    }
    // 右侧完整布局将卡片放到角色左边；检查大尺寸下标题工具与选择框仍可全部命中。
    await qa('setQaDock', ['right', true]);
    await page.locator('.pet-edge-peek').focus();
    await page.keyboard.press('Enter');
    await page.locator('.widget:visible').waitFor();
    for (const size of [96, 144, 192]) {
        // 尺寸变化后不再依赖透明空白的stage:hover留住卡片；实际移入卡片再操作。
        await page.locator('.widget').hover();
        await qa('setQaAppearance', [size, true]);
        await selectorsClear();
        await layoutMatches(false);
        await capture(`right-compact-${size}`);
        await page.getByRole('button', { name: '展开测试详情', exact: true }).click();
        await selectorsClear();
        await layoutMatches(true);
        await capture(`right-details-${size}`);
        await page.getByRole('button', { name: '收起结果', exact: true }).click();
    }
    await qa('setQaAppearance', [144, true]);
    await closePanel();
    await page.evaluate(() => document.activeElement?.blur());
    await page.mouse.move(790, 880);
    await page.locator('.pet-edge-peek').waitFor();
    await qa('setQaAnimation', false);
    await hoverPeek();
    await page.locator('.pet-character:visible').waitFor();
    await qa('setQaAnimation', true);
    await qa('setQaDock', [null, false]);
    await page.mouse.move(790, 880);
    await page.waitForTimeout(650);
    assert.equal(await page.locator('.pet-edge-peek:visible').count(), 0, '拖离边缘后又被旧计时器收起');
    // 高频 dock/undock 的最后事件必须获胜；过时异步任务不能重新恢复探头。
    await page.evaluate(async () => {
        const bridge = await import('/scripts/pet-preview-bridge.ts');
        for (let i = 0; i < 12; i++) {
            bridge.setQaDock(i % 2 ? null : 'left');
            await new Promise((resolve) => setTimeout(resolve, 30));
        }
    });
    await page.waitForTimeout(650);
    assert.equal(await page.locator('.pet-edge-peek:visible').count(), 0);
    await qa('setQaRun', 'running');
    await page.locator('.pet-actor[data-action="working"]').waitFor();
    const beforeDockDrag = await page.locator('.pet-character').boundingBox();
    await page.mouse.move(beforeDockDrag.x + 60, beforeDockDrag.y + 60);
    await page.mouse.down();
    await page.mouse.move(beforeDockDrag.x + 90, beforeDockDrag.y + 65, { steps: 4 });
    await page.locator('.pet-actor[data-action="dragging"]').waitFor();
    for (const side of dockSides) {
        await qa('setQaDock', [side, false]);
        await page.waitForTimeout(650);
        assert.equal(await page.locator('.pet-edge-peek:visible').count(), 0, '拖动期间被吸附计时器抢占');
    }
    await qa('setQaRun', 'passed');
    await page.mouse.up();
    await qa('finishQaDrag');
    await qa('setQaDock', [null, false]);
    await page.locator('.pet-actor[data-action="success"]').waitFor();
    // 换肤后遗留的贴边监听和计时器不能再把经典布局收成角色探头。
    await qa('setQaDock', ['left', false]);
    await page.mouse.move(790, 880);
    await page.locator('.pet-edge-peek').waitFor();
    await page.evaluate(async () => {
        const bridge = await import('/scripts/pet-preview-bridge.ts');
        const snapshot = await bridge.call('get_snapshot');
        snapshot.config.preferences.skin.id = 'classic';
        await bridge.call('save_preferences', { preferences: snapshot.config.preferences });
        bridge.setQaDock(null);
    });
    await page.locator('.classic-stage').waitFor();
    assert.equal(await page.locator('.pet-edge-peek').count(), 0);
    await page.evaluate(async () => {
        const bridge = await import('/scripts/pet-preview-bridge.ts');
        const snapshot = await bridge.call('get_snapshot');
        snapshot.config.preferences.skin.id = 'blue-whale-maid';
        await bridge.call('save_preferences', { preferences: snapshot.config.preferences });
    });
    await page.locator('.pet-character').waitFor();
    assert.equal(await page.locator('.pet-edge-peek:visible').count(), 0);
    // 换肤时原生拖动循环可能仍在后台；这里检查撤销命令与前端生命周期，不替代原生线程验收。
    await qa('setQaHistory', true);
    await page.locator('.pet-actor[data-action="idle"]').waitFor();
    const cancelBefore = await page.evaluate(async () => {
        const bridge = await import('/scripts/pet-preview-bridge.ts');
        return bridge.requests.filter((command) => command === 'cancel_pet_drag').length;
    });
    const skinDrag = await page.locator('.pet-character').boundingBox();
    await page.mouse.move(skinDrag.x + 60, skinDrag.y + 60);
    await page.mouse.down();
    await page.mouse.move(skinDrag.x + 90, skinDrag.y + 65, { steps: 4 });
    await page.locator('.pet-actor[data-action="dragging"]').waitFor();
    await page.evaluate(async () => {
        const bridge = await import('/scripts/pet-preview-bridge.ts');
        const snapshot = await bridge.call('get_snapshot');
        snapshot.config.preferences.skin.id = 'classic';
        await bridge.call('save_preferences', { preferences: snapshot.config.preferences });
    });
    await page.locator('.classic-stage').waitFor();
    const cancelAfter = await page.evaluate(async () => {
        const bridge = await import('/scripts/pet-preview-bridge.ts');
        return bridge.requests.filter((command) => command === 'cancel_pet_drag').length;
    });
    assert.equal(cancelAfter, cancelBefore + 1, '拖动中换肤没有且仅有一次撤销原生拖动');
    await page.mouse.up();
    await qa('finishQaDrag');
    await page.evaluate(async () => {
        const bridge = await import('/scripts/pet-preview-bridge.ts');
        const snapshot = await bridge.call('get_snapshot');
        snapshot.config.preferences.skin.id = 'blue-whale-maid';
        await bridge.call('save_preferences', { preferences: snapshot.config.preferences });
    });
    await page.locator('.pet-actor[data-action="idle"]').waitFor();
    await page.waitForTimeout(650);
    assert.equal(
        await page.locator('.pet-actor').getAttribute('data-action'),
        'idle',
        '旧拖动终态恢复了过期动作',
    );
    assert.equal(await page.locator('.pet-edge-peek:visible').count(), 0);
    await capture('skin-switch-during-drag');
    // 连续切换四种结论、三个尺寸和展开状态，保存视频与逐次截图检查头顶重绘。
    // 此压力检查发生在浏览器合成器中；不能据此声称原生偶发异常已经复现或消失。
    for (const size of [96, 144, 192]) {
        await qa('setQaAppearance', [size, true]);
        for (const status of ['running', 'passed', 'failed', 'error']) {
            await qa('setQaRun', status);
            await expectStatus(status);
            await openPanel();
            await selectorsClear();
            await capture(`top-stress-${size}-${status}`);
            const top = await page.locator('.pet-stage').boundingBox();
            await page.screenshot({
                path: resolve(output, `pet-top-crop-${size}-${status}.png`),
                clip: { x: top.x, y: top.y, width: Math.min(300, top.width), height: 80 },
                omitBackground: true,
            });
            topStressFrames++;
            await closePanel();
        }
    }
    const forbidden = await page.evaluate(async () => {
        const bridge = await import('/scripts/pet-preview-bridge.ts');
        return bridge.requests.filter((command) => /start_test|send.*mail|send.*email/.test(command));
    });
    assert.deepEqual(forbidden, [], '桌宠交互额外触发了模型测试或邮件');
    // 连续真实输入与业务事件竞争：逐帧采样，验证最后目标获胜、画布不闪空、不累积节点。
    await qa('setQaDock', [null, false]);
    const session = await context.newCDPSession(page);
    await session.send('HeapProfiler.collectGarbage');
    const heapBefore = (await session.send('Runtime.getHeapUsage')).usedSize;
    const stressSamples = await page.evaluate(async () => {
        const bridge = await import('/scripts/pet-preview-bridge.ts');
        const samples = [];
        for (let i = 0; i < 48; i++) {
            bridge.setQaRun(['running', 'passed', 'failed', 'error'][i % 4]);
            await new Promise((resolve) => setTimeout(resolve, 35));
            const canvas = document.querySelector('.pet-actor');
            const pixels = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data;
            let alpha = 0;
            for (let j = 3; j < pixels.length; j += 4) alpha += pixels[j];
            samples.push({ action: canvas.dataset.action, alpha });
        }
        return samples;
    });
    await page.locator('.pet-actor[data-action="error"][data-transitioning="false"]').waitFor();
    assert(
        stressSamples.every((s) => s.alpha > 0),
        '快速切换出现空白画布',
    );
    assert.equal(await page.locator('.pet-actor').count(), 1, '切换累积了播放器节点');
    await session.send('HeapProfiler.collectGarbage');
    const heapAfter = (await session.send('Runtime.getHeapUsage')).usedSize;
    writeFileSync(
        resolve(output, 'rapid-switch-samples.json'),
        JSON.stringify({ stressSamples, heapBefore, heapAfter }, null, 2),
    );
    await session.detach();
    await qa('setQaHistory', true);
    // 等上一业务动作真实退场，再开始自主活动验收；不能将尚未解码替换的旧error误当自主动作。
    await page.locator('.pet-actor[data-action="idle"][data-transitioning="false"]').waitFor();
    await qa('setQaAutonomous', true);
    await page.locator('.pet-actor:not([data-action="idle"])').waitFor({ timeout: 19000 });
    await capture('autonomous');
    await qa('setQaAutonomous', false);
    await page.locator('.pet-actor[data-action="idle"]').waitFor();
    await page.locator('.pet-character').click();
    await page.locator('.pet-actor[data-action="petting"]').waitFor();
    await capture('autonomous-off-still-interactive');
    // 功能入口实际点击，后台使用本地替身。这里允许且只允许明确点击的测试/取消命令。
    await page.goto(origin);
    await page.locator('.pet-actor').waitFor();
    await openPanel();
    await page.getByRole('button', { name: '重新测试', exact: true }).click();
    await expectStatus('running');
    await page.getByRole('button', { name: '取消测试', exact: true }).click();
    await expectStatus('cancelled');
    // 演示快照默认启用自动测试，先实际关闭再验证开启路径，不猜测fixture默认值。
    if (
        (await page
            .getByRole('button', { name: '切换自动测试', exact: true })
            .getAttribute('aria-pressed')) === 'true'
    )
        await page.getByRole('button', { name: '切换自动测试', exact: true }).click();
    await page.waitForFunction(
        () => document.querySelector('[aria-label="切换自动测试"]')?.getAttribute('aria-pressed') === 'false',
    );
    await page.getByRole('button', { name: '切换自动测试', exact: true }).click();
    assert.equal(
        await page.getByRole('button', { name: '切换自动测试', exact: true }).getAttribute('aria-pressed'),
        'true',
    );
    const countdown = await page.locator('.auto-countdown').innerText();
    // UI时钟按整秒采样，与点击时刻不对齐；等待可观察到的文本变化，不硬猜1100ms已跨两次采样。
    await page.waitForFunction(
        (before) => document.querySelector('.auto-countdown')?.textContent !== before,
        countdown,
        { timeout: 3500 },
    );
    assert.notEqual(await page.locator('.auto-countdown').innerText(), countdown, '自动测试倒计时未推进');
    await page.getByRole('button', { name: '切换自动测试', exact: true }).click();
    assert.equal(await page.locator('.auto-countdown').count(), 0);
    await page.locator('.widget button[role="combobox"]').nth(1).click();
    await page.getByRole('option', { name: 'demo-model', exact: true }).click();
    await expectStatus('stale');
    await capture('model-changed-stale');
    const explicitCalls = await page.evaluate(async () =>
        (await import('/scripts/pet-preview-bridge.ts')).requests.filter((c) =>
            /start_test|cancel_test|send.*mail|send.*email/.test(c),
        ),
    );
    assert.deepEqual(explicitCalls, ['start_test', 'cancel_test']);
    assert.equal(await page.locator('.inline-error').count(), 0);
    // 真正的HTML回答从本地fixture进入原沙箱，核对放大与恢复未丢桌宠布局。
    await page.goto(`${origin}/?example=html`);
    await page.locator('.pet-result-cloud').waitFor();
    await openPanel();
    await page.getByRole('button', { name: '展开测试详情', exact: true }).click();
    await page.locator('.html-preview[aria-label]').waitFor();
    await page.locator('.html-preview-loading').waitFor({ state: 'hidden' });
    await page.frameLocator('.html-preview iframe').locator('svg').first().waitFor();
    assert.equal((await page.locator('.widget').boundingBox()).height, 546, 'HTML详情高度与文字不一致');
    await capture('html-details');
    await page.getByRole('button', { name: '放大 HTML 预览', exact: true }).click();
    await page.locator('dialog[open]').waitFor();
    assert.equal(await page.locator('.pet-character').count(), 0);
    await page.waitForFunction(() => !document.querySelector('.html-preview-close')?.disabled);
    await page.getByRole('button', { name: '关闭放大预览', exact: true }).focus();
    await page.keyboard.press('Escape');
    await page.locator('.pet-character').waitFor();
    // 源码开关随同一回答保留；切回经典或资源失败回退后也必须有可用的返回预览入口。
    await page.getByRole('button', { name: '原始回答', exact: true }).click();
    await page.locator('.pet-raw-answer').waitFor();
    assert.equal((await page.locator('.widget').boundingBox()).height, 546, '原始回答切换改变卡片高度');
    await page.evaluate(async () => {
        const bridge = await import('/scripts/pet-preview-bridge.ts');
        const snapshot = await bridge.call('get_snapshot');
        snapshot.config.preferences.skin.id = 'classic';
        await bridge.call('save_preferences', { preferences: snapshot.config.preferences });
    });
    await page.locator('.classic-stage').waitFor();
    await page.getByRole('button', { name: '返回预览', exact: true }).click();
    await page.locator('.html-preview-loading').waitFor({ state: 'hidden' });
    await page.frameLocator('.html-preview iframe').locator('svg').first().waitFor();
    assert.equal(await page.locator('.pet-raw-answer').count(), 0);
    await page.evaluate(async () => {
        const bridge = await import('/scripts/pet-preview-bridge.ts');
        const snapshot = await bridge.call('get_snapshot');
        snapshot.config.preferences.skin.id = 'blue-whale-maid';
        await bridge.call('save_preferences', { preferences: snapshot.config.preferences });
    });
    await page.locator('.pet-character').waitFor();
    const rejectedWidgetScenes = await page.evaluate(
        async () => (await import('/scripts/pet-preview-bridge.ts')).rejectedScenes,
    );
    await page.goto(`${origin}/?view=settings`);
    await page
        .getByRole('navigation', { name: '设置分类' })
        .getByRole('button', { name: '皮肤管理', exact: true })
        .click();
    await page.locator('.skin-live-preview canvas').waitFor();
    for (const label of ['招手', '伸懒腰', '降智', '测试失败', '放下站稳', '鞠躬'])
        await page.getByRole('button', { name: label, exact: true }).click();
    const idleOptions = await page
        .locator('#skin-idle-action option')
        .evaluateAll((options) => options.map((o) => o.value));
    assert(idleOptions.includes('idle') && idleOptions.includes('seated'));
    assert.deepEqual(
        [...idleOptions].sort(),
        [
            'idle',
            'petting',
            'dragging',
            'working',
            'success',
            'seated',
            'wave',
            'stretch',
            'landing',
            'bow',
            'failed',
            'error',
        ].sort(),
    );
    for (const kind of idleOptions) {
        await page.getByLabel('默认待机动作', { exact: true }).selectOption(kind);
        await page.locator(`.skin-live-preview canvas[data-action="${kind}"]`).waitFor();
    }
    await page.getByLabel('默认待机动作', { exact: true }).selectOption('seated');
    await page.locator('.skin-live-preview canvas[data-action="seated"]').waitFor();
    await page.getByRole('button', { name: '保存皮肤设置', exact: true }).click();
    await page.getByRole('status').filter({ hasText: '已保存，更改已生效' }).waitFor();
    assert.equal(await page.locator('.inline-error').count(), 0);
    const savedIdle = await page.evaluate(
        async () =>
            (await (await import('/scripts/pet-preview-bridge.ts')).call('get_snapshot')).config.preferences
                .skin.idleAction,
    );
    assert.equal(savedIdle, 'seated', '默认待机没有进入保存后的权威快照');
    await page
        .getByRole('navigation', { name: '设置分类' })
        .getByRole('button', { name: '关于', exact: true })
        .click();
    await page
        .getByRole('navigation', { name: '设置分类' })
        .getByRole('button', { name: '皮肤管理', exact: true })
        .click();
    await page.locator('.skin-live-preview canvas[data-action="seated"]').waitFor();
    assert.equal(await page.getByLabel('默认待机动作', { exact: true }).inputValue(), 'seated');
    await page.getByLabel('默认待机动作', { exact: true }).scrollIntoViewIfNeeded();
    await page.screenshot({ path: resolve(output, 'settings-default-idle.png') });
    const video = await page.video().path();
    assert.deepEqual(errors, []);
    assert.deepEqual(rejectedWidgetScenes, [], '悬浮场景曾提交原生会拒绝的矩形');
    assert.deepEqual(
        await page.evaluate(async () => (await import('/scripts/pet-preview-bridge.ts')).rejectedScenes),
        [],
        '原生同等矩形校验拒绝过场景，不能仅凭页面无异常判定通过',
    );
    await context.close();
    const result = {
        screenshots: captures.length,
        captures,
        fourStates: true,
        staleCancelHtml: true,
        resultSurvivesBubbleOff: true,
        menuDoesNotResizeCard: true,
        sizes: [96, 144, 192],
        selectorsUnobscured: true,
        ninePointHeaderAndPickerHits: true,
        originalTwoRowVerticalLayout: true,
        dragDefersFeedbackUntilLanding: true,
        staticWaveAndStretch: true,
        nestedActionMenu: true,
        actionMenuKeyboardAndThreeEdgeBounds: true,
        slowDiagonalSubmenuPaths: true,
        rightCardEdgeWithActorAway: true,
        transparentSpaceDoesNotBlockDock: true,
        allTwelveActionsClicked: true,
        rapidBusinessSwitches: stressSamples.length,
        autonomousAndManualInteraction: true,
        longPressAndOutsideMenuClose: true,
        explicitStartCancelAndCountdown: true,
        actualModelSelectionInvalidatesResult: true,
        idlePixelsChange: true,
        htmlPreviewRestored: true,
        rawHtmlClassicFallbackRestored: true,
        skinPreviewSave: true,
        defaultIdlePreviewSave: true,
        edgePeekSides: dockSides,
        edgePanelTucksAndRestores: true,
        edgeAcknowledgedRunningPanelProgressVisible: true,
        edgeMotionHasIntermediateFrames: true,
        tuckedSideSwitchPreservesResult: true,
        dockCloudSurvivesBubbleOff: true,
        dockCloudNeverTransformed: true,
        rightEdgeHeaderAndPickerHits: [96, 144, 192],
        edgeHoverFocusClickRestore: true,
        edgePauseAndSkinCleanup: true,
        skinSwitchDuringDragRequestsCancel: true,
        edgeDoesNotPreemptDrag: true,
        edgeFourAccessibleResults: true,
        edgeDarkLightBackgrounds: true,
        topStressFrames,
        nativeIntermittentArtifactReproduced: false,
        pageErrors: 0,
        remoteRequests: 0,
        video,
    };
    writeFileSync(resolve(output, 'result.json'), JSON.stringify(result, null, 2) + '\n', 'utf8');
    writeFileSync(resolve(output, 'dock-scenes.json'), JSON.stringify(dockScenes, null, 2) + '\n', 'utf8');
    writeFileSync(
        resolve(output, 'dock-motion-samples.json'),
        JSON.stringify(dockMotionSamples, null, 2) + '\n',
        'utf8',
    );
    console.log(JSON.stringify(result));
} catch (error) {
    // 失败也保留当前画面与最后布局，便于区分组件故障和验收脚本自身的等待条件。
    // 不导出业务快照、接口参数或HTML回答，避免收集与本次视觉问题无关的数据。
    if (page && !page.isClosed()) {
        await page.screenshot({ path: resolve(output, 'failure.png') }).catch(() => {});
        const diagnostic = await page
            .evaluate(async () => {
                const bridge = await import('/scripts/pet-preview-bridge.ts');
                const stage = document.querySelector('.pet-stage');
                const setup = stage?.__vueParentComponent?.setupState;
                return {
                    dock: await bridge.call('get_widget_dock'),
                    lastScene: bridge.scenes.at(-1),
                    focus: document.activeElement?.className,
                    focusVisible: document.activeElement?.matches(':focus-visible'),
                    hovered: Boolean(stage?.matches(':hover')),
                    bodyHovered: document.body.matches(':hover'),
                    actorAction: document.querySelector('.pet-actor')?.getAttribute('data-action'),
                    peek: Boolean(document.querySelector('.pet-edge-peek')),
                    panel: Boolean(document.querySelector('.pet-panel:not([inert])')),
                    stageState: setup
                        ? Object.fromEntries(
                              [
                                  'edgeSide',
                                  'docked',
                                  'canDock',
                                  'pageVisible',
                                  'dragging',
                                  'menu',
                                  'panel',
                                  'hovering',
                                  'press',
                                  'previewExpanded',
                              ].map((key) => [key, setup[key]]),
                          )
                        : null,
                    dockResource: setup?.skin?.dockPoster ? 'available' : 'missing',
                };
            })
            .catch(() => null);
        writeFileSync(
            resolve(output, 'failure.json'),
            JSON.stringify({ message: String(error), errors, diagnostic }, null, 2) + '\n',
            'utf8',
        );
    }
    throw error;
} finally {
    await browser?.close();
    await server.close();
}
