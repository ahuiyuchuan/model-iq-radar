# 测智雷达 v1.1.0 验证记录

## v1.1.0 发布前检查（2026-10-31）

- 范围：按用户批准的文案发布桌宠与皮肤管理。README重组为正式功能介绍、启用方式、交互表和下载入口，移除“尚未发布”及开发排错叙述；CHANGELOG以相对v1.0.6的新增能力编写，不把开发期间的问题列为发布修复。新增 `docs/release-notes/v1.1.0-zh.md`，同步使用手册、贡献指南及当前版本说明。
- 版本：`npm version 1.1.0 --no-git-tag-version --ignore-scripts`更新npm清单／锁文件；Cargo和Tauri清单同步，Cargo锁文件由离线metadata重新生成，仅根包版本变化。`node scripts/generate_notices.mjs`离线重建第三方声明，37个前端包与315个crate，第三方许可证正文不变。没有新增依赖、配置或数据库迁移。`.gitignore`补充Python字节码缓存，原缓存和所有旧交付物保留在本机，不纳入提交。
- 自动检查：v1.1.0下重新执行 `npm test -- --reporter=dot`，335项／12文件通过；隔离 `run-build.ps1 -Action test`，111＋18＋22共151项通过。`npm run format:check`及专项文档／脚本Prettier、`cargo fmt --check`、`run-build.ps1 -Action clippy`全目标-D warnings通过。沿用项目Vue／TypeScript／Rust规范，Java/JVM/MyBatis专属要求不适用。
- 完整浏览器重跑：`node scripts/verify_pet_ui.mjs`，`C:/tmp/model-iq-radar-v110-release-20261031-r1/`，89主截图＋12顶部裁切，三尺寸三边、四种业务结果、十二姿态、48次快速状态、菜单／选择器、云朵、默认动作预览保存、换肤及HTML回归通过；0页面错误／远程请求／矩形拒绝。67.28秒录像 `video/page@5db067ccc03aed69170bc7be622a5c32.webm`，另导出20时刻总览与采样JSON。README桌宠截图直接取本轮 `pet-details-192.png`，展示确认云朵后的真实侧坐详情；其他经典预览保留其v1.0.4来源标识，不冒充本轮模型测试。
- README检查：本机离线Markdown渲染，本地图片无缺失、1200px视口无横向溢出，已查看顶部与桌宠预览，证据 `readme-preview.png`／`readme-top.png`在上述目录；网络徽章仅用本地占位验证排版，不声称已联网验证徽章。首次临时预览脚本因Windows路径斜杠比较错误拒绝静态资源，修正该验收辅助脚本后通过，README资源本身没有缺失。README相对文件链接核对通过。
- 生产构建：复用既有隔离MSVC／Windows SDK，`run-build.ps1 -Action build`完成TypeScript／Vite与Windows x64 release，Rust编译3分56秒；仅既有chunk体积与链接器导入库提示。最终dist执行 `node scripts/verify_pet_csp.mjs`，12动作解码、待机出画、坏资源阻止保存及经典回退通过，0页面错误。未启动正式应用、访问正式凭据、调用真实模型或发送邮件。
- 发布输入：只交付 `Model-IQ-Radar-v1.1.0-Windows-x64.exe`，由 `package_exe.ps1`校验版本、PE x64、生产输入新鲜度与复制摘要。代码、Tag、Release和EXE版本统一为v1.1.0；EXE为19,236,864字节，SHA-256 `b3769a7ca386116148e157b01c8e043bbe6886b3a20bc1b9e0b0fedad3596ef4`，文件／产品版本均为1.1.0，与release构建源摘要一致。GitHub自动生成的Source code归档不属于手工上传附件。
- 安全与范围：提交候选源码／文档严格UTF-8无BOM／LF及替换字符检查通过；191份仓库候选文件凭据模式扫描0匹配，无超过50MiB文件，源图／脚本与提取记录一并保留以便追溯。Git diff检查通过；不提交生产凭据、数据库、EXE、依赖缓存或临时验收录像。
- 未覆盖：原生窗口穿透／移动、物理多屏DPI、真实重启位置、偶发顶部合成残块和长期CPU／内存仍未实机验收；现有额外姿态为静态原画与短溶解，未制作连续全身动画。下文顶部锚点及菜单等专项的最终代码与本轮业务源码一致，历史专项结果保留，不把旧脚本记录表述为此次逐一重跑。

## 顶部锚点恢复与本轮全面复查（2026-10-31）

- 范围：处理右侧角色拖至顶部收起、悬停展开后位置错误，并按用户要求再次全面检查。保留全部既有未提交工作；不更换二维资源、渲染方案或数据库。最后一次生产修改仅在 `src-tauri/src/pet_window.rs`，对应组件回归为 `tests/pet-stage.test.ts`，新增 `scripts/verify_pet_top_restore.mjs`。这一轮累计改动大纲、文件职责和兼容性见 [DESKTOP_PET_CHANGE_SUMMARY](docs/DESKTOP_PET_CHANGE_SUMMARY.md)。Java/JVM专属规范不适用于当前Vue/TypeScript/Rust栈，遵循项目注释和格式约定。
- 证据与根因：原位置算法在进入探头时直接保留窗口原点；人物处于卡片右侧时，该原点比角色锚点偏左数百逻辑像素。恢复又读取旧normal原点，未以实际可见探头位置恢复。提取同一位置数学逻辑后新增Rust回归，修改前过滤运行 `right_actor_top_peek --lib` 实际失败在进入后的横向锚点断言，非仅凭截图猜测。
- 修复：纯函数 `transition_position` 在顶部进入时按前后角色逻辑锚点差×DPI换算物理横坐标；从顶部恢复时使用当前探头横坐标及前后锚点差。纵向、左右贴边、拖动独占与最终 `scene_position` 工作区夹紧沿用原规则。角落云朵夹紧后不返回过期卡片原点；完整卡片空间不足仍作必要向内夹紧，不承诺所有角落完全不移动。无配置、依赖、皮肤契约或数据库迁移；回滚可切回经典后使用保留旧EXE，未覆盖旧交付物。
- 自动测试：`npm test -- --reporter=dot`，12文件335项通过；左右方位回归补充两次完整顶部收纳／悬停恢复，核对提交锚点、主体范围、吸附范围及侧坐姿态。隔离 `run-build.ps1 -Action test`，111库＋18数据完整性＋22回归，共151项通过；新增真实生产几何测试覆盖100/125/150/200%数学缩放、负原点工作区及角落探头恢复，不冒充物理跨屏。`npx vue-tsc --noEmit`、`run-build.ps1 -Action clippy`全目标-D warnings、`cargo fmt --check`、项目与专项Prettier、`git diff --check`通过。
- 顶部专项：`node scripts/verify_pet_top_restore.mjs`，证据 `C:/tmp/model-iq-radar-top-anchor-20261031-r1/`。真实组件与鼠标拖动启动，使用原生等价原点／工作区夹紧的浏览器替身；左右人物×96/144/192尺寸×顶部中间／右角共12组，36次收纳恢复通过，右侧恢复锚点一致，三次横坐标无累计漂移，角色原侧不变。192用展开详情，其余用紧凑卡片；全程保留running云朵。24张前后截图、`result.json`含每次转换前后锚点，0页面错误／远程请求／矩形拒绝；此脚本不移动真实OS窗口。
- 完整浏览器：`node scripts/verify_pet_ui.mjs`，`C:/tmp/model-iq-radar-top-anchor-full-20261031-r1/`，89主截图＋12顶部裁切通过。覆盖四状态、取消／HTML／条件过期、三尺寸三边、十二动作点播、48次快速业务切换、慢速斜向与键盘菜单、选择器真实点击、测试／取消／倒计时、默认动作预览保存、HTML预览恢复、换肤异常回退。0页面错误／远程请求，矩形拒绝列表为空。截图和录像抽帧已查看；快速切换仍使用现有96ms短溶解，不把其混合帧解释为新制作连续动作。
- 菜单专项：`node scripts/verify_pet_picker.mjs`，`C:/tmp/model-iq-radar-top-anchor-picker-20261031-r1/`。24次选择器、4种气泡确认动作、5种菜单布局、三侧主动动作与回答中打开详情、二级自动关闭、4次经典换肤往返通过；记录0拒绝／0错误，保持下拉菜单无阴影。
- 贴边专项：`node scripts/verify_pet_dock.mjs`，`C:/tmp/model-iq-radar-top-anchor-dock-20261031-r1/`。静止指针下收纳稳定，三侧×动画开关共18轮，6次直接点击、6次探头云朵拖动与完整／展开云朵短按、小抖动、长按拖动均通过；0错误。
- 动态证据：顶部专项47.56秒 `video/page@806a7f633eea618ecfcbb24e6075f237.webm`；完整UI61.16秒 `video/page@420689ef5ae55aefff4eacc80bc47c63.webm`；菜单68.28秒 `video/page@cd6c3c71ea8fadde0683f0b1c3a024eb.webm`；贴边29.36秒 `video/page@755b92248a42e9fdb24c6c3e902d8abe.webm`。各目录均输出20个采样时刻的 `video-frames.json` 与 `video-contact-sheet.jpg`；已查看顶部／完整／菜单抽帧总览及右侧192右角恢复原截图，保留视频供连续回放。录屏及有限抽帧不能证明所有中间帧、物理窗口与长期性能均无问题。
- 构建与交付：复用既有隔离MSVC／Windows SDK，`run-build.ps1 -Action build`完成TypeScript／Vite及Windows x64 release，Rust编译2分36秒；仅既有chunk体积和链接器导入库提示。最终dist执行 `node scripts/verify_pet_csp.mjs`，12动作解码、待机出画、坏资源阻止保存与经典回退通过，0页面错误。`package_exe.ps1 -OutputDirectory release-artifacts/desktop-pet-top-anchor-test-20261031`校验版本、PE x64、源输入新鲜度和复制摘要通过。新包 `D:/other/ahui/model-iq-radar/release-artifacts/desktop-pet-top-anchor-test-20261031/Model-IQ-Radar-v1.0.6-Windows-x64.exe`，19,236,864字节，SHA-256 `226cf773c711f77f094110837fdbab1bb4d46a6289f6516bc75d0c8a0d42b27e`，与release源一致；构建后只整理QA与文档，未改生产源码、未启动正式程序、未提交／发布。
- 文档与安全：README、USER_GUIDE、DESKTOP_PET_DESIGN、SKIN_PACKS、CHANGELOG、本记录与新增改动大纲同步。检查本轮UTF-8无BOM／LF和替换字符，源码／脚本／文档／素材提示词凭据模式扫描0匹配。保留原有修改、历史证据和无关文件，未删除用户数据。
- 未覆盖／已知限制：依用户要求仅浏览器验收，未启动原生界面。Win32实际移动／穿透、多屏物理DPI、真实重启位置、偶发顶部合成残块及长期CPU／内存仍未实机覆盖；浏览器替身和Rust数学回归不能取代这些检查。未调用正式模型或邮件。现有额外姿态仍主要是静态图＋短溶解，未制作连续全身动画或Live2D绑定，不能承诺与连续透明动画素材相同的运动质量。

## 全动作默认外观与下拉菜单去阴影（2026-10-31）

- 用户明确要求放开默认待机选择，覆盖此前仅五种日常姿态的限制；同时去除截图中模型／规则下拉菜单的外侧阴影。保留现有未提交工作，不调整角色资源、渲染方案、业务请求或动画交接。Java专属规范不适用于当前Vue／TypeScript／Rust，沿用现有注释与格式约定。
- 根因与修改：`SkinManager.vue` 使用五种日常姿态白名单，`src/pet/types.ts`／`controller.ts` 和Rust `conditions.rs` 同样限制保存值，单改前端会造成保存失败。本轮前后端统一接受契约内全部十二动作，设置仅列当前皮肤实际有资源的动作；缺字段／缺资源仍回退idle。常态可为摸头、悬空、测试／结果姿态，但仅绘图，不派发输入、修改业务结论或发出请求；实际拖动、测试反馈、卡片侧坐仍优先。配置字段与包版本不变，回滚到仅支持五种姿态的测试版前先保存普通待机；更早不认识字段的版本仍按原规则忽略它。
- 阴影：`src/style.css` 共用 `.model-menu` 的外阴影和内高光改为none，覆盖接口／模型／规则／强度与设置共用下拉；保留背景、边框、选中态、滚动和焦点，不修改其他控件阴影。
- 测试：`pet.test.ts` 检查十二动作准入与业务优先级；`pet-stage.test.ts` 新增十二种默认姿态的真实组件挂载，验证尚未测试结论不变且不调用真实拖动／开始测试；`data_integrity.rs` 对新增结果姿态执行备份与隔离SQLite关闭重开，并验证十二白名单及非法值拒绝。`verify_pet_ui.mjs` 逐一预览十二种默认动作，`verify_pet_picker.mjs` 在24组真实下拉打开路径检查computed boxShadow为none。
- 自动检查：`npm test`12文件335项、`npx vue-tsc --noEmit`、项目与专项Prettier、`cargo fmt --check`、`git diff --check`通过；16份本轮文件UTF-8无BOM／LF及凭据模式检查无异常。复用隔离MSVC／SDK的 `run-build.ps1 -Action test`，Rust110＋18＋22共150项通过；`-Action clippy`全目标-D warnings通过。README、USER_GUIDE、DESKTOP_PET_DESIGN、SKIN_PACKS、CHANGELOG同步全部动作、业务隔离、去阴影和旧测试版回滚条件。
- 完整浏览器：`verify_pet_ui.mjs`输出 `C:/tmp/model-iq-radar-all-idle-full-20261031-r2/`，89主截图＋12顶部裁切、十二种默认动作逐一预览、保存／返回回显、三尺寸与三边／业务状态等流程通过，0页面错误／远程请求、矩形拒绝记录为空。完整录像为同目录 `video/page@1e9097fd30d08ae014a9b02bcffaa84c.webm`。r1在顶部恢复后抽样期间鼠标仍留原探头坐标，实际落在透明空白并合理再次收纳，脚本随后等待卡片超时；保留失败证据，r2在恢复后立即把真实指针移入卡片，重跑通过，无产品状态逻辑修改。
- 专项浏览器：`verify_pet_picker.mjs`，`C:/tmp/model-iq-radar-no-shadow-picker-20261031-r1/`，24组下拉实际打开后的computed boxShadow均为none；5种菜单空间、三边手动动作及菜单详情、确认与换肤恢复通过，无页面错误、远程请求或矩形拒绝。已实际查看picker-3.png，菜单边缘没有外阴影。录像为同目录 `video/page@619f27e471aae3933fb51f5cb2810ec0.webm`。
- 新选择保存补查：临时 `verify-all-idle-saved.mjs`确认选项12个、测试失败姿态预览、保存快照idleAction=error、离开分类再返回保持该选择，0页面错误；截图 `C:/tmp/model-iq-radar-all-idle-full-20261031-r2/settings-all-idle-saved.png`已查看。后端隔离SQLite重开也验证该新增值。最终dist的 `verify_pet_csp.mjs`十二动作解码、待机、损坏资源阻止保存与经典回退通过，0页面错误。
- 构建与交付：`run-build.ps1 -Action build`，TypeScript／Vite与Windows x64 release通过，Rust编译2分54秒，仅既有chunk体积和链接器导入库提示。`package_exe.ps1 -OutputDirectory release-artifacts/desktop-pet-all-actions-no-shadow-test-20261031`校验版本、PE架构、输入新鲜度和复制摘要通过；未覆盖旧交付物。EXE为 `D:/other/ahui/model-iq-radar/release-artifacts/desktop-pet-all-actions-no-shadow-test-20261031/Model-IQ-Radar-v1.0.6-Windows-x64.exe`，19,237,888字节，SHA-256 `354b7104352cbbf7a1679c1086a1b7de0717dbafb4fee9d1039571f6ab554481`，与release源一致。构建后未修改生产源码；未启动正式EXE、提交或发布。
- 边界：按用户要求只做隔离浏览器界面验收，不读取正式凭据，不请求真实模型或发送邮件；原生合成、点击穿透、物理拖拽、跨屏DPI及长期驻留性能不在本轮结论内。

## 菜单避让、详情等高、默认待机与回答进度修复（2026-10-31）

- 范围：合并菜单遮挡角色、HTML与文字详情统一缩短50px、皮肤默认待机动作，以及回答中展开面板后贴边进度气泡消失四项需求。保持现有二维PNG／Canvas方案，保护所有既有未提交工作，无素材或渲染引擎变更。按用户要求仅浏览器验收，不访问正式凭据或真实模型／邮件。Java专属规范不适用于本项目Vue／TypeScript／Rust。
- 菜单根因：仅按工作区夹紧整棵菜单，靠底边时上移穿过人物与云朵。`contextMenuLayout` 生成上下／左右候选，整体评估工作区溢出、完整角色画布与云朵重叠、移动距离和二级方向，继续复用会话预留几何；96／144／192和左右角色、角落回归通过。极小工作区不能保证容纳整棵菜单，按最少越界／重叠回退；没有引入窗口平移之外的新机制。
- 高度：`src/style.css` 桌宠文字、HTML预览、原始回答统一546px，以此前文字596px为基准缩短50px；`PetStage.vue` 同步默认测量值，测试同步实际尺寸。经典仍572px，紧凑仍176px，回答区吸收差值，选择器、时间信息、底部按钮、放大预览保留。已查看实际文字和HTML截图，按钮与说明未裁切。
- 默认待机：`SkinManager.vue` 仅列当前皮肤实际提供的idle／seated／wave／stretch／bow日常动作，选择立即预览、保存后应用；`src/pet/types.ts` 新增可选idleAction，`controller.ts` 和PetStage在更高优先级动作缺省时采用。Rust `SkinPreferences.idle_action` 缺省idle，`conditions.rs` 对所有保存与导入入口校验白名单；旧配置兼容、缺资源运行时回退、不更改条件指纹。`data_integrity.rs` 验证备份、重置与隔离SQLite关闭重开后的持久化；`pet_window.rs` 验证旧字段缺省。旧版本可忽略此字段，旧程序再保存可能丢失该选择，无数据库／皮肤包／备份版本升级。
- 回答进度根因与证据：点击云朵展开面板记录dismissedCloud，自动收纳复用同一确认身份，导致卡片和进度提示同时不可见。修复前左／右／顶部三项新增回归均在“探头云朵应显示”断言失败。现在任务进行中且docked时强制显示云朵，包含命中与吸附范围；恢复面板保留原确认状态，新结果重新显示。不重启／取消任务、不额外请求。测试明确检查v-show样式；真实可见性由隔离浏览器三边完整交互验证，不能只凭DOM文字判断通过。
- 自动检查：`npm test`12文件323项通过；`npx vue-tsc --noEmit`、Prettier、`cargo fmt --check`、`git diff --check`通过。复用隔离 `run-build.ps1 -Action test`，Rust110＋18＋22共150项通过；`-Action clippy` 全目标-D warnings通过。最初测试误用未安装的tempfile，改为已有uuid与标准库临时路径后重跑通过，未新增依赖；TS测试夹具的字面量类型已修正并实际重跑静态检查。
- 专项浏览器：`verify_pet_picker.mjs`，`C:/tmp/model-iq-radar-menu-avoid-20261031-r2/`，24次选择器、5种菜单空间、4种气泡确认、3侧手动动作及菜单详情、4次经典换肤通过，rejectedScenes为0、无页面错误或远程请求。已查看menu-bottom-right／menu-bottom-left实际图片，菜单避开角色和云朵。r1脚本固定鼠标坐标落入上移后的菜单，错误地等待自动关闭；r2使用人物中心作为菜单外目标，保留旧记录。
- 完整浏览器r2：`C:/tmp/model-iq-radar-menu-idle-full-20261031-r2/`，89主截图＋12顶部裁切，三尺寸／三边／十二动作、48次快速状态、菜单慢速斜向及键盘、HTML放大恢复／源文等高、设置默认待机预览保存通过，0页面错误／远程请求，原生等价矩形拒绝记录为空。已查看文字与HTML详情和右／顶部已确认回答中的收纳截图。最终r3增加关闭临时互动提示时进度可见、设置页离开重进后的默认姿态回显和选项截图，89主截图＋12顶部裁切全部通过，结果见 `C:/tmp/model-iq-radar-menu-idle-full-20261031-r3/result.json`。录像为同目录 `video/page@3f9c15890bafc5329c57217ca09abdd2.webm`；r3完整76.56秒、r2完整66.6秒与专项69.72秒录像均已抽20帧生成并实际查看总览；帧图位于各自video-frames目录。
- 文档同步：README、USER_GUIDE、DESKTOP_PET_DESIGN、SKIN_PACKS、CHANGELOG说明默认待机配置、兼容性、云朵确认例外、546px统一高度和菜单避让；旧验证章节保留历史事实。
- 设置视觉补查：选择器统一深色输入外观，300×36px；临时专项 `verify-default-idle-styled.mjs` 选择鞠躬、预览、保存、离开设置分类再返回均通过，0页面错误，截图 `C:/tmp/model-iq-radar-menu-idle-full-20261031-r3/settings-default-idle-styled.png`。最终dist的 `verify_pet_csp.mjs` 12动作解码、动画、坏资源阻止保存与经典回退通过，0页面错误。
- 构建与最终包：复用 `run-build.ps1 -Action build`，最终TypeScript／Vite及Windows x64 release通过，Rust编译2分22秒，仅既有chunk体积和链接器导入库提示。第一次构建后补查设置选择器外观，重新完整构建后才交付。`package_exe.ps1 -OutputDirectory release-artifacts/desktop-pet-menu-idle-test-20261031` 校验版本、PE x64、输入新鲜度及复制摘要通过，不覆盖旧交付物。新EXE为 `D:/other/ahui/model-iq-radar/release-artifacts/desktop-pet-menu-idle-test-20261031/Model-IQ-Radar-v1.0.6-Windows-x64.exe`，19,237,888字节，SHA-256 `dcb212ee49bba2cc189cc8a9ee8dc879a8a94841403ffd37baab15f7ac1e36c4`，与release源一致。未启动正式EXE、未提交或发布。
- 收尾：21份本轮源码、测试、QA、文档均严格UTF-8无BOM／LF，未发现替换字符或凭据模式；Prettier与git diff --check通过。保留工作区既有改动和文件；除上述范围不作清理。注释、配置契约及验证记录已同步。
- 未覆盖：按用户要求未验收原生界面合成残块、物理点击穿透／拖动、真实跨屏DPI；未使用正式模型／SMTP，未做长期CPU／内存驻留观察。静态姿态交接仍沿用现有短溶解，不代表制作了连续肢体动画。回滚可保存经典外观并使用保留的旧EXE。

## 二级自动收起、手动动作与详情越界回退修复（2026-10-31）

- 范围：处理用户连续反馈的二级菜单不自动关闭、贴边手动动作被探头抢占、测试中出现“桌宠角色或吸附轮廓超出场景”并退回经典的问题。保持二维素材和既有技术方案，保护全部未提交工作；只在隔离浏览器与本地替身验收，不读正式凭据、调用模型或发送邮件。Java专属规范不适用，按项目Vue／TypeScript规范实现。
- 二级菜单根因：此前为解决移入途中误关，取消了离开后的关闭逻辑，变为只能点击／键盘退出。本轮采用唯一600ms计时器：移到其他一级项或离开整棵菜单后只收二级，进入动作入口／子菜单取消计时，子菜单持有键盘焦点时不关闭；父菜单关闭、换肤与卸载清理。保留上轮两层预留几何，二级自动关闭不缩放窗口。
- 手动动作根因：原180ms收纳计时不区分主动动作，菜单点播后菜单消失即可将2600ms姿态改为探头。`PetStage.vue` 新增进程内手动呈现所有权，点播／摸头期间暂停收纳，动作结束重新判断；拖动、隐藏、面板切换、换肤清空姿态时释放，不延长模型任务。测试结算不抢占用户动作，真实结果仍更新，结束后再读取当前结论。
- 越界已复现：保留菜单退场几何的两帧内，从角色菜单打开596px详情，提交场景373×480，却包含右缘415／底缘709的卡片；Rust据真实边界拒绝并触发经典回退。修改前新增用例实际失败并记录该场景。`fitSceneLayout` 在保留菜单坐标期间纳入当前卡片高度／浮层，必要时统一平移负坐标并扩展场景，人物与卡片相对位置不变；没有放松Rust校验或吞掉错误。四组左右方位／176和596高度回归均通过。
- 修改文件：`PetStage.vue`、`src/pet/layout.ts`、`tests/pet-stage.test.ts`；`pet-preview-bridge.ts` 新增与Rust一致的矩形边界拒绝检查和拒绝场景记录，`verify_pet_picker.mjs`／`verify_pet_ui.mjs` 增加真实交互与跨页面检查。只修改上述生产前端和对应验证／文档，不改Rust、依赖、皮肤契约或数据库格式。README、USER_GUIDE、DESKTOP_PET_DESIGN、SKIN_PACKS、CHANGELOG同步实际行为。回滚可选择经典后换保留的旧EXE，不覆盖旧交付物。
- 自动检查：`npm test -- --reporter=dot`，12文件315项通过；菜单关闭和三侧动作在修复前新增四项实际失败，修复后通过。`npx vue-tsc --noEmit`、项目及专项文档／脚本Prettier、`git diff --check`通过。Rust无本轮修改，未重复执行其150项测试／Clippy；上节结果为历史检查，不冒充本轮重跑。
- 专项浏览器：`node scripts/verify_pet_picker.mjs`，输出 `C:/tmp/model-iq-radar-menu-hold-20261031-r2/`。24次选择器、四种气泡确认、三种菜单选边场景全部通过；真实鼠标停留其他一级／离开菜单自动收二级、键盘子菜单保护通过，1510个可见菜单矩形采样无越出视口。左／右／顶部手动招手在700ms仍显示完整动作、结算后500ms仍保持招手，播完后正常收纳。另三次“测试中收起详情→菜单打开详情”无经典回退，拒绝场景记录为空，三侧气泡与四次经典换肤回归通过，0页面错误／远程请求。
- 专项证据：上述目录的 `result.json`、`menu-frames.json`、`manual-edge-left.png`／`manual-edge-right.png`／`manual-edge-top.png`；61.2秒录像 `video/page@cce01d89c8f947200288ab72d77e0342.webm`，已抽20帧检查总览 `video-frames/contact-sheet.jpg`，并查看左右／顶部动作实际截图。r1脚本直接右击正在被悬停恢复移除的探头，未可靠打开菜单而超时；r2按真实流程先移动恢复角色，再右击完整角色，保留失败记录，不将旧脚本失败冒充产品已通过。
- 全面浏览器：`node scripts/verify_pet_ui.mjs`，`C:/tmp/model-iq-radar-menu-hold-full-20261031-r2/`，86主截图＋12顶部裁切，三尺寸、三边、十二动作、48次快速状态、菜单慢速斜向与键盘、测试／取消／倒计时／条件过期、HTML与换肤恢复通过，0页面错误／远程请求。悬浮页与设置页分别检查拒绝矩形列表为空；61.6秒录像 `video/page@cd43c267b191349df9395b5a7e98d794.webm`，已抽20帧查看 `video-frames/contact-sheet.jpg`。full-r1在所有交互完成后错误地于context关闭后读取桥接记录，脚本报页面关闭；r2把读取移到页面切换及关闭之前，重跑完整流程通过，保留旧失败现场。
- 生产与构建：最终dist执行 `node scripts/verify_pet_csp.mjs`，12动作解码、待机、损坏资源阻止保存和经典回退通过，0页面错误。复用隔离 `run-build.ps1 -Action build`，TypeScript／Vite与Windows x64 release通过，Rust编译2分32秒，仅既有chunk体积及链接器导入库提示。`package_exe.ps1` 校验版本、PE x64、输入新鲜度和复制摘要通过；新包 `D:/other/ahui/model-iq-radar/release-artifacts/desktop-pet-menu-hold-test-20261031/Model-IQ-Radar-v1.0.6-Windows-x64.exe`，19,229,184字节，SHA-256 `a352d8cadf1900227dc7e6e345b9b43b574e3c255437021fab8c30d387a87d76`，与release源摘要一致。构建后只整理QA／文档，未改生产源码；未启动正式EXE、提交或发布。
- 收尾：检查工作区与diff，保留所有既有改动和无关文件。12份本轮源码、测试、QA及文档严格UTF-8无BOM／LF、替换字符及凭据模式检查均无异常；最终文档Prettier和 `git diff --check`通过。
- 未覆盖：按用户要求不做原生界面验收；原生合成残块、窗口实际穿透／移动、物理跨屏DPI与位置重启恢复仍不在浏览器结论内。业务使用替身，未检查真实模型／邮件或长期CPU／进程内存。现有短溶解不是新增连续动作制作；模拟120ms异步宿主调整的录像包含主动快速切换过程，稳定内容与矩形校验不能替代原生每帧合成验收。

## 气泡确认复位、角色蓝框与菜单交接修复（2026-10-31）

- 范围：继续当前二维 PNG／Canvas 方案，不生成素材或改角色设计。保留所有既有未提交工作；隔离浏览器只使用本地内存替身，不读取正式凭据、调用真实模型或发送邮件。Java/JVM/MyBatis 专属规范不适用于本栈，按项目 Vue／TypeScript／Rust 注释与格式规范执行。
- 气泡根因与复现：此前只比较气泡确认身份来隐藏文案，`chooseAction` 仍接收同一结果姿态与 busy 工作姿态。新增四状态回归在修改前分别得到 success／failed／error／working，而期望 idle，实际四项失败后才修复。`PetStage.vue` 的结果姿态和短暂庆祝反馈现在与该确认身份共用呈现生命周期；点击后面板侧坐、收起待机，轮询、摸头／拖动收尾不复活旧表情。新记录／任务恢复真实表情，确认 running 不取消实际任务，也不修改卡片结论。
- 蓝框根因：全局 `button:focus-visible` 在透明角色画布上绘制矩形轮廓，超出宿主后被截成蓝色折线。角色按钮局部取消 outline，以透明角色轮廓柔光保留键盘焦点提示；菜单按钮和其他控件的键盘焦点样式保留。
- 菜单根因：原场景将一级和二级菜单一并按人物方向反射，固定右侧角色就向左；actionMenu 显隐直接参与场景宽高，DOM 先变、异步窗口后变，产生加载／销毁裁切帧。新增 `contextMenuLayout` 按工作区剩余空间优先右侧、右侧不足向左，底边向上避让；一级菜单会话预留两层范围，宿主确认后显示，二级显隐只改变命中掩码；整棵菜单移除并经过绘制后再缩窗。代次校验防止取消／换肤／卸载后的迟到回调重开，菜单已有焦点时不被就绪回调抢回首项。
- 原生适配：新增只读 `get_pet_menu_bounds`，限 widget 窗口，复用实际工作区、窗口原点和 DPI 换算，返回左／上／右／下逻辑边界，允许负值；不读写业务数据。`lib.rs` 注册命令，QA 桥接仅模拟边界与宿主延迟。无依赖、数据库、配置或皮肤格式迁移，旧皮肤无需重做；回滚选择经典外观后换保留的旧 EXE，前端与原生程序须配套。
- 自动检查：`npm test -- --reporter=dot`，12文件307项通过，新增四种确认表情／摸头／拖动收尾、新结果恢复、左右角色菜单选边、宿主等待、二级稳定几何与取消迟到回调回归。`npx vue-tsc --noEmit` 通过。隔离 `run-build.ps1 -Action test`：110+18+22共150项通过；`-Action clippy` 全目标 `-D warnings` 通过，rustfmt 检查通过。只使用既有 MSVC／Windows SDK 环境。
- 专项浏览器：`node scripts/verify_pet_picker.mjs`，`C:/tmp/model-iq-radar-ack-menu-20261031-r2/`。24次三尺寸／左右选择器打开、4种确认动作、3种菜单空间、气泡位置、左右方位和4次经典／桌宠切换通过。模拟120ms宿主扩高，菜单出画／关闭累计采样268个可见菜单矩形，无越出视口裁切帧；二级打开和关闭的视口尺寸不变。截图实际检查了右展开、左展开、底边避让与确认后待机，无透明画布矩形焦点框。0页面错误／远程请求；41.76秒录像 `video/page@b195eeadc3e793d0e6d60683d212d3d0.webm`，抽20帧检查总览 `video-frames/contact-sheet.jpg`，逐帧几何为 `menu-frames.json`。
- QA修正记录：专项 r1 的换肤检查沿用固定300ms等待，在新增异步布局队列尚未完成时读取328px旧高度而失败；r2改为等待真实宿主高度156／572到位，再检查完整内容与截图。保留 `C:/tmp/model-iq-radar-ack-menu-20261031-r1/failure.json` 与截图，不将其当作原生已复现的换肤错误或掩盖失败记录。旧 r1 气泡专项的304项测试／录像不作为最终代码的检查数字。
- 完整浏览器：`node scripts/verify_pet_ui.mjs`，`C:/tmp/model-iq-radar-ack-menu-full-20261031-r1/`，86张主截图＋12张顶部裁切，覆盖三尺寸、三侧、十二动作、48次快速状态、二级斜向路径与键盘返回、测试／取消／倒计时／条件过期、HTML与换肤异常恢复，0页面错误／远程请求。62.76秒录像 `video/page@410859c6624f0968ac4cca64a946907b.webm` 已抽20帧查看总览；原有96ms姿态短溶解仍非新制作的连续肢体动画。
- 贴边与生产：`node scripts/verify_pet_dock.mjs`，`C:/tmp/model-iq-radar-ack-menu-dock-20261031-r1/`，静止指针收纳、三侧18轮、6次探头点击、6次贴边云朵拖动和完整／展开拖动通过；录像 `video/page@60087b264033c868b702112e607f6063.webm`。最终 dist 执行 `node scripts/verify_pet_csp.mjs`，12动作解码、待机、损坏资源阻止保存与经典回退通过，0页面错误。
- 文档同步：README、USER_GUIDE、DESKTOP_PET_DESIGN、SKIN_PACKS、CHANGELOG及本记录同步确认复位、选边、菜单窗口生命周期和内部命令；移除当前文档中固定“右侧二级必向左”的旧描述。既有历史验证章节保留原记录。
- 交付构建：复用 `run-build.ps1 -Action build`，TypeScript／Vite及Windows x64 release通过，最终Rust编译2分31秒；仅既有chunk体积与链接器导入库提示。较早的仅气泡复位构建在菜单整改前完成，未当作最终包。`scripts/package_exe.ps1 -OutputDirectory release-artifacts/desktop-pet-ack-menu-test-20261031` 校验版本、PE x64、应用输入新鲜度与复制摘要通过。新EXE：`D:/other/ahui/model-iq-radar/release-artifacts/desktop-pet-ack-menu-test-20261031/Model-IQ-Radar-v1.0.6-Windows-x64.exe`，19,229,184字节，SHA-256 `6ea7c9eec300a506121c065a8c625246741c0086dbb9f6702b73caa7009566b5`；再次计算与release源文件一致。未覆盖旧交付物、未启动正式EXE、未提交或发布；使用前从托盘退出旧实例。
- 收尾检查：项目源文件／测试／根文档及本轮专项文档、QA桥接和脚本的Prettier通过；`git diff --check`通过。14份本轮源码、测试、脚本和文档通过严格UTF-8无BOM／LF、替换字符与凭据模式检查，无异常。检查工作区后保留用户原有修改和无关文件；构建后只同步文档，未改生产代码。
- 未覆盖：按用户要求未启动原生界面，浏览器采样不能证明 Win32 合成残块、物理窗口移动／穿透或跨屏 DPI 已根治；未测正式模型、邮件、真实位置重启和长期进程 CPU／内存。原生新增几何命令已编译／静态检查，但未进行物理多屏端到端验收。

## 选择器跳位、气泡确认与换肤裁切修复（2026-10-31）

- 范围：保留当前二维 PNG／Canvas 角色、卡片和边饰；保护既有未提交工作。只使用隔离浏览器与内存替身，不读取正式凭据、调用模型或发送邮件。Java/JVM/MyBatis 专属规范不适用于本项目，通用注释、格式与边界规范按 Vue／TypeScript／Rust 实现。
- 选择器根因与证据：按实际窗口尺寸模拟 120ms 原生扩高延迟，修改前 `C:/tmp/model-iq-radar-picker-before-20261031-r2/picker-frames.json` 记录卡片顶部 113→1→113。菜单先出画及选中项 `scrollIntoView` 滚动隐藏祖先共同造成跳位。`ModelPicker.vue` 现在先隐藏挂载菜单，等待 `Widget.vue` 测量并通过 `PetStage.syncLayout` 完成宿主布局后才显示；仅滚动菜单自身，代次检查阻止关闭后的迟到展开，布局失败关闭菜单并反馈错误。普通非场景选择器保留原使用方式。
- 气泡与方位：短按结果气泡确认并隐藏本次结果，同时保留原面板切换；拖动不确认，轮询、动作或面板切换不重新显示已确认结果，新任务／新记录／状态变化重新显示。隐藏后同步移除命中与贴边边界，保留角色布局空间并将键盘焦点交还角色。左右角色方位独立于当前贴边方向，拖离边缘和顶部停靠保持原方位；到另一水平边缘后在松手时更新。顶部气泡图形仍翻转、文字正向，通过相反内边距使文字居中。
- 换肤根因：`clear_pet_scene` 原先固定按紧凑高度恢复经典窗口，但 Vue 仍显示展开内容。`PetStage.restoreClassicLayout` 传入实际 `contentExpanded`，Rust `pet_window.rs` 新增可选 `expanded` 参数，缺省仍为 false，兼容旧调用。无数据库、皮肤格式或依赖迁移；可选择经典外观并换回保留的旧 EXE 回滚，旧交付物未覆盖。
- 同步文件：上述组件、Rust 窗口模块、相关组件回归；新增 `scripts/verify_pet_picker.mjs`，更新内存 QA 桥接及全面浏览器脚本；README、USER_GUIDE、DESKTOP_PET_DESIGN、SKIN_PACKS、CHANGELOG 与本记录同步。QA 观察器只用于替身，不进入生产程序。
- 自动检查：`npm test -- --reporter=dot`，12 文件 300 项通过；`npx vue-tsc --noEmit` 通过。复用隔离 `run-build.ps1 -Action test`，110+18+22 共 150 项通过；`-Action clippy` 全目标 `-D warnings` 通过；隔离工具链 rustfmt 检查通过。最终生产构建再次执行 TypeScript／Vite 与 Windows x64 release，Rust 编译 4分02秒，仅既有 chunk 体积与导入库提示。
- 选择器及换肤专项：`node scripts/verify_pet_picker.mjs`，输出 `C:/tmp/model-iq-radar-picker-after-20261031-r3/`。96／144／192 × 左右 × 四个选择器共24次打开，延迟扩高时首帧卡片位置稳定；气泡确认／轮询不重现／新结果重现、实际指针拖动后方位保持、三侧气泡文字位置均通过。紧凑／展开经典与桌宠四次往返恢复正确，经典高度分别156／572，0页面错误与远程请求。已查看气泡、选择器及经典展开截图，并抽取28.44秒录像36帧检查总览；录像 `video/page@305cad50de557244b2e95eb4ec639d18.webm`，总览 `video-frames/contact-sheet.jpg`。r2捕获时尚在替身窗口调整过程中，r3改为等待实际视口尺寸稳定后截图，未将该旧截图冒充最终结果。
- 全面回归：同隔离环境执行 `node scripts/verify_pet_ui.mjs`，输出 `C:/tmp/model-iq-radar-detail-full-20261031-r1/`，86张主截图＋12张顶部裁切、70.56秒录像，0页面错误与远程请求。覆盖三尺寸、三侧、十二动作、48次快速状态、二级菜单、测试／取消／倒计时／条件过期、HTML放大恢复及皮肤保存与异常恢复。气泡确认后改由实际角色键盘入口打开面板，不依赖已隐藏气泡；结果准确性仍检查实际状态。录像 `video/page@733a8d7a1ed67fe56fffc0a923be8096.webm`，已抽36帧查看总览。
- 贴边回归：`node scripts/verify_pet_dock.mjs` 输出 `C:/tmp/model-iq-radar-detail-dock-20261031-r1/`，静止指针重排、三侧×动画开关×三轮共18轮、6次探头点击、6次贴边云朵拖动，以及完整／展开云朵拖动通过；录像 `video/page@bd303886c47a725ba7c3e8e7da686054.webm`。最终 dist 执行 `node scripts/verify_pet_csp.mjs`，12动作解码、待机、损坏资源阻止保存和经典回退通过，0页面错误。
- 交付：`scripts/package_exe.ps1` 校验 PE x64、版本、应用输入新鲜度及复制摘要通过。新包 `D:/other/ahui/model-iq-radar/release-artifacts/desktop-pet-detail-fixes-test-20261031-1350/Model-IQ-Radar-v1.0.6-Windows-x64.exe`，19,215,360字节，SHA-256 `4bbc341d9dae3e7ae12365a3b473c845079ae42c3b09815ae157ec1799a074d0`。构建后只整理文档与 QA，不改生产源码；正式程序标识不变，未启动 EXE、提交或发布。使用前从托盘退出旧实例。
- 收尾核对：项目格式检查范围及本轮三份专项文档／四份 QA 脚本通过 Prettier；`git diff --check` 通过。17份本轮源码、测试、脚本与文档严格 UTF-8 无 BOM／LF、替换字符及凭据模式检查无异常；再次计算交付 EXE 的 SHA-256 与打包回执一致。检查工作区状态后保留全部既有改动，不清理无关文件。
- 未覆盖：按用户要求未做原生界面验收，浏览器替身不能证明 Win32 穿透、原生窗口拖动／顶部合成残块、物理跨屏 DPI、实际重启位置恢复。真实模型／邮件及长期 CPU／进程内存未测。现有姿态短溶解仍不是新制作的连续动作帧，本轮未扩大暂缓的动画素材路线。

## 录屏收纳反弹与结果云朵拖动（2026-10-31）

- 范围：继续当前二维PNG／Canvas方案，不启动暂缓的连续素材路线，不改角色与卡片设计、不改Rust代码。保护既有未提交工作，未reset/clean；仅隔离浏览器验收，不读取正式凭据、请求模型或发送邮件。遵循Vue/TypeScript的注释与格式约定；Java手册的Java/JVM/MyBatis专属条目不适用。
- 复现与根因：实际抽取用户 `QQ20261031-114129.mp4` 的36帧；右侧面板收纳后窗口原点／大小变化，探头移到静止鼠标下，pointerenter被误认为主动悬停，于约1.3秒内连续四次收起／恢复。之前固定页面原点的QA遗漏该路径。新增 `scripts/verify_pet_dock.mjs` 依据每次scene的actorBounds模拟工作区对齐，真实鼠标位于(815,132)；修改前失败记录 `C:/tmp/model-iq-radar-reflow-before-20261031/stationary-pointer-trace.json` 明确显示四对true/false。用户视频抽帧在 `C:/tmp/model-iq-radar-user-video-20261031/video-frames/`。
- 修改 `PetStage.vue`：探头enter只记录screenX/Y，继续移动至少2逻辑像素才恢复，避免client坐标随窗口重排变化而误判。无动画时同样生效；点击、右键和键盘无需等待。鼠标pointerdown阻止默认focus提前移除探头，避免mouseup/click落空；键盘聚焦仍交接到云朵，Enter打开卡片。
- 结果云朵与角色共用6逻辑像素阈值、捕获与原生拖动链路，禁用云朵长按菜单；短按仍切换面板，右键有菜单入口。贴边云朵按下先恢复完整场景再拖动；若只是短按则打开已有卡片，不误关。拖动期间不接受激活，松手后220ms吞鼠标合成click，键盘在松手后可立即操作。展开卡片仍侧坐，拖动不改写真实结论。隐藏后丢失pointerup会清除过期按下，换肤清除云朵来源。
- 同步：`tests/pet-stage.test.ts`；QA内存桥接增加scene观察和拖动请求记录，未进入生产包；更新全面QA的真实鼠标路径、README、用户手册、设计、皮肤契约和CHANGELOG。无依赖、素材、皮肤schema、数据库或备份迁移。回滚先选择经典外观，再换保留的旧EXE；本轮不会删除旧包。
- 自动检查：`npm test -- --reporter=dot`，12文件292项通过；`npx vue-tsc --noEmit`与生产构建内相同检查通过。`npx prettier --check src tests *.json *.ts *.md index.html scripts/pet-preview-bridge.ts scripts/verify_pet_ui.mjs scripts/verify_pet_dock.mjs`与 `git diff --check`通过；当前任务文本UTF-8无BOM／LF及替换字符、凭据模式检查通过。Rust无本轮修改，未重复执行Rust测试／Clippy；上一节150项仅为历史结果。
- 坐标专项：设置PET_PLAYWRIGHT_MODULE为现有Playwright路径、PET_CHROMIUM_EXECUTABLE为现有Edge、PET_QA_SCREENSHOT_DIR为 `C:/tmp/model-iq-radar-reflow-after-20261031-r4`，执行 `node scripts/verify_pet_dock.mjs`。三侧×动画开关×三轮，共18轮真实悬停恢复及重新收纳通过；另6组直接点击、6组贴边气泡拖动、完整桌宠／展开面板两种云朵拖动均通过。静止指针重排只记录一次true，不弹回；真实移动恢复。小抖动不启动、长按600ms不抢菜单、拖后不误切面板、状态不变，0页面／远程错误。替身只记录拖动调用，不移动系统窗口，不证明Win32实际抓取点或穿透。
- 全面浏览器：同环境变量，输出改为 `C:/tmp/model-iq-radar-reflow-full-20261031-r3`，执行 `node scripts/verify_pet_ui.mjs`。86张主截图＋12张顶部裁切、67.6秒录像，0页面错误和0远程请求；覆盖96／144／192尺寸，三边四状态，十二动作真实点播，48次快速状态竞争、双击／长按／二级菜单斜向路径、测试／取消、倒计时、真实模型选择使结果过期、HTML放大恢复、皮肤预览保存与异常回退。保留原32次Canvas交接采样和三侧每侧30次滑入采样；无透明闪空，短溶解仍非连续肢体动画。
- QA发现并处理的失败现场：full-r1揭示鼠标focus提前删除探头导致点击丢失，产品已修复；full-r2在右侧恢复后的截图耗时中已自动收纳，脚本却尝试点击隐藏角色。右侧恢复会移动角色，鼠标原位置变成透明空白，本应收起；现脚本先真实移到云朵再截图／操作，不以透明空白锁住窗口。均保留失败记录，不冒充全部首次通过。
- 生产资源：最终dist执行 `node scripts/verify_pet_csp.mjs` 通过，十二动作解码、待机播放、坏图／坏描述阻止保存及经典回退，0页面错误。录像各抽36帧并检查总览，另查看右侧展开卡片图；专项31.96秒录像含完整循环与云朵拖动，文件 `video/page@93c748f1ceb4270ab642f48b4a285885.webm`；全面录像 `video/page@84333c01496ad8c95d0c475049120c81.webm`。各自证据目录包含result.json、video-frames/contact-sheet.jpg；全面目录另有dock-scenes、dock-motion-samples、actor-motion-samples、rapid-switch-samples等JSON。初始加载前空帧与QA主动快速换肤不当作产品闪空。
- 构建：复用 `C:/Users/Admin/.codex/tmp/model-iq-radar-mail-build/run-build.ps1 -Action build`，TypeScript/Vite与Windows x64 release通过，Rust编译2分30秒；仅既有Vite chunk体积和链接器导入库提示。`scripts/package_exe.ps1`校验PE、版本、输入新鲜度及复制摘要通过。新包 `D:/other/ahui/model-iq-radar/release-artifacts/desktop-pet-reflow-cloud-test-20261031-1318/Model-IQ-Radar-v1.0.6-Windows-x64.exe`，19214848字节；SHA-256 `eced9d585efeef6b0d1e174f6fadc4b065d03a794d38cdfad06bedf108c9ad8c`。正式程序标识不变，未覆盖旧包、未启动原生EXE、未发布或提交。
- 未覆盖：按用户要求不做原生界面验收，因此Win32穿透、原生窗口拖动／合成残块、物理跨屏DPI、真实位置保存及重启恢复未复验；正式模型／邮件与长期驻留CPU／进程内存亦未测。当前静态姿态仍为96ms溶解，大幅姿态没有真实连续动作帧；本轮未承诺解决暂缓素材方案的限制。

## 右侧卡片触边与菜单路径整改（2026-10-31）

- 用户本轮要求保留现有动画方案，只在隔离浏览器验收。未生成新素材、未改角色外观、未增加渲染引擎，未读取正式凭据或调用真实模型/SMTP。已有未提交工作全部保留；Java手册的一般注释与边界规范适用，Java/JVM/MyBatis专属规则不适用于本栈。
- 根因：原生只用 `actorBounds` 检测触边，卡片右侧已到工作区边缘时，左上方角色仍离边缘很远。新增可选私有scene字段 `dockBounds`，前端只合并角色、常驻云朵和已打开的卡片，排除临时菜单/选择器；Rust在初始、查询和松手检测统一使用它，最终位置仍按角色轮廓对齐。字段缺省兼容旧前端，重复校验有限矩形，无皮肤/数据库迁移。另将整个stage的hover保护收窄到实际交互对象，透明空白不阻止收纳。
- 菜单根因：离开“动作”包装区域或经过其他一级菜单行即关闭二级。修改 `PetStage.vue`，两层作为共同交互树，鼠标斜向经过其他一级项仍保持展开；选择、外部点击、Escape/左方向键和父菜单关闭负责退出。右侧向左展开、滚动、掩码及业务隔离保持原规则。
- 修改范围：`PetStage.vue`、`pet_window.rs`；`tests/pet-stage.test.ts`及Rust内嵌回归；`scripts/verify_pet_ui.mjs`、`scripts/pet-preview-bridge.ts`；同步README、用户手册、设计、皮肤契约、CHANGELOG和本记录。QA替身补齐实际按钮所需的内存测试/取消、自动测试倒计时及模型条件变更，只用于本地验收，不进入生产包。
- 修改前斜向路径实际失败证据：`C:/tmp/model-iq-radar-boundary-before-20261031/failure.json`、`failure.png`，明确记录“经过一级菜单时动作子菜单自动收起”。修复后扩大矩阵，r1/r2/r3/r4保留了脚本问题的失败现场：旧用例依赖透明stage:hover阻止收起、默认自动测试开启却预期点击后开启、固定等待未跨越整秒采样。QA替身此前还返回固定剩余137秒，已改为截止时间快照；这些不冒充生产倒计时缺陷。
- 自动检查：`npm test -- --reporter=dot`，12文件282项通过；`npm run build` TypeScript/Vite通过。隔离工具链 `run-build.ps1 -Action test`：110+18+22，共150项通过；新回归包含卡片右侧触边但角色未触边、1/1.5/2倍率、负工作区、关闭吸附、缺字段和坏矩形。`-Action clippy` 全目标 `-D warnings`通过。Prettier、rustfmt、diff、UTF-8无BOM/LF检查通过；Vite仅既有chunk体积提示，链接器仅导入库提示。
- 最终隔离浏览器命令：设置 `PET_PLAYWRIGHT_MODULE` 指向现有Playwright、`PET_CHROMIUM_EXECUTABLE` 指向现有Edge、`PET_QA_SCREENSHOT_DIR=C:/tmp/model-iq-radar-boundary-audit-20261031-r5` 后运行 `node scripts/verify_pet_ui.mjs`。完整86主截图+12顶部裁切、64.28秒交互录像，0页面错误、0远程请求。真实慢速鼠标斜向进入菜单、来回路径和三边展开均通过；十二项点播实际点击（含滚动）、长按/双击/外部关闭、三尺寸控件九点命中、紧凑/展开、HTML沙箱放大恢复、皮肤预览保存、坏资源/CSP回退均有独立覆盖。`node scripts/verify_pet_csp.mjs`在最终生产dist通过：12动作解码、待机播放、坏图/坏描述阻止保存、经典恢复，0页面错误。
- 收纳验证：先打开实际卡片，读取其独立dockBounds；在角色尚未靠边的前提下用逻辑工作区替身计算right通知，而非直接强制宣告right通过。Rust真实判定另有上述回归。鼠标留在场景透明上方空白仍能收纳；三边面板恢复保持侧坐，四状态云朵常驻、顶部轮廓倒转但文字正向；测试中拖拽与结算、快速方向竞争、暂停和换肤清理通过。浏览器替身不移动系统窗口，不能据此声称原生实机吸附已验证。
- 动画证据：32个画布交接采样Alpha总量最小/最大约0.9344；三边每边30个滑入样本透明度均1。48次35ms间隔业务状态切换均有非空画布、最后error有效、无额外播放器节点。关闭自主仍可摸头；先等待idle真实稳定再观察7—16秒自主活动，避免把旧姿态当新动作。单帧姿态继续使用原96ms溶解，并非新增连续动作。浏览器GC后JS堆从约13.26MiB到13.99MiB，仅作短时采样，不能证明长时间无泄漏或进程CPU/总内存合格。
- 本地功能替身实测：真实点击“重新测试→取消测试”，状态正确；自动测试开关、倒计时推进、实际选择demo-model后旧结果过期。只有明确点击产生一次start_test及一次cancel_test，其他桌宠交互不触发测试/邮件；不对真实服务接口做结论。
- 已抽取录像36帧并查看总览，同时检查右侧紧凑卡片、向左二级菜单、自主坐姿和四状态截图。证据：`result.json`、`dock-scenes.json`、`dock-motion-samples.json`、`actor-motion-samples.json`、`rapid-switch-samples.json`；录像 `video/page@a2ec5d3617f758900587a51be2aab5e3.webm`；抽帧 `video-frames/contact-sheet.jpg`，均在上述r5目录。初始页面加载前的录像首帧为空不属于动作闪空。
- 交付构建：`run-build.ps1 -Action build`成功，release编译3分39秒。`package_exe.ps1 -OutputDirectory release-artifacts/desktop-pet-boundary-audit-test-20261031-1120`核对版本、PE x64、应用输入新鲜度和复制摘要通过。EXE为 `release-artifacts/desktop-pet-boundary-audit-test-20261031-1120/Model-IQ-Radar-v1.0.6-Windows-x64.exe`，19,214,848字节，SHA-256 `441c73e0f0efb3e055f949e1b5a1bbdfae62d07cfc093b7a9a8e0dbdfbae3a27`；正式标识不变，未覆盖旧包，未启动原生EXE、未提交或发布。测试前从托盘退出旧实例。
- 未覆盖/限制：原生偶发顶部合成残块、透明穿透、物理跨屏DPI、真实重启位置、正式服务与邮件、长时间驻留CPU/进程内存。现有静态姿态短溶解仍可能看到不同轮廓的重影，大幅姿态并无真实连续动作；用户本轮明确暂缓连续素材方案，不把此次稳定性回归表述为动画制作完成。回滚先保存经典，再换保留的旧EXE。

## 闪空、贴边排队与原生残块规避（2026-10-31）

- 用户复现：动作切换闪空、贴边要等待多个姿态、顶部云朵方向不对，以及头发/气泡上方残块随轻微原生移动消失。本轮新增截图 `codex-clipboard-6cbf25d1-bb89-4c01-86f9-a343e880e81f.png` 与 `codex-clipboard-e2657fa4-fc55-4624-96c0-2bc56c16582b.png` 为用户实机证据；本轮未启动原生程序，不将浏览器无残块等同于实机已根治。
- 已确认闪空根因：上一轮播放器在100ms中点将旧、新画面都变为全透明，贴边另有退场/隐藏阶段，叠加放下和反馈计时。现改为就绪后96ms预乘Alpha加权，双方权重之和为1；贴边180ms离开后直接接管，以最终布局内160ms不透明滑入呈现，跳过landing/庆祝驻留，自主动作在贴边让位。关闭动画直接呈现。未制作连续肢体帧，短溶解不代表完整动作制作。
- 顶部探头维持头朝右，云朵轮廓上下翻转、尾巴朝角色，文字和图标保持可读。普通面板可收纳，恢复同一详情状态；菜单、浮层、按住、键盘焦点及HTML放大仍保护输入。真实浏览器又发现并修复550ms补接点击吞掉卡片按钮的问题，新增立即操作卡片按钮的回归。
- 原生疑点与处理：之前反复把复杂命中轮廓传入SetWindowRgn，同时改变透明WebView2的显示裁剪。改用 `src-tauri/src/pet_input.rs`，保留矩形透明合成表面，仅由掩码控制鼠标穿透；首次启用移除旧裁剪。采样由主线程处理，显示约60Hz、隐藏5Hz，每次确认后才排下一帧；自有按下/拖动保留输入，释放恢复，预览/退出递增世代取消旧任务。未新增依赖或配置字段。此为规避可疑合成路径，不宣称已证明唯一根因或已原生验收。
- 联网读取两参考项目实际播放器和帧缓存，主分支SHA及文件记录于设计文档：dsh-pet前后台透明视频就绪交接、单待切换目标与迟到加载隔离；indesktop播放实际PNG连续帧、预取、缺帧等待及超时回退。完整动画来自制作好的视频/连续帧。当前十一种额外姿态仍为单帧，没有复制第三方角色素材，也没有假定静态生图能生成已验证视频或Live2D。
- 自动验证：`npm test` 最终12文件277项通过；隔离环境 `run-build.ps1 -Action test` 109+18+22共149项通过（含物理命中/DPI取整、自有按下保留和外部按住不锁定）；`-Action clippy` 全目标 `-D warnings` 通过。Prettier、rustfmt、TypeScript/Vite及 `git diff --check` 通过。首次直接cargo调用因PATH未加载失败，随后使用既有工具链/构建脚本成功，未安装工具。
- 第一轮浏览器暴露卡片补接点击缺陷，保留失败证据于 `C:/tmp/model-iq-radar-no-flash-20261031/`；修复后第二轮 `C:/tmp/model-iq-radar-no-flash-20261031-r2/` 完整通过：66主截图、12顶部裁切、34.84秒交互视频，0页面错误、0远程请求、0额外模型/邮件命令。三边每边30个滑入采样，透明度均为1；32个实际角色画布采样的Alpha总量最低/最高约0.9344，无闪空。数据在 `dock-motion-samples.json`、`actor-motion-samples.json`、`result.json`；视频 `video/page@d730de04823bff815ca48203b1d13708.webm`，已离线抽帧并查看总览及顶部气泡、右侧恢复面板截图。
- 最终生产dist的 `verify_pet_csp.mjs` 再次通过：十二动作解码、待机播放、坏描述/坏图片阻止保存和经典恢复，0页面错误。本轮不读取正式配置或凭据，不发送模型请求或SMTP；Java/JVM/MyBatis专属规范不适用于Vue/TypeScript/Rust。
- 未覆盖：用户所示原生残块的修复后实机复验、原生穿透与点击延迟、物理跨屏/DPI、重启位置及长时间资源驻留；浏览器仅验证组件、事件和布局。新增输入监视路径需特别复验透明处点击、拖动按住、释放和HTML放大恢复。皮肤schema1、数据库/备份v3、业务结论和接口流程不变；回滚先保存经典，再换旧EXE，旧交付物保留。

### 同轮补充：右键动作二级菜单

- 按用户追加要求，实际读取参考项目相同SHA的 `runtime/electron-helper/sprite/menu.js`，借鉴工具与动作树分离、菜单输入保护及关闭收尾，未引入其业务请求。当前一级保留面板、设置、当前状态、动作、隐藏；二级按当前皮肤资源列出动作，内置十二项，旧皮肤缺动作不列出。
- 修改 `PetStage.vue` 与 `src/pet/layout.ts`：一级/二级菜单独立矩形和掩码，右侧二级菜单向左展开，限高滚动；悬停/点击打开、方向键及Home/End导航、Escape/左方向键返回、外部点击关闭。动作点播收起卡片后预览，真实结果保持不变，不模拟真实拖动或额外测试。同步菜单回归、浏览器脚本、README、用户手册、设计、皮肤契约与变更记录，无皮肤/数据库格式迁移。
- `npm test` 最终12文件280项、TypeScript检查通过。第一轮实际浏览器发现子菜单因一级1px边框使显示和掩码错位，已修复并重跑；失败证据保留在 `C:/tmp/model-iq-radar-no-flash-submenu-20261031/`。
- 最终浏览器 `C:/tmp/model-iq-radar-no-flash-submenu-20261031-r2/`：70主截图+12顶部裁切，0页面错误/远程请求；新增十二动作列表、点击/悬停、分层键盘返回、三边子菜单显示范围与输入掩码断言，原有完整流程继续通过。视频 `video/page@8156f84408ed3d54d3a74a41a9328e17.webm`，已查看右侧向左展开的双层菜单截图。该最终前端不影响此前149项Rust测试和Clippy的实际覆盖；原生残块和点击穿透依然未实机复验。

- 最终交付：复用隔离MSVC/SDK执行 `run-build.ps1 -Action build`，包含二级菜单的最终release成功（5分09秒，JS580.69kB，仅既有chunk体积和MSVC导入库提示）。最终dist再次通过生产CSP检查，Prettier/rustfmt、diff、限定范围UTF-8无BOM/LF与凭据模式扫描通过。`package_exe.ps1` 的版本、PE x64、应用输入新鲜度和复制摘要校验通过。
- 新测试包：`release-artifacts/desktop-pet-no-flash-submenu-test-20261031-104212/Model-IQ-Radar-v1.0.6-Windows-x64.exe`，19,208,704字节；SHA-256 `8bb7a85dfeefa297f3a89c1b4f634e58e4b27d0773500065e8479a0c8b17ba79`。正式应用标识不变，未覆盖旧交付物、未启动原生EXE、未提交或发布。测试前应从托盘退出旧实例，避免单实例机制仍唤起旧程序。

## 历史：全动作交接、面板收纳和顶部朝右（2026-10-31）

- 根因：`PetStage.vue` 直接切换探头节点和窗口尺寸，且把 `panel` 作为禁止收纳条件；`PetActor.vue` 的单帧姿态绕过交接，只有部分连续图集使用短合成。本轮按用户最新要求更新，不再沿用下方历史记录的“面板打开不收纳／静态直接交接／顶部原旋转方向”结论。
- `PetStage.vue` 加入可取消的120ms退场→等待最终布局→200ms进场，沿屏边平移10逻辑像素并淡出淡入，不缩放或扭曲脸部。动画中途返回鼠标可取消退场，禁用吸附、暂停动画、换肤、隐藏和卸载正确清理。过渡期间临时扩当前场景输入轮廓，完成后恢复精确轮廓。普通卡片打开时仍可收纳，恢复保留同一控件和紧凑／详情状态；下拉浮层、右键菜单、按住鼠标、键盘焦点和HTML放大仍保护输入。
- `PetActor.vue` 对全部不同动作（含单帧）统一解码预载后的100ms淡出＋100ms淡入，同一帧仅绘制一种姿态，不叠脸、不旋转；静态循环姿态交接后停止重绘，非循环片段只结算一次。此为呈现过渡，没有新增连续肢体补帧或绑定。`controller.ts` 使面板显示时固定侧坐，包括标题栏拖动、测试和结果变化；业务结论仍独立显示。
- `layout.ts` 将顶部固定变换改为 `scaleX(-1) rotate(90deg)`，头朝右；点击区域使用同一坐标变换。没有修改源图、皮肤schema、业务请求、数据库或Rust。旧包沿用新宿主统一过渡，无需新增资源字段；关闭动画直接交接，旧交付物保留。
- 更新 `tests/pet-actor.test.ts`、`tests/pet-stage.test.ts`、`tests/pet.test.ts` 与 `scripts/verify_pet_ui.mjs`；同步README、CHANGELOG、用户手册、设计文档及皮肤契约。`npm test -- --reporter=dot`：12文件275项通过，覆盖十二单帧动作交接不叠脸、三方向面板收纳恢复、退场打断、浮层／按住／HTML保护、面板固定侧坐及顶部非对称热区。
- 最终浏览器验收 `C:/tmp/model-iq-radar-motion-panel-20261031-pass2/`：66主截图＋12顶部裁切、连续视频，0页面错误、0远程请求、0额外模型或邮件命令。新增三边已打开详情面板收纳／悬停恢复、四状态保持侧坐与真实CSS中间帧采样；每边30个采样，分别有22／23／23个中间透明度，业务结果不变，记录于 `dock-motion-samples.json`。视频 `video/page@e96752f417d2b3c063932bb21099dea7.webm`。已查看顶部头朝右、右侧恢复卡片截图；全量原有交互验收继续通过。
- TypeScript／Vite生产构建与格式检查通过，仅既有chunk体积提示。生产CSP脚本12动作解码、待机播放、坏资源阻止保存和经典恢复通过，0页面错误。本轮未改Rust，不重复声称重新执行了Rust测试；未启动原生EXE，Windows窗口动画合成、点击穿透、跨屏／DPI和此前偶发顶部问题仍未实机验证。没有读取正式凭据或发送模型／邮件请求；Java／JVM／MyBatis专属规范不适用于本轮Vue／TypeScript修改。
- 最终代码复用隔离MSVC／SDK构建成功（最终release 2分33秒，JS577.48kB，仅既有chunk及MSVC导入库提示）；`package_exe.ps1` 核对版本、PE x64、输入新鲜度及复制摘要通过。新版测试包 `release-artifacts/desktop-pet-motion-panel-test-20261031-095557/Model-IQ-Radar-v1.0.6-Windows-x64.exe`，19,177,472字节，SHA-256 `358e9d743f5d8f9d934114af80ff0d6febd3535d6f746f0ebd591efe67e876b1`。正式标识不变，未覆盖旧包、未启动原生程序、未提交或发布。最终源码／文档UTF-8无BOM及LF、限定凭据模式扫描、格式与diff检查通过。

## 三边探头与贴边状态云朵（2026-10-31）

- 用户明确授权同一探头调整角度用于右侧和顶部，覆盖上一轮“仅左侧”范围。未新增生图或角色素材：左侧原样、右侧水平镜像、顶部顺时针90度；固定宿主变换与命中区域使用相同公式，不拉伸脸部。`docking.left` 保留为历史通用源字段，schema1及旧包兼容不变。
- 三边收起均显示常驻正向状态云朵，测试中／不降智／降智／测试失败由实际业务驱动；云朵不随头旋转，右侧仅调整轮廓尾巴，文字不翻转。关闭临时互动气泡不隐藏结论；云朵和变换后的角色轮廓均进入原生Region。
- 松手先按未夹紧角色轮廓检测顶部／左／右，角落顶部优先；方向缓存等待前端右侧布局调整后再次对齐，避免云朵占宽使右侧永远不可达。按全部可见区域夹紧，保云朵屏内。右侧完整卡片向角色左边展开，恢复使用原位置和锚点；拖动中方向事件暂存到松手再应用，避免抢夺抓取点。
- 位置保存使用一次待保存意图，真实拖动松手后才建立，最终普通场景精确对边后消费，探头临时位置不落盘。`canonicalAnchor` 为可选私有场景字段，不是皮肤字段；将反射后原点换算回默认布局，首次几何提交完成后再订阅方向。新增五次右侧保存／重启往返不漂移、旧字段兼容和非法锚点拒绝回归。
- 独立审计发现并修复释放监视器与新拖动／取消之间的保存竞态：释放携带原世代，后台入口排原生事件线程，保存与取消／新拖动通过同一门禁串行，写盘前复查意图身份、场景版本和世代，成功后按身份消费；失败保留重试，旧回调不删除新意图。活动皮肤更换即使已松手也先取消原生意图。门禁内不调用窗口API或事件，独立只读复核未见新增死锁。
- `npm test -- --reporter=dot`：12文件257项通过；格式、TypeScript／Vite及diff检查通过。隔离原生测试106＋18＋22＝146项通过，严格clippy及rustfmt通过；新增三边、角落、禁用、负屏坐标／DPI、云朵范围、变换热区、旧包和保存竞态回归。生产CSP验证12动作解码、待机播放、坏资源阻止保存及经典恢复通过，0页面错误。
- `verify_pet_ui.mjs` 最终三边pass4通过：63主截图＋12头顶裁切＋1联系表，三边四状态对照图及连续视频，0页面错误、0远程请求、0额外测试／邮件命令。覆盖三方向深浅背景、真实hover／点击／Enter、恢复重收起、收起时切方向、拖动优先／换肤，以及右侧96／144／192紧凑／详情布局九点工具和选择器命中。证据 `C:/tmp/model-iq-radar-three-edge-browser-20261031-pass4/`，视频 `video/page@2d9f6c06894cb88f5c8e7e816d7d149f.webm`；`dock-scenes.json` 仅记录方向、轮廓与云朵Region几何。已查看三边12状态及右侧／顶部深浅背景。
- 浏览器不能代替Windows屏幕坐标、点击穿透、真实窗口拖动或物理跨屏／DPI验证；此前偶发顶部异常本轮仍未原生复现。静态探头方向转换不等于制作新的连续动作。无真实模型／邮件／正式凭据读写，无数据库迁移，旧包保留。
- 最终代码复用隔离MSVC／SDK执行 `run-build.ps1 -Action build` 成功，Windows x64 release编译2分16秒；TypeScript／Vite生产构建通过，JS576.19kB，仅既有chunk体积及MSVC导入库提示。`package_exe.ps1` 核对版本、PE x64、源码新鲜度与复制摘要通过，最终测试包 `release-artifacts/desktop-pet-three-edge-test-20261031-005654/Model-IQ-Radar-v1.0.6-Windows-x64.exe`，19,177,472字节，SHA-256 `293bd1365e2d2fe9fab1e966e3f7f9c3a8b81e2ac02dbf60fd8b617f4121c69a`。正式应用标识不变，未覆盖旧交付物、未启动原生EXE、未提交／推送／发布；最终源码与文档UTF-8无BOM／LF，限定密钥模式及diff检查通过。

## 左侧探头与顶部偶发现象排查（2026-10-31）

- 使用用户批准的探头原透明图，原图、处理参数和摘要保留在 `assets/pet/blue-whale-maid/edge-left-source.png`、`edge-extraction.json`。仅清理 alpha 1—3 外围杂点并等比归一到320方形，可见边界 `[0,4,209,316]`；`--edge-only` 两次生成摘要一致，未重制角色或改动其他姿态。深浅及紫绿背景检查未见白底、棋盘格或卡片碎片。
- 加入可选 schema1 `docking.left`，旧包缺省仍显示完整角色。开启吸附后按角色实际轮廓判左侧16逻辑像素，离开500ms收为65×96逻辑像素探头；悬停恢复、真实点击与键盘Enter展开，面板／菜单／拖动优先。修复鼠标焦点残留、窗口缩放漏发离开事件和探头节点替换吞点击；关闭吸附、换肤、隐藏及时清理。
- 对用户正常／异常截图逐张比较，确认偶发顶部断层不能仅归因于PNG。修复代码可证明的Region缓存缺陷：窗口宽高或DPI变化必须重新应用区域；HTML预览恢复后释放布局锁，再以最终DPI重建Region并同步缓存。云朵禁用原生按钮外观、SVG改为明确的非负层叠。按官方Win32行为保留 `SetWindowRgn(..., TRUE)`，没有盲加重绘消息。
- 取消桌宠拖动时同时撤销原生60Hz循环和220ms延迟移动回调；换肤、退出场景和隐藏不再让旧拖动移动新窗口或写位置，重复取消幂等。测试业务、邮件、规则和凭据路径未变。
- `npm test -- --reporter=dot`：12文件247项通过，0未处理错误；TypeScript、Vite生产构建、Prettier、Node脚本语法、Python编译及 `git diff --check` 通过。复用隔离MSVC／SDK的 `-Action test`：100单元＋18数据完整性＋22本机替身回归，共140项通过；`-Action clippy`（all-targets，warnings拒绝）通过。新增契约、迟到回调、关闭吸附、重收起、换肤取消等回归。
- 最终浏览器验收 `verify_pet_ui.mjs`：42张主截图＋12张头顶裁切与连续视频，0页面错误、0远程请求、0额外测试或邮件命令。覆盖96／144／192尺寸、四状态、悬停／真实点击／键盘恢复、暂停、拖动结算、换肤、HTML放大恢复和保存。证据目录 `C:/tmp/model-iq-radar-dock-browser-20261031-pass7/`；视频 `video/page@8d90f5c0962cb7de59668469535056dc.webm`。逐格查看顶部对照图，本轮浏览器未见所报断层。生产CSP解码12动作、坏资源阻止保存和经典恢复通过。
- 本轮遵循用户指定的浏览器验收范围，未启动原生EXE复现；因此不能宣称Windows偶发顶部异常已彻底解决。Windows底层点击穿透、原生拖动合成、物理跨屏／DPI、休眠和长期性能仍未覆盖。无真实模型调用、邮件或正式凭据读写；无数据库迁移，Java／JVM／MyBatis专属条款不适用。旧交付物保留，回滚先保存经典皮肤再更换旧EXE。
- `run-build.ps1 -Action build` 完成生产Windows x64构建，Vite JS 575.01 kB，仅既有chunk体积和MSVC导入库提示。`package_exe.ps1` 核对版本、PE x64、应用输入新鲜度和复制摘要通过；测试包 `release-artifacts/desktop-pet-edge-fix-test-20261031-002226/Model-IQ-Radar-v1.0.6-Windows-x64.exe`，19,159,552字节，SHA-256 `97a706585e3dbb96280485b577d15ba0d2b9d069444b82ea9f6a1e1600a103d6`。未覆盖旧文件、未启动交付程序、未提交／推送／发布。构建后生产CSP验收再跑通过。

## 原布局与当前桌宠外观整合（2026-10-30）

- 以用户最新两张紧凑／展开截图为布局基准，恢复360px宽、两行四独立选择器（接口／模型、规则／强度，默认70%／30%）；桌宠与经典模式共用结果、回答、元信息及操作模板。保留当前桌宠、常驻云朵、海洋配色与边饰；桌宠卡片176px紧凑、596px详情、660px HTML详情。左上角色避让标题，选择器位于完整透明角色按钮下方；菜单仍独立扩场景、不撑卡片。
- `npm test -- --reporter=dot`：12文件239项通过；`npm run format:check`、`git diff --check`通过。已确认源码与相关文档UTF-8无BOM、LF；前端TypeScript及Vite生产编译成功，JS 571.21 kB，仍有既有chunk体积提示。
- `node scripts/verify_pet_ui.mjs`：独立内存桥接、回环服务，23张截图＋连续交互视频，0页面错误、0远程请求。96／144／192三尺寸分别验证紧凑和展开，顶栏按钮和四选择器九点命中、两行列对齐、正文纵向顺序、底部无遮挡；保留四结果／条件过期／取消、拖动后反馈、菜单不撑卡、静态动作、皮肤保存和HTML放大恢复。证据目录：`C:/tmp/model-iq-radar-vertical-layout-20261030-final/`，视频：`video/page@b5122a89c998cffb1756fa1cb06b034b.webm`。README桌宠截图已同步最终浏览器画面。
- 独立复核实际复现并修复HTML源码切换后回退经典模式无法返回预览的问题；新增单元回归，并在本地真实iframe恢复SVG预览。`node scripts/verify_pet_csp.mjs`在最终生产dist及实际CSP下通过12动作解码、坏图／坏描述阻止保存与经典恢复，0页面错误。
- 用户要求贴边收起的小脑袋先做设计，本轮不改该逻辑：经典模式保留小标签，桌宠模式仍仅收起卡片、保留角色与结果。没有生图调用、真实模型请求、邮件或正式凭据读写；未启动原生界面，未验证Windows穿透／跨屏DPI／长期性能。本轮未修改Rust，不重复声称已执行Rust测试。
- 复用隔离MSVC／SDK执行 `run-build.ps1 -Action build`，最终Windows x64生产构建成功，保留既有MSVC导入库提示。`package_exe.ps1 -OutputDirectory release-artifacts/desktop-pet-vertical-test-20261030-223214` 核对版本、PE x64、源码新鲜度与复制摘要通过：`Model-IQ-Radar-v1.0.6-Windows-x64.exe`，19,039,744字节，SHA-256 `eb3385f0123bc3767d449db1c6778326c5bf7f32ffd8446bf270709685d9b441`。未覆盖旧包、未启动EXE、未提交或发布。

## Windows x64 测试包（2026-10-30）

- 用户要求打当前四组定稿桌宠测试版。复用 `C:/Users/Admin/.codex/tmp/model-iq-radar-mail-build/run-build.ps1 -Action build`，TypeScript/Vite 和 Tauri release 编译成功；前端 JS 575.85 kB，仅既有 chunk 体积及 MSVC 导入库提示。生产标识为 `com.ahui.model-probe`，未混入独立 QA 标识或验收桥接。
- `pwsh -NoProfile -File scripts/package_exe.ps1 -OutputDirectory release-artifacts/desktop-pet-test-20261030-2134` 核对版本、PE x64、应用输入新鲜度和复制摘要通过。产物为 `Model-IQ-Radar-v1.0.6-Windows-x64.exe`，19,039,744 字节，SHA-256 `6e95efb73882c9146b2bac1a8a6143bdd167a1aea7d433f5b46a9d38d3f85f95`。新目录保留旧交付物，未提交、推送或发布。
- 本次只进行编译及交付核验，未重新运行测试或启动 EXE；沿用下方浏览器及自动测试证据，不把打包成功描述为原生界面验收通过。未读写正式配置或凭据。

## 四组定稿桌宠与卡片复核（2026-10-30）

- 以用户确认的四状态、紧凑卡片、展开详情和额外姿态设计图为基准，实际浏览器检查了头部结果云朵、卡片搭接、海洋边饰、十二动作资源、96/144/192 三种角色尺寸及 HTML 详情。独立雷达球、全息入口和旧气泡已从组件、样式与资源引用中移除。
- `npm test -- --reporter=dot`：12 个文件、238 项通过；`npm run build` 通过（最终 Vite JS 575.85 kB，保留既有 chunk 体积提示）；`git diff --check`、Prettier、TypeScript 检查通过。新增动作契约后实际复用隔离环境执行 `-Action test`：95+18+22 共 135 项通过；`-Action clippy` 的 `--all-targets -- -D warnings` 通过。
- `node scripts/verify_pet_ui.mjs` 使用独立本地替身和回环服务：20 张截图、0 页面错误、0 远程请求；覆盖四种结果、待重新测试、关闭临时气泡仍保留结论、菜单不撑卡片、三种尺寸九点选择器命中、拖动放下后反馈、静态招手/伸懒腰、HTML 放大恢复、皮肤预览保存。连续交互视频证据：`C:/tmp/model-iq-radar-approved-design-20261030-r2/video/page@21673f2b7720979df753dd9cfe2594c4.webm`。
- `node scripts/verify_pet_csp.mjs`：十二动作完整解码、待机轻移、坏描述/坏图片阻止保存、经典恢复通过，0 页面错误。素材审计见 `C:/tmp/pet-assets-20261030/asset-validation.json`；所有姿态 320×320、透明 Alpha，静态姿态不使用整图旋转或交叉淡入。未制作专用闭眼素材，因此不宣称已完成眨眼动画。
- 视觉检查发现并修复了同一记录从通过变为失败时旧庆祝姿态残留，以及拖动放下期间迟到结果复活的问题；回归用例覆盖 ID 和状态双重校验。未启动原生程序、未重新打 Windows EXE、未读取正式凭据/数据库、未调用真实模型或 SMTP；Windows 点击穿透、跨屏/DPI、休眠和长期性能仍未覆盖。

## 旧版桌宠恢复与必要文件清理（2026-10-30）

- 按用户最新四组设计恢复二维蓝鲸女仆、头部结果云朵、十二动作 PNG 图集、新卡片及海洋边饰，独立雷达球已移除。来源为目标时段历史源码与资源；不宣称重建源码与原 EXE 逐字节相同。已移除后续试验资源及实现，保留运行、构建、测试、原设计与素材重建依赖。
- 从用户最新列出的文件中清理 15 项：11 张未被 README 引用的桌宠截图、未引用的 preview 拼图、2 个后续卡片效果文件及原生临时替身。清理采用 Windows 回收站；README 引用的 pet-expanded.png 保留。验收脚本默认将新截图写入系统临时目录，旧条目的图片和临时脚本路径仅是历史记录，不是当前运行入口。
- 实际执行 `npm test -- --reporter=dot`：12 文件、221 项通过；复用既有隔离环境 `run-build.ps1 -Action test`：95 单元、18 完整性、22 regression，共 135 项通过。`npm run build` 通过；`verify_pet_ui.mjs` 12 张截图、0 页面错误、0 远程请求，浏览器 smoke 通过。恢复后运行 `verify_pet_csp.mjs`：八动作完整解码、待机动画、描述/图片损坏阻止保存、经典恢复通过，0 页面错误。
- 原图集离线重建的 16 张 PNG 与目标素材逐字节及像素一致。浏览器恢复证据位于 `C:/tmp/pet-restore-20261030/browser/`；未重新启动原生应用、读写正式配置、调用真实模型或 SMTP。
- 沿用当时原包 `release-artifacts/desktop-pet-atlas-20261029/Model-IQ-Radar-v1.0.6-Windows-x64.exe`，32,723,968 字节，SHA-256 `5cd84d0b3d92dee924f8df6d63214627b341668706fb336ad678edb90dfa9b6f`；本轮未重新打包，不用后续 QA 程序冒充原版。原生透明边界、跨屏/DPI、休眠与长时性能未重验。姿态合成等旧版限制随回退保留。
- 同步皮肤契约、使用手册、设计说明和贡献指南。项目为 Vue/TypeScript/Rust，Java/JVM/MyBatis 专属条款不适用；沿用当前代码风格、UTF-8 无 BOM 和 LF。

## 2026-10-29：恢复原画图集并优化动画

- 用户明确要求恢复之前的二维原画方式，并删除全部三维相关内容。已删除对应播放器/加载器/姿态辅助、模型/纹理/建模脚本、专属测试/方案/对照文档、原交付包和本轮临时验收包；卸载运行依赖并重新生成锁文件。皮肤格式只接受 PNG 图集，内置 ID 不变，配置与数据库无需迁移。
- 新播放器在同一画布上预载后交接，加载期间旧动作继续；动作间 160ms 可打断合成、普通图集帧间插值、暂停/隐藏后保留进度，拖动速度驱动受限阻尼悬摆。两姿态轮廓在过渡期合并，结束恢复当前轮廓。它是二维关键姿态过渡，不宣称完整肢体补帧或骨骼动画。
- 六状态按定稿源像素统一缩放；坐姿不再为了铺满画布而扩大头部。原生直接拖动、皮肤保存前全动作解码、启动位置与故障恢复时序修复保留。
- 新增皮肤可选 `decorations: none/ocean/stars`：蓝鲸皮肤四角鲸尾、两侧水纹、底部铃铛；沿用配色，旧包缺省不绘制，不改数据库。装饰位于卡片内缘，完全不接收指针/辅助功能输入、不扩大原生 Region；动画开关、页面隐藏和系统减少动态效果均可停止底部低频明暗变化。前端、Rust 和打包脚本同步约束预设值。
- `python scripts/build_pet_assets.py` 重新生成原画八动作、描述和预览；`package_skin.py` 打出 19,647,947 字节的含边饰 QA 包，用于真实导入/预览/确认移除。未调用生图接口，未增加新的角色造型。
- `npm test -- --reporter=dot`：12 个文件、224 项通过。新增真实播放器的预载期间继续绘制、可见画布不重设、快速打断/迟到解码、轮廓交接、暂停/隐藏进度、非循环一次结算与卸载后错误抑制回归；新增边饰预设兼容及不受信输入验证。
- 复用既有隔离 MSVC/SDK 的 `run-build.ps1 -Action test`：95 单元 + 18 数据完整性 + 22 本机请求回归，共 135 项通过；`-Action clippy` 的 `--all-targets -- -D warnings` 通过。Java/JVM/MyBatis 专属规范不适用于 Vue/TypeScript/Rust，遵循现有 Prettier/rustfmt 与文档约定。
- `node scripts/verify_pet_ui.mjs`：12 张真实组件截图，0 页面错误、0 远程请求；验证四角装饰/无鼠标拦截、选择器未遮挡、同一角色画布、拖中通过松手后反馈、八动作、自主活动向测试让位、暂停冻结、真实资源包导入与确认移除、HTML 放大 Escape 恢复。实际截图见 `docs/screenshots/pet-*.png`，已替换旧渲染截图。
- `node scripts/verify_pet_csp.mjs`：生产 dist/CSP 下八图集实际解码、持续待机、整套保存、后续动作 PNG 损坏/描述读取失败均阻止保存及经典恢复通过。最终移除不再需要的 `blob:` 图片权限后再次执行通过。构建不包含 Three/模型加载分块；删除 Three、对应类型包及仅用于旧试验的直接 Node 类型依赖，第三方声明由生成器更新（37 前端生产包、315 Cargo 包）。
- 原生验收使用独立 `com.ahui.model-probe.petqa-20261028` 标识、虚构本机配置和 `127.0.0.1:19339` 替身。设置实际保存 192px 成功，重启含边饰验收 EXE 后仍为蓝鲸原画/192px；位置保留 `(1151,45)`。实际拖动从 `(1135,6)` 移到 `(1151,45)`，松手后原画仍可交互。原生截图观察到四角/侧边/底纹、测试中书写与“正在测试…”、约 5.02 秒后庆祝与“本次通过！”，再回侧坐；按钮可正常触发。未调用真实模型/SMTP、未读取正式数据库/凭据；验收和替身进程已停止。
- `npm run format:check`、受影响脚本/文档 Prettier、`cargo fmt -- --check` 和 `git diff --check` 通过。变更文本严格 UTF-8 无 BOM/LF、无替换字符；凭据模式扫描及桌宠文档本地链接检查通过。注释、使用说明、皮肤契约、变更日志与第三方声明同步。
- 最终 `run-build.ps1 -Action build` 完成生产标识 Windows x64 `tauri build --no-bundle`（JS 562.60 kB/gzip 202.12 kB；保留既有 chunk 大小与 MSVC 导入库提示）。`package_exe.ps1 -OutputDirectory release-artifacts/desktop-pet-atlas-20261029` 核验版本、PE x64、输入新鲜度和复制摘要：`Model-IQ-Radar-v1.0.6-Windows-x64.exe`，32,723,968 字节，SHA-256 `5cd84d0b3d92dee924f8df6d63214627b341668706fb336ad678edb90dfa9b6f`。二进制包含生产标识、不含独立 QA 标识；目录只交付 EXE，未提交/推送/发布。
- 未覆盖物理多屏/跨 DPI、休眠与长时驻留性能；原生透明区域仍为保守横带，未逐点验收全部底层穿透。原画之间仍是短姿态合成，可能短暂出现轮廓叠影，没有连续全身补帧。悬空原画中的外部示意手在画布上沿裁切，以保持角色本体与其他动作统一缩放。无数据库迁移；回滚先切经典再换旧 EXE。以下较早二维验收记录仅作历史。

## 桌宠正向流程修复与原生复核（2026-10-28）

- 用户反馈打包程序皮肤预览报 `TypeError: Failed to fetch`，保存成功后桌面仍为经典外观。根因是 Tauri CSP 的 `connect-src` 未包含 `'self'`，阻止同源皮肤 JSON，图片的 `img-src` 因而仍可显示卡片缩略图；此前 Vite/内存桥接验证没有覆盖生产 CSP，原交付不能视为桌面正向流程通过。
- 修复 `tauri.conf.json` 同源读取、`skinLoader.ts` 可读错误、Settings 保存前全部动作解码与失败不写盘/清除旧成功提示；删掉用户指定说明。角色动画由本应用的明确开关控制，默认开启，待机持续呼吸/摆尾/眨眼，测试与交互切换原图动作。未替换已确认的六姿态或引入第三方角色素材。
- 原生实测发现快速拖动接管前的位移会被消费；`PetStage` 保留拖中完整输入区域，`prepare_widget_drag` 接收可选逻辑位移并按 DPI 补偿，松手恢复轮廓与持久位置。省略参数维持经典标题栏行为。配置、数据库、备份和条件指纹无迁移。
- `npm test -- --reporter=dot`：11 文件、216 项通过；新增描述失败/后续动作损坏不保存及经典恢复，覆盖拖中完整区域、松手恢复和位移传递。复用原隔离环境 `run-build.ps1 -Action test`：93 + 18 + 22，共 133 项通过；`-Action clippy` 的 `--all-targets -- -D warnings` 通过。
- `node scripts/verify_pet_csp.mjs` 使用实际生产 dist 与生产 CSP：六种动作真实解码、两次画布不同、模拟系统减少动态效果仍持续动画、资源失败阻止保存与经典恢复，0 page error。`node scripts/verify_pet_ui.mjs`：8 张最新组件截图、0 page error、0 远程请求、控件无遮挡、拖中结算松手反馈与 HTML Escape 恢复通过；本轮未重复导入包浏览器用例，上一轮记录保留。
- 使用独立 Tauri identifier `com.ahui.model-probe.petqa-20261028` 构建并实际启动 Windows EXE，通过 Windows Computer Use 点击：打开设置 → 皮肤管理 → 选择 → 预览出现 → 保存成功 → 桌面完整角色。只读独立数据库确认皮肤已保存；重启后仍显示该角色。摸头出现闭眼姿态与气泡，雷达可展开侧坐搭接面板。未读取/修改正式数据库或真实模型凭据。
- 通过 `scripts/pet-native-mock.mjs` 的回环替身，独立配置空 Key/虚构模型，真实按钮执行两次测试，分别约 5.02 秒，历史均为 passed；定时原生截图实际观察到写字姿态与“正在测试…”，随后腾空庆祝与“本次通过！”，再回侧坐。快速拖动实测窗口从 `(1433,220)` 移至 `(1446,255)`，松手恢复待机，独立数据库记录同一新位置。未调用真实模型或 SMTP；验收进程与替身在收尾后停止。
- 最终 `run-build.ps1 -Action build` 完成生产 identifier 的 Windows x64 构建（JS 554.18 kB/gzip 199.15 kB）；仅既有 chunk 大小与 MSVC 导入库提示。`package_exe.ps1 -OutputDirectory release-artifacts/desktop-pet-fix-20261028` 核验输入新鲜度、版本、PE x64 与复制摘要。产物 `Model-IQ-Radar-v1.0.6-Windows-x64.exe`，29,985,280 字节，SHA-256 `d95f98d88dd5ea98e5c11a1e6bf59506c9839b6959238f3c008ba0c026fe983c`。保留旧预览、不提交/推送/发布；Java 专属规范不适用于 Vue/TS/Rust。
- 仍未覆盖物理多屏/DPI 切换、休眠、长时驻留性能与全部透明边界的底层点击；不宣称不存在任何问题。Windows 单屏正向流程的本次实际结果不能扩大为这些项目通过。动画仍是原图局部形变与帧动画，未制作视频级连续全身补帧。回滚先切经典，再更换旧 EXE。

## 初次桌宠预览交付（2026-10-28，后续修复见上）

- 以用户六状态定稿制作，后续明确纠正与原图不一致的素材：运行资源全部直接从 `assets/pet/blue-whale-maid/design.png` 提取，保留眨眼托腮侧坐、腾空庆祝及对应手势；不使用此前重新生成的六格替代图。展开角色搭接 420px 面板左侧，76px 留白保护原控件；经典布局仍为 360px。素材仅含局部呼吸/摆尾、待机眨眼和庆祝轻跳，不宣称已完成视频级全身补帧。
- 新增 `src/pet`、`PetActor/PetStage/SkinManager`、`pet_window.rs`、内置资源与生成/打包/验收脚本；修改窗口桥接、Widget、Settings、外观类型与校验。配置 `preferences.skin` 缺省经典；备份 v3 与重置、条件指纹独立性已有回归。未新增依赖、数据库表或 schema 迁移，未使用参考仓库角色资源，未将用户生图接口密钥写入项目。
- `npm test -- --reporter=dot`：11 个文件、214 项通过，0 失败。包含动作优先级、恢复/重复历史、拖中结算、拖后 click、资源迟到释放/坏资源回退、轮廓/预算、皮肤草稿保存/放弃；保留原测试/HTML/邮件/历史组件回归。同步修正历史页测试对异步分页的等待，以及 Vitest 5 把 beforeEach 返回函数当清理回调的测试夹具问题，未改对应 Markdown 业务逻辑。
- 复用用户指出的 `C:/Users/Admin/.codex/tmp/model-iq-radar-mail-build/run-build.ps1` 隔离 MSVC/SDK 环境，无需安装。`-Action test`：93 单元 + 18 数据完整性 + 22 本机请求回归，共 133 项通过；`-Action clippy` 使用 `--all-targets -- -D warnings` 通过。测试仅使用虚构数据、本机 HTTP Mock 与临时数据库，不调用真实模型或 SMTP，不读取生产凭据。
- `node scripts/verify_pet_ui.mjs` 配合已有 Playwright/Chrome：生成 8 张真实组件截图，0 page error、0 远程请求；检查选择器中心未被角色覆盖、拖动中通过在松手后反馈、HTML 放大后 Escape 恢复。用 `package_skin.py` 打出 16,292,596 字节的 QA 自包含包，浏览器实际解码全部 PNG 并验证导入、预览和确认移除，所有安装写入仅在独立 QA 内存。截图见 `docs/screenshots/pet-*.png`。
- `npm run format:check`、受影响文档/脚本 Prettier、rustfmt 和 `git diff --check` 通过。`-Action build` 完成 TypeScript/Vite 与 Windows x64 `tauri build --no-bundle`；JS 554.05 kB / gzip 199.13 kB，保留既有 chunk 大小和 MSVC import library 提示。构建使用真实生产桥接，QA 内存桥接不会进入 EXE。
- 最终编码审计发现生成的 `skin.json` 使用 Windows 默认 CRLF；已在生成脚本显式固定 `newline="\n"`，重新生成资源并完成原生构建。35 个变更文本均为严格 UTF-8 无 BOM/LF、带结尾换行，无替换字符；常见凭据模式扫描及方案文档本地链接检查通过。
- `scripts/package_exe.ps1 -OutputDirectory release-artifacts/desktop-pet-preview-20261028-r2` 已核对 PE x64、版本、源码/静态资源输入时间与复制 SHA-256。最终产物为 `Model-IQ-Radar-v1.0.6-Windows-x64.exe`，29,982,720 字节，SHA-256 `09efe37c1e772263912c304443c5b42304e104e3e596e31aae4dfc021457983d`。工程版本保持 1.0.6，目录明确为未发布预览；不覆盖旧正式交付物和前一轮预览，未提交、推送、Tag 或发布。
- 尚未启动本次 EXE、执行 Windows 真正点击穿透/原生拖动、DPI/物理多屏、休眠和长时驻留性能验收；这些不由浏览器或编译成功代替。已保留经典回退，不能把本轮描述为原生桌面全量验收通过。首次启用在设置 → 皮肤管理保存“蓝鲸女仆”；回滚先切经典，再更换旧 EXE。文档/注释同步见 `docs/DESKTOP_PET_DESIGN.md`、`docs/SKIN_PACKS.md`、使用手册、贡献指南和 CHANGELOG。

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
