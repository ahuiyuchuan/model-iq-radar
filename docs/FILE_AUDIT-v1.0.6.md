# v1.0.6 逐文件复核

作者：ahui。日期：2026-10-28。基准：`edb2422`（v1.0.5），覆盖本轮全部 29 个修改/新增文本；保留并复核既有未提交改动，发布阶段仅同步版本、正式构建标记和必要说明。

| 文件                                    | 核心检查                                                                           |
| --------------------------------------- | ---------------------------------------------------------------------------------- |
| `CHANGELOG.md`                          | 空前导兼容、双协议原文、历史优化及实际验证边界。                                   |
| `CONTRIBUTING.md`                       | Vitest 的 Node 引擎范围、当前复核入口与既有构建规范。                              |
| `README.md`                             | v1.0.6 下载、响应诊断、历史分页和数据兼容；预览明确沿用 v1.0.4 演示图。            |
| `THIRD_PARTY_NOTICES.md`                | 既有工具离线生成，仅产品版本变化；生产依赖无变化。                                 |
| `VALIDATION.md`                         | 发布授权、实际命令、人工证据、未运行回归及二进制摘要。                             |
| `docs/USER_GUIDE.md`                    | 当前正式标记、日志目录/字段/留存、历史分页/导出和升级回滚。                        |
| `docs/FILE_AUDIT-v1.0.6.md`             | 当前全部变更索引，不收录真实样本或构建文件。                                       |
| `docs/release-notes/v1.0.6-zh.md`       | 用户可见变化、兼容边界、验证范围和下载升级。                                       |
| `package.json`                          | 版本 1.0.6，开发 Vitest 锁定 5.0.5，生产依赖和脚本保持。                           |
| `package-lock.json`                     | npm 工具同步项目版本和测试依赖，生产包条目无变化。                                 |
| `src-tauri/Cargo.toml`                  | 版本 1.0.6，依赖、Rust 工具链要求和 feature 保持。                                 |
| `src-tauri/Cargo.lock`                  | Cargo 离线更新本项目版本，不升级 crate。                                           |
| `src-tauri/tauri.conf.json`             | 版本 1.0.6，identifier、权限、CSP 及窗口配置保持。                                 |
| `src-tauri/src/api.rs`                  | 使用既有缓冲保存两协议失败原文、区分 JSON/SSE 诊断、正式标记；请求和回退参数保持。 |
| `src-tauri/src/chat_sse.rs`             | 严格 ID 校验保持，补失败事件与两侧 ID 定位及未执行断言。                           |
| `src-tauri/src/lib.rs`                  | 默认快照仅返回最近原文及集合版本，新增只读历史 IPC，扫描不持有应用存储锁。         |
| `src-tauri/src/model.rs`                | 快照新增历史版本字段，持久化和配置备份结构保持。                                   |
| `src-tauri/src/response_diagnostics.rs` | 格式 3、实际协议文件名、SSE 安全字段、按时间共同留存及正文写失败标记。             |
| `src-tauri/src/responses_sse.rs`        | 空结构白名单、消息关联、一次切换、终态和混流保护；补充未执行的合成边界用例。       |
| `src-tauri/src/storage.rs`              | 独立只读事务、摘要/原文分离、最多十条参数绑定查询、后台搜索与未执行快照用例。      |
| `src-tauri/tests/regression.rs`         | 同步正式构建标记、格式 3 与无正文诊断断言，未执行。                                |
| `src/bridge.ts`                         | 浏览器虚构内存预览遵循新增索引、正文页和搜索契约，不读取正式数据。                 |
| `src/components/HistoryPanel.vue`       | 十条分页、250ms 防抖、在途检索合并、世代/卸载隔离和错误刷新；详情使用完整原文。    |
| `src/components/MailSettings.vue`       | 提示准确覆盖保存后的总开关/事件、HTML 和取消例外，不改变通知规则。                 |
| `src/components/Settings.vue`           | 历史按版本加载索引，导出明确读原文且覆盖整个筛选集合，重复与迟到操作隔离。         |
| `src/style.css`                         | 分页布局自然换行，不遮挡筛选和既有操作。                                           |
| `src/types.ts`                          | 历史索引及快照版本契约，明确摘要不能用于原文导出。                                 |
| `src/useProbe.ts`                       | 默认快照范围说明同步，保留旧完整历史调用兼容。                                     |
| `tests/settings.test.ts`                | 更新 IPC Mock 并补分页/完整详情/迟到页面用例，仅类型检查，未运行。                 |

## 兼容与复核结果

遵循 Vue/TypeScript/Rust 的既有风格；Java/JVM/MyBatis 专属语法和工具不适用。数据库 schema 和备份均为 v3，不改配置选择、凭据、邮件规则或 HTML 展示，不添加模型请求、重试或自动换档。

解析器必须保留完整 EOF、2 MiB、唯一正式终态、共享期限及真实混流保护。诊断不改写原文、自动上传或影响结算；历史摘要不得用于复制/导出，异步结果不得跨筛选与页面回写。新版本仅将构建标记由诊断 Beta 更新为 `sse_empty_prelude`，旧日志不改写。

用户已授权提交、推送、Tag 和 Release，未授权恢复自动化回归；本轮仅静态检查、格式化和发布构建，不操控桌面、不请求真实模型/SMTP。具体结果和既有人工证据见 [VALIDATION.md](../VALIDATION.md)，不把编译或四次成功日志表述为全量测试通过。
