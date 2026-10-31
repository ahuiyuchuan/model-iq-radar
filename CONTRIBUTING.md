# 开发与贡献

作者：ahui。提交前请使用虚构数据，不提交 API Key、真实回答、数据库或本机凭据。

## 环境

- Windows x64、WebView2 Runtime。
- Node.js `^22.12.0 || ^24.0.0 || >=26.0.0`、npm（Vitest 5.0.5 的引擎范围，生产 EXE 无需 Node）。
- Rust stable MSVC 工具链、MSVC C++ Build Tools 与 Windows SDK。首次编译需要下载 crates。
- Python/Pillow 仅在重新生成图标/桌宠素材或打包皮肤资源时需要，普通构建不依赖 Python。

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

可执行文件默认位于 `src-tauri/target/release/model-probe.exe`；设置 `CARGO_TARGET_DIR` 时以该目录为准。自 v1.0.3 起，本地测试和 GitHub Release 均只交付单个 `Model-IQ-Radar-v<版本>-Windows-x64.exe`，不提供 ZIP、文档包或独立校验文件。Tag 仍使用 `v<语义版本>`；GitHub 自动生成的源码归档不属于手工上传资产。许可证、使用说明和发布说明保留在仓库。

构建后用 PowerShell 7 执行 `./scripts/package_exe.ps1`，校验版本、PE x64、构建输入时间与复制前后 SHA-256，输出到 `release-artifacts/v<版本>/`。仅向控制台输出校验信息，不生成附带文件；新轮次可用 `-OutputDirectory <目录>` 保留旧交付物，非默认 Cargo 目录可用 `-SourcePath <EXE绝对路径>`。已有同名不同内容时明确报错，不覆盖旧文件。

依赖安装完成且 Cargo 在 PATH 中时，运行 `node scripts/generate_notices.mjs` 可重新生成第三方许可证合订本；生成器不联网，缺少依赖或声明会失败，不能手工删改第三方原文。原始声明的上游补充来源见 `docs/licenses/README.md`。

邮件图标为已提交的 PNG，常规构建不需要图像依赖。需要重新生成时，在 `npm ci` 后准备已安装的 `sharp`，执行 `node scripts/generate_mail_assets.mjs [sharp 模块绝对目录]`；省略路径时从当前环境解析 `sharp`。脚本仅将现有 Lucide 图标写入 `src-tauri/assets/mail` 白名单，不联网、不改第三方原文。邮件模板位于 `src-tauri/src/notifications/template.rs`，采用内联样式、表格和 CID 图标；所有记录字段必须安全转义，完整回答不得截断或渲染成 HTML。设置 `MODEL_PROBE_EMAIL_PREVIEW_DIR` 后执行 Rust 测试，可将虚构记录的 HTML/EML 导出到指定 QA 目录；这条路径仅在测试代码中使用，不会发信或读取正式凭据。须以真实 MIME 中的 CID 附件预览并核对长内容与窄屏，不把浏览器预览等同于 Outlook/Gmail 实际投递验证。

HTML 预览采用页面内 iframe。组件回归在 `tests/html-answer.test.ts`；桌面验收必须用独立 identifier、虚构数据库和模型，避免读取正式数据。验证有界画布缩放、无滚动条、HTML 悬停、收起裁剪、页内放大、Escape/焦点恢复，以及列表实时动画不弹出额外窗口。

桌面集成检查应只在隔离 QA 标识、虚构数据库和模型下运行，正式构建不开放调试端口。iframe 共享渲染资源，不能将旧工作进程的死循环硬终止检查当作当前保证。当前版本的实际检查与未覆盖边界见 `VALIDATION.md`，当前桌宠模块范围见 `docs/DESKTOP_PET_CHANGE_SUMMARY.md`；`docs/FILE_AUDIT-v1.0.6.md`仅记录历史版本。

## 目录职责

桌宠模块为 `src/pet`、`PetActor/PetStage/SkinManager` 与 `src-tauri/src/pet_window.rs`；皮肤包规范和制作命令见 [SKIN_PACKS.md](docs/SKIN_PACKS.md)。`python scripts/build_pet_assets.py` 从本轮四组定稿中的角色源图生成十二动作PNG及提取记录，勿手改生成结果。源图为 `source-states.png`、`source-actions.png`、`card-compact-design.png`、`card-expanded-design.png`，均在 `assets/pet/blue-whale-maid`；裁切与归一尺度可追溯到 `extraction.json`。播放器只接受 PNG 图集，普通构建直接复制 `public` 资源，EXE 打包检查也包含该目录。

浏览器交互验收执行 `node scripts/verify_pet_ui.mjs`；使用已有 Playwright/Chrome 时可设置 `PET_PLAYWRIGHT_MODULE` 与 `PET_CHROMIUM_EXECUTABLE`。脚本使用独立虚构内存桥接、仅监听回环并拒绝远程请求，截图默认写入系统临时目录的 `model-iq-radar-pet-qa`，可用 `PET_QA_SCREENSHOT_DIR` 指定其他目录；不读取正式数据。仓库只保留 README 引用的桌宠截图，其他验收图片不进入源码目录。Windows 点击穿透、跨屏 DPI、真实拖动和休眠需另行在隔离 QA 标识下验收，浏览器截图不能替代。

还须先 `npm run build` 再运行 `node scripts/verify_pet_csp.mjs`，使用生产 CSP 与真实 dist 验证内置描述、十二动作图集解码、待机轻移、资源失败阻止保存及经典恢复。交互脚本另验证统一常驻结果云朵、关闭临时文字仍保留结果、单击/双击、拖动抢占与过期反馈丢弃。桌宠紧凑主体176px，文字／HTML／原始回答普通详情统一546px；浮层单独测量，仅扩展窗口和输入掩码，不改变卡片主体。本轮只做浏览器验收，已清理独立原生模拟脚本；正式配置、凭据和数据库不得用于验收。具体已完成与未完成的项目见 VALIDATION；播放器与原生结构见 [DESKTOP_PET_DESIGN.md](docs/DESKTOP_PET_DESIGN.md)。

- `src/components`：悬浮窗、设置、共用 Markdown、选择器和关于组件。
- `src/bridge.ts` / `src/types.ts`：IPC 入口与契约；浏览器预览只保留虚构内存状态。
- `src/markdown.ts`：不可信回答的安全显示副本，不能参与判定、保存和复制。
- `src/htmlAnswer.ts` / `src/htmlSandbox.ts`：与后端共用 html-v1 识别语义、临时 iframe 文档包装和会话消息验证；不生成或缓存截图。
- `src-tauri/src/api.rs` / `discovery.rs`：兼容模型请求、目录拉取、独立连接诊断。
- `src-tauri/src/lib.rs`：运行门禁、定时、凭据与窗口生命周期。
- `src-tauri/src/html.rs` / `preview.rs`：自动识别、请求状态结算、本地临时文档协议、响应头 CSP 和既有 WebView 的 iframe 导航保护；不创建预览窗口。
- `src-tauri/src/storage.rs` / `model.rs`：SQLite、配置、规则与历史契约。
- `src-tauri/src/autostart.rs` / `updates.rs`：当前用户自启、只读更新检查与系统浏览器入口。
- `src-tauri/src/notifications.rs` / `notifications/template.rs`：加密 SMTP、模型分组通知状态、当次记录的 HTML/纯文本正文与内嵌图标。
- `tests` / `src-tauri/tests`：组件与后端回归，接口只使用本机 Mock；自启测试只操作随机私有注册表键并清理。

## 变更约束

角色运行资源沿用schema1；新增 `landing/bow/failed/error` 为可选动作，旧包缺动作回退待机，无数据库迁移。招手、伸懒腰等额外姿态是单帧静态原画，不能再用网格、横带或整体旋转模拟；静态交接使用资源就绪后的96ms短溶解，不能冒充连续肢体动画。待机16帧仅整体上下平移最多1源像素，未制作眨眼或呼吸绑定，不得把独立姿态图称为连续动画。Java/JVM/MyBatis专属规范不适用于此Vue/TypeScript/Rust项目。

源码采用 UTF-8 无 BOM、LF、四空格缩进。TypeScript/Vue/CSS 用 Prettier，Rust 用 rustfmt；注释解释业务目的、异常、安全边界和兼容性。

保持文字的完整原文连续字符串判定、原文持久化/复制、规则快照，以及手动和自动测试的共用门禁。HTML 使用中性展示状态，不参与判定/邮件；UI 展示不得改变已保存结果。变更配置、默认值和 IPC 时同步文档与有意义的回归；不要直接修改锁文件或生成文件，使用依赖工具和生成脚本。

GPL-3.0-only 适用于本项目代码。依赖保留各自许可证；新增依赖需同步第三方声明。发布 Tag 使用 `v<语义版本>`，发布标题采用“测智雷达 v<版本>”，发布说明保存在 `docs/release-notes`，用户可见变化同时写入 CHANGELOG。

请勿把没有执行的测试写成已通过。原生多屏、注销登录等不能由组件测试覆盖的场景，应说明验证范围。
