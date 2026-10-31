/** 桌宠契约：图集使用源像素，角色/窗口使用逻辑像素；时间为毫秒或显式 fps。@author ahui */
export const ACTIONS = [
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
] as const;
export type PetAction = (typeof ACTIONS)[number];
export type Rect = [number, number, number, number];
/** 宿主已实现的贴边方向；底边保留给任务栏，不自动收起。 */
export type DockSide = 'left' | 'right' | 'top';

/** 可持久化外观偏好；旧配置缺省为经典布局，避免升级后自动改变桌面形态。 */
export interface SkinPreferences {
    /** 已安装 ID；classic 为原布局，资源缺失时只在运行时回退，不改其他设置。 */
    id: string;
    /** 角色画布边长，96—192 逻辑像素；展开保持同一源像素尺度，避免头部突然变大。 */
    size: number;
    animation: boolean;
    bubbles: boolean;
    /** 允许空闲时招手、伸懒腰和侧坐休息；关闭后仍保留用户操作反馈。 */
    autonomous?: boolean;
    /** 当前皮肤任一动作均可作空闲外观；不表示真实业务结果，缺资源时回退 idle。 */
    idleAction?: PetAction;
}
export const DEFAULT_SKIN: SkinPreferences = {
    id: 'classic',
    size: 144,
    animation: true,
    bubbles: true,
    autonomous: true,
    idleAction: 'idle',
};

/** 图集使用等大网格；hitBands 是所有帧可见轮廓的保守并集，不能拿来裁单帧。 */
export interface SkinClip {
    /** 文件名必须为同包 PNG，不允许目录、URL 或脚本。 */
    atlas: string;
    poster: string;
    frameSize: number;
    frames: number;
    columns: number;
    /** 每秒帧数，1—30；按经过时间选帧，不要求每个显示刷新都重绘。 */
    fps: number;
    loop: boolean;
    hitBands: Rect[];
    /** 可选独立眨眼帧段；前 start 帧循环呼吸，眨眼以随机间隔和独立帧率播放。 */
    blink?: { start: number; frames: number; fps: number; minDelayMs: number; maxDelayMs: number };
}
export interface SkinManifest {
    schemaVersion: 1;
    id: string;
    name: string;
    author: string;
    description: string;
    tokens: { panelBackground: string; panelBorder: string; primary: string; textPrimary: string };
    /** 面板边缘装饰预设；缺省/none 不绘制。只接受内置矢量样式，不执行皮肤提供的 SVG/CSS。 */
    decorations?: 'none' | 'ocean' | 'stars';
    /** 可选通用探头源图，left保留历史字段；宿主固定镜像到右侧、顺时针旋转90度到顶部。缺省保持完整角色。 */
    docking?: { left: SkinClip };
    actions: Partial<Record<PetAction, SkinClip>> & { idle: SkinClip };
}

/** 导入包自包含 PNG data URL；不接受远程资源、脚本、任意 CSS 或绝对文件路径。 */
export interface SkinPack {
    manifest: SkinManifest;
    assets: Record<string, string>;
}
export interface SkinEntry {
    id: string;
    name: string;
    author: string;
    builtin: boolean;
}
export interface LoadedSkin {
    manifest: SkinManifest;
    image: (action: PetAction) => Promise<HTMLImageElement>;
    poster: string;
    /** 已完成解码的通用探头源PNG；三方向只使用宿主可信变换，不执行包内脚本。 */
    dockPoster?: string;
    /** 清除解码缓存引用并拒绝迟到加载；每个成功加载的句柄必须由宿主释放。 */
    dispose: () => void;
}
