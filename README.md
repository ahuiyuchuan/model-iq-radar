<div align="center">

<img src="src-tauri/icons/128x128.png" alt="测智雷达" width="88" height="88" />

# 测智雷达 · Model IQ Radar

### 用你的问题，观察模型的每一次回答

通过自定义测试题目与连续字符串规则，记录 AI 模型回答的变化。一个基于 Tauri 2、Vue 3、TypeScript 和 Rust 的 Windows 桌面悬浮工具。

[![Version](https://img.shields.io/github/v/release/ahuiyuchuan/model-iq-radar?color=blue&label=version)](https://github.com/ahuiyuchuan/model-iq-radar/releases)
[![Platform](https://img.shields.io/badge/platform-Windows%20x64-lightgrey)](https://github.com/ahuiyuchuan/model-iq-radar/releases/latest)
[![Tauri](https://img.shields.io/badge/built%20with-Tauri%202-orange)](https://tauri.app/)
[![Downloads](https://img.shields.io/github/downloads/ahuiyuchuan/model-iq-radar/total)](https://github.com/ahuiyuchuan/model-iq-radar/releases)
[![License](https://img.shields.io/badge/license-GPL--3.0--only-blue)](LICENSE)

**[下载免安装版](#下载安装) · [快速开始](#快速开始) · [功能特性](#功能特性) · [常见问题](#常见问题) · [使用手册](docs/USER_GUIDE.md) · [更新日志](CHANGELOG.md)**

</div>

## 为什么选择测智雷达？

- **题目与规则可控**：完整原始回答包含配置的连续字符串即命中，支持自定义题目和判定内容。
- **留在桌面，随时查看**：紧凑悬浮、展开详情、边缘小图标三种状态，悬停恢复，测试中显示动画。
- **持续记录变化**：默认每三分钟自动测试，手动测试重置周期，历史保留当次规则和原文。
- **本地保存数据**：配置与历史存入 SQLite，API Key 单独存入 Windows 凭据管理器。

> **适用范围：适用于 GPT 6 及以上系列模型，其他模型无法保证测试准确度。** 这是产品预设说明，尚无统计准确率验证。结果只表示本次原始回答是否命中自定义字符串，不代表模型整体能力，也不能证明服务商存在或不存在降智行为。

## 界面预览

所有预览使用虚构配置和测试内容。

|                                    紧凑悬浮                                    |                                        展开回答                                        |
| :----------------------------------------------------------------------------: | :------------------------------------------------------------------------------------: |
| <img src="docs/screenshots/widget-compact.png" alt="紧凑悬浮窗" width="360" /> | <img src="docs/screenshots/widget-expanded.png" alt="Markdown 回答详情" width="360" /> |

<details>
<summary>查看设置、历史筛选和关于页面</summary>

![悬浮设置与开机自启](docs/screenshots/settings.png)
![历史分组筛选](docs/screenshots/history.png)
![关于与检查更新](docs/screenshots/about.png)

</details>

## 下载安装

### 系统要求

- Windows 10 / 11，x64 架构。
- 已安装 Microsoft Edge WebView2 Runtime；如缺失，请从 [微软官网](https://developer.microsoft.com/microsoft-edge/webview2/)安装。
- 使用远程模型时需可访问对应 API，模型费用由服务商收取。

### Windows 免安装版

从 [Releases](https://github.com/ahuiyuchuan/model-iq-radar/releases/latest) 下载 `Model-IQ-Radar-v1.0.0-Windows-x64-Portable.zip`，解压到固定目录后运行其中的 `测智雷达.exe`。无需安装 Node.js、Rust 或其他开发工具。

同时提供 `SHA256SUMS.txt` 用于校验。源码以仓库与 `v1.0.0` Tag 为准；GitHub 自动生成的 Source code 压缩包不包含已编译的应用。本版仅提供 Windows x64 免安装包。

“免安装”指无需安装程序，配置仍保存到用户数据目录，API Key 保存到系统凭据管理器。更新时退出旧版再替换 EXE，现有配置和历史继续沿用。

## 快速开始

1. 运行应用，点击右上角“设置”，在“接口配置”填写分组名称、API 地址和密钥。
2. 点击“拉取模型”并选择模型 ID，或手动输入；“测试连接”会发送独立短消息，确认模型可响应，可能产生少量 API 费用。
3. 保存配置。在“测试规则”自定义题目与连续字符串，保存后生效。
4. 回到悬浮窗点击“开始测试”，使用右上角箭头查看回答。未命中显示“降智”，命中显示“不降智”；网络、协议、截断和空响应等异常显示“测试失败”。
5. 在“悬浮设置”调整定时测试、边缘吸附、请求超时等；“开机自启”切换立即生效，其余设置保存后生效。

接口地址示例：`https://api.example.com/v1`，也可填写完整的 `/chat/completions` 路径。应用不会猜测或自动添加 `/v1`；无鉴权的本地服务可以不填密钥，仅本机 HTTP 地址被允许。当前支持 Chat Completions 兼容协议，不支持网页订阅登录、Responses 或 Anthropic 原生协议。

默认测试题目如下，正文五行无空行，可在设置中修改：

```text
请快速按顺序回答：
1. 仅依据你的记忆，你所知道的最新一位日本首相是谁？
要求：
- 全程禁止联网、搜索互联网、读取本机文件。
- 只能依据已有的模型知识回答，不得查询或核实实时信息。
```

默认判定字符串为 `高市早苗`。题目里的“禁止联网”约束被测模型；客户端必须联网请求远程 API，无法核实服务商内部是否启用了隐藏检索。

## 功能特性

| 模块       | 功能                                                                              |
| ---------- | --------------------------------------------------------------------------------- |
| 模型与接口 | 多分组配置、模型列表拉取、深色选择器、独立连接测试、密钥安全存储                  |
| 自定义判定 | 自定义题目及连续字符串，区分大小写、保留有效空白，仅依据完整原始最终回答          |
| 测试调度   | 手动测试、取消、定时测试，默认3分钟；手动开始重置计时，忙碌时跳过，不积压补跑     |
| 桌面交互   | 同宽紧凑/展开视图、固定顶部按钮、左/右/顶部吸边、动画收起、悬停恢复、托盘与快捷键 |
| 状态与时间 | 测试时间、请求耗时、自动测试倒计时；边缘图标按结果变色，测试中圆环旋转            |
| 回答显示   | CommonMark + GFM、只读任务列表、表格、代码本地高亮，表格/代码块内部横向滚动       |
| 历史       | 最近500条、分组筛选与搜索组合、规则快照、Markdown详情、原文复制                   |
| 系统设置   | 置顶、透明度、请求超时、开机自启；设置窗口不透明，底部操作栏固定                  |
| 关于与更新 | 版本、项目链接、反馈入口、手动检查正式Release；系统浏览器下载，无自动安装         |

### 回答与安全边界

- Markdown **只改变显示**。保存、复制和判定始终使用完整原文。例如 `**高市**早苗` 看起来连续，但原文不包含 `高市早苗`，不会命中该规则。
- 禁用原始 HTML 和危险链接协议，远程图片只显示说明，不自动加载；解析和高亮均在本机完成，失败回退纯文本。
- 测智与连接测试使用相同的已保存请求超时（默认60秒，可设10—300秒）；拉取模型独立使用15秒总体超时。
- 只发送独立用户消息，不携带历史、判定字符串、工具或本机文件内容；测试连接只发送“请只回复 OK。”。
- 密钥不写入配置数据库、测试历史或应用日志。浏览器开发预览不调用模型 API、不读写系统自启，也不持久化密钥。

更多约束、超长回答降级、数据备份和回滚方式见 [使用手册](docs/USER_GUIDE.md)。

## 常见问题

<details>
<summary><strong>“不降智”是否代表模型整体能力正常？</strong></summary>

不代表。它只表示本次完整原始回答包含设定字符串；“降智”只表示未命中。单题关键词结果不是综合能力评测，接口异常、截断或空响应也不会被归为降智。

</details>

<details>
<summary><strong>拉取模型失败怎么办？连接测试会写历史吗？</strong></summary>

部分服务不提供 `/models`，仍可手动填写准确的模型 ID。连接测试发送独立短消息，可能计费，不执行测智判定、不写历史、不重置自动周期。超时沿用“悬浮设置”里已保存的请求超时。

</details>

<details>
<summary><strong>数据存在哪里？换电脑如何迁移？</strong></summary>

配置与历史位于 `%APPDATA%\com.ahui.model-probe\probe.db`，API Key 位于 Windows 凭据管理器。退出应用后可备份数据目录；该备份不含系统凭据，换电脑或 Windows 账户后需要重新输入密钥。

</details>

<details>
<summary><strong>开机自启为什么要求固定 EXE 位置？</strong></summary>

自启项指向当前 EXE 的完整路径。移动、重命名后请在新位置重新开启；删除程序或回滚到旧版前先关闭自启。仅影响当前 Windows 用户登录，不需要管理员权限。

</details>

<details>
<summary><strong>关闭窗口后为什么还在运行？</strong></summary>

关闭悬浮窗会隐藏到托盘，定时测试继续。完全退出请使用托盘菜单。快捷键 `Ctrl + Alt + P` 可显示/隐藏悬浮窗。

</details>

<details>
<summary><strong>检查更新是否自动下载、覆盖程序？</strong></summary>

不会。只有点击检查更新才请求本项目 GitHub 正式 Release，按语义版本比较；无发布、网络失败和已是最新分开提示。用户主动点击后通过系统浏览器下载。同为1.0.0的不同构建不会互相提示更新。

</details>

## 开发与贡献

欢迎提交 [Issue](https://github.com/ahuiyuchuan/model-iq-radar/issues) 或 Pull Request。开发依赖、目录职责、构建命令及验证边界见 [CONTRIBUTING.md](CONTRIBUTING.md) 与 [VALIDATION.md](VALIDATION.md)。反馈请使用虚构配置或脱敏内容，不要提交 API Key、数据库或凭据。

**技术栈**：Tauri 2 · Vue 3 · TypeScript · Rust · SQLite · Windows 凭据管理器。

## 致谢

[CC Switch](https://github.com/farion1231/cc-switch) 的模型列表交互、关于页内容元素，以及 README 和 Release 组织方式为本项目提供了参考。本项目的功能范围、品牌和发布内容独立维护；未复制其赞助商、下载平台或用户数据。

第三方依赖许可证见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。

## License

Copyright © 2026 ahui。项目采用 **GNU General Public License v3.0 only（GPL-3.0-only）**，完整条款见 [LICENSE](LICENSE)。第三方依赖保留各自许可证。
