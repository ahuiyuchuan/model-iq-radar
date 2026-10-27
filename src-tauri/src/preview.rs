//! 页面内沙箱 iframe 的临时文档仓库，不创建窗口、进程或截图文件。
//! 响应头 CSP 不可被模型放宽；文档绑定宿主窗口，关闭后幂等释放。作者：ahui。

use std::{collections::HashMap, sync::Mutex};
use tauri::{Manager, State, WebviewWindow};

/// 不透明源只允许内联代码和内存媒体，禁止网络、IPC 和嵌套页面。
const CSP: &str = "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data: blob:; font-src data:; media-src data: blob:; connect-src 'none'; frame-src 'none'; child-src 'none'; worker-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'";
const MAX_DOCUMENT: usize = 2 * 1024 * 1024;
// 可见历史行与悬浮卡片共用仓库；保留有界容量，避免六行历史加一张卡片误报忙碌。
const MAX_DOCUMENTS: usize = 16;
const MAX_TOTAL: usize = 16 * 1024 * 1024;

struct Document {
    owner: String,
    html: String,
}

/// 最多十六份/16 MiB；由前端卸载或宿主销毁释放，不设置运行/文档过期时长。
/// 保留页面期间允许继续放大、收回；不承诺共享渲染线程的硬终止。
#[derive(Default)]
pub struct PreviewState {
    documents: Mutex<HashMap<String, Document>>,
    secured: Mutex<HashMap<String, bool>>,
}

impl PreviewState {
    /// 原生销毁可能不会触发 Vue 卸载；补充按宿主释放，避免关闭设置窗口后遗留文档占用。
    /// 锁异常时只做尽力清理，剩余内存随进程退出释放，不阻止窗口正常销毁。
    pub fn release_owner(&self, owner: &str) {
        if let Ok(mut documents) = self.documents.lock() {
            documents.retain(|_, document| document.owner != owner);
        }
        if let Ok(mut secured) = self.secured.lock() {
            secured.remove(owner);
        }
    }
}

/// 接收已包装文档；随机 URL 只由所属宿主加载，不能跨窗口读取另一份回答。
#[tauri::command]
pub fn prepare_html_preview(
    window: WebviewWindow,
    state: State<'_, PreviewState>,
    document: String,
    source: String,
) -> Result<String, String> {
    let owner = window.label();
    if !matches!(owner, "widget" | "settings")
        || !state
            .secured
            .lock()
            .map_err(|_| "预览状态不可用。")?
            .get(owner)
            .copied()
            .unwrap_or(false)
    {
        return Err("沙箱安全设置尚未就绪，无法运行预览。".into());
    }
    if document.is_empty() || document.len() > MAX_DOCUMENT {
        return Err("HTML 文档为空或超过预览容量。".into());
    }
    if source.is_empty() || source.len() > 1024 * 1024 {
        return Err("HTML 为空或超过 1 MiB 预览上限。".into());
    }
    if let Some(issue) = crate::html::preview_issue(&source) {
        return Err(issue.into());
    }
    let mut documents = state.documents.lock().map_err(|_| "预览状态不可用。")?;
    if documents.len() >= MAX_DOCUMENTS
        || documents
            .values()
            .map(|item| item.html.len())
            .sum::<usize>()
            + document.len()
            > MAX_TOTAL
    {
        return Err("预览正在忙碌，请关闭其他预览后重试。".into());
    }
    let id = uuid::Uuid::new_v4().to_string();
    documents.insert(
        id.clone(),
        Document {
            owner: owner.into(),
            html: document,
        },
    );
    Ok(format!("http://html-preview.localhost/{id}"))
}

/// 只删除所属宿主的内存文档；未知或迟到 URL 可安全重复释放。
#[tauri::command]
pub fn release_html_preview(
    window: WebviewWindow,
    state: State<'_, PreviewState>,
    url: String,
) -> Result<(), String> {
    let Some(id) = document_id(&url) else {
        return Ok(());
    };
    let mut documents = state.documents.lock().map_err(|_| "预览状态不可用。")?;
    if documents
        .get(id)
        .is_some_and(|item| item.owner == window.label())
    {
        documents.remove(id);
    }
    Ok(())
}

/// 严格匹配本地随机文档路径，拒绝查询、片段、伪造主机和多级路径。
fn document_id(url: &str) -> Option<&str> {
    // Wry 把 Windows 伪 HTTP 地址还原成协议 URI 后再调用 handler；导航事件仍是 HTTP 地址。
    let id = url
        .strip_prefix("http://html-preview.localhost/")
        .or_else(|| url.strip_prefix("html-preview://localhost/"))?;
    (id.len() == 36 && uuid::Uuid::parse_str(id).is_ok()).then_some(id)
}
fn allowed(state: &PreviewState, owner: &str, url: &str) -> bool {
    document_id(url).is_some_and(|id| {
        state
            .documents
            .lock()
            .is_ok_and(|docs| docs.get(id).is_some_and(|doc| doc.owner == owner))
    })
}

/// 不设置 CORS；sandbox 不开放 allow-same-origin，宿主和模型不能互读 DOM。
pub fn respond(app: &tauri::AppHandle, owner: &str, url: &str) -> tauri::http::Response<Vec<u8>> {
    let state = app.state::<PreviewState>();
    let html = document_id(url).and_then(|id| {
        state.documents.lock().ok().and_then(|docs| {
            docs.get(id)
                .filter(|doc| doc.owner == owner)
                .map(|doc| doc.html.clone())
        })
    });
    tauri::http::Response::builder()
        .status(if html.is_some() { 200 } else { 404 })
        .header("Content-Type", "text/html; charset=utf-8")
        .header("Content-Security-Policy", CSP)
        .header("Cache-Control", "no-store")
        .header("X-Content-Type-Options", "nosniff")
        .body(html.unwrap_or_default().into_bytes())
        .expect("固定合法响应头")
}

/// 在既有 WebView 上限制子框架导航，不创建 WebView2 子窗口。
/// CSP 阻止资源请求；此回调另阻止 location/meta refresh 离开受控文档。
/// iframe 共享浏览器渲染资源，恶意同步死循环仍可能影响宿主，不再承诺独立进程硬隔离。
pub fn secure(window: &WebviewWindow) -> Result<(), String> {
    #[cfg(windows)]
    {
        use webview2_com::{
            take_pwstr, DownloadStartingEventHandler,
            Microsoft::Web::WebView2::Win32::{
                ICoreWebView2Settings3, ICoreWebView2_4, ICoreWebView2_9,
            },
            NavigationStartingEventHandler,
        };
        use windows::core::Interface;
        let app = window.app_handle().clone();
        let owner = window.label().to_owned();
        window
            .with_webview(move |webview| {
                // COM 回调只读取文档表，不访问模型 DOM；失败时 prepare 命令拒绝执行预览。
                let result = unsafe {
                    (|| -> windows::core::Result<()> {
                        let core = webview.controller().CoreWebView2()?;
                        // 宿主是桌面应用而非浏览器：关闭右键保存/打印/刷新菜单和浏览器快捷入口。
                        // Ctrl+C/V/A 等编辑快捷键仍由输入控件处理；应用自己的复制/导出入口继续使用。
                        let settings = core.Settings()?;
                        settings.SetAreDefaultContextMenusEnabled(false)?;
                        settings
                            .cast::<ICoreWebView2Settings3>()?
                            .SetAreBrowserAcceleratorKeysEnabled(false)?;
                        // 导出仍由现有 Blob 下载流程保存，只接管默认下载 UI，不取消正常导出。
                        // 避免另存为或导出后出现覆盖应用的 Edge 下载浮层；也关闭此前残留的浮层。
                        core.cast::<ICoreWebView2_9>()?
                            .CloseDefaultDownloadDialog()?;
                        core.cast::<ICoreWebView2_4>()?.add_DownloadStarting(
                            &DownloadStartingEventHandler::create(Box::new(|_, args| {
                                if let Some(args) = args {
                                    args.SetHandled(true)?;
                                }
                                Ok(())
                            })),
                            &mut 0,
                        )?;
                        let callback_app = app.clone();
                        let callback_owner = owner.clone();
                        core.add_FrameNavigationStarting(
                            &NavigationStartingEventHandler::create(Box::new(move |_, args| {
                                if let Some(args) = args {
                                    let mut uri = windows::core::PWSTR::null();
                                    args.Uri(&mut uri)?;
                                    let url = take_pwstr(uri);
                                    args.SetCancel(!allowed(
                                        &callback_app.state::<PreviewState>(),
                                        &callback_owner,
                                        &url,
                                    ))?;
                                }
                                Ok(())
                            })),
                            &mut 0,
                        )?;
                        Ok(())
                    })()
                };
                if let Ok(mut secured) = app.state::<PreviewState>().secured.lock() {
                    secured.insert(owner, result.is_ok());
                }
            })
            .map_err(|_| "无法配置沙箱导航保护。")?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        Err("当前平台未配置 HTML 沙箱导航保护。".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn documents_are_owner_bound() {
        let state = PreviewState::default();
        let id = uuid::Uuid::new_v4().to_string();
        let url = format!("http://html-preview.localhost/{id}");
        state.documents.lock().unwrap().insert(
            id,
            Document {
                owner: "widget".into(),
                html: "demo".into(),
            },
        );
        assert!(allowed(&state, "widget", &url));
        for (owner, value) in [
            ("settings", url.clone()),
            ("widget", url.clone() + "?x"),
            ("widget", url.replace("http:", "https:")),
            ("widget", "https://example.com".into()),
        ] {
            assert!(!allowed(&state, owner, &value));
        }
        assert!(CSP.contains("connect-src 'none'"));
        assert!(!CSP.contains("unsafe-eval"));
    }
}
