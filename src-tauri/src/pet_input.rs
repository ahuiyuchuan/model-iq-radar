//! 桌宠输入掩码：显示保持完整透明表面，轮廓仅控制光标穿透。作者：ahui。
//!
//! 所有窗口操作在主线程执行；每次采样确认后才排下一次，无队列积压。
//! 退出/HTML预览递增世代，旧闭包不能覆盖新皮肤的输入模式。不读凭据或持久化配置。

use super::PetWindowState;
use tauri::{Manager, WebviewWindow};
use windows_sys::Win32::{
    Foundation::POINT,
    Graphics::Gdi::{ScreenToClient, SetWindowRgn},
    UI::{
        Input::KeyboardAndMouse::GetAsyncKeyState,
        WindowsAndMessaging::{GetCursorPos, IsWindow, IsWindowVisible},
    },
};

/// mask=None表示经典/HTML完整输入；世代与输入锁独立于场景尺寸缓存。
#[derive(Default)]
pub(super) struct InputState {
    mask: Option<Vec<[f64; 4]>>,
    generation: u64,
    ignored: bool,
    press: PressOwnership,
}

/// 只保留在本窗开始的左键按下；从其他应用按住移入不能被桌宠抢走。
#[derive(Default)]
struct PressOwnership {
    down: bool,
    owned: bool,
}

impl PressOwnership {
    /// 自有拖动始终保留输入；释放后立即恢复轮廓命中，避免卡住透明穿透。
    fn update(&mut self, down: bool, hit: bool, ignored: bool, dragging: bool) -> bool {
        if !down {
            self.owned = false;
        } else if !self.down {
            self.owned = hit && !ignored;
        }
        self.down = down;
        dragging || self.owned || hit
    }
}

/// 与Canvas轮廓相同的向外取整；使用客户区物理坐标，支持负屏幕坐标和实时DPI。
fn contains(mask: &[[f64; 4]], cursor: (i32, i32), scale: f64) -> bool {
    let (px, py) = (f64::from(cursor.0), f64::from(cursor.1));
    mask.iter().any(|[x, y, width, height]| {
        px >= (x * scale).floor()
            && py >= (y * scale).floor()
            && px < ((x + width) * scale).ceil()
            && py < ((y + height) * scale).ceil()
    })
}

/// 更新有界命中掩码。首次启用移除旧视觉裁剪；后续动作不触碰WindowRgn。
/// None恢复完整输入并取消采样；失败保留可操作窗口，由上层执行经典回退。
pub(super) fn apply(window: &WebviewWindow, mask: Option<&[[f64; 4]]>) -> Result<(), String> {
    let app = window.app_handle();
    let state = app.state::<PetWindowState>();
    let (start, generation) = {
        let mut input = crate::lock(&state.1)?;
        let start = input.mask.is_none() && mask.is_some();
        if start || mask.is_none() {
            input.generation = input.generation.wrapping_add(1);
            input.press = PressOwnership {
                // 启用时已按住的键不属于新场景。
                down: unsafe { GetAsyncKeyState(1) } < 0,
                owned: false,
            };
        }
        input.mask = mask.map(<[_]>::to_vec);
        (start, input.generation)
    };
    if start {
        let hwnd = window.hwnd().map_err(|_| "无法读取桌宠输入句柄。")?;
        // 仅恢复矩形透明表面，不创建复杂GDI区域；像素透明仍由WebView2合成。
        if unsafe { SetWindowRgn(hwnd.0 as _, std::ptr::null_mut(), 1) } == 0 {
            return Err("无法恢复桌宠透明表面。".into());
        }
    }
    if mask.is_none() {
        window
            .set_ignore_cursor_events(false)
            .map_err(|_| "无法恢复完整窗口输入。")?;
        crate::lock(&state.1)?.ignored = false;
    } else if start {
        sample(window, generation)?;
        monitor(window.clone(), generation);
    }
    Ok(())
}

/// 返回false终止本世代；不可见窗口保留监视但降低频率，重新显示无需前端重载。
fn sample(window: &WebviewWindow, generation: u64) -> Result<bool, String> {
    let hwnd = window.hwnd().map_err(|_| "无法读取桌宠输入句柄。")?;
    if unsafe { IsWindow(hwnd.0 as _) } == 0 {
        return Ok(false);
    }
    let visible = unsafe { IsWindowVisible(hwnd.0 as _) } != 0;
    let scale = window
        .scale_factor()
        .map_err(|_| "无法读取桌宠显示比例。")?;
    let mut cursor = POINT { x: 0, y: 0 };
    let known =
        unsafe { GetCursorPos(&mut cursor) != 0 && ScreenToClient(hwnd.0 as _, &mut cursor) != 0 };
    let down = unsafe { GetAsyncKeyState(1) } < 0;
    let app = window.app_handle();
    let dragging = app
        .state::<crate::AppState>()
        .dragging
        .load(std::sync::atomic::Ordering::Relaxed);
    let state = app.state::<PetWindowState>();
    let (ignored, changed) = {
        let mut input = crate::lock(&state.1)?;
        if input.generation != generation || input.mask.is_none() {
            return Ok(false);
        }
        let hit = known && contains(input.mask.as_ref().unwrap(), (cursor.x, cursor.y), scale);
        let old_ignored = input.ignored;
        let accepts = input.press.update(down, hit, old_ignored, dragging);
        // 读取指针失败保守保留输入，不能让用户失去整个窗口的操作能力。
        let ignored = visible && known && !accepts;
        (ignored, ignored != old_ignored)
    };
    if changed {
        window
            .set_ignore_cursor_events(ignored)
            .map_err(|_| "无法更新桌宠透明穿透。")?;
        crate::lock(&state.1)?.ignored = ignored;
    }
    Ok(true)
}

/// 约60Hz输入采样，与动作重绘独立；单个异步任务每次只等待一个主线程回执。
fn monitor(window: WebviewWindow, generation: u64) {
    tauri::async_runtime::spawn(async move {
        loop {
            let (send, receive) = tokio::sync::oneshot::channel();
            let target = window.clone();
            if window
                .run_on_main_thread(move || {
                    let result = sample(&target, generation);
                    if result.is_err() {
                        // 输入API异常时恢复可操作模式，并废弃掩码；下次布局可重新启用。
                        let _ = apply(&target, None);
                    }
                    let hidden = target.is_visible().is_ok_and(|visible| !visible);
                    let _ = send.send((result.unwrap_or(false), hidden));
                })
                .is_err()
            {
                break;
            }
            match receive.await {
                Ok((true, hidden)) => {
                    tokio::time::sleep(std::time::Duration::from_millis(if hidden {
                        200
                    } else {
                        16
                    }))
                    .await;
                }
                _ => break,
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_mask_rounding_and_dpi_are_independent_of_desktop_origin() {
        let mask = [[-2.1, 0.2, 4.2, 3.0], [20.0, 20.0, 5.0, 5.0]];
        assert!(contains(&mask, (-4, 0), 1.5));
        assert!(!contains(&mask, (-5, 0), 1.5));
        assert!(contains(&mask, (3, 4), 1.5));
        assert!(!contains(&mask, (4, 5), 1.5));
        assert!(!contains(&mask, (12, 12), 1.5));
        assert!(contains(&mask, (30, 30), 1.5));
    }

    #[test]
    fn owned_press_survives_leaving_mask_until_release() {
        let mut press = PressOwnership::default();
        assert!(press.update(true, true, false, false));
        assert!(press.update(true, false, false, false));
        assert!(!press.update(false, false, false, false));
    }

    #[test]
    fn foreign_press_is_not_latched_and_drag_overrides_mask() {
        let mut press = PressOwnership::default();
        assert!(!press.update(true, false, true, false));
        assert!(press.update(true, true, true, false));
        assert!(!press.update(true, false, false, false));
        assert!(press.update(true, false, true, true));
        assert!(!press.update(false, false, false, false));
    }
}
