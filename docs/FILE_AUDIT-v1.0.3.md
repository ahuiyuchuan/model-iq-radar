# v1.0.3 全部改动文件复核

作者：ahui。日期：2026-10-27。范围：相对基准 `23848a0` 的全部 47 个修改/新增文本文件。构建产物、依赖目录、正式用户数据和凭据不纳入源码交付。用户已完成人工 Beta 预览、授权全面测试，并在审阅提交消息和发布文案后批准提交、推送、Tag 和 Release。

## 本轮发现与处理

- 追加修复用户反馈的清空失败：恢复悬浮设置原先会注册默认启用的全局组合键，占用冲突中断事务。移除设置项、偏好字段、原生插件与注册/注销逻辑；旧配置/备份兼容读取并丢弃该字段，覆盖单独恢复、全选清空和重启持久化。
- 修复分类清空在首次 `await` 前未占位导致的重复点击竞态；增加两入口、全选/取消、空选禁用和在途去重回归。
- 实测发现 iframe 移入/移出放大弹层后，新的 `load` 可能晚于画面清空。提前进入等待态，并保留至新页面 `ready`；回归验证两帧布局完成后仍不会提前撤掉加载反馈。
- 按追加需求统一加载动画最少 300ms；快速就绪只补剩余时长，慢加载没有额外延时。新增 0/100/500ms 就绪和等待期间重新 load/切换记录的计时隔离回归；失败及时反馈，卸载取消计时。
- 更新落后于现有产品交互的测试：糖果常量 Mock、已移除工具栏/截图队列的选择器、102 个内置及自定义规则容量预期；补齐测试必需字段，修正 Clippy 风格问题。
- 删除未使用的截图模块和 `html-to-image` 依赖，移除旧原生窗口方案残留的 Windows feature。最终 npm 锁文件与基准相比仅版本变化；Cargo 工具移除全局快捷键专用依赖，保留其他依赖版本，许可证生成器同步移除不再使用的声明。
- 修正文档中固定 1024×768、截图列表、原生工作进程、心跳租约和等待测试授权等旧描述；历史验证记录明确标为旧实现证据。

## 逐文件索引

| 文件                                      | 核对重点                                                                              |
| ----------------------------------------- | ------------------------------------------------------------------------------------- |
| `CHANGELOG.md`                            | v1.0.3 变化、交互、兼容及发布日期。                                                   |
| `CONTRIBUTING.md`                         | 单 EXE 流程、隔离桌面验收、iframe 资源边界及现有目录职责。                            |
| `README.md`                               | 三处统一预览、HTML 邮件边界、三套内置题目、调度和迁移说明。                           |
| `THIRD_PARTY_NOTICES.md`                  | 使用既有脚本离线生成，37 个前端包/315 个 Windows crate；保留依赖的第三方原文不改写。  |
| `VALIDATION.md`                           | 当前实测与历史验证分开，记录实际命令、数字、构建提示和未覆盖项。                      |
| `docs/USER_GUIDE.md`                      | 超时、默认规则、五类清空、通知依据、历史筛选及预览生命周期。                          |
| `docs/FILE_AUDIT-v1.0.3.md`               | 本索引覆盖当前全部改动文件，不列出已删除的过渡模块。                                  |
| `docs/release-notes/v1.0.3-zh.md`         | 已审阅的正式发布正文，下载与文档链接指向 v1.0.3。                                     |
| `package.json`                            | 版本 1.0.3，现有前端依赖保留，无截图依赖。                                            |
| `package-lock.json`                       | npm 工具生成，版本与清单一致，无既有依赖升级。                                        |
| `src-tauri/Cargo.toml`                    | 版本与直接引用的 regex/WebView2/Windows 依赖匹配；无预览子窗口或全局快捷键插件。      |
| `src-tauri/Cargo.lock`                    | Cargo 生成，本项目版本/直接引用更新，既有依赖版本保持。                               |
| `src-tauri/tauri.conf.json`               | 正式 identifier 保留，版本 1.0.3；只开放指定本地 iframe 源。                          |
| `src-tauri/src/api.rs`                    | 正整数/精度校验、系统时钟加法溢出保护；非法预算在联网/回退前拒绝。                    |
| `src-tauri/src/backup.rs`                 | 导出 v3，兼容读取 v1/v2，不写旧接口超时、目录或凭据。                                 |
| `src-tauri/src/conditions.rs`             | 正式测试/连接诊断共用规则预算，摘要使用有效值，悬空绑定明确报错。                     |
| `src-tauri/src/discovery.rs`              | 短消息诊断适配规则结构；原目录与鉴权边界保留。                                        |
| `src-tauri/src/lib.rs`                    | 结算后计时、运行/诊断门禁、模型快捷选择、分类清空补偿、通知世代和同窗口放大恢复。     |
| `src-tauri/src/model.rs`                  | 规则超时/默认 300 秒、旧接口超时和快捷键字段兼容读取、HTML 中性记录及分类选择契约。   |
| `src-tauri/src/notifications.rs`          | HTML/部分 HTML 不发信、不消费文字状态；QQ SMTP 默认值不自动启用通知。                 |
| `src-tauri/src/storage.rs`                | schema v3 启动事务、迁移立即落盘、模型目录归属、分类清空原子提交及空通知依据持久化。  |
| `src-tauri/src/html.rs`                   | html-v1 共用样例；取消/错误优先，HTML 不匹配关键词，部分源码禁止执行。                |
| `src-tauri/src/preview.rs`                | owner 绑定、随机文档、16 份/16 MiB 限制、响应头 CSP、导航白名单、右键与下载浮层控制。 |
| `src-tauri/tests/data_integrity.rs`       | v3/schema/备份往返、旧接口超时忽略、规则迁移保存/失败回滚/删除后重启。                |
| `src-tauri/tests/regression.rs`           | 非法超时网络前拒绝、1 秒实际超时、600 秒预算和现有请求/取消回归。                     |
| `src/bridge.ts`                           | IPC 与浏览器虚构预览契约、初始题目、分类重置；浏览器不伪造原生 HTML 能力。            |
| `src/types.ts`                            | 规则预算、HTML 状态/版本、分类选择和结算后倒计时与 Rust 一致。                        |
| `src/htmlAnswer.ts`                       | 自动候选提取、多块独立、源码边界、旧历史只展示不重判。                                |
| `src/htmlSandbox.ts`                      | 引导脚本先行、背景/一次测量/首帧消息、随机会话校验、临时文档幂等释放。                |
| `src/historyExport.ts`                    | HTML 状态/识别字段导出，完整原始回答和 CSV 防公式规则保留。                           |
| `src/components/AboutPanel.vue`           | 规则级超时和三处隔离动画说明与当前实现一致。                                          |
| `src/components/ActionDialog.vue`         | 动态禁用操作，Tab 跳过禁用按钮，保持原焦点约束。                                      |
| `src/components/HistoryPanel.vue`         | 四项 AND 筛选、缺失/已删除快照、多列详情、列表信息/HTML 操作分离。                    |
| `src/components/MailSettings.vue`         | SMTP 默认仅补新建/旧空草稿，不覆盖自定义值或自动保存/启用。                           |
| `src/components/ModelPicker.vue`          | 分组触发器精简，菜单仍显示模型 ID 和规则；原键盘/去重行为保留。                       |
| `src/components/Settings.vue`             | 移除接口超时、正整数规则/全局预算、糖果默认、分类清空防重、剪贴板世代隔离和草稿保护。 |
| `src/components/Widget.vue`               | 两行比例、空目录拉取、迟到隔离、结算倒计时、菜单保留预览和放大防收起。                |
| `src/components/AnswerContent.vue`        | 文本/HTML 共用入口，保留无候选原文，错误/取消不能执行部分回答。                       |
| `src/components/HistoryAnswerPreview.vue` | 可见行运行，放大期间保留，离屏/卸载释放；文字摘要和 HTML 分别交互。                   |
| `src/components/HtmlPreview.vue`          | 首帧等待、消息与尺寸校验、加载超时、放大互斥/焦点恢复、迟到释放和统一重载。           |
| `src/style.css`                           | 比例、弹窗选择、历史多列/响应式、无应用外层预览、纯色背景与实色关闭按钮。             |
| `tests/settings.test.ts`                  | 默认绑定、规则/全局超时、草稿合并、两入口清空防重、四项筛选和异步复制跨页隔离。       |
| `tests/widget.test.ts`                    | 目录/模型选择、分组隔离、结算倒计时、四菜单保留同一 iframe，以及原交互回归。          |
| `tests/html-answer.test.ts`               | 共用识别、加载/尺寸/背景边界、放大等待、过期文档回收、可见历史与重载。                |
| `tests/fixtures/html-answers.json`        | 20 例虚构回答由 TypeScript/Rust 共同读取，不包含用户真实回答。                        |
| `scripts/package_exe.ps1`                 | 仅复制 EXE，核对版本、PE x64、输入时间与摘要，拒绝覆盖不同旧文件。                    |
| `vite.config.ts`                          | 开发监听排除原生/交付产物，避免 Windows 文件锁触发 EBUSY。                            |

## 结论与边界

前端 185 项、Rust 92 项（61 单元、17 数据完整性、14 请求回归）通过；最终静态检查、桌面实测和构建证据见 [VALIDATION.md](../VALIDATION.md)。测试仅使用本机 Mock、隔离 QA identifier 和虚构配置；用户提供的动画仅用于隔离预览，不提交进仓库。未发现尚未处理的阻塞问题，不作“没有任何缺陷”的保证。

分类清空的 SQLite 部分原子提交；Windows 凭据、自启与外观无法与 SQLite 共用事务，采用尽力补偿，补偿失败明确报错，已发送邮件不能撤回。普通历史“清空”保留通知依据，管理弹窗选择“测试历史”会一并清除，不能混为同一种语义。

iframe 使用浏览器软隔离并共享渲染资源，不能承诺恶意死循环的硬终止。未实测真实模型/邮件投递、全部物理 DPI/多屏、休眠唤醒与屏幕阅读器组合。Java 手册已阅读，本项目没有 Java/JVM/MyBatis；采用适用通用条款及 Vue/TypeScript/Rust 规范。

正式升级前退出旧版并备份完整数据目录。schema/备份 v3 不能由旧版直接读取，整体降级应恢复升级前备份；本次预览样式和加载修复没有额外 schema 升级。提交及发布范围遵循用户最终批准的文案，手工上传资产仅含单个 EXE。
