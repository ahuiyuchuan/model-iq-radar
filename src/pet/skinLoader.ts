/** 版本化皮肤校验与有界图片缓存；资源失败由宿主恢复经典外观。@author ahui */
import { call, desktop } from '../bridge';
import { ACTIONS, type LoadedSkin, type SkinClip, type SkinManifest, type SkinPack } from './types';

export const BUILTIN_SKINS = [
    { id: 'classic', name: '经典雷达', author: 'ahui', builtin: true },
    { id: 'blue-whale-maid', name: '蓝鲸女仆', author: 'ahui', builtin: true },
];
const SAFE_FILE = /^[a-zA-Z0-9_-]+\.png$/;
const MAX_PACK_BYTES = 48 * 1024 ** 2;

/** 同一文件只能声明一种尺寸；共享动作引用按实际图片计费，避免重复预算和缓存解释不一致。 */
function resources(manifest: SkinManifest): Map<string, [number, number]> {
    const files = new Map<string, [number, number]>();
    for (const clip of [
        ...Object.values(manifest.actions),
        ...(manifest.docking ? [manifest.docking.left] : []),
    ]) {
        for (const kind of ['atlas', 'poster'] as const) {
            const expected: [number, number] =
                kind === 'atlas'
                    ? [clip.frameSize * clip.columns, clip.frameSize * Math.ceil(clip.frames / clip.columns)]
                    : [clip.frameSize, clip.frameSize];
            const previous = files.get(clip[kind]);
            if (previous && (previous[0] !== expected[0] || previous[1] !== expected[1]))
                throw new Error('同一 PNG 不能声明不同尺寸。');
            files.set(clip[kind], expected);
        }
    }
    return files;
}

/** 只读取有界 PNG 头核对尺寸；完整解码在预览/保存阶段进行，不把伪造头视为有效图片。 */
function pngDimensions(data: unknown): [number, number] {
    if (
        typeof data !== 'string' ||
        data.length > 24 * 1024 ** 2 ||
        !/^data:image\/png;base64,[A-Za-z0-9+/]+={0,2}$/.test(data)
    )
        throw new Error('皮肤包必须包含所引用的 PNG 图片。');
    const raw = data.slice('data:image/png;base64,'.length);
    if (raw.length < 44 || raw.length % 4 !== 0) throw new Error('PNG 编码无效。');
    const header = atob(raw.slice(0, 32));
    if (header.slice(0, 16) !== '\x89PNG\r\n\x1a\n\0\0\0\rIHDR') throw new Error('PNG 文件头无效。');
    const view = new DataView(Uint8Array.from(header, (c) => c.charCodeAt(0)).buffer);
    return [view.getUint32(16), view.getUint32(20)];
}

/** 校验外部数据而非断言类型；拒绝不完整必需动作、超量解码、失控热区和不可解释配色。
 * @param value 未受信任的 JSON 数据，不允许为空。
 * @returns 校验通过的描述；只有 idle 必需，其他动作统一回退 idle。
 * @throws 格式、版本、图片预算或热区边界不合法时抛出可读错误。
 */
export function validateManifest(value: unknown): SkinManifest {
    const m = value as SkinManifest;
    if (
        !m ||
        m.schemaVersion !== 1 ||
        typeof m.id !== 'string' ||
        !/^[a-z0-9][a-z0-9-]{0,63}$/.test(m.id) ||
        typeof m.name !== 'string' ||
        !m.name.trim() ||
        m.name.length > 60 ||
        typeof m.author !== 'string' ||
        m.author.length > 80 ||
        typeof m.description !== 'string' ||
        m.description.length > 300 ||
        !m.actions?.idle ||
        !m.tokens
    )
        throw new Error('皮肤描述无效，必须包含版本、名称和待机动作。');
    for (const key of ['panelBackground', 'panelBorder', 'primary', 'textPrimary'] as const)
        if (!/^#[0-9a-fA-F]{6}$/.test(m.tokens[key])) throw new Error('皮肤配色必须是六位十六进制颜色。');
    // 装饰只选择宿主内置矢量，不允许导入包插入代码；缺省兼容已有 PNG 包。
    if (m.decorations !== undefined && !['none', 'ocean', 'stars'].includes(m.decorations))
        throw new Error('皮肤面板装饰预设无效。');
    const clips: [string, SkinClip][] = [];
    for (const action of Object.keys(m.actions)) {
        if (!ACTIONS.includes(action as (typeof ACTIONS)[number])) throw new Error('皮肤包含不支持的动作。');
        clips.push([action, m.actions[action as (typeof ACTIONS)[number]] as SkinClip]);
    }
    // left保留历史源图键名；三边适配由宿主固定变换完成，不允许包内注入变换或脚本。
    if (m.docking !== undefined) {
        if (
            !m.docking ||
            typeof m.docking !== 'object' ||
            Array.isArray(m.docking) ||
            Object.keys(m.docking).length !== 1 ||
            !Object.hasOwn(m.docking, 'left')
        )
            throw new Error('皮肤探头须使用 docking.left 声明通用静态源图。');
        const left = m.docking.left;
        if (!left || left.frames !== 1 || left.columns !== 1 || left.blink !== undefined)
            throw new Error('皮肤探头必须提供单帧方形 PNG。');
        clips.push(['dock-left', left]);
    }
    for (const [action, c] of clips) {
        if (
            !c ||
            typeof c.atlas !== 'string' ||
            typeof c.poster !== 'string' ||
            c.atlas.length > 90 ||
            c.poster.length > 90 ||
            !SAFE_FILE.test(c.atlas) ||
            !SAFE_FILE.test(c.poster) ||
            !Number.isInteger(c.frameSize) ||
            c.frameSize < 32 ||
            c.frameSize > 512 ||
            !Number.isInteger(c.frames) ||
            c.frames < 1 ||
            c.frames > 64 ||
            !Number.isInteger(c.columns) ||
            c.columns < 1 ||
            c.columns > 8 ||
            !Number.isFinite(c.fps) ||
            c.fps < 1 ||
            c.fps > 30 ||
            typeof c.loop !== 'boolean' ||
            !Array.isArray(c.hitBands) ||
            !c.hitBands.length ||
            c.hitBands.length > 256
        )
            throw new Error('皮肤动作参数超出支持范围。');
        if (c.blink !== undefined) {
            if (!c.blink || typeof c.blink !== 'object') throw new Error('眨眼帧段或间隔无效。');
            const b = c.blink;
            if (
                action !== 'idle' ||
                !c.loop ||
                !Number.isInteger(b.start) ||
                b.start < 1 ||
                !Number.isInteger(b.frames) ||
                b.frames < 2 ||
                b.start + b.frames > c.frames ||
                !Number.isFinite(b.fps) ||
                b.fps < 1 ||
                b.fps > 30 ||
                !Number.isFinite(b.minDelayMs) ||
                !Number.isFinite(b.maxDelayMs) ||
                b.minDelayMs < 1000 ||
                b.maxDelayMs > 60000 ||
                b.minDelayMs > b.maxDelayMs
            )
                throw new Error('眨眼帧段或间隔无效。');
        }
        for (const r of c.hitBands)
            if (
                !Array.isArray(r) ||
                r.length !== 4 ||
                r.some((v) => !Number.isFinite(v) || v < 0) ||
                r[2] <= 0 ||
                r[3] <= 0 ||
                r[0] + r[2] > c.frameSize ||
                r[1] + r[3] > c.frameSize
            )
                throw new Error('角色轮廓超出源画布。');
    }
    const pixels = [...resources(m).values()].reduce((sum, [width, height]) => sum + width * height, 0);
    if (pixels * 4 > 64 * 1024 ** 2) throw new Error('皮肤解码资源超过 64 MiB 预算。');
    return m;
}

/** 导入文件仅允许自包含 PNG，检查字节预算和引用完整性，忽略任何未引用附加字段。 */
export function validatePack(value: unknown): SkinPack {
    const p = value as SkinPack;
    validateManifest(p?.manifest);
    if (!p.assets || typeof p.assets !== 'object') throw new Error('皮肤包缺少图片。');
    if (['classic', 'blue-whale-maid'].includes(p.manifest.id)) throw new Error('不能覆盖内置皮肤。');
    const referenced = resources(p.manifest);
    let length = 0;
    for (const [file, expected] of referenced) {
        const data = p.assets[file];
        const dimensions = pngDimensions(data);
        if (dimensions[0] !== expected[0] || dimensions[1] !== expected[1])
            throw new Error('PNG 实际尺寸与皮肤描述不一致。');
        length += data.length;
    }
    if (Object.keys(p.assets).some((key) => !referenced.has(key))) throw new Error('皮肤包含未引用资源。');
    if (length > MAX_PACK_BYTES || new TextEncoder().encode(JSON.stringify(p)).length > MAX_PACK_BYTES)
        throw new Error('皮肤包超过 48 MiB。');
    return p;
}

/** 装载内置或本机导入的皮肤；只有当前/相邻动作保留解码引用，不在每帧读取资源。
 * @param id 已选皮肤 ID，classic 不通过此播放器加载。
 * @param supplied 导入预览时使用的已验证自包含资源包，可省略。
 * @returns 已预载待机图集的句柄；卸载必须调用 dispose。
 */
export async function loadSkin(id: string, supplied?: SkinPack): Promise<LoadedSkin> {
    let pack = supplied ? validatePack(supplied) : undefined;
    if (!pack && id !== 'blue-whale-maid') {
        if (!desktop) throw new Error('本机自定义皮肤需要在桌面应用中打开。');
        pack = validatePack(await call('get_skin', { id }));
    }
    let manifest: SkinManifest;
    if (pack) manifest = validateManifest(pack.manifest);
    else {
        // 读取应用同源的内置文件，受打包版 connect-src 'self' 约束；不向远程站点取资源。
        try {
            // 内置角色使用定稿原画图集，与自定义皮肤遵循同一个资源契约。
            const response = await fetch('/skins/blue-whale-maid/skin.json');
            if (!response.ok) throw new Error(`资源读取失败（${response.status}）`);
            const data = await response.json();
            manifest = validateManifest(data);
        } catch {
            throw new Error('内置皮肤资源无法读取，请使用完整的新版程序重试。');
        }
    }
    if (manifest.id !== id) throw new Error('皮肤标识与资源不一致。');
    const urls = pack?.assets;
    const cache = new Map<string, Promise<HTMLImageElement>>();
    const pending = new Set<() => void>();
    let disposed = false;
    const resource = (file: string) => urls?.[file] ?? `/skins/blue-whale-maid/${file}`;
    const files = resources(manifest);
    const decode = async (file: string): Promise<HTMLImageElement> => {
        if (disposed) throw new Error('皮肤已释放。');
        const cached = cache.get(file);
        if (cached) {
            // 真正按最近使用淘汰；否则“共用 atlas + 各动作 poster”会把正在复用的 atlas
            // 误判为最老资源，动作切换时反复创建 Image 并解码同一张大图。
            cache.delete(file);
            cache.set(file, cached);
            return cached;
        }
        const promise = new Promise<HTMLImageElement>((resolve, reject) => {
            const img = new Image();
            let finished = false;
            const cleanup = () => {
                clearTimeout(timeout);
                img.onload = null;
                img.onerror = null;
                pending.delete(cancel);
            };
            // dispose 必须立即结算未完成加载；不能等待迟到的浏览器回调才释放宿主。
            const cancel = () => {
                if (finished) return;
                finished = true;
                cleanup();
                img.src = '';
                reject(new Error('皮肤已释放。'));
            };
            const fail = (message: string) => {
                if (finished) return;
                finished = true;
                cleanup();
                img.src = '';
                reject(new Error(message));
            };
            const timeout = setTimeout(() => fail('皮肤图片加载超时。'), 12000);
            pending.add(cancel);
            img.onload = () => {
                if (disposed) return cancel();
                const expected = files.get(file)!;
                if (img.width !== expected[0] || img.height !== expected[1])
                    return fail('皮肤图集尺寸与描述不一致。');
                finished = true;
                cleanup();
                resolve(img);
            };
            img.onerror = () => fail('皮肤图片加载失败。');
            img.src = resource(file);
        });
        cache.set(file, promise);
        // 加载失败允许后续主动重试；仅删除自己，迟到失败不能清掉新请求的缓存。
        void promise.catch(() => {
            if (cache.get(file) === promise) cache.delete(file);
        });
        // 原生解码内存交由浏览器回收；只保留最近两个图集的强引用。
        if (cache.size > 2) cache.delete(cache.keys().next().value!);
        return promise;
    };
    const image = async (action: (typeof ACTIONS)[number]): Promise<HTMLImageElement> => {
        const clip = manifest.actions[action] ?? manifest.actions.idle;
        // 保存/导入会逐动作调用本入口，因此连未显示的 poster 也要完成实际解码预检。
        // 否则伪造 PNG 头能通过尺寸校验，却把损坏资源持久化为“可用”皮肤。
        const [atlas] = await Promise.all([decode(clip.atlas), decode(clip.poster)]);
        return atlas;
    };
    const dispose = () => {
        if (disposed) return;
        disposed = true;
        for (const cancel of [...pending]) cancel();
        cache.clear();
    };
    try {
        await image('idle');
        // 保存/启用前验证可选探头，损坏资源不能等贴边后才暴露；失败统一释放本次句柄。
        if (manifest.docking)
            await Promise.all([decode(manifest.docking.left.atlas), decode(manifest.docking.left.poster)]);
    } catch (cause) {
        dispose();
        throw cause;
    }
    return {
        manifest,
        image,
        poster: resource(manifest.actions.idle.poster),
        dockPoster: manifest.docking ? resource(manifest.docking.left.poster) : undefined,
        dispose,
    };
}
