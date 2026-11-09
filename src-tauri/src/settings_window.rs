//! 单一设置窗口的创建、恢复和关闭重开边界，不读写配置或模型结果。
//! Windows 复用主窗口的 WebView2 环境，避免常驻进程在 Runtime 更新后重新协商同一数据目录。
//! 窗口操作与等待均在异步门禁内串行；原生边界可替换，以便不操作用户桌面完成回归。
//! @author ahui

use tauri::{Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// 只为已开始销毁但尚未注销的同名窗口短暂等待，不对其他创建失败自动重试。
const CLOSE_RECHECKS: usize = 20;
const CLOSE_RECHECK_DELAY: std::time::Duration = std::time::Duration::from_millis(25);

/// 应用进程唯一的设置生命周期门禁；不与运行任务或存储锁相互嵌套。
#[derive(Default)]
pub struct SettingsWindows {
    /// 使用异步锁，连续点击和托盘请求不会阻塞 Tokio 工作线程或主窗口的事件处理。
    gate: tokio::sync::Mutex<()>,
}

/// 将可等待的标签注销与普通创建失败分开，不能根据错误文本推测是否应反复重试。
struct OpenError {
    message: String,
    closing: bool,
}

/// 只抽象窗口生命周期边界；原生实现负责尺寸、环境、导航保护和可读错误。
trait WindowHost {
    type Window: Clone;
    fn find(&self) -> Option<Self::Window>;
    /// 返回本次创建的隐藏窗口；失败不修改配置，只有标签冲突可声明正在关闭。
    async fn create(&self) -> Result<Self::Window, OpenError>;
    fn secure(&self, window: &Self::Window) -> Result<(), String>;
    fn present(&self, window: &Self::Window) -> Result<(), String>;
    fn discard_new(&self, window: &Self::Window) -> Result<(), String>;
    fn report(&self, result: &Result<(), String>);
}

impl SettingsWindows {
    /// 门禁同时覆盖查找、创建、展示和结果事件，跨入口结果按实际完成顺序到达。
    async fn open_with(&self, host: &impl WindowHost) -> Result<(), String> {
        let _gate = self.gate.lock().await;
        let result = self.ensure_open(host).await;
        host.report(&result);
        result
    }

    /// 现有窗口优先恢复，绝不销毁含草稿的窗口；只回收本次尚未交给用户的新窗口。
    async fn ensure_open(&self, host: &impl WindowHost) -> Result<(), String> {
        for attempt in 0..=CLOSE_RECHECKS {
            if let Some(window) = host.find() {
                match host.present(&window) {
                    Ok(()) => return Ok(()),
                    // 操作期间销毁完成，下一步允许创建；仍存活则保留草稿并返回阶段错误。
                    Err(message) if host.find().is_some() => return Err(message),
                    Err(_) => (),
                }
            }
            match host.create().await {
                Ok(window) => {
                    let result = host.secure(&window).and_then(|_| host.present(&window));
                    if let Err(message) = &result {
                        if let Err(cleanup) = host.discard_new(&window) {
                            return Err(format!("{message} {cleanup}"));
                        }
                    }
                    return result;
                }
                Err(error) if error.closing && attempt < CLOSE_RECHECKS => {
                    tokio::time::sleep(CLOSE_RECHECK_DELAY).await;
                }
                Err(error) => return Err(error.message),
            }
        }
        unreachable!("最后一次失败始终返回错误")
    }
}

/// 所有设置入口共用同一状态，不依赖前端按钮防抖保证窗口唯一性。
/// 成功表示已恢复并请求聚焦；失败为可直接显示的阶段提示，不会触发模型测试或提交草稿。
pub async fn open(app: &tauri::AppHandle) -> Result<(), String> {
    app.state::<SettingsWindows>()
        .open_with(&NativeHost(app))
        .await
}

/// 借用本次命令持有的应用句柄；不缓存窗口句柄，避免关闭重开后操作过期实例。
struct NativeHost<'a>(&'a tauri::AppHandle);

impl WindowHost for NativeHost<'_> {
    type Window = WebviewWindow;

    fn find(&self) -> Option<Self::Window> {
        self.0.get_webview_window("settings")
    }

    async fn create(&self) -> Result<Self::Window, OpenError> {
        let widget = self
            .0
            .get_webview_window("widget")
            .ok_or_else(|| OpenError {
                message: "主窗口尚未就绪，无法打开设置。请完全退出后重开应用。".into(),
                closing: false,
            })?;
        // 保留原尺寸与高 DPI 工作区余量，不因本次生命周期修复重排设置页面。
        let (width, height) = super::work_area(&widget)
            .ok()
            .zip(widget.scale_factor().ok())
            .filter(|(_, scale)| scale.is_finite() && *scale > 0.0)
            .map(|((left, top, right, bottom), scale)| {
                (
                    1120.0_f64.min(((right - left) as f64 / scale - 40.0).max(700.0)),
                    900.0_f64.min(((bottom - top) as f64 / scale - 40.0).max(520.0)),
                )
            })
            .unwrap_or((1120.0, 900.0));

        let builder = WebviewWindowBuilder::new(
            self.0,
            "settings",
            WebviewUrl::App("index.html?view=settings".into()),
        )
        .title("测智雷达 · 设置")
        .inner_size(width, height)
        .min_inner_size(700.0, 520.0)
        .decorations(false)
        .transparent(true)
        .visible(false)
        .focused(false)
        .center();

        #[cfg(windows)]
        let builder = {
            let (send, receive) = tokio::sync::oneshot::channel();
            widget
                .with_webview(move |webview| {
                    // COM 环境只在所属主线程取得、由 Tauri 在同一主线程用于新 Controller。
                    // 使用已锁定 runtime 的 Send 容器转交，应用层不声明 COM Send 或调用跨线程 COM。
                    let mut attributes = tauri_runtime::webview::WebviewAttributes::new(
                        WebviewUrl::App("index.html".into()),
                    );
                    attributes.environment = Some(webview.environment());
                    let _ = send.send(attributes);
                })
                .map_err(|error| creation_error("读取主窗口运行环境", &error))?;
            let attributes = receive.await.map_err(|_| OpenError {
                message: "主窗口运行环境已关闭，无法打开设置。请完全退出后重开应用。".into(),
                closing: false,
            })?;
            // 仅传递句柄；build 保持在异步命令线程，不能放进 with_webview 回调导致主循环重入。
            builder.with_environment(attributes.environment.expect("只发送含环境的属性"))
        };

        builder
            .build()
            .map_err(|error| creation_error("创建设置窗口", &error))
    }

    fn secure(&self, window: &Self::Window) -> Result<(), String> {
        super::preview::secure(window)
    }

    fn present(&self, window: &Self::Window) -> Result<(), String> {
        // 先恢复最小化再显示/聚焦，每阶段都检查；失败不能伪装成已经成功打开。
        window
            .unminimize()
            .map_err(|error| stage_error("恢复设置窗口", &error))?;
        window
            .show()
            .map_err(|error| stage_error("显示设置窗口", &error))?;
        window
            .set_focus()
            .map_err(|error| stage_error("聚焦设置窗口", &error))
    }

    fn discard_new(&self, window: &Self::Window) -> Result<(), String> {
        window
            .destroy()
            .map_err(|error| stage_error("回收未就绪的设置窗口", &error))
    }

    fn report(&self, result: &Result<(), String>) {
        // null 清除旧的设置故障；不携带配置/凭据，不覆盖模型测试与已保存结果。
        let _ = self
            .0
            .emit_to("widget", "settings-window-result", result.as_ref().err());
    }
}

/// 仅同名标签冲突可等待销毁完成；其他环境/资源错误返回用户，不循环创建浏览器环境。
fn creation_error(stage: &str, error: &tauri::Error) -> OpenError {
    OpenError {
        message: stage_error(stage, error),
        closing: matches!(
            error,
            tauri::Error::WindowLabelAlreadyExists(_) | tauri::Error::WebviewLabelAlreadyExists(_)
        ),
    }
}

/// 只公开阶段和标准 HRESULT，不将可能含本机路径或 URL 的底层整段错误送往界面/日志。
fn stage_error(stage: &str, error: &tauri::Error) -> String {
    let raw = error.to_string();
    let code = raw
        .as_bytes()
        .windows(10)
        .find(|part| part.starts_with(b"0x") && part[2..].iter().all(u8::is_ascii_hexdigit));
    let code = code
        .map(|value| format!("（错误码 {}）", String::from_utf8_lossy(value)))
        .unwrap_or_default();
    format!("无法{stage}{code}。请再次点击设置；若持续失败，请从托盘完全退出后重开应用。")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Mutex,
    };

    /// 模拟原生生命周期，不创建 WebView、访问用户配置或修改桌面状态。
    #[derive(Default)]
    struct Host {
        exists: AtomicBool,
        creates: AtomicUsize,
        presents: AtomicUsize,
        discards: AtomicUsize,
        closing: Mutex<usize>,
        fail_create: AtomicBool,
        fail_secure: AtomicBool,
        fail_present: AtomicBool,
        vanish_on_present: AtomicBool,
        reports: Mutex<Vec<bool>>,
    }

    impl WindowHost for Host {
        type Window = ();
        fn find(&self) -> Option<()> {
            self.exists.load(Ordering::SeqCst).then_some(())
        }
        async fn create(&self) -> Result<(), OpenError> {
            self.creates.fetch_add(1, Ordering::SeqCst);
            tokio::task::yield_now().await;
            let mut closing = self.closing.lock().unwrap();
            if *closing > 0 {
                *closing -= 1;
                return Err(OpenError {
                    message: "标签尚未注销".into(),
                    closing: true,
                });
            }
            if self.fail_create.load(Ordering::SeqCst) {
                return Err(OpenError {
                    message: "运行环境失败".into(),
                    closing: false,
                });
            }
            assert!(
                !self.exists.swap(true, Ordering::SeqCst),
                "不能创建第二个窗口"
            );
            Ok(())
        }
        fn secure(&self, _: &()) -> Result<(), String> {
            if self.fail_secure.load(Ordering::SeqCst) {
                Err("安全配置失败".into())
            } else {
                Ok(())
            }
        }
        fn present(&self, _: &()) -> Result<(), String> {
            self.presents.fetch_add(1, Ordering::SeqCst);
            if self.vanish_on_present.swap(false, Ordering::SeqCst) {
                self.exists.store(false, Ordering::SeqCst);
                return Err("旧窗口已销毁".into());
            }
            if self.fail_present.load(Ordering::SeqCst) {
                Err("聚焦失败".into())
            } else {
                Ok(())
            }
        }
        fn discard_new(&self, _: &()) -> Result<(), String> {
            self.discards.fetch_add(1, Ordering::SeqCst);
            self.exists.store(false, Ordering::SeqCst);
            Ok(())
        }
        fn report(&self, result: &Result<(), String>) {
            self.reports.lock().unwrap().push(result.is_ok());
        }
    }

    #[tokio::test]
    async fn concurrent_entries_share_one_window_and_preserve_existing_drafts() {
        let state = SettingsWindows::default();
        let host = Host::default();
        let (a, b, c) = tokio::join!(
            state.open_with(&host),
            state.open_with(&host),
            state.open_with(&host)
        );
        assert!(a.is_ok() && b.is_ok() && c.is_ok());
        assert_eq!(host.creates.load(Ordering::SeqCst), 1);
        assert_eq!(host.presents.load(Ordering::SeqCst), 3);
        assert_eq!(host.discards.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn closing_label_is_rechecked_but_other_creation_errors_are_not_retried() {
        let state = SettingsWindows::default();
        let host = Host::default();
        *host.closing.lock().unwrap() = 2;
        state.open_with(&host).await.unwrap();
        assert_eq!(host.creates.load(Ordering::SeqCst), 3);
        host.exists.store(false, Ordering::SeqCst);
        host.fail_create.store(true, Ordering::SeqCst);
        assert_eq!(state.open_with(&host).await.unwrap_err(), "运行环境失败");
        assert_eq!(host.creates.load(Ordering::SeqCst), 4);
        host.fail_create.store(false, Ordering::SeqCst);
        state.open_with(&host).await.unwrap();
        assert_eq!(*host.reports.lock().unwrap(), [true, false, true]);
    }

    #[tokio::test(start_paused = true)]
    async fn label_wait_is_bounded_and_does_not_keep_the_gate_after_failure() {
        let state = SettingsWindows::default();
        let host = Host::default();
        *host.closing.lock().unwrap() = 100;
        assert!(state.open_with(&host).await.is_err());
        assert_eq!(host.creates.load(Ordering::SeqCst), CLOSE_RECHECKS + 1);
        *host.closing.lock().unwrap() = 0;
        state.open_with(&host).await.unwrap();
    }

    #[tokio::test]
    async fn destroyed_during_restore_is_recreated_but_live_drafts_are_never_discarded() {
        let state = SettingsWindows::default();
        let host = Host::default();
        host.exists.store(true, Ordering::SeqCst);
        host.vanish_on_present.store(true, Ordering::SeqCst);
        state.open_with(&host).await.unwrap();
        assert_eq!(host.creates.load(Ordering::SeqCst), 1);
        host.fail_present.store(true, Ordering::SeqCst);
        assert!(state.open_with(&host).await.is_err());
        assert!(host.exists.load(Ordering::SeqCst));
        assert_eq!(host.discards.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn failed_new_window_is_reclaimed_before_user_retry() {
        for security in [true, false] {
            let state = SettingsWindows::default();
            let host = Host::default();
            host.fail_secure.store(security, Ordering::SeqCst);
            host.fail_present.store(!security, Ordering::SeqCst);
            assert!(state.open_with(&host).await.is_err());
            assert_eq!(host.discards.load(Ordering::SeqCst), 1);
            assert!(!host.exists.load(Ordering::SeqCst));
            host.fail_secure.store(false, Ordering::SeqCst);
            host.fail_present.store(false, Ordering::SeqCst);
            state.open_with(&host).await.unwrap();
        }
    }

    #[test]
    fn native_errors_expose_only_stage_and_hresult() {
        let error = tauri::Error::Io(std::io::Error::other(
            "C:/private/profile?key=secret 0x8007139F",
        ));
        let message = stage_error("创建设置窗口", &error);
        assert!(message.contains("0x8007139F") && message.contains("再次点击设置"));
        assert!(!message.contains("private") && !message.contains("secret"));
        assert!(
            creation_error(
                "创建设置窗口",
                &tauri::Error::WindowLabelAlreadyExists("settings".into())
            )
            .closing
        );
        assert!(!creation_error("创建设置窗口", &error).closing);
    }
}
