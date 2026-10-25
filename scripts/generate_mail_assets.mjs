/**
 * 将项目已有 Lucide 图标生成为邮件 CID 使用的双倍像素 PNG，避免邮件客户端屏蔽 SVG/远程图片。
 * 仅重新生成 src-tauri/assets/mail 下列白名单文件，不联网、不修改原图标或用户数据。
 * 先 npm ci，再执行 node scripts/generate_mail_assets.mjs [已安装的 sharp 模块目录]；
 * sharp 只用于重新生成静态资源，桌面运行和常规构建不依赖它。缺少模块时直接失败。
 * @author ahui
 */
import { mkdir, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { h } from 'vue';
import { renderToString } from '@vue/server-renderer';
import * as icons from 'lucide-vue-next';

const require = createRequire(import.meta.url);
const sharp = require(process.argv[2] || 'sharp');
const output = new URL('../src-tauri/assets/mail/', import.meta.url);
// 名称是邮件模板中的固定 CID；颜色沿用首版深色卡片与绿/黄/红结果色。
const assets = [
    ['brand', 'Target', '#8cd5ff', 48],
    ['degraded', 'CircleAlert', '#f5b63f', 56],
    ['recovered', 'CircleCheck', '#50d694', 56],
    ['failure', 'CircleX', '#f37670', 56],
    ['test', 'Mail', '#8cd5ff', 56],
    ['profile', 'Box', '#91b5d5', 24],
    ['model', 'Network', '#91b5d5', 24],
    ['rule', 'ListChecks', '#91b5d5', 24],
    ['effort', 'Brain', '#91b5d5', 24],
    ['protocol', 'Cloud', '#91b5d5', 24],
    ['source', 'ShieldCheck', '#91b5d5', 24],
    ['time', 'Clock3', '#91b5d5', 24],
    ['duration', 'Zap', '#91b5d5', 24],
    ['note-degraded', 'FileText', '#f5b63f', 22],
    ['note-recovered', 'FileText', '#50d694', 22],
    ['note-failure', 'FileText', '#f37670', 22],
    ['github', 'Github', '#9eb8d4', 24],
];
await mkdir(output, { recursive: true });
for (const [name, icon, color, size] of assets) {
    const svg = await renderToString(h(icons[icon], { size, color, strokeWidth: 1.8 }));
    const png = await sharp(Buffer.from(svg))
        .resize(size * 2, size * 2)
        .png({ compressionLevel: 9 })
        .toBuffer();
    await writeFile(new URL(`${name}.png`, output), png);
}
process.stdout.write(`${assets.length} mail icons generated in ${fileURLToPath(output)}\n`);
