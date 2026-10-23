# 依赖许可证补充

整理：ahui。用于补充部分 crate 发布包未携带的完整许可证文件，不修改原文。生成器读取这些副本，将其与依赖目录中的声明一起收录进根目录 THIRD_PARTY_NOTICES.md。

| 本地文件 | 对应依赖 | 上游或标准文本来源 |
| --- | --- | --- |
| alloc-stdlib-LICENSE | alloc-stdlib 0.3.0 | https://github.com/dropbox/rust-alloc-no-stdlib/blob/master/LICENSE |
| tauri-LICENSE-MIT | tauri-plugin 2.7.2 | https://github.com/tauri-apps/tauri/blob/dev/LICENSE-MIT |
| webview2-LICENSE | webview2-com / webview2-com-macros / webview2-com-sys | https://github.com/wravery/webview2-rs/blob/main/LICENSE |
| selectors-LICENSE-MPL-2.0 | selectors 0.38.0 | 标准 MPL-2.0 全文，使用锁定的 DOMPurify 包内 LICENSE-MPL 原文；selectors 源码文件头明确声明 MPL-2.0 |

依赖自身的版本与许可证字段以 package-lock.json / Cargo.lock 及其包内元数据为准。这里保留声明，不把本项目 GPL-3.0-only 应用于第三方代码。
