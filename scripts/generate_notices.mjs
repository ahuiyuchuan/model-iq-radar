/**
 * 从已安装依赖和 Cargo 元数据生成许可证合订本，不联网或读取用户配置。
 * 先执行 npm ci / cargo fetch，再运行 node scripts/generate_notices.mjs。
 * 缺少原始声明时使用仓库中已核对的上游副本；未覆盖的缺项直接失败。
 * @author ahui
 */
import { readFileSync, writeFileSync, readdirSync, existsSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const text = (path) => readFileSync(path, 'utf8').replace(/\r\n/g, '\n').trimEnd();
const parts = [
    '# Third-party notices\n\n测智雷达 v1.0.0。整理：ahui。\n\n本项目采用 GPL-3.0-only。第三方依赖保留各自许可证；本文件由 scripts/generate_notices.mjs 从锁定版本的依赖声明生成，收录生产前端和 Windows Cargo 依赖（含构建依赖）的 LICENSE / NOTICE 原文。未修改第三方许可证内容。\n',
];
const licenseFiles = (directory) =>
    readdirSync(directory, { withFileTypes: true })
        .filter(
            (file) =>
                file.isFile() && /^(LICENSE|LICENCE|COPYING|NOTICE|UNLICENSE)([-._].*)?$/i.test(file.name),
        )
        .map((file) => join(directory, file.name));

/** 原样收录许可证；fence 长于内容中反引号串，防止第三方文本破坏文档边界。 */
function appendFiles(paths) {
    for (const path of paths) {
        const content = text(path);
        const fence = '`'.repeat(Math.max(3, ...[...content.matchAll(/`+/g)].map((m) => m[0].length + 1)));
        parts.push(`### ${path.split(/[\\/]/).at(-1)}\n\n${fence}text\n${content}\n${fence}\n`);
    }
}

const lock = JSON.parse(text(join(root, 'package-lock.json')));
let frontend = 0;
for (const [path, entry] of Object.entries(lock.packages).sort(([a], [b]) => a.localeCompare(b))) {
    if (!path || entry.dev) continue;
    const directory = join(root, path);
    if (!existsSync(directory)) throw new Error(`请先安装锁定依赖：${path}`);
    const manifest = JSON.parse(text(join(directory, 'package.json')));
    const licenses = licenseFiles(directory);
    if (!licenses.length) throw new Error(`缺少 npm 许可证：${manifest.name}`);
    parts.push(`## npm: ${manifest.name} ${manifest.version}\n\nLicense: ${manifest.license}\n`);
    appendFiles(licenses);
    frontend++;
}

const metadata = JSON.parse(
    execFileSync(
        'cargo',
        [
            'metadata',
            '--manifest-path',
            'src-tauri/Cargo.toml',
            '--format-version',
            '1',
            '--locked',
            '--offline',
            '--filter-platform',
            'x86_64-pc-windows-msvc',
        ],
        { cwd: root, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 },
    ),
);
const active = new Set(metadata.resolve.nodes.map((node) => node.id));
const fallbacks = {
    'alloc-stdlib': 'alloc-stdlib-LICENSE',
    selectors: 'selectors-LICENSE-MPL-2.0',
    'tauri-plugin': 'tauri-LICENSE-MIT',
    'webview2-com': 'webview2-LICENSE',
    'webview2-com-macros': 'webview2-LICENSE',
    'webview2-com-sys': 'webview2-LICENSE',
};
let crates = 0;
for (const pkg of metadata.packages.sort((a, b) =>
    `${a.name} ${a.version}`.localeCompare(`${b.name} ${b.version}`),
)) {
    if (!active.has(pkg.id) || pkg.id === metadata.resolve.root) continue;
    let licenses = licenseFiles(dirname(pkg.manifest_path));
    if (!licenses.length && fallbacks[pkg.name])
        licenses = [join(root, 'docs/licenses', fallbacks[pkg.name])];
    if (!licenses.length) throw new Error(`缺少 crate 许可证：${pkg.name}`);
    parts.push(
        `## crate: ${pkg.name} ${pkg.version}\n\nLicense: ${pkg.license}\n\nSource: ${pkg.repository || 'crates.io'}\n`,
    );
    appendFiles(licenses);
    crates++;
}
writeFileSync(join(root, 'THIRD_PARTY_NOTICES.md'), `${parts.join('\n')}\n`, 'utf8');
console.log(JSON.stringify({ frontend, crates }));
