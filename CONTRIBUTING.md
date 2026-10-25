# 开发与贡献

作者：ahui。提交前请使用虚构数据，不提交 API Key、真实回答、数据库或本机凭据。

## 环境

- Windows x64、WebView2 Runtime。
- Node.js 22.12+（或兼容版本）、npm。
- Rust stable MSVC 工具链、MSVC C++ Build Tools 与 Windows SDK。首次编译需要下载 crates。
- Python/Pillow 仅在重新生成图标时需要，普通构建不依赖 Python。

## 构建与检查

```powershell
npm ci
npm run dev             # 浏览器界面预览，不调用真实 API
npm run tauri -- dev    # Windows 桌面开发
npm run test
npm run build
npm run format:check
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run tauri -- build --no-bundle
```

可执行文件默认位于 `src-tauri/target/release/model-probe.exe`；设置 `CARGO_TARGET_DIR` 时以该目录为准。发布只使用免安装包，不需要执行安装器构建。

依赖安装完成且 Cargo 在 PATH 中时，运行 `node scripts/generate_notices.mjs` 可重新生成第三方许可证合订本；生成器不联网，缺少依赖或声明会失败，不能手工删改第三方原文。原始声明的上游补充来源见 `docs/licenses/README.md`。

邮件图标为已提交的 PNG，常规构建不需要图像依赖。需要重新生成时，在 `npm ci` 后准备已安装的 `sharp`，执行 `node scripts/generate_mail_assets.mjs [sharp 模块绝对目录]`；省略路径时从当前环境解析 `sharp`。脚本仅将现有 Lucide 图标写入 `src-tauri/assets/mail` 白名单，不联网、不改第三方原文。邮件模板位于 `src-tauri/src/notifications/template.rs`，采用内联样式、表格和 CID 图标；所有记录字段必须安全转义，完整回答不得截断或渲染成 HTML。设置 `MODEL_PROBE_EMAIL_PREVIEW_DIR` 后执行 Rust 测试，可将虚构记录的 HTML/EML 导出到指定 QA 目录；这条路径仅在测试代码中使用，不会发信或读取正式凭据。须以真实 MIME 中的 CID 附件预览并核对长内容与窄屏，不把浏览器预览等同于 Outlook/Gmail 实际投递验证。

## 目录职责

- `src/components`：悬浮窗、设置、共用 Markdown、选择器和关于组件。
- `src/bridge.ts` / `src/types.ts`：IPC 入口与契约；浏览器预览只保留虚构内存状态。
- `src/markdown.ts`：不可信回答的安全显示副本，不能参与判定、保存和复制。
- `src-tauri/src/api.rs` / `discovery.rs`：兼容模型请求、目录拉取、独立连接诊断。
- `src-tauri/src/lib.rs`：运行门禁、定时、凭据与窗口生命周期。
- `src-tauri/src/storage.rs` / `model.rs`：SQLite、配置、规则与历史契约。
- `src-tauri/src/autostart.rs` / `updates.rs`：当前用户自启、只读更新检查与系统浏览器入口。
- `src-tauri/src/notifications.rs` / `notifications/template.rs`：加密 SMTP、通知去重、当次记录的 HTML/纯文本正文与内嵌图标。
- `tests` / `src-tauri/tests`：组件与后端回归，接口只使用本机 Mock；自启测试只操作随机私有注册表键并清理。

## 变更约束

源码采用 UTF-8 无 BOM、LF、四空格缩进。TypeScript/Vue/CSS 用 Prettier，Rust 用 rustfmt；注释解释业务目的、异常、安全边界和兼容性。

保持完整原文连续字符串判定、原文持久化/复制、规则快照，以及手动和自动测试的共用门禁。UI 展示不得改变测智结果。变更配置、默认值和 IPC 时同步文档与有意义的回归；不要直接修改锁文件或生成文件，使用依赖工具和生成脚本。

GPL-3.0-only 适用于本项目代码。依赖保留各自许可证；新增依赖需同步第三方声明。发布 Tag 使用 `v<语义版本>`，发布标题采用“测智雷达 v<版本>”，发布说明保存在 `docs/release-notes`，用户可见变化同时写入 CHANGELOG。

请勿把没有执行的测试写成已通过。原生多屏、注销登录等不能由组件测试覆盖的场景，应说明验证范围。
