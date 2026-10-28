# 测智雷达 v1.0.6 本地复核记录

作者：ahui。日期：2026-10-28。环境：Windows x64，Vue 3 / TypeScript / Tauri 2 / Rust / SQLite。

## v1.0.6 正式发布复核（2026-10-28）

- 用户在逐文件复核后明确授权提交、推送、Tag 和 Release；沿用仍有效的测试限制，不运行前端/Rust 测试、Clippy、桌面/界面自动化或真实模型/SMTP。远端 main 与本地基准 `edb2422` 一致，已有 v1.0.5 按原样保留；本轮按现有补丁版本惯例发布 v1.0.6，涵盖全部 29 个修改/新增文本，见 [FILE_AUDIT-v1.0.6.md](docs/FILE_AUDIT-v1.0.6.md)。
- `npm version 1.0.6 --no-git-tag-version --ignore-scripts` 更新 npm 清单；Cargo 完整离线 Windows 元数据同步本项目锁文件版本，Tauri/Cargo 清单同为 1.0.6。生产 npm 包条目无变化，Cargo 锁文件仅本项目版本变化。`node scripts/generate_notices.mjs` 离线生成 37 个前端包/315 个 Windows crate 声明，逐字比较仅产品版本变化，没有手改第三方许可证。
- 生产代码相对已复核 Beta 仅将日志构建标记改为 `sse_empty_prelude`，对应断言同步但未执行；协议、思考强度、超时、回退、HTML、邮件及 schema/备份 v3 保持。同步 README 下载链接、使用手册、CHANGELOG、正式发布说明和逐文件索引；历史人工日志与 Beta 记录保留，不将其改写为正式版测试结果。
- `run-build.ps1 -Action build` 完成 `vue-tsc --noEmit`、Vite 及 Windows x64 `tauri build --no-bundle`，构建成功。前端为 531.72kB / gzip 190.50kB，只有既有 chunk 体积和 MSVC import library 提示，无编译错误；没有编译或执行新增 Rust 测试，不把生产编译表述为回归通过。
- `scripts/package_exe.ps1 -OutputDirectory release-artifacts/v1.0.6` 校验 PE x64、工程/文件/产品版本、应用输入时间及复制摘要。目录仅一份 `Model-IQ-Radar-v1.0.6-Windows-x64.exe`，16,852,992 字节，SHA-256 `727e1e4d583daee9e786abcb3e3894c739521a3f922e1179f1cb0056475e6b5e`。未启动交付程序，不覆盖旧正式版/Beta，不纳入用户日志或本机数据。
- 受影响 Prettier/rustfmt 和 `git diff --check` 已完成；最终 29 个文本的严格 UTF-8 无 BOM/LF/结尾换行、逐文件索引一致性、37 处本地路径链接及变更范围均已核对。发布通过现有 Git Credential Manager 凭据调用 GitHub API，不将凭据输出或写盘，先创建草稿、上传单个 EXE 并比对服务端 SHA-256，再公开正式 Release；不移动旧 Tag 或覆盖旧发布。
- 已有四组 Beta 人工成功日志及其局限见下文：不能证明每次触发空前导兼容、确认历史 Chat 根因或替代未执行的自动化回归。Java/JVM/MyBatis 专属条款不适用于本项目，沿用 Vue/TypeScript/Rust 规范。退出后更换 v1.0.5 EXE 可回滚，无新增数据迁移；已清理的数据不会恢复。

以下提交前复核、Beta 构建及旧版本通过记录均为历史记录，不替代本次验证范围。

## 提交前静态复核（2026-10-28，未执行测试）

- 逐项复核当前 23 个未提交文本，覆盖 SSE 兼容与混流保护、原始诊断/并发留存、历史摘要/分页/搜索/导出、邮箱提示、开发依赖及文档。未发现新的运行逻辑阻断问题；本次仅补充合成回归用例、必要注释和说明，未改变 Beta 的生产逻辑、版本、数据库或配置备份格式。
- 校准 Chat EOF 诊断序号的说明：它沿用最后完整事件序号，不保证为 0；未定位类型/ID 仍为空。贡献指南同步 Vitest 5.0.5 的 Node 引擎范围，修正使用手册的中英文间距。
- 新增 Responses 空前导正常切换、非空/推理/工具/拒绝/未知结构、关联索引、序号断裂、再次切换及矛盾终态的合成用例，补充两侧 ID 诊断断言。新增只读文件库快照与清空并发、摘要/原文分离、分页上限与失效记录、关键词检索，以及前端分页/完整详情和迟到页面隔离用例；这些 Rust/前端测试均未执行，Rust 新用例也未编译，不作通过声明。
- 只读核对用户提供的四份新 Beta/Astra 日志：Responses low/medium 分别耗时 147,507 / 135,389ms，Chat low/medium 分别 153,566 / 127,473ms。均为 `sse_empty_prelude_diagnostic_beta`、流式请求、HTTP 200、attempt_index=1、success、完整无截断。本次没有代用户调用模型；成功日志未留正文或切换计数，不能证明兼容分支实际触发，也不能据此确认历史 Chat 故障根因或保证所有中转稳定。
- 本次执行 `npm run build`（`vue-tsc --noEmit`、Vite）成功，产物仍为 531.72kB / gzip 190.50kB，保留既有体积提示；对比 HEAD 与当前锁文件确认生产依赖条目无变化，当前 Node v24.18.0。全部受影响文件的 Prettier/rustfmt 检查、23 个文本的严格 UTF-8/无 BOM/LF/结尾换行核对及 `git diff --check` 均完成；未运行前端/Rust 测试、Clippy、界面/桌面自动化或真实模型/SMTP。
- 本次未重建或重新交付 EXE：此前 Windows x64 编译与 Beta 摘要见下一节，后补内容不改变生产逻辑；旧 EXE 的编译成功不能代表新增回归已通过。不提交、不推送、不修改 Tag/Release。

## 未发布 SSE 兼容诊断 Beta（2026-10-28）

- 本轮从干净的 `edb2422` 工作区开始，工程版本保持 1.0.5；用户明确只授权静态检查、格式化、出包必需类型检查与编译。未执行前端/Rust 测试、Clippy、桌面/界面自动化、真实模型或 SMTP 验证；本文其他章节的通过记录不代表本 Beta 已通过回归。
- 只读核对两份用户 Responses 原文及元数据，长度分别 1,393,749 / 1,495,798 字节，HTTP 200、完整读取、未超限、原错误 invalid_event_stream；均在前五个空事件后以新 created 切换 ID。未把样本或其中服务商正文复制到仓库。只读核对 sub2api 副本提交 `f178e5ae32659e8901644a9f2378a0439ffa36ae` 的 created 转换；Chat 线上部署与失败原文未知，本版只补诊断，不据此放宽 Chat ID 校验。
- 静态逐项核对空前导白名单、消息/内容索引、一次切换和真实混流保护；两协议原文使用现有缓冲、2 MiB 上限，SSE 结构信息独立于 JSON 位置。ID/类型/错误文本经过凭据保护，正文不可改写，原文/transport 分别有界，UUID/create_new 防并发覆盖，双协议原文按时间共同清理；故障不改变 Completion 或历史结算。
- 历史列表独立只读事务在后台扫描摘要，常规快照仅附集合版本；每页最多十条完整正文、250ms 搜索防抖并合并过期检索，离页/切换隔离迟到结果。导出明确读取完整原文，不受当前页限制；复制/详情均使用完整记录。已有 Mock/断言按新 IPC 与 Beta 标记同步，但未运行；新兼容和分页行为等待用户人工测试及后续明确授权的回归。
- Vitest 由 3.2.7 升至锁定 5.0.5，当前 Node 24.18.0 满足其引擎范围；安装使用 `npm install --save-dev --save-exact vitest@5.0.5 --registry=https://registry.npmjs.org --ignore-scripts`。`npm audit --registry=https://registry.npmjs.org` 报告 0 漏洞；对比锁文件确认生产依赖条目无变化，生产第三方声明无需变更。未用强制 audit fix 或升级生产依赖。
- 已执行受影响文件 Prettier、rustfmt、`git diff --check` 和 TypeScript/Vite 生产编译。Windows 使用既有隔离工具链脚本的 `-Action build` 执行 `tauri build --no-bundle`，最终构建成功；保留既有 chunk 体积和 MSVC import library 提示。22 个修改文本严格 UTF-8/无 BOM/LF，无替换字符；编译成功不代表测试通过。
- `scripts/package_exe.ps1` 校验 PE x64、版本、输入时间及复制摘要后，在全新目录重命名为 `release-artifacts/v1.0.5-sse-compat-diagnostic-beta1-20261028-111249/Model-IQ-Radar-v1.0.5-SSE-Compat-Diagnostic-Beta.1-Windows-x64.exe`。目录仅一份 EXE，文件/产品版本 1.0.5，16,852,992 字节，SHA-256 `2ec71d7e86bcc7209069ecdff8ffedb1ec4458443b3095f5dac48de0b169d5e1`；未启动应用，不覆盖此前交付物，不包含日志、安装包或文档包。
- README、使用说明、变更日志、邮箱条件提示与业务注释同步；工程为 Vue/TypeScript/Rust，Java/MyBatis 专属规范不适用，遵循项目语言规范。schema/配置备份 v3 不变，退出 Beta 后可换回 v1.0.5 正式 EXE；已清理日志无法恢复。连接测试取消、HTML 隔离及新增功能不在本轮修改范围。不提交、不推送、不修改 Tag/Release。

## v1.0.5 正式发布复核（2026-10-28）

- 用户明确授权全面复核、测试、提交/推送并发布 Tag/Release，覆盖此前仅本地 Beta 的限制；继续禁止桌面操控，不调用真实模型或 SMTP。本轮相对 `ba4a0aa` 核对全部 22 个修改/新增文本，见 [FILE_AUDIT-v1.0.5.md](docs/FILE_AUDIT-v1.0.5.md)，保护原有诊断、流式和卡片改动，无重置或无关重构。
- `npm test`：191 项通过，9 个测试文件、0 失败。保留并核对条件不匹配时的历史文字/HTML、耗时/复制、旧协议与重测提示、同一 iframe 会话及切回原条件恢复，不沿用旧绿色判定。
- `run-build.ps1 -Action test`（`cargo test --locked --manifest-path src-tauri/Cargo.toml`）：126 项通过，0 失败（87 单元、17 数据完整性、22 请求回归）。新增 Chat SSE 的原文空白、推理排除、用量、BOM/换行/多行 data、唯一响应/候选、角色/工具/拒绝、缺失/重复结束及非正常结束；两种协议的实际本机 HTTP 路径均核对 stream:true、一次请求、完整 EOF、中断、超限、取消和共享期限。
- 新增传输诊断回归核对成功/失败日志、同 trace_id 回退顺序及剩余预算、等待响应头/首块时间、无响应头的空字段、网络库错误链采集入口、应用总期限与取消区分、凭据/URL 脱敏和长度、二十份留存、四个并发写唯一性、无关文件保留、队列满/磁盘故障后仍能继续写入。只用随机临时目录；与原正文诊断共用队列的用例通过仅测试互斥避免自身抢占，生产仍非阻塞丢弃。原始响应字节保真与失败隔离回归继续通过。
- `run-build.ps1 -Action clippy`（`cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`）通过；`npm run format:check`、`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` 通过。受影响文件格式化、UTF-8 无 BOM/LF、链接与最终差异检查作为交付收尾单独核对。
- `npm version 1.0.5 --no-git-tag-version --ignore-scripts`、Cargo 离线 Windows 目标元数据更新本项目锁文件版本；`node scripts/generate_notices.mjs` 离线生成 37 个前端包/315 个 Windows crate 声明。清单/锁文件/声明仅产品版本变化，无依赖升级；同步 README、使用手册、变更日志、业务注释及正式发布说明，删除当前使用流程中的 Beta 临时指导。Java 手册已查阅，Java/JVM/MyBatis 专属语法和工具不适用于本项目。
- `run-build.ps1 -Action build` 完成 `vue-tsc --noEmit`、Vite 与 Windows x64 `tauri build --no-bundle`；只有既有 chunk 体积提示（526.87 kB，gzip 188.67 kB）和 MSVC 导入库 stdout 提示，无编译错误。`scripts/package_exe.ps1 -OutputDirectory release-artifacts/v1.0.5` 校验 PE x64、工程/文件/产品版本、应用输入时间和复制摘要；目录仅一个 `Model-IQ-Radar-v1.0.5-Windows-x64.exe`，16,693,760 字节，SHA-256 `2456a2dfd4e423acd9a49070a8a1242f5705afdbfee5b5539f28f68b4927fc0a`。未启动交付程序，原 Beta 文件保留。
- 最终 22 个文本均通过严格 UTF-8 无 BOM、LF、结尾换行及无替换字符检查；本地文档路径/锚点、版本清单、逐文件索引及常见凭据模式扫描正常，`git diff --check` 无错误。构建产物、用户日志、临时发布脚本均未纳入 Git；第三方声明只变更产品版本，未手改依赖生成内容。
- 流式请求与严格成功条件同时生效，不追加隐式非流式请求、不修改回退开关、超时或降智规则。正式构建标记改为 `dual_protocol_stream`，历史 Beta 日志原样保留。数据库 schema v3/备份 v3 不变，无迁移；退出后更换 v1.0.4 EXE 可回滚请求/展示逻辑，清理过的日志和历史不可恢复。
- 已有用户取证确认两种协议分别流式成功；旧非流式的 TLS EOF 均发生于等待响应头阶段，而非 600 秒应用期限。具体中转断链层级与所有网络环境的稳定性仍未知，不能将模拟回归或用户单次成功写成永不再断连。HTTP 403 按用户指定不在本轮排查范围；真实外部服务、物理 DPI/多屏和最终原生交互不追加验证。

以下 Beta 约束、构建与检查数字是历史记录，不替代本次正式发布检查。

## 双协议流式再次复核（2026-10-28，静态检查及人工日志核验）

- 按用户要求重新审阅全部十三个修改/新增文本，包含 `chat_sse.rs`、请求/诊断/连接适配、卡片与快照、回归源码及说明。未重置、覆盖已有改动，本次只补充此复核记录，未改变应用源码或重新生成 EXE；没有发现当前修复中的阻断问题。
- 再次读取此前三份失败和两份成功的原始 transport JSON。旧非流式构建的 Responses 在 132,308 / 165,570 毫秒、Chat Completions 在 149,967 毫秒等待响应头时遇到 TLS `UnexpectedEof`，HTTP/首块时间均未取得，不是 600 秒应用期限。流式 Responses 在 3,450 毫秒取得响应头、187,862 毫秒完整成功；本轮双协议 Beta 的 Chat Completions 在 1,669 毫秒取得响应头及首块、183,098 毫秒完整成功，HTTP 200 / text/event-stream、960,864 字节、无截断。Responses 成功来自上一轮单协议对照包，其解析路径在本轮保持；不能把这些记录写成同一构建的全量回归，也不能由客户端日志定位中转内部的具体断链节点或证明固定静默超时。
- 源码确认两协议均发送流式，实际值写入诊断；完整 JSON 兼容继续保留，SSE 按协议分别验证，Chat 检查同一标识/候选、结束原因及 [DONE]，Responses 检查完整最终响应。推理不混入答案；中断、超限、超时和取消不会凭部分正文判成功。关闭协议回退只尝试一次，开启仍最多另一协议一次、共享期限，无隐式非流式重发。
- 展示确认按完整条件优先查找；条件不匹配仅撤销当前有效结论，卡片保留同分组历史回答/HTML、时间、耗时及原文复制，注明上次协议和待重测；不会跨分组取记录或因查看触发请求、历史重判及邮件。截图仅能确认当次 HTML 展示，未执行协议切换的桌面自动化验收。诊断写入继续脱敏、限量并隔离失败，数据库及配置格式不变。
- 实际命令：受影响 Rust 的 `rustfmt --edition 2021 --config skip_children=true --check`、Vue/TypeScript/Markdown 的 `prettier --check`、`git diff --check` 均完成；十三个文本严格 UTF-8 无 BOM/LF、无替换字符。只读 SHA-256 与源码时间检查确认此前交付的双协议 Beta 与构建文件一致，版本 1.0.4、16,693,760 字节，摘要仍为 `fc242ae924abe6ceab1d50ec483b934d99c302c0d42473ff85daed6451276948`，应用源码没有晚于该 EXE 的修改。
- 遵守此前限制：本轮不运行前端/Rust 回归、Clippy、界面自动化、真实模型或 SMTP，不启动应用，不提交、推送或发布。现有新增回归用例仍未执行；静态检查、此前编译及用户成功取证不能表述为全量测试通过或所有服务商永久不再断连。

## 双协议流式及协议切换展示 Beta（2026-10-27 出包，次日已收到 Chat 成功日志）

- 用户已提供上轮真实日志：Responses 请求流式，在 3,450 毫秒收到响应头、3,451 毫秒收到首个正文块，187,862 毫秒完整完成，HTTP 200 / text/event-stream，1,246,850 字节、无截断。本轮据此覆盖两种协议，不能由一次成功断言中转长期稳定或已定位内部断链节点。
- `api.rs` 两种协议直接发送 `stream: true`，移除上轮临时 Cargo feature；新增 `chat_sse.rs`，按 Chat 专属分块/结束契约还原正文后进入既有判定。Responses 原终态解析、JSON 兼容、EOF/2 MiB/总期限/取消及原回退开关保持；连接测试复用同一请求方式，不新增请求或界面选项。传输格式仍为 2，构建标记为 `dual_protocol_stream_beta`，原 Responses 原文留存范围不扩展。
- 已查明展示根因：后端返回分组历史兜底记录，前端 latest 只允许精确条件匹配；Widget 错将 latest 同时用于回答和耗时。现改为 latest 决定当前状态、latestRecord 展示/复制历史，协议变化保留 HTML 会话与真实耗时，标注旧协议和待重测；不弱化指纹、不重算历史/邮件。源码同步现有断言并补 Chat 合成回归，均未运行。
- README、使用手册、业务注释及变更日志同步。本轮仅必要代码检查、格式化和出包类型检查/Windows 编译，不运行前端/Rust 测试、Clippy、桌面/浏览器自动化、真实模型/SMTP 或交付 EXE；不提交、推送或发布 Tag/Release，保留原改动及旧包。Rust/Vue 沿用项目规范，Java/JVM/MyBatis 专属工具和语法不适用；无数据库、备份或工程版本迁移。
- 实际完成受影响 Rust 的 `rustfmt --edition 2021 --config skip_children=true`、Vue/TypeScript/Markdown 的 Prettier、`git diff --check` 及十三个修改/新增文本的 UTF-8 无 BOM/LF/替换字符核对。`run-build.ps1 -Action build` 完成 `vue-tsc --noEmit`、Vite、Windows x64 `tauri build --no-bundle`；收尾同步注释和用例状态后重新编译最终文件，只有既有 Vite chunk 体积与 MSVC 导入库 stdout 提示，没有编译错误。不把格式、类型检查或编译表述为测试通过，新增/调整用例均待用户允许后运行。
- `package_exe.ps1` 核对 PE x64、工程/文件/产品版本、应用输入时间及复制摘要；新目录仅一个 `release-artifacts/v1.0.4-dual-protocol-stream-beta1/Model-IQ-Radar-v1.0.4-Dual-Protocol-Stream-Beta.1-Windows-x64.exe`，版本 1.0.4，16,693,760 字节，SHA-256 `fc242ae924abe6ceab1d50ec483b934d99c302c0d42473ff85daed6451276948`。此前传输诊断及 Responses 流式 Beta 的摘要未改变；未启动本轮 EXE，Chat 实际流式结果和卡片切换交互待人工验收。

## Responses 流式对照 Beta（2026-10-27，历史出包记录，已收到成功日志）

- 已读取用户新日志：关闭回退的单次 Responses 在 165,570 毫秒发生 TLS `UnexpectedEof`，未收到响应头或正文，未触发 600 秒应用期限。此前两协议尝试也出现同类错误；这些证据不能定位断链节点或证明固定时限。原 Beta 仍为非流式，本轮增加主动流式对照，不宣称根因已经修复。
- `Cargo.toml` 新增默认关闭的 `responses-stream-beta` feature；`api.rs` 在带此 feature 的构建中仅对 Responses 发送 `stream: true`，日志复用同一值。正式手动/自动测试与复用适配器的连接测试适用，不改持久配置；连接测试仍不采集日志。`response_diagnostics.rs` 的传输格式 2 增加 `request_stream`/`build_variant`，旧文件不改写，原 Responses 原文诊断格式不变。
- 沿用既有 JSON/SSE 解析、完整 EOF、唯一 `response.completed`、最终助手正文、2 MiB、取消和共享总期限；不追加请求、不开启回退、不关闭 TLS 校验、不改降智规则。已用 OpenAI 官方流式文档核对 `stream: true` 和结束事件；不能据此保证第三方中转遵循同样行为。README、使用手册和未发布日志同步；已有 SSE 请求回归只同步 feature 断言，未执行。
- 本轮仅必要源码复核、受影响文件格式化及出包必需的类型检查和 Windows 编译；不运行前端/Rust 测试、Clippy、桌面/浏览器自动化、真实模型/SMTP 或交付程序。不提交、推送或变更 Tag/Release；保留既有未提交改动及旧 EXE。Java/JVM/MyBatis 专属条款不适用于本项目 Rust/Vue 技术栈。
- 实际执行受影响 Rust 文件的 `rustfmt --edition 2021 --config skip_children=true`、四份 Markdown 的 Prettier、`git diff --check` 和八个修改文件的 UTF-8 无 BOM/LF/替换字符核对。`npm run tauri -- build --no-bundle --features responses-stream-beta` 完成 `vue-tsc --noEmit`、Vite 和 Windows x64 release 编译；仅保留既有 chunk 体积与 MSVC 导入库 stdout 提示，无编译错误。Cargo 最新构建指纹包含 `responses-stream-beta`，编译成功不代表真实服务断连已修复或测试通过。
- `package_exe.ps1` 校验版本、PE x64、应用输入时间与复制摘要，随后在全新目录重命名；目录仅一份 `release-artifacts/v1.0.4-responses-stream-beta1/Model-IQ-Radar-v1.0.4-Responses-Stream-Beta.1-Windows-x64.exe`，工程/文件/产品版本 1.0.4，16,679,424 字节，SHA-256 `c03aa9e643b5318770f371418e91262a7ad3a5d1322da92ca54c87b171f3fdb6`。未启动交付程序，原非流式诊断 Beta 保留；待用户关闭回退、手动请求并提供格式 2 日志。

## 本地传输诊断 Beta（2026-10-27，已收到人工取证）

- 本轮用户授权仅补本地诊断并快速提供单个 Windows x64 Beta。最新限制覆盖历史测试授权：不运行前端/Rust 测试、Clippy、桌面或浏览器自动化、真实模型/SMTP 请求；只做必要代码检查、受影响文件格式化及出包类型检查/编译。不提交、推送、修改 Tag/Release，不覆盖之前 EXE。
- `api.rs` 为两种协议的每次正式请求记录发送/等待响应头、收到响应头、读取正文与解析阶段，记录实际时间线、完整性和 reqwest 错误链；应用总期限与网络库超时区分，回退逐次关联，原提示、判定、请求参数和重试边界保持。`response_diagnostics.rs` 新增独立有界 transport 子目录，失败前没有响应头也能保存纯元数据；成功保存对照，不保存请求体或回答，不额外请求，不伪造状态码或正文。
- 同步 README、使用手册及未发布变更说明；旧 Responses 原始诊断格式/留存、数据库与配置备份保持。现有请求回归仅调整新子目录计数和后台完成等待，未执行这些用例，不宣称测试通过。Java/JVM/MyBatis 专属语法和工具不适用于 Rust 模块，日志脱敏和异常隔离沿用项目规范。
- 实际执行受影响 Rust 文件 `rustfmt --edition 2021 --config skip_children=true`、四份 Markdown 的 Prettier、`git diff --check` 及 UTF-8 无 BOM/LF、README/使用手册本地链接和锚点检查；均通过。`run-build.ps1 -Action build` 完成 `vue-tsc --noEmit`、Vite 和 Windows x64 `tauri build --no-bundle`；仅保留既有 chunk 体积与 MSVC 导入库 stdout 提示，没有编译错误。未运行任何测试或交付程序，编译不等于故障已修复或诊断已人工验证。
- `package_exe.ps1` 校验 PE x64、工程/文件/产品版本及应用输入时间，复制后核对 SHA-256，并在新目录按诊断 Beta 命名。唯一交付物为 `release-artifacts/v1.0.4-transport-diagnostic-beta1/Model-IQ-Radar-v1.0.4-Transport-Diagnostic-Beta.1-Windows-x64.exe`，版本 1.0.4，16,678,912 字节，SHA-256 `84c3296a87484a4e2d108d7c1291f73ea1efe11211826aa875136e851d6fb3e5`；此前 EXE 保留，没有 ZIP、安装包或附带文件。后续已收到人工日志，传输证据及下一步对照见上方记录；仍未确认中转内部断链原因。

## v1.0.4 发布前全面复核（2026-10-27）

- 用户已确认 SSE 修复不再报错，并授权复核全部未提交改动、提交、推送、Tag 与 Release；相对基准 `34d3c52` 核对全部 27 个修改/新增文本，索引见 [FILE_AUDIT-v1.0.4.md](docs/FILE_AUDIT-v1.0.4.md)。未重置或覆盖原有预览、历史和诊断改动。以下 Beta 的“暂不测试/不发布”仅记录当时约束，本轮以此次明确授权为准；用户禁止桌面操控的要求继续遵守。
- `npm test -- --reporter=json --outputFile=<仓库外报告>`：9 个文件、191 项通过，0 失败。覆盖实际内容测量/回退、非法坐标与放大一致、HTML 切回文字恢复原回答/状态/时间/耗时、分组及旧指纹隔离，并沿用 300ms 加载、生命周期和现有交互回归。关于页版本断言改为读取工程版本。
- `run-build.ps1 test`（`cargo test --locked --manifest-path src-tauri/Cargo.toml`）：115 项通过，0 失败（79 单元、17 数据完整性、19 请求回归）。新增合成 SSE 的终态、换行/BOM/多行 data、损坏位置、矛盾 ID/事件、失败/重复终态及元数据尾部；本机 HTTP 验证 SSE 一次请求/stream:false、错误 MIME 下 JSON 优先、Chat 不套用 SSE，以及中断、超限、取消、超时不误判成功。
- 诊断回归验证原始字节/CRLF/非 UTF-8 保真、语法行列和事件范围、完整与超限标志、二十组及孤立文件留存、无关文件保留、并发唯一文件、已知密钥省略、磁盘故障/队列满隔离与后续请求正常。首轮两个用例因测试自身同时占用全局信号量而失败，增加仅测试用的互斥后重跑全套通过；不修改生产队列的非阻塞语义。复核另收紧清理匹配为完整 UTC 时间戳/UUID，避免碰到无关文件。
- `run-build.ps1 clippy`：全目标 `-D warnings` 通过；`npm run format:check`、`npm exec -- prettier --check docs`、`cargo-fmt fmt --manifest-path src-tauri/Cargo.toml -- --check`、`git diff --check` 通过。仓库外只读核对脚本确认 27 文件严格 UTF-8/无 BOM/LF/无替换字符、24 个相对 Markdown 目标存在、六处工程版本一致、逐文件索引无遗漏，明显令牌格式扫描无命中。
- `npm version 1.0.4 --no-git-tag-version --ignore-scripts`、Cargo Windows 目标离线元数据更新本项目锁文件版本；`node scripts/generate_notices.mjs` 离线生成 37 个前端包/315 个 Windows crate 声明。锁文件及第三方声明与基准相比仅本项目版本标识变化，无依赖升级。README、使用手册、变更日志、发布说明和相关注释已同步；Java/JVM/MyBatis 专属工具及语法不适用于 Vue/TypeScript/Rust。
- `run-build.ps1 build` 完成 `vue-tsc --noEmit`、Vite 和 Windows x64 `tauri build --no-bundle`；生产构建成功，保留既有 Vite chunk 体积提示（526.47 kB，gzip 188.54 kB）及 MSVC 导入库 stdout 提示，无构建错误。`scripts/package_exe.ps1 -OutputDirectory release-artifacts/v1.0.4` 校验 PE x64、工程/文件/产品版本、应用输入时间与复制摘要；目录仅一个 `Model-IQ-Radar-v1.0.4-Windows-x64.exe`，16,644,096 字节，SHA-256 `25b510cfea4695d222f13797403c9d8997613e1724483cae3645b4df90c198e2`，此前 Beta 均保留。
- 数据库 schema/备份均保持 v3，诊断格式为 2，旧格式 1 文件不迁移或重写；当前协议及回退设置保持。回退到 v1.0.3 不需数据迁移，诊断不参与结果恢复；历史或诊断已经清理的内容不可恢复。诊断只是有界后台取证，队列满或程序退出可能缺文件，不自动重试或补发。
- 本轮不操控桌面、不启动正式程序，不访问真实模型/SMTP；只用 jsdom 组件、临时数据库及本机随机端口模拟 HTTP。用户确认“不再报错”不等同于全部真实服务均通过。物理 DPI/多屏、真实邮件投递和服务商差异未遍历；HTML 首帧测量和共享 iframe 资源边界保持。未发现尚未处理的阻塞问题，编译和模拟回归不作真实服务兼容性保证。

## Responses SSE 兼容 Beta（2026-10-27，历史出包记录，用户已确认不再报错）

- 用户提供的诊断证明该次 HTTP 200 实际为 `text/event-stream`，正文 23,356 字节、已读到 EOF、未截断，包含明确 `response.completed` 与完整最终助手文本“21颗”；旧客户端将保活注释开头的整段 SSE 当作 JSON 导致 `invalid_json`。不能据此确定是哪层忽略 `stream:false`，也不能替代此前其他请求的原始证据。
- 新增 `responses_sse.rs`；普通 JSON 保留优先路径，Responses 才增加 MIME/正文前缀识别和 SSE 事件解析。只传完整结束事件中的原 response 给既有 `parse_response`，不拼 delta、不执行推理/中间说明、不修改判定规则；读取 EOF、2 MiB、取消/超时和用户回退开关保持。缺终态、失败、损坏、重复/矛盾事件均报错，无额外请求或重连。
- `response_diagnostics.rs` 与 `api.rs` 将异常 SSE 原文纳入原诊断留存；格式 2 增加 JSON 错误范围及事件序号，原文件不修改，留存/凭据过滤/失败隔离不变。使用手册和未发布日志同步，数据库、配置备份及工程版本仍沿用现有值；撤销本轮解析入口/模块即可回退，不需要数据迁移。
- 遵循最新用户限制：本轮仅必要代码检查、受影响文件格式化及出包编译，不运行前端/Rust 测试、Clippy、样例执行、界面自动化或真实模型/SMTP 验证；不使用桌面操控。尚未运行实际 SSE 解析回归，编译成功不能当作测试通过，等待用户用关闭回退的实际请求人工复测。保护此前全部未提交改动，不提交、推送或发布；Java 专属工具/语法条款不适用于本项目 Rust 模块。
- `run-build.ps1 build` 完成 TypeScript/Vite 和 Windows x64 `tauri build --no-bundle`；保留既有 chunk 大小与 MSVC 导入库提示，无编译错误。本轮 7 个文本确认 UTF-8 无 BOM/LF，必要差异检查完成。`package_exe.ps1` 核对版本、PE x64、输入时间及复制摘要，交付目录仅含 `release-artifacts/v1.0.3-responses-sse-beta1/Model-IQ-Radar-v1.0.3-Responses-SSE-Beta.1-Windows-x64.exe`；工程/文件版本 1.0.3，16,616,448 字节，SHA-256 `5af9c3459c3ed807939404b4aba5c568834ef26bc2ccd0fc8f835fae0fd1aa6c`。未启动交付程序，不覆盖此前诊断 Beta。

## Responses 原始响应诊断 Beta（2026-10-27，历史出包记录，已收到人工取证）

- 用户最新要求覆盖此前测试授权。本轮不运行前端/Rust 测试、全量测试、Clippy、界面自动化或真实模型/SMTP 验证；仅做代码检查、受影响文件格式化和出包必需的类型检查/编译。保留全部既有未提交预览/历史改动，不提交、推送或修改 Tag/Release。
- 已核对 `api.rs`：两种协议仍发送 `stream: false`，Responses 当前按完整 JSON 解析；JSON 失败原先丢弃原始正文。后台“流式”标记不能确认实际响应类型，此 Beta 不修复猜测中的根因、不实现 SSE、不放宽成功条件、不切换用户协议或回退开关。
- 新增 `response_diagnostics.rs`；`api.rs` 在原读取缓冲上采集已收到字节，`lib.rs` 仅为正式测试传入应用数据目录。语法失败及 2xx 正文未完整读取的异常尽力后台落盘；最多 2 MiB/组、20 组、四个在途写任务，失败不改变测试结果。时间/协议/HTTP/Content-Type/请求编号/字节数/JSON 行列与原始正文分开保存；不复制请求对象或凭据，正文命中已知请求密钥时明确省略，不静默改写原文。schema、备份与工程版本保持原样。
- 准确目录、命名、完整性字段及取证步骤见使用手册“Responses 诊断 Beta”；新目录属于本地运行数据，不打入 EXE。尚未人工复现或验证文件写入，不把编译成功写成测试通过；收到实际响应文件后再区分证据与猜测，未经明确允许不开始后续测试。
- `run-build.ps1 build` 完成 `vue-tsc --noEmit`、Vite 与 Windows x64 `tauri build --no-bundle`；仅有既有 chunk 大小及 MSVC 导入库提示。受影响 Rust/Markdown 已格式化，必要差异与 UTF-8 无 BOM/LF 文件核对完成；未运行任何测试或启动交付程序。单 EXE 位于 `release-artifacts/v1.0.3-responses-diagnostic-beta1/Model-IQ-Radar-v1.0.3-Responses-Diagnostic-Beta.1-Windows-x64.exe`，工程/文件版本 1.0.3，16,604,672 字节，SHA-256 `bbbaa036538344215f19c0084bc390b7d754dadacca27efbadf00ac26606b9ce`；独立新目录未覆盖以前交付。

## 发布后本地 Beta：预览大小与切换历史（2026-10-27，历史出包记录）

- 本轮仅本地修改，不覆盖正式提交、Tag 或 Release。三处 HTML 共用可见正文范围的等比缩放，保留源页面阴影余量，无法测量时回退完整画布；历史列表预览增高到 120px，最少 300ms 加载反馈保持。
- 悬浮快照优先查询同一接口分组、完整条件指纹相同的最近保留记录，前端完整快照采用相同优先级。无匹配回退分组最近时间并提示重测；失败/取消不被跳过，清理或缺指纹的旧历史不恢复结论。查询不写历史或通知状态，不新增请求；schema v3、备份格式及凭据不变，回滚本轮源码即可撤销这两项改动。
- 用户要求停止测试之前，已运行前端 191 项及 Rust 95 项（64 单元、17 数据完整性、14 请求回归），均通过；新增内容覆盖留白测量及范围校验、HTML 切回文字、恢复原时间/耗时、分组隔离、失败/取消、保留清理及重启。类型检查发现测试 Mock 的 `this` 缺少显式类型，已补齐并重新编译；这些数字不表示后续 Beta 已完成人工验收。
- 使用独立虚构数据与用户提供 HTML 曾查看卡片和放大效果；Computer Use 被用户物理 Escape 中止，随后用户明确要求今后不使用桌面操控测试，并要求 Beta 先人工验收。本轮未继续桌面操作，未验证历史列表/详情和规则切换的最终桌面效果，不将组件回归当作人工验收；隔离程序已退出。
- 按最新要求暂停后续测试及全面复核，只进行格式整理、出包必需的 TypeScript/Vite/Windows 编译及单 EXE 文件核对。不执行新一轮自动化测试、Clippy、真实模型或 SMTP；后续测试等待用户明确允许，且不使用桌面操控。README、使用手册与未发布变更日志已同步；Java/JVM/MyBatis 专属规范不适用于本项目。
- `run-build.ps1 build` 已完成 TypeScript/Vite 和 Windows x64 生产编译，仅有既有 chunk 大小及 MSVC 导入库提示。`package_exe.ps1` 核对 PE、版本、应用输入时间和复制摘要；交付目录仅一个 `release-artifacts/v1.0.3-beta5-content-history/Model-IQ-Radar-v1.0.3-beta.5-Windows-x64.exe`，文件/产品版本 1.0.3，16,539,648 字节，SHA-256 `54ecfc2e917e1ae703a95208c4951b08395e9df7a37c560e17ff7f6925f7b1ed`。未启动交付 EXE，等待用户人工预览。

## v1.0.3 发布追加修复：清空与快捷键移除（2026-10-27）

- 用户反馈清空失败，错误为 `Ctrl+Alt+P` 被占用。已确认根因：清空“悬浮设置”使用默认偏好，原默认会重新注册该组合键，注册失败导致清空事务中断。移除前端设置、偏好字段、全局插件、注册/注销与启动回退逻辑，显示窗口保留托盘及重复启动入口。
- 旧配置和 v1/v2/v3 备份中的 `shortcutEnabled` 兼容忽略；后续保存、重置及导出不再写出。其余偏好、schema v3 和备份 v3 保持，Escape/日期选择/复制等页面键盘操作保留；无需删除或重建正式数据库。
- `npm test -- --reporter=json --outputFile=<仓库外报告>`：9 个文件、185 项通过，0 失败。新增两个管理入口单独重置悬浮设置的成功回归，并核对已移除快捷键选项和原防重逻辑。
- `run-build.ps1 test`：92 项通过，0 失败（61 单元、17 数据完整性、14 请求回归）。新增旧快捷键 true/false、单独重置/全选重置、落盘后重启，以及旧备份字段忽略并保留其他偏好的回归；沿用全部 32 分类组合及中途失败回滚测试。
- `cargo metadata --offline --filter-platform x86_64-pc-windows-msvc` 生成锁文件，移除快捷键专用依赖且不升级剩余依赖；初次全平台离线元数据因未缓存 Android crate 失败，改用实际 Windows 目标后成功。`node scripts/generate_notices.mjs` 重新生成 37 个前端包/315 个 Windows crate 声明，不手改第三方原文。
- `run-build.ps1 clippy` 全目标 `-D warnings`、`npm run format:check`、`npm exec -- prettier --check docs`、rustfmt 和 `git diff --check` 均通过。`run-build.ps1 build` 完成 TypeScript/Vite 与 Windows x64 生产构建；保留既有 chunk 体积提示（524.57 kB，gzip 187.96 kB）及 MSVC 导入库 stdout 提示，无构建错误。
- `scripts/package_exe.ps1 -OutputDirectory release-artifacts/v1.0.3-reset-fix` 检查版本、PE x64、应用输入时间及复制摘要；文件/产品版本 1.0.3，16,536,064 字节，生成时间 2026-10-27 12:42:52（本机时间），SHA-256 `66ccf19e0d3f825d31e932f52d2e6c021d01ddf49c9a83995d6ac51a80b2e95a`。目录仅一个 EXE，正式二进制无 QA identifier；此前 16,701,440 字节的 EXE 已被替代。
- 首次 v1.0.3 发布已完成后收到追加修复，随即转为草稿；用户已明确授权修复后覆盖原提交/Tag，提交消息保持此前批准的原文。发布前重新生成唯一 EXE，不复用下方旧构建；未继续使用已被 Escape 中止的桌面控制，也未访问正式数据库或凭据。

## v1.0.3 首次发布前全面复核（2026-10-27，追加修复前记录）

- 用户已完成人工 Beta 预览并授权测试；本轮相对基准 `23848a0` 检查全部 47 个修改/新增文本。复核阶段未暂存、提交、推送或创建 Tag/Release；用户审阅通过后已另行授权提交与正式发布。以下 Beta 与辅助进程数字是历史证据，不能替代当前结果。
- `npm test -- --reporter=json --outputFile=<仓库外报告>`：9 个文件、183 项通过，0 失败，日志无 Vue/测试警告。覆盖 20 个前后端共用 HTML 识别样例，以及预览加载/尺寸/背景/迟到隔离、四项筛选、菜单保留 iframe、清空防重和复制跨页反馈。
- `run-build.ps1 test`（`cargo test --locked --manifest-path src-tauri/Cargo.toml`）：90 项通过，0 失败，包括 60 单元、16 数据完整性、14 请求回归。新增全部 32 种分类选择边界、事务中途失败回滚、清空通知依据持久化，以及内置规则迁移立即落盘/写失败保留/删除后重启不复建。
- `run-build.ps1 clippy`：全目标 `-D warnings` 通过。`npm run format:check`、`npm exec -- prettier --check docs`、`cargo-fmt --manifest-path src-tauri/Cargo.toml -- --check`、`git diff --check` 通过；PowerShell AST 检查交付脚本无语法错误。
- 本轮修复：分类清空在首个 `await` 前增加占位，阻止双击重入；放大/收回在移动 iframe 前进入等待态，修复 load 事件晚于空白帧。按追加要求统一最少 300ms，新增 0/100/500ms 就绪、等待中再次 load/切换记录及卸载计时清理用例；失败及时反馈、12 秒超时不变。
- 删除废弃截图模块/依赖和旧原生方案的 Windows feature；修正过时测试 Mock/选择器、102 条规则容量预期及新增样例类型字段。`node scripts/generate_notices.mjs` 离线重生成 37 个前端包/322 个 Windows crate；与 HEAD 相比仅产品版本标识变化，第三方原文保持。
- 原生桌面实测使用独立 `com.ahui.model-probe.qa-review-20261027`、虚构配置和用户提供的黑天鹅动画。实际检查卡片直接显示、完整画面缩放、页内放大/Escape 后恢复尺寸位置、放大内右键不弹菜单、历史列表实时动画及点击放大、详情多列与动画直接显示、复制后返回列表无旧提示；未出现额外预览窗口。只通过隔离程序操作，无真实模型或 SMTP 请求。
- 最后一次启动桌面复验时 Computer Use 报告用户按物理 Escape 停止；随后停止全部界面控制。新增 300ms 的精确时间由可控时钟回归验证，最终 QA 构建成功，未完成该追加改动的最后一次原生桌面复验，不能将之前桌面结果写成最终 300ms 原生实测。
- 仓库外只读文本核对脚本：47 个改动文件严格 UTF-8/无 BOM/LF/无替换字符，22 个本地 Markdown 目标存在，六处工程版本均 1.0.3，逐文件索引无遗漏，明显令牌格式扫描无命中。发布说明改用正式 Tag 的绝对链接，发布后可直接用于 GitHub。检查注释与使用手册，移除旧截图/心跳/固定 1024×768 描述；仅历史验证记录保留旧实现说明。
- `run-build.ps1 build`：`vue-tsc --noEmit`、Vite 和 Windows x64 `tauri build --no-bundle` 均通过；仅有既有前端 chunk 体积提示（525.03 kB，gzip 188.08 kB）及 MSVC 导入库 stdout 提示，未出现构建错误。
- `scripts/package_exe.ps1 -OutputDirectory release-artifacts/v1.0.3-reviewed`：检查 PE x64、EXE 版本、应用输入时间及复制摘要；输出目录仅含 `Model-IQ-Radar-v1.0.3-Windows-x64.exe`。文件/产品版本 1.0.3，16,701,440 字节，生成时间 2026-10-27 12:19:46（本机时间），SHA-256 `adac703e47a710e19634af16bcce048567686c7aa65ec0252c66880fe224a4fb`，另行确认正式二进制不包含 QA identifier；未生成 ZIP 或附带资产。
- 未实测真实模型服务、真实邮件投递、全部物理 DPI/多屏、登录自启、休眠唤醒和屏幕阅读器组合。iframe 共享浏览器资源，不能沿用旧工作进程的死循环硬终止结论；外部系统状态补偿无法保证与 SQLite 完全原子。指定 Java 手册已阅读，本项目采用 Vue/TypeScript/Rust 规范，Java/JVM/MyBatis 专属条款不适用。

## Beta 4（2026-10-27，历史出包记录，人工预览已通过）

- 交付：`release-artifacts/v1.0.3-beta4-unified-preview/Model-IQ-Radar-v1.0.3-beta.4-Windows-x64.exe`，文件/产品版本 1.0.3，16,709,632 字节，SHA-256 `7fd7755f0a5223f23a9ee94b1c0475ad129446ccb5791c51800f53c3d58ecceb`；目录仅一个 EXE，无 ZIP。
- 实际执行：受影响文件 `npm exec prettier -- --write ...`、`cargo-fmt fmt --manifest-path src-tauri/Cargo.toml` 和 `run-build.ps1 build` 均完成；其中 `vue-tsc --noEmit`、Vite 和 Windows `tauri build --no-bundle` 成功。构建仅有既有 chunk 大小与链接器导入库提示；`package_exe.ps1` 核对 PE x64、版本、应用输入时间与复制摘要。15 个本轮文件通过有效 UTF-8/无 BOM/LF 检查，读取 `git status` 与目标文件变更统计；未提交、推送或创建 Tag/Release。
- 本轮范围：三处 HTML 共用预览、加载动画、片段选择、点击放大和关闭；放大取消应用额外面板/留白，历史列表放大期间保留会话。打开菜单不再暂停预览或显示“预览已停止”；列表“思考强度”左对齐，详情复制反馈按页面世代隔离。
- 主要实现文件：`HtmlPreview.vue`、`AnswerContent.vue`、`HistoryAnswerPreview.vue`、`htmlSandbox.ts` 与 `style.css` 统一预览；`HistoryPanel.vue`、`Settings.vue` 处理列表布局/入口与复制反馈；`Widget.vue`、`bridge.ts`、`model.rs` 调整默认选择及规则恢复。README、使用手册、变更日志和本地发布说明已同步；沿用 Vue/TypeScript/Rust 规范，Java 专属接口和工具条款不适用于本项目。
- 新建接口默认绑定现存糖果预设，规则编辑页首次打开也选糖果；已有接口绑定保留，糖果已删时新建回退现存默认规则。明确清空/恢复规则时重建内置题目并把保留接口绑定糖果；不升级 schema 或配置备份格式。
- 按用户要求，只进行格式整理、出包必需的类型/编译检查和单 EXE 交付核对；不启动应用，不运行测试、Clippy、真实模型/SMTP 或全面未提交文件审阅。加载时序、三处视觉/交互、菜单连续播放、规则默认值与剪贴板导航回归仍等待人工预览后明确授权；以下旧测试数字不代表本轮通过。

## 先前 iframe Beta 3（2026-10-27，历史出包记录）

- 最终交付：`release-artifacts/v1.0.3-beta3-preview-rules-history/Model-IQ-Radar-v1.0.3-beta.3-Windows-x64.exe`，文件/产品版本均为 1.0.3，16,709,632 字节，SHA-256 `31d632705e6e3f806a7173773d80255772179e2c0956388e3ee1371b60165438`；交付目录仅一个 EXE。
- 本轮实际命令：受影响文件 `npm exec prettier -- --write ...`、`cargo-fmt fmt --manifest-path src-tauri/Cargo.toml`、`run-build.ps1 build`（`vue-tsc --noEmit`、Vite 生产构建及 `tauri build --no-bundle`）均完成。最终 Windows release 构建成功；仅保留既有前端 chunk 大小和链接器导入库提示。`scripts/package_exe.ps1` 检查 PE x64、工程/文件版本、输入时间和复制摘要，随后按 Beta 3 命名交付。没有启动应用、运行自动化测试、Clippy、全面审查或真实模型/SMTP 请求；没有 Git 提交、推送、Tag 或 Release。

- 最终 Beta 3 另含 SMTP 默认 `smtp.qq.com`：前后端新建/重置一致，旧空表单只补草稿，自定义主机不覆盖；关闭通知时允许未填写账户的默认表单保存/备份，不改变启用和试发的凭据要求。未执行 SMTP 投递或回归测试。

- 本轮另按最新要求删除预览十分钟自动停止计时器及原生文档过期判断；保留正常卸载释放，并在原生宿主销毁时释放文档。本项未进行时长测试，包含在待人工预览的最终 Beta 3。

- Beta 3 继续包含最新预览反馈：关闭按钮独立实色 hover/focus/active；1440×1000 基准视口在 load/字体就绪后测量一次完整页面，限制在 4096×8192 内并整体居中适配。此项同样仅编译出包，尚未执行运行或视觉验证。

- 本轮 Beta 3 新增鹈鹕内置题目、日本首相正文精确迁移、五类可选清空/恢复初始状态，及历史四项筛选、快照摘要和自适应详情。按用户再次明确要求，仅格式化及编译单 EXE，不执行自动化测试、桌面验证或全面未提交文件复核；相关回归与最终验收等待人工预览后明确授权。旧测试数字不代表本轮变更已验证。

- 最新 Beta 2 将历史列表右侧改为与悬浮卡片相同的 iframe 实时预览，仅可见行挂载，取消截图生成；预览留边与放大层优先沿用 HTML 自身纯色背景，透明或未声明时使用卡片深色；关闭宿主默认右键菜单、浏览器保存/打印快捷入口和下载浮层，放大改为保留标题栏与边缘留白的页内面板。按用户要求本轮仅执行格式化、编译出包及交付文件核对，未启动新一轮测试，等待人工预览后的明确通知。
- 已用页面内 sandbox iframe 替代原生预览子窗口/辅助进程，1440×1000 基准画布、首帧有界扩展后等比缩放，小预览不接管鼠标，放大为页内弹层。
- 根据最新反馈去掉外层工具栏和说明，画面直接填满回答区域，点击放大；这次界面简化之后只执行出包必需的编译，不将此前测试结果视为当前版本的最终验证。
- 用户明确要求先提供单 EXE Beta，待人工预览并明确允许后，才开始后续自动化测试与全面复核。组件测试适配、集成检查、文档索引和完整未提交文件审阅仍待继续。
- 以下原生进程方案的历史证据已被当前实现替代，不能据此宣称当前 Beta 支持进程级硬隔离、无响应强制回收，或已经完成全面检查。

## v1.0.3 早期原生辅助进程验证（2026-10-27，历史实现，已被 iframe 替代）

- `npm version 1.0.3 --no-git-tag-version --ignore-scripts` 更新 npm 清单；随后单独同步 Cargo/Tauri 清单并用 Cargo 元数据生成 Cargo.lock；HTML 预览显式引用现有锁定版本的原生依赖，未升级既有依赖。
- `npm test -- --reporter=dot`：9 个文件、169 项通过；包含正整数超时、快捷模型选择、html-v1 共用样例、源码保留、历史默认预览、失败重载后状态恢复、迟到启动取消和缩略图队列回归。新增首帧等待 7 秒仍不误发心跳、慢心跳合并及旧心跳不覆盖新会话两项回归。
- `run-build.ps1 test`：86 项 Rust 测试通过（57 单元、15 数据完整性、14 请求回归），覆盖 HTML 不进入关键词判定、新旧 HTML 历史不发信且不推进文字通知状态，以及原有调度、超时、备份/schema v3 和模型目录回归。
- `run-build.ps1 clippy`：所有 target 按 `-D warnings` 通过；`cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`、`npm run build` 及 Windows `tauri build --no-bundle` 通过。最终生产 EXE 版本为 1.0.3，大小 16,923,648 字节，SHA-256 为 `5dd15eda8d0515669420aa2c4e621dc96c4162c236db5c382a322207017535b9`，本次生成时间为 2026-10-27 09:02（本机时间）。此前同版本构建不能代替该文件。
- `npm run format:check`、`npx prettier --check docs`、`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` 和 `git diff --check` 通过；改动文本严格 UTF-8、无 BOM、LF，无替换字符，六处工程版本均为 1.0.3。
- `python -B scripts/check_html_preview.py --exe <debug/release EXE> --html-file <用户提供的 UTF-8 HTML>`：调试和最终生产辅助进程均验证 SVG/HTML 实际绘制、宿主桥接隔离、零本机 TCP/HTTP 请求、拒绝其他 data 页面导航、data/blob 图片、脚本异常报告、死循环有界停止，共七例通过。所有截图严格为 640×360；工作进程从启动到回收每 10ms 采样，未出现用户可见的顶层承载窗口。Tao 的透明线程消息接收器按已核实源码单独排除。用户提供的“黑天鹅的单车漫游”连续两帧像素不同，画面已实际查看；最终复验未指定 `--output`，没有保存产品截图。
- 本轮修复原生文档源码转义及 data 文档导航被拒绝导致的空白截图；文档 URL 按本次完整源码精确放行，子框架导航全部拒绝。HTML 新旧记录隔离通知状态；预览失败/恢复不改写历史结论。原生辅助进程不初始化 Tauri/正式数据库/凭据。
- 针对用户反馈再次定位内嵌空白根因：顶层 Wry `set_bounds` 会移动复用的承载 HWND。改为真正的 WS_CHILD 承载窗口及 `build_as_child` 独立 WebView 子容器，保持裁剪和宿主坐标；缩略图宿主始终隐藏，显式初始化尺寸。首帧就绪后开始运行租约并发送前端心跳；监督错误保留具体安全原因。
- CUA 原生桌面使用独立 identifier `com.ahui.model-probe.qa-html-20261027`、虚构 SQLite 配置及用户提供的动画，确认悬浮直接显示、源码/效果切换、移动后跟随、收起、列表缩略图、详情直接显示与滚动裁剪、停止/重载、放大及关闭后的具体提示。隔离构建/种子和运行副本均在仓库外；不读取正式数据库或凭据，不请求真实模型/SMTP。
- 全部 48 个未提交文本逐文件检查见 [FILE_AUDIT-v1.0.3.md](docs/FILE_AUDIT-v1.0.3.md)。严格 UTF-8、无 BOM、LF、无替换字符，六处版本一致，审阅索引覆盖所有修改/新增文件；本地 Markdown 目标检查通过。没有覆盖、删除或重置此前 v1.0.3 工作。
- `scripts/package_exe.ps1 -OutputDirectory release-artifacts/v1.0.3-html-preview-fix` 校验 PE x64、文件/产品版本、构建输入时间及 SHA-256；交付目录仅一个 `Model-IQ-Radar-v1.0.3-Windows-x64.exe`，与最终构建逐字节一致，没有 ZIP 或附带文件。
- `node scripts/generate_notices.mjs` 离线重新生成第三方声明：37 个前端包、322 个 Windows crate。新增直接依赖均已在既有传递依赖锁定集合内，许可证正文未变化。
- CUA 隔离浏览器夹具核对 360px 悬浮窗有目录/空目录的 70%/30% 与 65%/25%/10% 布局、规则页独立超时字段、运行中“本次结束后计时”；夹具不访问真实 API、SMTP、数据库或凭据。
- 开发预览排除监听 `src-tauri/target` 和 `release-artifacts`，修复 Windows 构建/交付 EXE 引起的 Vite `EBUSY`；前轮已验证独占打开交付 EXE 并更新文件时间后预览仍运行。最终资源仍有约 512.53 kB 的 chunk 体积提示，原生链接器的导入库 stdout 提示不影响构建成功，独立 Clippy 严格检查通过。
- 按用户反馈取消规则/全局的 10—300 秒业务限制，300 秒仅为默认值。回归覆盖 1/9/600/3600 秒保存和备份往返、空规则继承、零/负数/小数/精度溢出拒绝，以及 1 秒真实超时和 600 秒预算的本机请求。CUA 浏览器实查规则输入 600 秒和新提示、全局默认 300 秒及说明，无布局裁切。
- 未实测真实模型/SMTP 投递、原生 WebView2 多 DPI、多屏及硬件休眠唤醒；浏览器布局和确定性回归不能代替这些端到端验证。已阅读指定 Java 手册并采用通用要求，本项目 Vue/TypeScript/Rust 没有 Java/JVM/MyBatis 组件，语言专属规范不适用。
- 本轮源码、验证记录和单个 EXE 测试文件均仅在本地保留，未创建 Git 提交、Tag 或 GitHub Release；历史 v1.0.2 验证数字和下载链接仍表示已发布版本。

### 前轮 v1.0.3 本地测试包（HTML 功能加入前）

- 以下 EXE/摘要属于 HTML 功能加入前的交付，不能作为本轮 HTML 功能验证或最新构建。该轮文件/产品版本均为 1.0.3；只复制单个 EXE，不启动正式应用或读取正式数据库/凭据。
- 通过 `scripts/package_exe.ps1` 核对 PE x64、文件版本和 SHA-256，将唯一的 EXE 复制到 `release-artifacts/v1.0.3/`；EXE SHA-256 为 `bf120d658d16a76107b3af9a5854cdfacd1904edab2a6fac39114a35a05f3f54`。后续 Tag/Release 同样只上传该 EXE，不生成 ZIP。旧版 ZIP 仅作为历史交付物保留，不代表 v1.0.3 构建。
- 本地测试沿用正式应用标识，首次运行前应退出旧版并备份完整 `%APPDATA%\com.ahui.model-probe` 数据目录；schema v3 升级后不能直接切回旧版 EXE。

以下保留 v1.0.2 及更早的历史验证记录，旧格式说明和数字仅对当时版本成立；当前兼容边界以 v1.0.5 使用手册为准。

## v1.0.2 发布核验（2026-10-26）

- 根据 GitHub 既有发布，采用附注 Tag `v1.0.2`、标题“测智雷达 v1.0.2”、正式 Windows x64 便携 ZIP 和独立 `SHA256SUMS.txt`；源码发布说明位于 `docs/release-notes/v1.0.2-zh.md`。保留原 v1.0.1 发布内容，邮件修复单独写入 1.0.2 更新日志。
- `npm version 1.0.2 --no-git-tag-version --ignore-scripts` 同步 npm 清单及锁文件；Tauri/Cargo 清单同步，Cargo 工具离线更新锁文件，仅本项目版本变化，未升级依赖。设置页和脱敏诊断改用工程版本，和关于页保持一致。
- `node scripts/generate_notices.mjs` 离线重生成第三方声明：37 个前端包、322 个 Windows crate，正文只变更本项目版本标识，第三方原文不变。
- `npm test -- --reporter=dot`：8 个文件、132 项通过，包含新版关于信息、设置草稿保护、邮件试发和保存防重回归。
- `run-build.ps1 test`：正式 1.0.2 清单下 79 项 Rust 测试通过（52 单元、14 数据完整性、13 请求回归）；`run-build.ps1 clippy` 全目标 `-D warnings` 通过。`npm run format:check`、`npx prettier --check docs`、`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` 和 `git diff --check` 通过。
- `run-build.ps1 build`：TypeScript/Vite 与 Windows x64 release 构建通过，原生 EXE 文件版本为 1.0.2；未启动程序、读取正式凭据或发送真实邮件。生成 import library 的 linker stdout 提示不影响构建成功，独立 Clippy 严格检查通过。
- `package-v1.0.2.py` 沿用远程 v1.0.1 便携包文件白名单，增加本次发布说明；逐条目核对 ZIP、EXE、公开文档和 CRC，独立 `SHA256SUMS.txt` 记录 ZIP 摘要。正式资产位于 `release-artifacts/v1.0.2/`，不混入本地测试包或用户数据。
- 六处工程版本一致，21 个改动文本严格 UTF-8、无 BOM、LF，无替换字符；21 处本地 Markdown 目标有效。v1.0.1 发布说明保留原文，第三方声明只更新项目版本，不改第三方许可证正文。

## 发布前邮件通知修复核验（2026-10-26，本地构建显示 1.0.1）

- 邮件通知改为按模型分组记录最近一次有效判定：每次降智和测试失败都发送，恢复仅在该分组此前降智后再次不降智时发送；手动、自动测试和规则切换均适用，取消仍不发送。
- 启动时兼容旧版“模型 + 条件摘要”状态，依据保留历史重建模型分组最近一次有效判定并立即保存，避免把旧条件的插入顺序误作更新时间；旧历史已清空时从未确认状态开始，新格式恢复不再额外读取整份历史。同步更新 README、使用手册、更新日志和发布说明。
- 再次复查先通过新增回归复现“迁移只在内存生效，未测试就清空历史并重启会丢失恢复依据”，修复后该回归通过；新增迁移保存失败保留旧 payload、新格式不加载历史、旧历史读取错误和未知状态版本拒绝的覆盖。
- `npm test -- --reporter=dot`：8 个文件、132 项通过；`npm run build`、`npm run format:check` 和 `npx prettier --check docs` 通过。
- 临时官方 MSVC/SDK 环境执行 `cargo test --locked --manifest-path src-tauri/Cargo.toml`：79 项通过（52 单元、14 数据完整性、13 请求回归）。覆盖手动/自动、持续异常、跨规则恢复、分组隔离、通知关闭后启用、旧状态迁移及保存失败、迁移后立即清空历史再重启、取消及并发通知不丢弃。SMTP 仅连接随机本机端口，服务不提供 STARTTLS，因此不会发送真实邮件或读取正式授权码。
- `cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`、`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` 和 `git diff --check` 通过；13 个改动文本严格 UTF-8、无 BOM、LF，无替换字符，23 处本地 Markdown 目标有效。
- 原终端缺少 MSVC linker 和 SDK，首次 Rust 测试被环境阻止；随后仅在 `C:/Users/Admin/.codex/tmp/model-iq-radar-mail-build` 解包微软官方构建依赖，使用该目录的 `run-build.ps1 test / clippy` 完成验证，未执行系统安装或修改全局环境。
- `run-build.ps1 build`（`npm run tauri -- build --no-bundle`）：Windows x64 release 构建通过，EXE 为 15,138,816 字节，显示版本仍为 1.0.1。`package-fix.py` 只打包固定 EXE、许可证及使用说明，逐 ZIP 条目核对内容与 SHA-256；交付于 `release-artifacts/email-notifications-20261026/`，ZIP SHA-256 为 `18229071888afb0e65d42d04d14228e9a6f14e187b6adb7b3f53c84eb6cd184d`。上轮包保留于 `release-artifacts/previous-20261026-before-startup-migration-fix/`，避免用旧构建冒充复查后的代码。编译器的 import library 创建提示产生 linker stdout 警告，不影响成功结果；Clippy 严格检查独立通过。
- 本次未调用真实模型、发送真实 SMTP 邮件、替换正在运行的 EXE，未提交 Git、发布 Tag 或 Release。Vue/TypeScript/Rust 使用对应语言的文档与工具链，Java/JVM、MyBatis 和 Maven 专属规范不适用。

以上本地构建记录保留发布准备前的验证过程；正式 v1.0.2 验证以顶部发布核验为准。以下为先前 v1.0.1 的 17 项需求复核和发布记录，旧截图和旧便携包不作为 v1.0.2 结果。

再次逐文件复核的范围、逐项结论与本轮新发现见 [FILE_AUDIT-v1.0.1.md](docs/FILE_AUDIT-v1.0.1.md)。源码逐文件阅读，生成内容和第三方原文采用来源/一致性核对；不声称审计了全部第三方依赖源码。

## 最新执行：默认协议与选择器尺寸统一（2026-10-25）

- 去掉默认规则选项和当前值中的“沿用默认：”，仅显示名称，保存仍使用 null 绑定默认规则。新建表单默认 Responses/失败切协议开启，允许手动选择 Chat 或关闭开关；已有配置保留保存值，缺字段旧配置仍按 Chat/关闭读取。没有批量数据迁移，不修改请求协议回退的次数、总超时及取消边界。
- 修复展开态单独放大选择器、等级单独缩字、可编辑模型输入额外增高的三处来源。卡片三框两态均为 29px 高、11px 字号、15px 箭头；规则/等级约 80:20，等级列至少 70px，避免统一字号后 medium 被省略。接口/规则表单统一 36px 高、12px 字号，历史筛选保留独立的紧凑尺寸。
- `npm test -- --reporter=dot`：8 文件、132 项通过。新增覆盖新建与空配置默认值、手动关闭、连接与保存的策略一致性、旧配置兼容、已有显式协议/回退组合不被覆盖以及默认规则 null 绑定；原有快捷切换去重、菜单互斥和草稿保护回归继续通过。
- CUA 隔离内存预览复核 360×156/360×572 卡片及 700×520 设置窗口，实测高度/字号/箭头一致；medium 与“不指定”完整显示，设置表单普通/可编辑选择器均为 36px。实际点击新增确认 Responses 和已开启开关；未调用真实模型、SMTP 或桌面凭据。截图位于工作目录 `work/screenshots/v101-selector-defaults/`，临时 QA 页面、组件和桥接副本已清理。
- `npm run format:check`、`npx prettier --check docs`、隔离工具链 `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`、`git diff --check` 通过；本轮 12 个改动文本均为严格 UTF-8、无 BOM、LF，无替换字符。检查 Git 状态保留原有未提交工作，未提交或发布。
- `run-build.ps1 portable`：vue-tsc、Vite 与 Windows x64 release 构建通过，新前端已嵌入 EXE。上轮便携包及校验和归档至 `release-artifacts/previous-20261025-before-selector-defaults/`，本轮交付以新 ZIP 和独立 SHA-256 文件为准。
- 注释、类型说明、README、使用手册、变更日志、发布说明、逐文件索引与一致性说明同步新建/旧配置边界。本轮后端仅修改注释，未重跑 Rust 单元测试；真实第三方服务与原生 WebView2 多 DPI 未实测。Java/MyBatis 专项不适用于此 Vue/CSS 修复。

## 前轮执行：恢复紧凑态测试时间标签（2026-10-25）

- 根因是紧凑态模板只渲染日期，将“测试时间”留在 title；恢复第一行可见标签，完整本机日期时间和第二行耗时/倒计时保留。展开态原有“测试时间”及条件变更后的“上次测试”文案保持原样。
- `npm test -- --reporter=dot`：8 文件、128 项通过。补充直接断言可见标签与完整日期同在第一行，并覆盖展开、收起、关闭自动测试及条件失效后的显示，避免仅校验 title 漏检正文。`npm run build`：vue-tsc 与 Vite 通过。
- CUA 以真实组件及隔离内存数据复核 360×156 紧凑态：通过、待重测、关闭自动测试、尚未测试、测试中均无时间行和操作行横向溢出；最窄待重测时间行仍有 163px，完整标签与年月日时分秒可见。实际点击展开后，360×572 布局仍显示标签与完整日期。截图保存在工作目录 `work/screenshots/v101-time-label/`；临时 QA 页面、组件及桥接副本已移除，不进入生产包。
- `npm run format:check`、`npx prettier --check docs`、`git diff --check` 通过；本轮 6 个改动文本严格 UTF-8 解码通过，无 BOM、CRLF 或替换字符。检查 Git 状态并保留原有未提交工作，未执行 Git 提交。
- `run-build.ps1 portable`：Windows x64 release 构建通过，前端修复已嵌入新 EXE。上轮便携包及校验和归档到 `release-artifacts/previous-20261025-before-time-label/`，保留可回溯副本；本轮交付以新 ZIP 及其独立 SHA-256 文件为准。
- 使用手册、变更日志及首版一致性说明同步可见标签要求。本轮仅修改前端显示与断言，没有数据结构、请求、调度或凭据变更；Java/MyBatis 专项不适用于此 Vue 模板修复。未重跑 Rust 单元测试，未实测原生 WebView2 多 DPI；前轮后端验证记录保留如下，不冒充本轮结果。

## 前轮执行：手动更新与邮件卡片（2026-10-25）

- 恢复“关于”手动检查，移除页面中的后台周期/静默说明；后台任务保留。前端忙碌去重、失败可重试并保留下载标记，迟到响应不覆盖更新事件；后端合并同时到来的请求，失败保留成功缓存。
- 三状态深色邮件包含当次配置、模型、规则、等级、实际协议、来源、完整时间与耗时，以及完整模型回答。失败附错误详情、HTTP 状态、请求编号和有效超时；空回答与部分原文明示。正文安全转义，不渲染模型 HTML 或远程图片；multipart/alternative 的纯文本与 HTML 内容等价，图标通过 multipart/related 的 CID 内嵌，右下角仅可点击的 GitHub 图标。
- `npm test -- --reporter=dot`：8 文件、128 项通过；`run-build.ps1 test`：72 项通过（45 单元、14 数据完整性、13 请求回归）。新增主动检查去重/重试/迟到响应、后台共用请求与失败缓存、模板快照、旧字段空态、长回答末尾、HTML 转义、真实 MIME/CID 对应及取消不发信回归；全部使用虚构数据与本机服务。
- `npm run build`、`run-build.ps1 clippy`（all-targets、`-D warnings`）、隔离工具链 `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` 均通过；Prettier、`git diff --check` 通过。65 个项目文本严格 UTF-8 解码，无 BOM、CRLF 或替换字符；24 个 PNG 解码通过，22 处本地 Markdown 链接有效，临时 QA 文件已从源码移除。第三方声明离线重新生成仍为 37 个前端包、322 个 Windows crate，字节与生成前一致。17 个邮件图标重复生成哈希相同。
- CUA 实际浏览器验证关于页 700×520：手动检查入口、真实浏览器预览错误反馈和有新版本时的双按钮布局。虚构新版本 1.10.100 时，两个按钮底部 506.5px，右侧 677px，页面宽与 scrollWidth 均 700px；没有挤压或横向溢出。真实 MIME 解码的三种邮件模板核对配色、完整原文区和图标；375px/620px 长内容邮件 viewport 分别为 360px/605px（扣除滚动条），scrollWidth 与之相同，图标全部解码且回答结束标记保留。截图保存在本机 `work/screenshots/v101-mail-update`，不进入生产源码。
- README、使用手册、贡献指南、更新日志、正式发布说明与审计索引同步。本轮未改数据库、配置/备份格式或凭据契约；Vue/TypeScript/Rust 项目没有 Java/MyBatis 接口，Java 手册中相应语言及 SQL 专项不适用。
- `run-build.ps1 portable`：Windows x64 release 构建通过，EXE 15,162,368 字节，文件版本 1.0.1；包含本轮前端、邮件模板与内嵌图标。未启动正式标识读取用户数据。旧交付文件已保存在 `release-artifacts/previous-20261025-before-manual-update-email`；新包通过固定白名单脚本生成并逐 ZIP 条目核对输入文件 SHA-256，独立校验文件随包交付。

本轮未发送真实 SMTP 邮件，未执行 Outlook/Gmail/其他邮件客户端投递与强制主题变换验收；浏览器模板与 MIME 验证不能代替客户端兼容性实测。未调用真实模型、读取正式用户数据库/凭据或进行 Git 提交、Tag、Release。以下为历轮记录，测试数字和包不能代替本轮结果。

本轮追加修复：凭据实际变化未使结果失效、Responses 中间说明及 Chat 非助手/拒绝回答混入判定、保存/导入成功后读取失败导致重复写入、保存期间输入冻结遗漏、下拉异步状态竞态、旧读取错误覆盖新快照、取消失败按钮锁定、外部自动开关最后 IPC 竞态、超限备份可导出但无法导入、历史空查询的无意义大字符串复制、邮件保存成功后未等待权威快照同步，以及可解析但不可执行的本地配置与悬浮窗全量历史传输。启动现在完整校验配置且不改写失败数据；悬浮窗只传当前模型最近记录，设置页显式读取完整历史。

最新复核另外修复首版 101 个以上合法模型配置被新增数量校验阻止启动的问题，以及已提交配置后窗口退出吸边失败被误报为保存/导入失败的问题。前者保留本机旧配置使用能力、独立限制新增与备份；后者返回成功及跨页保留的独立警告，继续同步快照，不要求重复提交。

上一轮交付复核通过 75 项前端测试、54 项 Rust 测试、生产构建、格式及静态检查，未发现新增缺陷。随后用户继续反馈 UI 一致性问题，本轮已实际修改组件与样式，不沿用上一轮“未改代码”或旧包作为交付结论。逐项首版规范复核及新增问题见 [UI_CONSISTENCY-v1.0.1.md](docs/UI_CONSISTENCY-v1.0.1.md)。

前次 UI 修复涵盖新增入口混用、关于排序、邮件图标/忙碌反馈、确认弹窗配色、数值框提示截断、预览说明遮挡底栏，以及日期只点小图标打开、系统白色日历配色不一致。共用深色日历补齐键盘、焦点、年界与小窗口避让；真实浏览器还复现并修复导航按钮到达年界时禁用引起的失焦误关闭，以及隐藏导入文件字段被全宽输入规则覆盖引起的横向溢出。

本轮继续完成卡片规则/强度摘要与快捷绑定、设置排序、预设还原隔离、默认规则改名、多答案 OR、可编辑持久模型目录，以及自带日本首相测试和糖果测试。另修复当前模型不在目录时无法浏览候选、输入法/光标键拦截、双层输入边框、晚到缓存覆盖新目录与凭据版本捕获竞态；新增数据契约采用明确的格式版本，文档不再沿用 schema v1 可直接降级的描述。

## 本轮发现和修复

| 需求           | 复核结果与相关入口                                                                                                                                 |
| -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1 自动快捷开关 | Widget 顶部顺序固定为自动测试、设置、展开；托盘与设置使用同一持久化状态。IPC 等待期间防重复，失败重读，关闭不取消在途请求。                        |
| 2 计时规则     | 外观保存不改期限；成功领取手动请求、重新启用或修改间隔重新计时。预检与领取二次核验，慢请求错过的期限不补跑。                                       |
| 3 思考强度     | 默认 low，深色下拉与“不指定”保存语义一致；仅对官方确认的精确型号限制选项，未知服务明确说明支持范围未知，不自动换档。                               |
| 4 邮箱通知     | 补齐 lettre TLS/STARTTLS、系统凭据、草稿试发、事件选择、模型分组状态、取消不通知及独立失败提示。SMTP 目标与密码同锁配对，网络等待不持业务锁。      |
| 5 结果失效     | conditions.rs 对规范化端点、模型、完整规则、协议、强度、有效超时生成稳定摘要；旧记录无摘要时待重测，改名/外观不影响，历史不重判。                  |
| 6 时效与排版   | 两态可见 yyyy-MM-dd HH:mm:ss，相对时间只作悬停补充；紧凑日期独占一行。边缘图标没有额外提示卡或 title，保留颜色、旋转和动画恢复。                   |
| 7 历史详情     | 分组、搜索、结果、来源、本机日期范围组合；保存实际请求设置，旧字段显示未记录。独立 HistoryPanel 复用安全 Markdown。                                |
| 8 保留策略     | 修复只 LIMIT 隐藏、时区字符串比较及清理不落盘。count/day/128 MiB 逻辑正文硬限制同时生效；清理和配置一次事务，收紧前预览确认。                      |
| 9 导出         | 可选全部或筛选集、JSON/CSV。JSON 原文无损；CSV 对公式危险前缀转义，并明确提示用 JSON 留存原文。                                                    |
| 10 备份迁移    | 4 MiB/100 模型/100 自定义预设＋1 自带糖果边界，格式 v2 读取兼容 v1；重复策略、引用重映射与原子提交，拒绝未知版本、凭据字段及悬空规则。             |
| 11 规则绑定    | 支持新建、命名、编辑和每模型绑定；引用删除明确确认并原子解绑到默认规则。运行发现悬空绑定报错，不能静默换题。                                       |
| 12 诊断        | 错误分类、HTTP 状态及受限请求编号；诊断不含完整题答、授权头或密钥，不能回显服务商原始错误正文。                                                    |
| 13 超时        | 每配置覆盖或继承全局；连接诊断与正式请求使用相同有效值，在请求开始时冻结。                                                                         |
| 14 草稿保护    | 页面/编辑对象/原生关闭统一保存、放弃、取消。失败保留内容；预览与邮件发送期间锁定交互。外部自动开关只合并同名字段，不覆盖其他草稿。                 |
| 15 恢复调度    | 单门禁、单期限，不维护补跑队列；识别长轮询间隔并重排未来周期，断网按周期失败，不恢复即重试，锁屏不作为暂停条件。                                   |
| 16 Responses   | 按选定协议构造及严格解析完整最终正文；缺失完成状态、截断、空响应等按错误处理；回退默认关闭，显式启用后最多切另一协议一次，共用总期限，不自动换档。 |
| 17 边缘拖动    | 记录起点/真实位移；独立鼠标释放监视覆盖无 Moved 的单击路径；收尾幂等，保留边缘状态，旧释放世代不结束新拖动。                                       |

额外修正了保存配置后持锁广播可能导致托盘监听重入锁的问题；所有相关写入先释放 store 锁再广播。原生草稿关闭路径增加所需 destroy 能力，避免点击“保存/放弃”后仍因权限被拒绝。

## 不变量、兼容和回滚

判定仅对完整非空最终原文逐项执行区分大小写的连续字符串匹配，任一答案命中即通过，保留有效空白；旧多行 keyword 不拆分。Markdown 显示、复制和历史不改变原文。错误/取消不归为降智。客户端不提供搜索/文件工具，不能核实服务商内部隐藏检索。

数据库升级为 schema v2，保留原有配置/测试表，新增 notification_state、model_catalog、最近记录派生索引 latest_test_record 及可选 JSON 字段；完整校验与历史整理成功后同事务写版本标记，旧程序拒绝新库，防止忽略多答案后误判。索引只含模型 ID 与历史序号，与历史同事务更新，启动重建以接纳升级前已有记录。旧配置协议为 Chat Completions、强度默认 low、超时继承全局、协议回退默认关闭；首版超过 100 个模型的合法配置仍可启动、编辑和删除，新增与备份/导入边界分别限制。启动校验失败不改写配置正文或清理历史，但此前可能已初始化兼容附加表及 WAL。新增可空 credentialRevision，仅在实际替换/移除 API Key 时生成随机标识，不包含密钥及其散列；重复保存相同凭据保持修订。协议回退关闭时单答案条件摘要保留版本 2 的字节契约、多答案使用版本 3，显式启用回退使用版本 4；名字不参与，缺摘要记录待重测，历史不改判定。备份不迁移凭据修订或缓存句柄，替换可保留同端点本机值，新建/复制不继承。通知摘要不含题答、密钥或 SMTP 授权码，最多跟踪 1000 个模型分组；历史清理不删除它。自带糖果在内存一次补入，下一次配置保存持久化标记，不覆盖原配置正文或同 ID 用户修改；删除前保存标记，因此重启不会复建。

历史上限为 JSON 正文字节，不等于 SQLite 文件物理大小，不自动 VACUUM。升级前备份完整数据目录和需要留存的历史；降级必须退出新版、恢复升级前数据库备份，再运行旧 EXE。备份格式 v2 可读 v1，旧程序拒绝 v2；仅换 EXE 不能降级，不可撤销已发邮件或找回已清理历史。导入事务或凭据保存失败保留旧配置，运行时设置失败执行回滚并报告恢复失败。

## 前轮执行（统一开关、完整时间及后续交互）

- `npm test -- --reporter=dot`：8 个测试文件、125 项全部通过。新增 12 个表单开关的统一样式/角色/可访问名称、草稿与保存去重/禁用/失败保留，以及本机完整日期、午夜补零和条件变更后保留上次时间回归；模型菜单互斥、快捷强度草稿隔离和小时更新监听继续通过。
- `run-build.ps1 test`：65 项通过（38 单元、14 数据完整性、13 请求回归），覆盖双向协议回退、关闭/正常回答不回退、最多两次尝试、原始总超时、取消、备份快照与小时虚拟时钟。修正本机 HTTP 测试服务有界 accept 后 Windows 套接字继承非阻塞模式的问题，完整重跑通过；测试服务不再永久等待未出现的请求。
- `npm run build`：vue-tsc 与 Vite 生产构建通过。`run-build.ps1 clippy`：所有 target 按 `-D warnings` 通过；隔离工具链 `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` 通过。
- CUA 实际浏览器：700×520 设置页核对协议和邮件事件滑动开关；点击文字、鼠标点击及空格键均能切换，焦点可见；底栏固定、无文字裁切。虚构内存历史验证 360×156/360×572 两态完整 `yyyy-MM-dd HH:mm:ss`，待重测仍保留上次时间，关闭自动测试仅隐藏倒计时；回答任务标记共享开关外观但保持禁用。未触发真实请求、发信或保存凭据。
- `npm run format:check`、`npx prettier --check docs`、`git diff --check` 通过；74 个源码文本文件严格 UTF-8 解码通过，无 BOM、CRLF 或替换字符；23 处本地 Markdown 相对链接有效。临时 QA 页面与源码副本已清理，用户已有改动保留，未执行 Git 提交。
- README、使用手册、变更日志、正式发布说明、逐文件索引及一致性说明同步开关、完整时间、小时静默检查与默认关闭的可选协议回退。后续明确需求分别覆盖原第6项的相对时间主展示与第16项的禁止切协议；兼容和回滚边界仍按上文 schema v2/备份 v2 说明。

- `run-build.ps1 portable`：本轮 Windows x64 release EXE 构建通过，文件版本 1.0.1、大小 15,020,032 字节，应用源码及前端资源均纳入新构建；未启动正式标识读取用户数据。第三方声明离线重新生成（37 个前端包、322 个 Windows crate），SHA-256 与生成前一致。旧包归档后使用原白名单脚本重新打包，逐条解压比较输入文件哈希，交付链接与独立校验和为准。

本轮未再次实测真实第三方模型、SMTP 投递、原生 WebView2 全部键盘/高对比度/多 DPI 组合、多屏与硬件休眠唤醒；浏览器和 Mock 回归不能替代这些检查。以下历轮记录的测试数字、截图和包不代表本轮构建。

## 前轮执行（规则与持久模型列表；本轮改动前）

- `npm test -- --reporter=dot`：114 项通过，8 个测试文件。新增多答案原文/Unicode 边界、默认改名/恢复、预设隔离、设置排序、卡片快捷绑定去重与失败恢复、外部绑定草稿合并、缓存更新失败/晚返回隔离，以及可编辑输入的键盘、输入法和过滤行为。
- `run-build.ps1 test`：60 项通过（37 单元、13 数据完整性、10 请求回归）。新增糖果一次迁移、多答案条件摘要/历史/备份往返，缓存重启/端点隔离/替换、100 份及 16 MiB 容量与清理失败回滚。使用隔离 SQLite 和本机 HTTP/SMTP，不操作用户正式数据库或凭据。
- `npm run build`：vue-tsc 与 Vite 生产构建通过；`run-build.ps1 portable`：最新 Tauri Windows x64 release EXE 构建通过。应用源码和前端资源已纳入新构建，不沿用旧便携包。
- `run-build.ps1 clippy`：所有 target 按 `-D warnings` 通过；隔离工具链 `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` 通过。
- CUA 实际浏览器：1280×720 核对糖果原题、名称、数量表与答案 `21颗`；700×520 iframe 检查多答案新增/输入/还原、主体滚动和固定底栏，视觉无横向溢出。真实 Widget 挂载虚构内存配置，两态测得 360×156 / 360×572，三个顶栏按钮坐标逐项一致；长规则名不挤压完整 `xhigh`。模型输入的当前 ID 不在缓存时仍能打开并选择候选；正式页面 computed style 确认输入内部无重复边框/阴影。该组件验收标签页未捕获 warn/error，临时 HTML 已移除。
- README、使用手册、变更日志、正式发布说明、逐文件索引及首版风格核对已同步默认名、多答案、缓存、恢复与快捷绑定行为，并明确 schema/备份 v2 的兼容与降级边界。历史版本说明不改写。
- `npm run format:check`、额外文档格式检查和 `git diff --check` 通过。82 个源码交付文件中 74 个文本严格 UTF-8 解码通过，无 BOM、CRLF 或替换字符；23 处本地 Markdown 链接有效，8 个 PNG/ICO 实际解码通过，无临时 QA HTML。版本清单与 EXE 均为 1.0.1，EXE 生成时间晚于应用源码。
- `node scripts/generate_notices.mjs` 使用隔离 Cargo 环境离线重生成 37 个前端包和 322 个 Windows crate 声明，结果 SHA-256 与原文件一致；首次普通终端缺 Cargo，补齐仅当前进程的工具链路径后重做成功，未修改系统环境。

交付包由本机 `work/qa/package-v101.ps1` 生成，固定收集 EXE、公开文档和许可证，逐个解压比对 SHA-256，拒绝数据库、凭据、PDB、日志、危险路径及重复条目；现有包先归档，最终包与独立校验和以本轮交付链接为准。

本轮尚未再次执行真实第三方模型/SMTP、全部原生 WebView2 键盘/缩放、多屏/DPI、休眠唤醒或登录自启的硬件端到端测试；浏览器布局和自动化回归不能代替这些验收。正式数据库与凭据未被读取或修改。以下保留前几轮执行证据，其测试数字、尺寸和包不代表当前构建。

## 历轮执行记录（本轮新增改动前）

- `npm test`：100 项通过，涵盖 Markdown 5、Widget 17、Settings 45、ModelPicker 6、DatePicker 14、导出 8、思考强度 5；新增入口与草稿保护、关于末位、邮件门禁，以及日期输入/选择/清空、键盘、闰日、年界、边界定位、宿主快捷键与监听清理回归。
- `npm run build`：Vue/TypeScript 检查及 Vite 生产构建通过，无 CDN。
- `npm run format:check`：通过；Rust 使用隔离工具链执行 `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`，通过。
- `run-build.ps1 portable`：本轮 UI 修复完成后重新执行 Tauri Windows x64 release 免安装 EXE 构建，产物为 `work/cargo-target/release/model-probe.exe`；包含新的深色日历及完整数字框，旧构建不再作为本轮交付包。
- 本地打包：`release-artifacts/Model-IQ-Radar-v1.0.1-Windows-x64-Portable.zip`，配套 `SHA256SUMS.txt`。包含 EXE、说明文档及许可证；不包含数据库、凭据、开发依赖或 PDB。压缩条目与源文件逐个比对 SHA-256，验证无危险路径和意外文件；未启动正式标识的 EXE 读写真实用户数据，未提交或发布。
- 本轮通过 CUA 在实际浏览器检查 897×884 六个设置页，关于保持末位、底栏可见且无横向溢出。另用 700×520 iframe 验证新增入口和日期弹层：完整日历在滚动主体内移位，日期、焦点、今天/清空和固定底栏均可见；右侧弹层向左避让。实际点击选日、手动输入、两端切换、方向键和 Escape，并复验 0001 年边界禁用；日历背景为 `#1a2b3e`、边框为 `#58769875`，与首版菜单一致。截图位于本机 `work/screenshots/v101-creation-consistency`，不把自动化组件测试当作视觉检查。
- `run-build.ps1 clippy`：`--all-targets -- -D warnings` 通过。
- `run-build.ps1 test`：54 项通过（35 单元、9 数据完整性、10 请求回归），涵盖 Rust 单元、本机 HTTP/SMTP、非法配置拒绝、首版 101 配置启动兼容，以及最近记录索引重建与失败回滚。旧库用例先在修复前复现失败，修复后通过。
- `node work/qa/v101-second-audit.mjs`：Edge/Playwright 以虚构内存数据检查 700×520、1120×900、1920×1080 六页，共 18 组，并覆盖模型保存及邮件保存后的“已提交但读取失败”恢复/重读路径。确认两类保存均只提交一次、编辑被冻结、密码草稿清空、恢复入口可用；700×520 底栏底部分别为 519px，无横向溢出。长内容只滚动主体。两态宽 360、高 124/540，三个顶栏按钮坐标完全一致；回答盒 332px 宽、195px 高，长表格/代码内容没有撑宽盒子；无页面异常。
- `node work/qa/v101-save-warning.mjs`：在最新预览契约上再次执行上述 18 组布局及保存恢复路径，并模拟事务已提交后原生恢复失败的 warning。确认偏好只提交一次、离页无草稿对话框、警告跨页保留；700×520 无横向溢出、底栏底部 519px，两态按钮坐标与回答盒尺寸一致，页面异常为 0。已查看警告截图；这是模拟后端警告，未实际破坏原生窗口操作。
- `npm audit --omit=dev --registry=https://registry.npmjs.org --json`：生产依赖已知漏洞报告为 0。本机默认镜像不支持 audit 接口（404），实际改用本次命令参数指定官方源，未修改 npm 全局配置；此结果不代表 Rust 或开发依赖已接受在线漏洞审计。
- 本地 Markdown/HTML 相对文档链接检查通过，8 个图标/截图可解码；版本清单一致为 1.0.1，逐文件审阅索引与源码同步。
- 许可证脚本实际生成：37 个前端包、322 个 Windows crate（包含构建依赖）；新增 SMTP/hash 依赖来自锁文件，未手工修改生成结果。
- `git diff --check` 通过；当前 80 个源码交付文件中 72 个文本文件通过严格 UTF-8 解码、无 BOM 与 LF 检查，23 处 Markdown 本地链接有效。第三方声明重新生成字节一致；EXE 检查为 AMD64、文件版本 1.0.1，生成时间晚于最新应用源码。源码/测试/文档同步，未保留临时布局 HTML。

隔离 MSVC/Rust 工具链、历史浏览器脚本及本轮 CUA 截图位于本机 work 目录，不纳入源码。仓库包含可重复执行的 Vitest、Rust 与本机 Mock 回归；临时布局 HTML 在交付前移除。

## 尚未实测的边界

- 未调用真实第三方模型、发送真实邮件或使用真实用户凭据；本机 SMTP 验证服务未提供 STARTTLS 时拒绝发送账号和授权码，但不能代替各服务商的连通性验收。
- 真实 SMTP 并发投递仍需服务商端到端验收；当前每封使用独立连接，无全局忙碌拦截，无自动重试或历史补发，发送失败不影响测试结果。
- 全套物理多屏/DPI、屏幕热插拔、系统睡眠/唤醒及断网恢复未进行硬件端到端遍历；已覆盖确定性时钟、门禁与纯坐标边界。
- 前一轮原生隔离验收使用独立 identifier 与虚构 SQLite 数据：启动顶部吸附收为图标、恢复卡片、标题栏单击无位移后仍可收回、打开设置和历史筛选入口、JSON 实际下载并核对原始 Markdown、Alt+F4 弹出草稿保护、保存后窗口销毁并核对持久化，均已执行。本轮新增恢复态与关闭失败主要由组件/真实浏览器覆盖，未再进行原生实机复验；已有原生验收不代表所有 DPI/拖动路径已遍历。
- 本轮深色日历已在实际浏览器验收并构建进桌面 EXE，尚未再次启动正式标识的桌面程序验收 WebView2 中的全部键盘/缩放路径，避免读写真实用户数据库；不同系统语言及屏幕阅读器的完整组合未遍历。
- 浏览器在临时 iframe 验收阶段捕获一条不带源码地址的 `MutationObserver.observe` 参数类型错误；正式页面六页切换和日期复验未再次出现。应用源码未直接使用 MutationObserver，生产 Vite 预加载代码会观察 document；现有证据不足以确定该日志来源，不盲改框架或工具代码，也不声称浏览器日志完全无错误。若独立 WebView2 仍能复现，需补充调用栈定位。
- 项目不是 Java/JVM/MyBatis 项目。已阅读指定 Java 手册并采用通用安全、命名、注释和测试原则，语言专属规则按 Rust/TypeScript 规范执行。
