/** 场景使用逻辑像素；浮层只扩窗口和输入区域，不拉长功能卡片。@author ahui */
import type { DockSide, Rect, SkinClip } from './types';
export interface SceneLayout {
    width: number;
    height: number;
    pet: Rect;
    panel: Rect;
    bubble: Rect;
    menu: Rect;
    /** 动作二级菜单，与一级菜单相邻，单独参与窗口尺寸及输入掩码。 */
    actions: Rect;
    /** 已转为场景坐标的选择器浮层；关闭后为空，恢复透明穿透。 */
    floating: Rect[];
}

/** 探头图片、点击矩形和原生命中轮廓共用同一变换，避免视觉旋转后仍按旧方向裁剪。 */
export interface PeekLayout {
    width: number;
    height: number;
    peek: Rect;
    bubble: Rect;
    imageLeft: number;
    transform: string;
    regions: Rect[];
}

/**
 * 同一批准源图适配三边，云朵始终正向放在屏内一侧。角色固定96逻辑像素方形，
 * 镜像/直角旋转只改变朝向，不拉伸脸部；热区逐矩形应用完全相同的几何变换。
 * @param clip 已通过校验的单帧源图，hitBands使用frameSize源像素。
 * @param side 屏幕边缘，顶部旋转后水平镜像使头朝右，右侧水平镜像。
 * @returns 包含常驻132×50云朵的场景、探头点击范围和实际轮廓。
 */
export function peekLayout(clip: SkinClip, side: DockSide): PeekLayout {
    const size = 96;
    const scale = size / clip.frameSize;
    const extent = Math.min(
        size,
        Math.ceil(Math.max(...clip.hitBands.map(([x, , w]) => (x + w) * scale))) + 2,
    );
    const gap = 4;
    const peek: Rect =
        side === 'top'
            ? [0, 0, size, extent]
            : side === 'right'
              ? [132 + gap, 0, extent, size]
              : [0, 0, extent, size];
    const bubble: Rect =
        side === 'right'
            ? [0, 16, 132, 50]
            : side === 'top'
              ? [size + gap, 8, 132, 50]
              : [extent + gap, 16, 132, 50];
    const imageLeft = side === 'right' ? extent - size : 0;
    const regions = clip.hitBands.map(([x, y, w, h]): Rect => {
        const [left, top, width, height] = [x * scale, y * scale, w * scale, h * scale];
        if (side === 'right') return [peek[0] + imageLeft + size - left - width, top, width, height];
        if (side === 'top') return [top, left, height, width];
        return [left, top, width, height];
    });
    return {
        width: Math.ceil(Math.max(peek[0] + peek[2], bubble[0] + bubble[2]) + 2),
        height: Math.ceil(Math.max(peek[1] + peek[3], bubble[1] + bubble[3]) + 2),
        peek,
        bubble,
        imageLeft,
        regions,
        transform: side === 'right' ? 'scaleX(-1)' : side === 'top' ? 'scaleX(-1) rotate(90deg)' : 'none',
    };
}

/**
 * 完整角色停在右边时将卡片和云朵布置到角色左侧，只改矩形位置，不镜像文字或角色。
 * 浮层仍跟随其真实卡片原点，不能反转选择器内部坐标。
 * @param layout 默认左侧角色场景。
 * @returns 与原角色尺寸相同、内容正向的右侧布局。
 */
export function rightSceneLayout(layout: SceneLayout): SceneLayout {
    const reflect = ([x, y, w, h]: Rect): Rect => [layout.width - x - w, y, w, h];
    const panel = reflect(layout.panel);
    const shift = panel[0] - layout.panel[0];
    return {
        ...layout,
        pet: reflect(layout.pet),
        panel,
        bubble: reflect(layout.bubble),
        menu: reflect(layout.menu),
        actions: reflect(layout.actions),
        floating: layout.floating.map(([x, y, w, h]) => [x + shift, y, w, h]),
    };
}

/**
 * 为整棵右键菜单预留同一窗口范围，子菜单显隐不再反复缩放宿主。
 * @param base 不包含菜单的完整场景，人物方位已经应用。
 * @param bounds 工作区在base窗口内的逻辑边界[left, top, right, bottom]，允许负坐标。
 * @returns 菜单几何及实际方向；所有坐标统一平移，原生端以人物锚点补偿窗口原点。
 */
export function contextMenuLayout(base: SceneLayout, bounds: Rect): { layout: SceneLayout; left: boolean } {
    const gap = 8;
    const width = base.actions[2];
    // 人物画布与结果云朵都是受保护区；菜单阴影也留出间距，不只检查是否越出工作区。
    const protectedRects = [base.pet, base.bubble];
    const top = Math.min(...protectedRects.map(([, y]) => y));
    const bottom = Math.max(...protectedRects.map(([, y, , h]) => y + h));
    const nearLeft = Math.min(...protectedRects.map(([x]) => x));
    const nearRight = Math.max(...protectedRects.map(([x, , w]) => x + w));
    const height = Math.min(base.actions[3], Math.max(80, bounds[3] - bounds[1] - 4));
    const clampX = (x: number) => Math.min(Math.max(x, bounds[0] + 2), bounds[2] - base.menu[2] - 2);
    const clampY = (y: number) => Math.min(Math.max(y, bounds[1] + 2), bounds[3] - height - 2);
    const candidates: { menu: Rect; actions: Rect; left: boolean; score: number }[] = [];
    for (const x of [clampX(base.menu[0]), nearRight + gap, nearLeft - base.menu[2] - gap])
        for (const y of [clampY(base.menu[1]), bottom + gap, top - height - gap])
            for (const left of [false, true]) {
                const menu: Rect = [x, y, base.menu[2], Math.min(base.menu[3], height)];
                const actions: Rect = [left ? x - width + 1 : x + menu[2] - 1, y, width, height];
                // 工作区外与遮挡人物的代价远高于移动距离；正常桌面优先采用完全无覆盖的候选。
                let outside = 0,
                    overlap = 0;
                for (const r of [menu, actions]) {
                    const inside =
                        Math.max(0, Math.min(r[0] + r[2], bounds[2]) - Math.max(r[0], bounds[0])) *
                        Math.max(0, Math.min(r[1] + r[3], bounds[3]) - Math.max(r[1], bounds[1]));
                    outside += r[2] * r[3] - inside;
                    for (const p of protectedRects)
                        overlap +=
                            Math.max(0, Math.min(r[0] + r[2] + 4, p[0] + p[2]) - Math.max(r[0] - 4, p[0])) *
                            Math.max(0, Math.min(r[1] + r[3] + 4, p[1] + p[3]) - Math.max(r[1] - 4, p[1]));
                }
                const score =
                    outside * 100000 +
                    overlap * 1000 +
                    Math.abs(x - base.menu[0]) +
                    Math.abs(y - base.menu[1]) +
                    (left ? 20 : 0);
                candidates.push({ menu, actions, left, score });
            }
    candidates.sort((a, b) => a.score - b.score);
    const { menu, actions, left } = candidates[0]!;
    const dx = Math.max(0, -Math.min(menu[0], actions[0]));
    const dy = Math.max(0, -Math.min(menu[1], actions[1]));
    const shift = ([x, y, w, h]: Rect): Rect => [x + dx, y + dy, w, h];
    return {
        left,
        layout: {
            ...base,
            width: Math.ceil(Math.max(base.width, menu[0] + menu[2], actions[0] + actions[2]) + dx + 2),
            height: Math.ceil(Math.max(base.height, menu[1] + menu[3], actions[1] + actions[3]) + dy + 2),
            pet: shift(base.pet),
            panel: shift(base.panel),
            bubble: shift(base.bubble),
            menu: shift(menu),
            actions: shift(actions),
            floating: base.floating.map(shift),
        },
    };
}

/**
 * 交接期间按当下可见内容扩展场景，防止保留的旧菜单范围裁掉刚显示的详情卡片。
 * @param layout 当前几何，包含保留的菜单坐标及最新卡片高度。
 * @param visible 当下必须容纳的矩形，允许尚未归一化的左／上负坐标。
 * @returns 一起平移并扩展后的几何；人物与卡片相对位置不变，原生锚点同步补偿原点。
 */
export function fitSceneLayout(layout: SceneLayout, visible: Rect[]): SceneLayout {
    const dx = Math.max(0, -Math.min(0, ...visible.map(([x]) => x)));
    const dy = Math.max(0, -Math.min(0, ...visible.map(([, y]) => y)));
    const shift = ([x, y, w, h]: Rect): Rect => [x + dx, y + dy, w, h];
    return {
        ...layout,
        width: Math.ceil(Math.max(layout.width, ...visible.map(([x, , w]) => x + w + 2)) + dx),
        height: Math.ceil(Math.max(layout.height, ...visible.map(([, y, , h]) => y + h + 2)) + dy),
        pet: shift(layout.pet),
        panel: shift(layout.panel),
        bubble: shift(layout.bubble),
        menu: shift(layout.menu),
        actions: shift(layout.actions),
        floating: layout.floating.map(shift),
    };
}

/**
 * @param size 用户选择的角色画布边长，96—192 逻辑像素。
 * @param open 是否显示功能面板。
 * @param panelHeight 实测卡片高度，不包括下拉浮层，至少 156。
 * @param bubble 是否为常驻状态云朵预留区域。
 * @param menu 是否显示右键菜单。
 * @param floating 相对卡片左上角的可见浮层矩形，只允许有限正尺寸。
 * @param actions 是否展开动作二级菜单；缺省false，旧调用保持一级菜单布局。
 * @returns 可见场景范围及区域；菜单关闭后不保留空白鼠标占位。
 */
export function sceneLayout(
    size: number,
    open: boolean,
    panelHeight: number,
    bubble: boolean,
    menu: boolean,
    floating: Rect[] = [],
    actions = false,
): SceneLayout {
    const top = 4;
    // 让面板左边穿过角色身体偏左处，形成原稿中的搭接；按旧 size-44 会悬在卡片外。
    const panelX = Math.round(size * 0.38);
    // 卡片贴近坐姿下缘，只让标题栏与角色搭接；选择器完整置于角色交互矩形下方。
    const panelY = top + Math.round(size * 0.76);
    const panel: Rect = [panelX, panelY, 360, Math.max(156, panelHeight)];
    const pet: Rect = [0, top, size, size];
    const cloud: Rect = [Math.round(size * 0.76), Math.round(size * 0.12), 132, 50];
    const contextMenu: Rect = [4, top + size + 4, 184, 208];
    const actionMenu: Rect = [contextMenu[0] + contextMenu[2] - 1, contextMenu[1], 184, 326];
    const overlays: Rect[] = open
        ? floating
              .filter((r) => r.length === 4 && r.every(Number.isFinite) && r[2] > 0 && r[3] > 0)
              .slice(0, 8)
              .map(([x, y, w, h]) => [Math.max(0, panelX + x), Math.max(0, panelY + y), w, h])
        : [];
    const visible: Rect[] = [
        pet,
        ...(open ? [panel] : []),
        ...(bubble ? [cloud] : []),
        ...(menu ? [contextMenu] : []),
        ...(menu && actions ? [actionMenu] : []),
        ...overlays,
    ];
    return {
        width: Math.ceil(Math.max(...visible.map(([x, , w]) => x + w)) + 2),
        height: Math.ceil(Math.max(...visible.map(([, y, , h]) => y + h)) + 2),
        pet,
        panel,
        bubble: cloud,
        menu: contextMenu,
        actions: actionMenu,
        floating: overlays,
    };
}
