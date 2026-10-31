//! 桌宠原生适配和本机皮肤存储。布局只作用于 widget；资源包不执行代码。作者：ahui。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs, path::PathBuf, sync::Mutex};
use tauri::{Manager, PhysicalPosition, WebviewWindow};

#[cfg(windows)]
#[path = "pet_input.rs"]
mod input;

// 只串行本机资源增删；不与业务 store/layout 同时持锁，防止设置页和桌宠互相阻塞。
static SKIN_IO: Mutex<()> = Mutex::new(());

/// 只有活动桌宠可以取消共享拖动状态，避免换回经典后误取消标题栏新拖动。
/// 隐藏、换肤和退出场景共用；重复调用只在首次实际取消时发送一次结束事件。
pub fn cancel_active_drag(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<crate::AppState>();
    let cancelled = {
        let gate = crate::lock(&crate::DRAG_STATE_GATE)?;
        let pet_state = app.state::<PetWindowState>();
        let mut current = crate::lock(&pet_state.0)?;
        if !current.active {
            return Ok(());
        }
        // 无拖动但仍等待right反射时也必须清理，A→B同一Vue session不能保留A的保存意图。
        current.pending_position_save = None;
        crate::cancel_pet_drag_state_locked(&state, &gate)?
    };
    if cancelled {
        use tauri::Emitter;
        app.emit("widget-drag-ended", ())
            .map_err(|_| "无法取消桌宠拖动。")?;
    }
    Ok(())
}

/// 前端换任意皮肤前主动调用；只允许 widget 宿主，不调用模型、邮件或持久化配置。
#[tauri::command]
pub fn cancel_pet_drag(window: WebviewWindow, app: tauri::AppHandle) -> Result<(), String> {
    if window.label() != "widget" {
        return Err("此窗口不能取消桌宠拖动。".into());
    }
    cancel_active_drag(&app)
}

/// 桌宠拖动直接更新透明窗口位置，不调用 startDragging/系统移动循环，
/// 因而 Windows 关闭“拖动时显示窗口内容”时也不会出现空的黑色轮廓框。
/// 一次只排队一帧；主线程取最新物理光标坐标，跨屏时不累积逻辑像素误差。
#[tauri::command]
pub async fn start_pet_drag(
    window: WebviewWindow,
    app: tauri::AppHandle,
    pointer_delta: [i32; 2],
) -> Result<(), String> {
    if window.label() != "widget" || !crate::lock(&app.state::<PetWindowState>().0)?.active {
        return Err("当前窗口不是活动桌宠。".into());
    }
    #[cfg(windows)]
    {
        use std::sync::atomic::Ordering;
        use windows_sys::Win32::{Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos};
        crate::prepare_widget_drag(app.clone(), Some(pointer_delta))?;
        let position = match window.outer_position() {
            Ok(position) => position,
            Err(_) => {
                // prepare 已进入拖动态；读取失败也要立即通知前端恢复，不能等下一次松手。
                let _ = crate::finish_widget_drag(&app);
                return Err("无法读取桌宠位置。".into());
            }
        };
        let mut cursor = POINT { x: 0, y: 0 };
        if unsafe { GetCursorPos(&mut cursor) } == 0 {
            let _ = crate::finish_widget_drag(&app);
            return Err("无法读取拖动光标。".into());
        }
        let offset = (cursor.x - position.x, cursor.y - position.y);
        let generation = app
            .state::<crate::AppState>()
            .drag_generation
            .load(Ordering::Relaxed);
        tauri::async_runtime::spawn(async move {
            let mut previous = (cursor.x, cursor.y, std::time::Instant::now());
            let mut sample = 0u64;
            loop {
                let (send, receive) = tokio::sync::oneshot::channel();
                let target = window.clone();
                let handle = app.clone();
                let prior = previous;
                let report_motion = sample % 2 == 0;
                sample += 1;
                if window
                    .run_on_main_thread(move || {
                        use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
                        let state = handle.state::<crate::AppState>();
                        let current = state.drag_generation.load(Ordering::Relaxed) == generation
                            && state.dragging.load(Ordering::Relaxed);
                        let pressed = unsafe { GetAsyncKeyState(1) } < 0;
                        let mut cursor = POINT { x: 0, y: 0 };
                        let updated =
                            current && pressed && unsafe { GetCursorPos(&mut cursor) } != 0;
                        let mut keep = updated;
                        if updated {
                            let position =
                                PhysicalPosition::new(cursor.x - offset.0, cursor.y - offset.1);
                            if target.outer_position().ok() != Some(position) {
                                keep = target.set_position(position).is_ok();
                                if keep {
                                    crate::mark_widget_drag_moved(
                                        &handle,
                                        (position.x, position.y),
                                    );
                                }
                            }
                        }
                        if current && !keep {
                            let _ = crate::finish_widget_drag(&handle);
                        }
                        // 窗口随光标移动后，网页可能收不到相对 pointermove；原生提供低频、有限速度。
                        // 物理位移按当前窗口 DPI 转回逻辑像素，跨屏不会使摆动突然倍增。
                        let now = std::time::Instant::now();
                        if updated && report_motion {
                            use tauri::Emitter;
                            let dt = now.duration_since(prior.2).as_secs_f64().max(0.008);
                            let scale = target.scale_factor().unwrap_or(1.0).max(0.5);
                            let speed = [
                                ((f64::from(cursor.x) - f64::from(prior.0)) / dt / scale)
                                    .clamp(-4000.0, 4000.0),
                                ((f64::from(cursor.y) - f64::from(prior.1)) / dt / scale)
                                    .clamp(-4000.0, 4000.0),
                            ];
                            let _ = target.emit("pet-drag-motion", speed);
                        }
                        let _ = send.send((keep, cursor.x, cursor.y, now));
                    })
                    .is_err()
                {
                    break;
                }
                match receive.await {
                    Ok((true, x, y, now)) => previous = (x, y, now),
                    _ => break,
                }
                // 约 60Hz，仅按住时存在；等待主线程确认后才发下一帧，避免排队拖尾。
                tokio::time::sleep(std::time::Duration::from_millis(16)).await;
            }
        });
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (window, app, pointer_delta);
        Err("此桌宠拖动目前仅支持 Windows。".into())
    }
}

/// 场景切换只保留内存，避免每个动作写数据库；锚点为当前窗口内逻辑像素。
#[derive(Default)]
pub struct PetWindowState(
    pub Mutex<SceneState>,
    // 输入监视生命周期独立于SceneState，退场重置场景时仍可取消已排队的采样。
    #[cfg(windows)] Mutex<input::InputState>,
);
#[derive(Default)]
pub struct SceneState {
    pub active: bool,
    anchor: [f64; 2],
    scene: Option<SceneRequest>,
    /// 探头只改变呈现，不覆盖完整角色位置；恢复时还原此物理坐标再按工作区夹紧。
    normal_position: Option<(i32, i32)>,
    /// 完整场景恢复时可能更换靠右几何；保存原锚点才能维持角色的屏幕位置。
    normal_anchor: Option<[f64; 2]>,
    /// 命中掩码应用时的DPI；预览移除掩码时置零，使同一布局也能恢复输入。
    region_scale: f64,
    /// 只有真实拖动释放创建；等待右侧反射完成时跨普通布局保留，换肤/新拖动即失效。
    pending_position_save: Option<PendingPositionSave>,
}

/// 延迟保存只属于一次已释放拖动，不能被其他session或之后的动画/拖动复用。
#[derive(Clone, PartialEq)]
struct PendingPositionSave {
    session: String,
    generation: u64,
    side: Option<String>,
}

/// 前端只提交有限矩形与尺寸；Rust 负责验证和实际工作区约束。
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneRequest {
    session: String,
    revision: u64,
    width: f64,
    height: f64,
    anchor: [f64; 2],
    regions: Vec<[f64; 4]>,
    /// 探头方向由宿主可信布局决定；缺省兼容旧前端，资源包仍只提供原始left素材。
    #[serde(default)]
    docking: Option<String>,
    /// 已实际出画角色的保守并集，逻辑像素；与面板、状态云朵和菜单无关。
    #[serde(default)]
    actor_bounds: Option<[f64; 4]>,
    /// 可见主体并集（角色、云朵和可选卡片），仅用于边缘检测；不包含临时菜单。
    /// 缺省沿用旧前端角色轮廓；吸附后的对齐仍使用actor_bounds，避免角色留在屏外。
    #[serde(default)]
    dock_bounds: Option<[f64; 4]>,
    /// 未反射的完整默认布局锚点；持久化位置统一换算到此坐标，避免重启丢失右侧位置。
    #[serde(default)]
    canonical_anchor: Option<[f64; 2]>,
}

pub fn valid_skin_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .enumerate()
            .all(|(i, c)| c.is_ascii_lowercase() || c.is_ascii_digit() || (i > 0 && c == b'-'))
}

/// 数据边界必须在原生层重复验证，不能信任 WebView 传来的坐标和数量。
fn validate_scene(scene: &SceneRequest) -> Result<(), String> {
    if scene.session.is_empty()
        || scene.session.len() > 64
        || !scene
            .session
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        || !(1.0..=1600.0).contains(&scene.width)
        || !(1.0..=1600.0).contains(&scene.height)
        || scene.regions.is_empty()
        || scene.regions.len() > 300
        || scene
            .anchor
            .iter()
            .any(|n| !n.is_finite() || *n < 0.0 || *n > 1600.0)
    {
        return Err("桌宠场景尺寸或区域数量无效。".into());
    }
    if scene
        .docking
        .as_deref()
        .is_some_and(|side| !["left", "right", "top"].contains(&side))
    {
        return Err("桌宠探头方向无效。".into());
    }
    if scene.canonical_anchor.is_some_and(|anchor| {
        anchor
            .iter()
            .any(|n| !n.is_finite() || *n < 0.0 || *n > 1600.0)
    }) {
        return Err("桌宠标准锚点无效。".into());
    }
    for [x, y, width, height] in [scene.actor_bounds, scene.dock_bounds]
        .into_iter()
        .flatten()
    {
        if [x, y, width, height]
            .iter()
            .any(|n| !n.is_finite() || *n < 0.0)
            || width <= 0.0
            || height <= 0.0
            || x + width > scene.width + 1.0
            || y + height > scene.height + 1.0
        {
            return Err("桌宠角色或吸附轮廓超出场景。".into());
        }
    }
    for r in &scene.regions {
        if r.iter().any(|n| !n.is_finite() || *n < 0.0)
            || r[2] <= 0.0
            || r[3] <= 0.0
            || r[0] + r[2] > scene.width + 1.0
            || r[1] + r[3] > scene.height + 1.0
        {
            return Err("桌宠显示区域超出窗口。".into());
        }
    }
    Ok(())
}

/// 兼容旧场景只提供脚底锚点：以原有 92% 脚底比例恢复保守角色画布。
/// 新前端必须提供实际出画轮廓，避免云朵和展开卡片改变贴边检测。
fn actor_bounds(scene: &SceneRequest) -> [f64; 4] {
    scene.actor_bounds.unwrap_or_else(|| {
        let size = (scene.anchor[1] / 0.92).min(scene.height).max(1.0);
        [(scene.anchor[0] - size / 2.0).max(0.0), 0.0, size, size]
    })
}

/// 检测可见卡片/云朵触边，不要求左上角的角色先越过屏幕右侧；旧消息保持原判定。
fn dock_bounds(scene: &SceneRequest) -> [f64; 4] {
    scene.dock_bounds.unwrap_or_else(|| actor_bounds(scene))
}

/// 命中输入缓存需随窗口尺寸、逻辑轮廓和DPI失效；None/预览恢复不能跳过更新。
/// 兼容原有region命名，但不再对显示表面设置复杂WindowRgn。
fn region_needs_update(
    previous: Option<&SceneRequest>,
    scene: &SceneRequest,
    old_scale: f64,
    scale: f64,
) -> bool {
    previous.is_none_or(|old| {
        old.width != scene.width || old.height != scene.height || old.regions != scene.regions
    }) || old_scale != scale
}

/// 使用未夹紧释放坐标与可见主体检测边缘；顶部在角落优先，不使用菜单撑大的窗口外框。
/// 新前端传dock_bounds，旧前端仅有角色时兼容原判定；最终角色对齐由scene_position负责。
fn scene_dock_side(
    enabled: bool,
    position: (i32, i32),
    actor: [f64; 4],
    scale: f64,
    area: (i32, i32, i32, i32),
    margin: i32,
) -> Option<&'static str> {
    if !enabled {
        return None;
    }
    let x = f64::from(position.0) + actor[0] * scale;
    let y = f64::from(position.1) + actor[1] * scale;
    if y <= f64::from(area.1 + margin + 1) {
        Some("top")
    } else if x <= f64::from(area.0 + margin + 1) {
        Some("left")
    } else if x + actor[2] * scale >= f64::from(area.2 - margin - 1) {
        Some("right")
    } else {
        None
    }
}

/// 根据实际Region并集夹紧可见部分，允许纯透明外沿留在屏幕外。
/// 显式吸附方向只调整法向坐标；切向夹紧仍保证状态云朵和全部可见内容在工作区内。
fn scene_position(
    position: (i32, i32),
    scene: &SceneRequest,
    scale: f64,
    area: (i32, i32, i32, i32),
    side: Option<&str>,
) -> (i32, i32) {
    let actor = actor_bounds(scene);
    let visible_left = scene
        .regions
        .iter()
        .map(|r| r[0])
        .fold(f64::INFINITY, f64::min);
    let visible_top = scene
        .regions
        .iter()
        .map(|r| r[1])
        .fold(f64::INFINITY, f64::min);
    let visible_right = scene
        .regions
        .iter()
        .map(|r| r[0] + r[2])
        .fold(0.0, f64::max);
    let visible_bottom = scene
        .regions
        .iter()
        .map(|r| r[1] + r[3])
        .fold(0.0, f64::max);
    let min_x = area.0 - (visible_left * scale).floor() as i32;
    let max_x = (area.2 - (visible_right * scale).ceil() as i32).max(min_x);
    let min_y = area.1 - (visible_top * scale).floor() as i32;
    let max_y = (area.3 - (visible_bottom * scale).ceil() as i32).max(min_y);
    let mut x = position.0.clamp(min_x, max_x);
    let mut y = position.1.clamp(min_y, max_y);
    match side {
        Some("left") => x = area.0 - (actor[0] * scale).floor() as i32,
        Some("right") => x = area.2 - ((actor[0] + actor[2]) * scale).ceil() as i32,
        Some("top") => y = area.1 - (actor[1] * scale).floor() as i32,
        _ => {}
    }
    // 初次检测right时前端可能仍是左侧布局，此时先保完整云朵可见；方向缓存
    // 会让前端反射几何后再次对齐角色，而不是把旧布局的云朵推到屏幕外。
    (x.clamp(min_x, max_x), y.clamp(min_y, max_y))
}

/// 获取桌宠自己的贴边状态；业务配置和原生窗口只读，不改写测试结果或经典布局。
pub fn dock_notice(app: &tauri::AppHandle) -> Result<crate::DockNotice, String> {
    let scene = crate::lock(&app.state::<PetWindowState>().0)?.scene.clone();
    let Some(scene) = scene else {
        return Ok(crate::DockNotice {
            side: None,
            tucked: false,
        });
    };
    let state = app.state::<crate::AppState>();
    let enabled = crate::lock(&state.store)?.config.preferences.snap_to_edge;
    if let Some(side) = scene.docking.as_deref() {
        return Ok(crate::DockNotice {
            side: enabled.then(|| side.into()),
            tucked: true,
        });
    }
    let window = app.get_webview_window("widget").ok_or("悬浮窗不存在。")?;
    let position = window.outer_position().map_err(|_| "无法读取桌宠位置。")?;
    let area = crate::work_area(&window)?;
    let scale = window.scale_factor().map_err(|_| "无法读取显示比例。")?;
    // 松手可能先用旧的左侧布局检测到right，随后前端才反射布局；保留已判定方向，
    // 不用中间夹紧坐标推翻本次释放。真实Moved已统一清除此缓存，禁用吸附不复用缓存。
    let remembered = crate::lock(&state.layout)?.side.clone();
    Ok(crate::DockNotice {
        side: if enabled {
            remembered.or_else(|| {
                scene_dock_side(
                    enabled,
                    (position.x, position.y),
                    dock_bounds(&scene),
                    scale,
                    area,
                    (16.0 * scale).round() as i32,
                )
                .map(String::from)
            })
        } else {
            None
        },
        tucked: false,
    })
}

/// 主体触边即检测吸附，再将角色对齐到该边；拖动期间和探头状态保持原样，
/// 松手后才夹紧/吸附，防止拖动中途被布局回调抢夺位置。
pub fn snap_scene(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<crate::AppState>();
    if state.dragging.load(std::sync::atomic::Ordering::Relaxed)
        || crate::lock(&state.layout)?.preview_restore.is_some()
    {
        return Ok(());
    }
    let scene = crate::lock(&app.state::<PetWindowState>().0)?.scene.clone();
    let Some(scene) = scene.filter(|s| s.docking.is_none()) else {
        return Ok(());
    };
    let window = app.get_webview_window("widget").ok_or("悬浮窗不存在。")?;
    let position = window.outer_position().map_err(|_| "无法读取桌宠位置。")?;
    let scale = window.scale_factor().map_err(|_| "无法读取显示比例。")?;
    let area = crate::work_area(&window)?;
    let enabled = crate::lock(&state.store)?.config.preferences.snap_to_edge;
    // 必须在窗口或scene宽度夹紧之前检测，特别是角色初次从左布局拖到屏幕右侧。
    let remembered = crate::lock(&state.layout)?.side.clone();
    let side = scene_dock_side(
        enabled,
        (position.x, position.y),
        dock_bounds(&scene),
        scale,
        area,
        (16.0 * scale).round() as i32,
    )
    .or(if enabled { remembered.as_deref() } else { None });
    crate::lock(&state.layout)?.side = side.map(String::from);
    let (x, y) = scene_position((position.x, position.y), &scene, scale, area, side);
    if (x, y) != (position.x, position.y) {
        window
            .set_position(PhysicalPosition::new(x, y))
            .map_err(|_| "无法吸附桌宠位置。")?;
    }
    Ok(())
}

/// 命中轮廓只用于指针穿透，不再用复杂WindowRgn裁剪WebView2的透明合成表面。
#[cfg(windows)]
fn apply_region(window: &WebviewWindow, regions: Option<&[[f64; 4]]>) -> Result<(), String> {
    input::apply(window, regions)
}
#[cfg(not(windows))]
fn apply_region(_window: &WebviewWindow, _regions: Option<&[[f64; 4]]>) -> Result<(), String> {
    Err("此桌宠命中方案目前仅支持 Windows。".into())
}

/// 获取工作区相对当前窗口的逻辑边界，供菜单按真实剩余空间选边。
/// 只读几何，不读取配置或改变窗口；负坐标表示工作区延伸到当前窗口左／上方。
#[tauri::command]
pub fn get_pet_menu_bounds(window: WebviewWindow) -> Result<[f64; 4], String> {
    if window.label() != "widget" {
        return Err("此窗口不能读取桌宠菜单边界。".into());
    }
    let scale = window.scale_factor().map_err(|_| "无法读取显示比例。")?;
    if !scale.is_finite() || scale <= 0.0 {
        return Err("显示比例无效。".into());
    }
    let origin = window.outer_position().map_err(|_| "无法读取窗口位置。")?;
    let (left, top, right, bottom) = crate::work_area(&window)?;
    Ok([
        (f64::from(left) - f64::from(origin.x)) / scale,
        (f64::from(top) - f64::from(origin.y)) / scale,
        (f64::from(right) - f64::from(origin.x)) / scale,
        (f64::from(bottom) - f64::from(origin.y)) / scale,
    ])
}

/// 应用尺寸、位置和显示区域，保持角色屏幕锚点；失败尝试恢复完整窗口区域。
#[tauri::command]
pub async fn set_pet_scene_layout(
    window: WebviewWindow,
    app: tauri::AppHandle,
    scene: SceneRequest,
) -> Result<(), String> {
    if window.label() != "widget" {
        return Err("此窗口不能调整桌宠场景。".into());
    }
    validate_scene(&scene)?;
    // 原生尺寸、命中掩码与状态提交统一在事件线程顺序执行；不跨原生调用持有 Mutex。
    // WebView 刷新生成新 session，允许序号从 1 重新开始。
    let (send, receive) = tokio::sync::oneshot::channel();
    let target = window.clone();
    window
        .run_on_main_thread(move || {
            let _ = send.send(commit_scene(&target, &app, scene));
        })
        .map_err(|_| "无法排队桌宠布局。")?;
    receive.await.map_err(|_| "桌宠窗口已关闭。")?
}

/// 换算场景原点，保持角色锚点的物理屏幕位置；所有输入原点均为物理像素，锚点为逻辑像素。
/// 左右探头沿用保存的完整布局原点；顶部保留角色横向位置，纵向仍由完整布局与吸附决定。
/// 顶部恢复读取当前探头横坐标，避免云朵在角落夹紧后又跳回旧卡片原点。
/// 最后的可见范围夹紧交给scene_position，本函数不承诺超出工作区的卡片仍能原位展示。
/// 此函数不移动窗口、不写配置，便于独立验证左右人物布局与缩放下的收纳往返。
fn transition_position(
    position: (i32, i32),
    previous: Option<&SceneRequest>,
    scene: &SceneRequest,
    normal_position: Option<(i32, i32)>,
    normal_anchor: Option<[f64; 2]>,
    scale: f64,
) -> (i32, i32) {
    let was_docked = previous.is_some_and(|s| s.docking.is_some());
    let is_docked = scene.docking.is_some();
    let mut target = position;
    if was_docked && !is_docked {
        target = normal_position.unwrap_or(position);
        if let Some(anchor) = normal_anchor {
            target.0 += ((anchor[0] - scene.anchor[0]) * scale).round() as i32;
            target.1 += ((anchor[1] - scene.anchor[1]) * scale).round() as i32;
        }
        if let Some(previous) = previous.filter(|s| s.docking.as_deref() == Some("top")) {
            target.0 = position.0 + ((previous.anchor[0] - scene.anchor[0]) * scale).round() as i32;
        }
    } else if scene.docking.as_deref() == Some("top") && !was_docked {
        // 卡片在角色左边时，窗口原点与角色相差数百像素；收纳不能直接沿用窗口原点。
        if let Some(previous) = previous {
            target.0 += ((previous.anchor[0] - scene.anchor[0]) * scale).round() as i32;
        }
    } else if let Some(previous) = previous.filter(|_| !is_docked) {
        target.0 += ((previous.anchor[0] - scene.anchor[0]) * scale).round() as i32;
        target.1 += ((previous.anchor[1] - scene.anchor[1]) * scale).round() as i32;
    }
    target
}

fn commit_scene(
    window: &WebviewWindow,
    app: &tauri::AppHandle,
    scene: SceneRequest,
) -> Result<(), String> {
    if crate::lock(&app.state::<crate::AppState>().layout)?
        .preview_restore
        .is_some()
    {
        return Ok(());
    }
    let state = app.state::<PetWindowState>();
    let (active, anchor, previous, normal_position, normal_anchor, region_scale) = {
        let current = crate::lock(&state.0)?;
        (
            current.active,
            current.anchor,
            current.scene.clone(),
            current.normal_position,
            current.normal_anchor,
            current.region_scale,
        )
    };
    if active
        && previous
            .as_ref()
            .is_some_and(|s| s.session == scene.session && scene.revision <= s.revision)
    {
        return Ok(());
    }
    let scale = window.scale_factor().map_err(|_| "无法读取显示比例。")?;
    let old = window.outer_position().map_err(|_| "无法读取角色位置。")?;
    let area = crate::work_area(window)?;
    if !active {
        // 数据库保存默认normal布局的原点。首次场景先检测未经云朵夹紧的位置，
        // 否则right角色靠边却因默认云朵在右被左移，初次订阅将永远读不到right。
        let app_state = app.state::<crate::AppState>();
        let enabled = crate::lock(&app_state.store)?
            .config
            .preferences
            .snap_to_edge;
        let mut layout = crate::lock(&app_state.layout)?;
        layout.tucked = false;
        layout.anchor = None;
        layout.side = scene_dock_side(
            enabled,
            (old.x, old.y),
            dock_bounds(&scene),
            scale,
            area,
            (16.0 * scale).round() as i32,
        )
        .map(String::from);
    }
    let was_docked = previous.as_ref().is_some_and(|s| s.docking.is_some());
    let is_docked = scene.docking.is_some();
    let (mut x, mut y) = transition_position(
        (old.x, old.y),
        previous.as_ref().filter(|_| active),
        &scene,
        normal_position,
        normal_anchor,
        scale,
    );
    // 拖动时仍更新命中掩码，但位置由光标循环独占；出画回调不得夹紧正在移动的窗口。
    if app
        .state::<crate::AppState>()
        .dragging
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        (x, y) = (old.x, old.y);
    } else {
        let app_state = app.state::<crate::AppState>();
        let enabled = crate::lock(&app_state.store)?
            .config
            .preferences
            .snap_to_edge;
        let remembered = crate::lock(&app_state.layout)?.side.clone();
        let side = scene
            .docking
            .as_deref()
            .or(if enabled { remembered.as_deref() } else { None });
        (x, y) = scene_position((x, y), &scene, scale, area, side);
    }
    // 轮廓低频变化不代表窗口尺寸变化；避免每次轮廓更新都触发 WebView 重布局和 Moved。
    let resized = previous
        .as_ref()
        .is_none_or(|old| old.width != scene.width || old.height != scene.height);
    if resized {
        window
            .set_size(tauri::LogicalSize::new(scene.width, scene.height))
            .map_err(|_| "无法调整桌宠尺寸。")?;
    }
    if old.x != x || old.y != y {
        window
            .set_position(PhysicalPosition::new(x, y))
            .map_err(|_| "无法调整桌宠位置。")?;
    }
    if let Err(error) = if region_needs_update(previous.as_ref(), &scene, region_scale, scale) {
        apply_region(window, Some(&scene.regions))
    } else {
        Ok(())
    } {
        let _ = apply_region(window, None);
        return Err(error);
    }
    {
        let app_state = app.state::<crate::AppState>();
        let mut layout = crate::lock(&app_state.layout)?;
        layout.tucked = is_docked;
        if is_docked {
            layout.side = scene.docking.clone();
        }
    }
    let mut current = crate::lock(&state.0)?;
    current.active = true;
    current.anchor = scene.anchor;
    if is_docked && !was_docked {
        current.normal_position = Some((old.x, old.y));
        current.normal_anchor = Some(anchor);
    } else if !is_docked {
        current.normal_position = None;
        current.normal_anchor = None;
    }
    current.region_scale = scale;
    current.scene = Some(scene);
    drop(current);
    flush_position_save(app)
}

/// 放大HTML时临时恢复完整输入；退出重设命中掩码，尺寸由现有预览流程恢复。
pub fn preview_region(app: &tauri::AppHandle, expanded: bool) -> Result<(), String> {
    let state = app.state::<PetWindowState>();
    let scene = {
        let current = crate::lock(&state.0)?;
        if current.active {
            current.scene.clone()
        } else {
            None
        }
    };
    if scene.is_none() {
        return Ok(());
    }
    let window = app.get_webview_window("widget").ok_or("悬浮窗不存在。")?;
    apply_region(
        &window,
        if expanded {
            None
        } else {
            scene.as_ref().map(|s| s.regions.as_slice())
        },
    )?;
    let scale = window.scale_factor().map_err(|_| "无法读取显示比例。")?;
    let mut current = crate::lock(&state.0)?;
    record_region_application(
        &mut current,
        scene.as_ref().unwrap(),
        if expanded { None } else { Some(scale) },
    );
    Ok(())
}

/// 预览移除掩码时使缓存失效；恢复后只为同一场景记录实际DPI，
/// 不让迟到恢复把新皮肤/新revision标成已正确应用。None 表示当前没有轮廓。
fn record_region_application(state: &mut SceneState, scene: &SceneRequest, scale: Option<f64>) {
    if state.scene.as_ref().is_some_and(|current| {
        current.session == scene.session && current.revision == scene.revision
    }) {
        state.region_scale = scale.unwrap_or(0.0);
    }
}

/// DPI 变化只重算原生轮廓，不重置动作或测试；前端仍使用逻辑像素。
pub fn refresh_region(app: &tauri::AppHandle) {
    // DPI 回调可能由预览正在持锁的 set_size 同步产生；此时由预览收尾恢复轮廓。
    if app
        .state::<crate::AppState>()
        .layout
        .try_lock()
        .map(|s| s.preview_restore.is_some())
        .unwrap_or(true)
    {
        return;
    }
    let _ = preview_region(app, false);
}

/// 真实moved松手后创建一次保存意图；当前几何已对边时立即保存，否则等待最终反射提交。
/// 无位移点击、动作播放、菜单和探头均不创建意图，不造成每帧写库。
pub fn queue_position_save(app: &tauri::AppHandle, generation: u64) -> Result<(), String> {
    let state = app.state::<crate::AppState>();
    {
        let _gate = crate::lock(&crate::DRAG_STATE_GATE)?;
        if state
            .drag_generation
            .load(std::sync::atomic::Ordering::Relaxed)
            != generation
            || state.dragging.load(std::sync::atomic::Ordering::Relaxed)
        {
            return Ok(());
        }
        let side = crate::lock(&state.layout)?.side.clone();
        let pet_state = app.state::<PetWindowState>();
        let mut current = crate::lock(&pet_state.0)?;
        if let Some(scene) = current
            .scene
            .as_ref()
            .filter(|scene| scene.docking.is_none())
        {
            current.pending_position_save = Some(PendingPositionSave {
                session: scene.session.clone(),
                generation,
                side,
            });
        }
    }
    // 松手可能来自40ms后台监视器；窗口取样和最终保存统一排入事件线程，
    // 与commit_scene的窗口移动/状态提交保持先后，后台线程不读取一半更新的几何。
    let window = app.get_webview_window("widget").ok_or("悬浮窗不存在。")?;
    let handle = app.clone();
    window
        .run_on_main_thread(move || {
            // 写盘失败保留pending，后续普通场景提交可重试；绝不改写为已完成。
            let _ = flush_position_save(&handle);
        })
        .map_err(|_| "无法排队保存桌宠位置。".into())
}

/// 清理只消费观察到的同一意图；旧flush不能删除之后新拖动/新皮肤建立的pending。
fn clear_matching_position_save(state: &mut SceneState, expected: &PendingPositionSave) {
    if state.pending_position_save.as_ref() == Some(expected) {
        state.pending_position_save = None;
    }
}

/// 几何取样在事件线程完成；门禁内再次核对身份，写入成功后才消费同一意图。
/// persist仅执行SQLite配置事务，不能调用原生窗口API或事件，保持门禁无回调。
fn commit_position_save(
    state: &mut SceneState,
    sample: &PositionSaveSample,
    generation: u64,
    dragging: bool,
    _gate: &std::sync::MutexGuard<'_, ()>,
    persist: impl FnOnce((i32, i32)) -> Result<(), String>,
) -> Result<(), String> {
    let pending = &sample.pending;
    let scene = &sample.scene;
    if pending.generation != generation || pending.session != scene.session {
        clear_matching_position_save(state, pending);
        return Ok(());
    }
    if state.pending_position_save.as_ref() != Some(pending)
        || state
            .scene
            .as_ref()
            .is_none_or(|live| live.session != scene.session || live.revision != scene.revision)
    {
        return Ok(());
    }
    if !position_save_ready(
        pending,
        scene,
        generation,
        dragging,
        sample.position,
        sample.scale,
        sample.area,
    ) {
        return Ok(());
    }
    persist(canonical_position(sample.position, scene, sample.scale))?;
    clear_matching_position_save(state, pending);
    Ok(())
}

/// 保存需要的不可变窗口快照；写库前必须通过commit_position_save重新验证所有者。
struct PositionSaveSample {
    pending: PendingPositionSave,
    scene: SceneRequest,
    position: (i32, i32),
    scale: f64,
    area: (i32, i32, i32, i32),
}

/// 返回当前释放意图是否已经到达最终可保存几何；探头临时位置和旧拖动绝不能消费。
fn position_save_ready(
    pending: &PendingPositionSave,
    scene: &SceneRequest,
    generation: u64,
    dragging: bool,
    position: (i32, i32),
    scale: f64,
    area: (i32, i32, i32, i32),
) -> bool {
    if pending.session != scene.session
        || pending.generation != generation
        || dragging
        || scene.docking.is_some()
    {
        return false;
    }
    let [x, y, width, _] = actor_bounds(scene);
    // 与scene_position同一floor/ceil规则，只接受最终精确对齐，不能把保云朵的中间夹紧当完成。
    match pending.side.as_deref() {
        Some("left") => position.0 == area.0 - (x * scale).floor() as i32,
        Some("right") => position.0 == area.2 - ((x + width) * scale).ceil() as i32,
        Some("top") => position.1 == area.1 - (y * scale).floor() as i32,
        None => true,
        _ => false,
    }
}

/// 只变更保存的原点，不改当前窗口；重启默认布局使用同一角色屏幕锚点。
fn canonical_position(position: (i32, i32), scene: &SceneRequest, scale: f64) -> (i32, i32) {
    let canonical = scene.canonical_anchor.unwrap_or(scene.anchor);
    (
        position.0 + ((scene.anchor[0] - canonical[0]) * scale).round() as i32,
        position.1 + ((scene.anchor[1] - canonical[1]) * scale).round() as i32,
    )
}

/// 没有pending时只读取内存；最终normal位置至多保存一次，失败保留有效意图供后续提交重试。
fn flush_position_save(app: &tauri::AppHandle) -> Result<(), String> {
    let pet_state = app.state::<PetWindowState>();
    let (pending, scene) = {
        let current = crate::lock(&pet_state.0)?;
        let Some(pending) = current.pending_position_save.clone() else {
            return Ok(());
        };
        let Some(scene) = current.scene.clone() else {
            return Ok(());
        };
        (pending, scene)
    };
    let state = app.state::<crate::AppState>();
    let window = app.get_webview_window("widget").ok_or("悬浮窗不存在。")?;
    let position = window.outer_position().map_err(|_| "无法读取桌宠位置。")?;
    let scale = window.scale_factor().map_err(|_| "无法读取显示比例。")?;
    let area = crate::work_area(&window)?;
    // 窗口只读放门禁外，不能持锁跨同步原生调用。取样后重新核对世代、pending和场景revision；
    // 随后的SQLite写入及消费与prepare/cancel共用门禁，校验后不会插入新拖动或换肤取消。
    let _gate = crate::lock(&crate::DRAG_STATE_GATE)?;
    let mut current = crate::lock(&pet_state.0)?;
    let generation = state
        .drag_generation
        .load(std::sync::atomic::Ordering::Relaxed);
    let sample = PositionSaveSample {
        pending,
        scene,
        position: (position.x, position.y),
        scale,
        area,
    };
    commit_position_save(
        &mut current,
        &sample,
        generation,
        state.dragging.load(std::sync::atomic::Ordering::Relaxed),
        &_gate,
        |saved| {
            let mut store = crate::lock(&state.store)?;
            if store.config.widget_position != Some(saved) {
                let mut config = store.config.clone();
                config.widget_position = Some(saved);
                store.save_config(config)?;
            }
            Ok(())
        },
    )
}

/// 退出场景时沿用页面真实详情状态；旧前端未传expanded时保持经典紧凑回退。
#[tauri::command]
pub async fn clear_pet_scene(
    window: WebviewWindow,
    app: tauri::AppHandle,
    expanded: Option<bool>,
) -> Result<(), String> {
    if window.label() != "widget" {
        return Err("此窗口不能关闭桌宠场景。".into());
    }
    let (send, receive) = tokio::sync::oneshot::channel();
    let target = window.clone();
    window
        .run_on_main_thread(move || {
            let result = (|| {
                cancel_active_drag(&app)?;
                let restore = {
                    let state = app.state::<PetWindowState>();
                    let mut scene = crate::lock(&state.0)?;
                    let restore = scene.normal_position;
                    *scene = SceneState::default();
                    restore
                };
                // 经典回退不继承探头的临时坐标和 tucked；普通场景退出仍从当前位置恢复。
                if let Some((x, y)) = restore {
                    target
                        .set_position(PhysicalPosition::new(x, y))
                        .map_err(|_| "无法恢复桌宠原位置。")?;
                }
                {
                    let state = app.state::<crate::AppState>();
                    let mut layout = crate::lock(&state.layout)?;
                    layout.tucked = false;
                    layout.side = None;
                    layout.anchor = None;
                }
                apply_region(&target, None)?;
                crate::set_widget_layout(app.clone(), expanded.unwrap_or(false), 0, false)
            })();
            let _ = send.send(result);
        })
        .map_err(|_| "无法排队恢复经典窗口。")?;
    receive.await.map_err(|_| "桌宠窗口已关闭。")?
}

fn directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|_| "无法读取本机皮肤目录。")?
        .join("skins"))
}

/// 只解码 PNG 头部以核对真实像素预算；全部 Base64 字符/填充仍检查，图片内容由 WebView 解码。
/// 不执行任何资源内容，也不接受 SVG、URL 或文件路径。损坏 PNG 最终由播放器安全回退。
fn png_dimensions(data: &str) -> Result<(u64, u64), String> {
    let raw = data
        .strip_prefix("data:image/png;base64,")
        .ok_or("图片必须为内嵌 PNG。")?;
    let body = raw.trim_end_matches('=');
    if raw.len() < 44
        || raw.len() % 4 != 0
        || raw.len() - body.len() > 2
        || body
            .bytes()
            .any(|c| !c.is_ascii_alphanumeric() && c != b'+' && c != b'/')
    {
        return Err("PNG 编码无效。".into());
    }
    let mut header = Vec::with_capacity(24);
    let (mut bits, mut count) = (0u32, 0u32);
    for c in body.bytes().take(32) {
        let value = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => unreachable!(),
        };
        bits = (bits << 6) | u32::from(value);
        count += 6;
        if count >= 8 {
            count -= 8;
            header.push((bits >> count) as u8);
        }
    }
    if header.get(..16) != Some(&b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR"[..]) {
        return Err("PNG 文件头无效。".into());
    }
    let width = u32::from_be_bytes(header[16..20].try_into().map_err(|_| "PNG 头不完整。")?);
    let height = u32::from_be_bytes(header[20..24].try_into().map_err(|_| "PNG 头不完整。")?);
    Ok((u64::from(width), u64::from(height)))
}

/// 检查资源类型和预算；禁止路径逃逸、内置 ID 覆盖及无约束 JSON/图集数据。
fn validate_pack(pack: &Value) -> Result<&str, String> {
    let m = &pack["manifest"];
    let id = m["id"].as_str().ok_or("皮肤缺少标识。")?;
    if !valid_skin_id(id)
        || matches!(id, "classic" | "blue-whale-maid")
        || m["schemaVersion"].as_u64() != Some(1)
        || m["name"]
            .as_str()
            .is_none_or(|s| s.trim().is_empty() || s.chars().count() > 60)
        || m["author"].as_str().is_none_or(|s| s.chars().count() > 80)
        || m["description"]
            .as_str()
            .is_none_or(|s| s.chars().count() > 300)
    {
        return Err("皮肤标识、版本或名称无效，不能覆盖内置皮肤。".into());
    }
    for key in ["panelBackground", "panelBorder", "primary", "textPrimary"] {
        let color = m["tokens"][key].as_str().ok_or("皮肤配色无效。")?;
        if color.len() != 7
            || !color.starts_with('#')
            || !color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
        {
            return Err("皮肤配色必须是六位十六进制颜色。".into());
        }
    }
    // 资源包仅能选择宿主可信边饰，不接收脚本、任意 SVG/CSS；缺省保留旧包外观。
    if let Some(decoration) = m.get("decorations") {
        if !matches!(decoration.as_str(), Some("none" | "ocean" | "stars")) {
            return Err("皮肤面板装饰预设无效。".into());
        }
    }
    let actions = m["actions"].as_object().ok_or("皮肤缺少动作。")?;
    // 四个可选姿态不改变schema版本；旧皮肤仍只需idle，缺失动作由前端回退待机。
    if !actions.contains_key("idle") || actions.len() > 12 {
        return Err("皮肤必须提供待机动作。".into());
    }
    // 可选探头与动作复用同一 PNG/热区边界，但不加入动作名枚举。
    // 旧包没有 docking 仍有效；只接受已获批 left，不执行包内脚本或额外资源。
    let docking = m
        .get("docking")
        .map(|value| {
            value
                .as_object()
                .filter(|entries| entries.len() == 1 && entries.contains_key("left"))
                .ok_or("皮肤探头只允许配置左侧资源。")
        })
        .transpose()?;
    let assets = pack["assets"].as_object().ok_or("皮肤缺少图片。")?;
    let mut budget = 0u64;
    let mut referenced = std::collections::HashSet::new();
    for (action, clip, is_docking) in actions
        .iter()
        .map(|(action, clip)| (action, clip, false))
        .chain(
            docking
                .into_iter()
                .flat_map(|entries| entries.iter())
                .map(|(side, clip)| (side, clip, true)),
        )
    {
        if !is_docking
            && ![
                "idle", "petting", "dragging", "working", "success", "seated", "wave", "stretch",
                "landing", "bow", "failed", "error",
            ]
            .contains(&action.as_str())
        {
            return Err("未知动作。".into());
        }
        let size = clip["frameSize"]
            .as_u64()
            .filter(|v| (32..=512).contains(v))
            .ok_or("图集尺寸无效。")?;
        let frames = clip["frames"]
            .as_u64()
            .filter(|v| (1..=64).contains(v))
            .ok_or("图集帧数无效。")?;
        let columns = clip["columns"]
            .as_u64()
            .filter(|v| (1..=8).contains(v))
            .ok_or("图集列数无效。")?;
        if is_docking && (frames != 1 || columns != 1) {
            return Err("侧边探头必须是单帧 PNG。".into());
        }
        budget += size * size * (columns * frames.div_ceil(columns) + 1) * 4;
        if clip["fps"]
            .as_f64()
            .is_none_or(|v| !(1.0..=30.0).contains(&v))
            || !clip["loop"].is_boolean()
        {
            return Err("动作播放参数无效。".into());
        }
        if let Some(blink) = clip.get("blink") {
            let start = blink["start"].as_u64().filter(|v| *v >= 1);
            let count = blink["frames"].as_u64().filter(|v| *v >= 2);
            let rate = blink["fps"].as_f64().filter(|v| (1.0..=30.0).contains(v));
            let min = blink["minDelayMs"]
                .as_f64()
                .filter(|v| (1000.0..=60000.0).contains(v));
            let max = blink["maxDelayMs"]
                .as_f64()
                .filter(|v| (1000.0..=60000.0).contains(v));
            if action != "idle"
                || clip["loop"] != true
                || start.is_none()
                || count.is_none()
                || rate.is_none()
                || min.is_none()
                || max.is_none()
                || start
                    .unwrap_or(u64::MAX)
                    .saturating_add(count.unwrap_or(u64::MAX))
                    > frames
                || min > max
            {
                return Err("眨眼帧段或间隔无效。".into());
            }
        }
        let bands = clip["hitBands"]
            .as_array()
            .filter(|v| !v.is_empty() && v.len() <= 256)
            .ok_or("角色轮廓无效。")?;
        for band in bands {
            let r = band
                .as_array()
                .filter(|v| v.len() == 4)
                .ok_or("角色轮廓矩形无效。")?;
            let mut numbers = [0.0; 4];
            for (i, n) in r.iter().enumerate() {
                numbers[i] = n
                    .as_f64()
                    .filter(|v| v.is_finite() && *v >= 0.0)
                    .ok_or("角色轮廓坐标无效。")?;
            }
            if numbers[2] <= 0.0
                || numbers[3] <= 0.0
                || numbers[0] + numbers[2] > size as f64
                || numbers[1] + numbers[3] > size as f64
            {
                return Err("角色轮廓超出画布。".into());
            }
        }
        for field in ["atlas", "poster"] {
            let name = clip[field].as_str().ok_or("缺少图片名。")?;
            if !name.ends_with(".png")
                || name.len() > 90
                || name.len() <= 4
                || !name[..name.len() - 4]
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
            {
                return Err("图片路径无效。".into());
            }
            let expected = if field == "atlas" {
                (size * columns, size * frames.div_ceil(columns))
            } else {
                (size, size)
            };
            let data = assets
                .get(name)
                .and_then(Value::as_str)
                .ok_or("图片引用缺失。")?;
            if !data.starts_with("data:image/png;base64,iVBORw0KGgo")
                || data.len() > 24 * 1024 * 1024
            {
                return Err("图片不是受支持的 PNG。".into());
            }
            if png_dimensions(data)? != expected {
                return Err("PNG 实际尺寸与皮肤描述不一致。".into());
            }
            referenced.insert(name);
        }
    }
    if assets.keys().any(|key| !referenced.contains(key.as_str())) {
        return Err("皮肤包含未引用资源。".into());
    }
    if budget > 64 * 1024 * 1024 {
        return Err("皮肤解码图片超过 64 MiB。".into());
    }
    Ok(id)
}

#[derive(Serialize)]
pub struct SkinEntry {
    id: String,
    name: String,
    author: String,
    builtin: bool,
}
#[tauri::command]
/// 返回最多 64 个本机已安装皮肤摘要；后台读取并跳过损坏包，不包含位图或远程目录。
pub async fn list_skins(app: tauri::AppHandle) -> Result<Vec<SkinEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = directory(&app)?;
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut result = Vec::new();
        for entry in fs::read_dir(dir)
            .map_err(|_| "无法读取皮肤目录。")?
            .take(64)
            .flatten()
        {
            if entry.path().extension().is_none_or(|e| e != "json") {
                continue;
            }
            if entry.metadata().is_ok_and(|m| m.len() > 48 * 1024 * 1024) {
                continue;
            }
            if let Ok(data) = fs::read(entry.path()) {
                if let Ok(pack) = serde_json::from_slice::<Value>(&data) {
                    if let Ok(id) = validate_pack(&pack) {
                        if entry.path().file_stem().and_then(|s| s.to_str()) != Some(id) {
                            continue;
                        }
                        result.push(SkinEntry {
                            id: id.into(),
                            name: pack["manifest"]["name"].as_str().unwrap_or(id).into(),
                            author: pack["manifest"]["author"].as_str().unwrap_or("").into(),
                            builtin: false,
                        });
                    }
                }
            }
        }
        result.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(result)
    })
    .await
    .map_err(|_| "本机皮肤操作未完成。")?
}
#[tauri::command]
/// 按受限 ID 后台读取自包含资源包；不存在、超限、内容损坏或标识不一致时返回可读错误。
pub async fn get_skin(app: tauri::AppHandle, id: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_skin_id(&id) {
            return Err("皮肤标识无效。".into());
        }
        let path = directory(&app)?.join(format!("{id}.json"));
        if fs::metadata(&path).map_err(|_| "皮肤不存在。")?.len() > 48 * 1024 * 1024 {
            return Err("皮肤文件过大。".into());
        }
        let data = fs::read(path).map_err(|_| "读取皮肤失败。")?;
        let pack: Value = serde_json::from_slice(&data).map_err(|_| "皮肤格式损坏。")?;
        if validate_pack(&pack)? != id {
            return Err("皮肤文件标识不一致。".into());
        }
        Ok(pack)
    })
    .await
    .map_err(|_| "本机皮肤操作未完成。")?
}

/// 不覆盖现存 ID；临时文件写完后硬链接原子提交，失败不会破坏已安装皮肤。
#[tauri::command]
pub async fn install_skin(app: tauri::AppHandle, pack: Value) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _gate = crate::lock(&SKIN_IO)?;
        let id = validate_pack(&pack)?;
        let bytes = serde_json::to_vec(&pack).map_err(|_| "皮肤编码失败。")?;
        if bytes.len() > 48 * 1024 * 1024 {
            return Err("皮肤包超过 48 MiB。".into());
        }
        let dir = directory(&app)?;
        fs::create_dir_all(&dir).map_err(|_| "无法创建皮肤目录。")?;
        let dest = dir.join(format!("{id}.json"));
        if dest.exists() {
            return Err("此皮肤标识已存在，请使用新标识导入。".into());
        }
        if fs::read_dir(&dir)
            .map_err(|_| "读取皮肤目录失败。")?
            .count()
            >= 64
        {
            return Err("本机最多保留 64 套皮肤。".into());
        }
        let temp = dir.join(format!("{}.tmp", uuid::Uuid::new_v4()));
        let result: Result<(), String> = (|| {
            fs::write(&temp, bytes).map_err(|_| "保存皮肤失败。")?;
            // 同目录硬链接以“目标不得存在”原子提交，禁止竞态覆盖；成功后移除临时名字。
            fs::hard_link(&temp, dest).map_err(|_| "提交皮肤失败，可能已有同名皮肤。".into())
        })();
        let _ = fs::remove_file(temp);
        result
    })
    .await
    .map_err(|_| "本机皮肤操作未完成。")?
}
#[tauri::command]
/// 删除未启用的自定义包；内置、当前启用、无效 ID 和磁盘故障均拒绝，不删除外部备份。
pub async fn remove_skin(app: tauri::AppHandle, id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_skin_id(&id) || matches!(id.as_str(), "classic" | "blue-whale-maid") {
            return Err("不能移除内置皮肤。".into());
        }
        if crate::lock(&app.state::<crate::AppState>().store)?
            .config
            .preferences
            .skin
            .id
            == id
        {
            return Err("请先保存切换到其他皮肤，再移除此皮肤。".into());
        }
        let _gate = crate::lock(&SKIN_IO)?;
        fs::remove_file(directory(&app)?.join(format!("{id}.json")))
            .map_err(|_| "移除皮肤失败。".into())
    })
    .await
    .map_err(|_| "本机皮肤操作未完成。")?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_and_regions_are_bounded() {
        assert!(!valid_skin_id("../escape"));
        assert!(!valid_skin_id(""));
        assert!(valid_skin_id("blue-whale-maid"));
        assert!(validate_scene(&SceneRequest {
            session: "test".into(),
            revision: 1,
            width: 200.0,
            height: 200.0,
            anchor: [100.0, 180.0],
            regions: vec![[0.0, 0.0, 200.0, 200.0]],
            docking: None,
            actor_bounds: None,
            dock_bounds: None,
            canonical_anchor: None,
        })
        .is_ok());
        assert!(validate_scene(&SceneRequest {
            session: "test".into(),
            revision: 1,
            width: 200.0,
            height: 200.0,
            anchor: [f64::NAN, 0.0],
            regions: vec![[0.0, 0.0, 300.0, 200.0]],
            docking: None,
            actor_bounds: None,
            dock_bounds: None,
            canonical_anchor: None,
        })
        .is_err());
    }

    /// 老消息无需新字段；三个宿主方向可用，不能夹带无效方向或窗口外命中区。
    #[test]
    fn docking_scene_keeps_legacy_messages_and_rejects_unapproved_edges() {
        let mut scene: SceneRequest = serde_json::from_value(serde_json::json!({
            "session":"test","revision":1,"width":200,"height":200,
            "anchor":[100,180],"regions":[[0,0,200,200]]
        }))
        .unwrap();
        assert!(validate_scene(&scene).is_ok());
        assert!(scene.docking.is_none());
        scene.docking = Some("left".into());
        scene.actor_bounds = Some([12.0, 4.0, 120.0, 150.0]);
        assert!(validate_scene(&scene).is_ok());
        assert_eq!(actor_bounds(&scene), [12.0, 4.0, 120.0, 150.0]);
        for side in ["left", "right", "top"] {
            scene.docking = Some(side.into());
            assert!(validate_scene(&scene).is_ok());
        }
        scene.docking = Some("bottom".into());
        assert!(validate_scene(&scene).is_err());
        scene.docking = None;
        scene.actor_bounds = Some([12.0, 4.0, 300.0, 150.0]);
        assert!(validate_scene(&scene).is_err());
    }

    /// 相同逻辑轮廓仍会因尺寸/DPI失效；这条回归只证明失效规则，不冒充视觉验收。
    #[test]
    fn region_cache_invalidates_for_dimensions_dpi_and_actual_bounds() {
        let mut scene: SceneRequest = serde_json::from_value(serde_json::json!({
            "session":"test","revision":1,"width":200,"height":200,
            "anchor":[100,180],"regions":[[0,0,100,100]]
        }))
        .unwrap();
        let previous = scene.clone();
        assert!(!region_needs_update(Some(&previous), &scene, 1.0, 1.0));
        assert!(region_needs_update(Some(&previous), &scene, 1.0, 1.5));
        scene.width = 220.0;
        assert!(region_needs_update(Some(&previous), &scene, 1.0, 1.0));
        scene = previous.clone();
        scene.height = 220.0;
        assert!(region_needs_update(Some(&previous), &scene, 1.0, 1.0));
        scene = previous.clone();
        scene.regions[0][0] = 1.0;
        assert!(region_needs_update(Some(&previous), &scene, 1.0, 1.0));
    }

    /// 右边检测在夹紧前使用角色边，角落顶部优先；关闭吸附和负坐标/DPI同样确定。
    #[test]
    fn scene_edges_use_raw_actor_position_with_top_corner_priority() {
        let area = (0, 0, 1920, 1040);
        let actor = [12.0, 4.0, 120.0, 140.0];
        assert_eq!(
            scene_dock_side(true, (-12, 200), actor, 1.0, area, 16),
            Some("left")
        );
        assert_eq!(
            scene_dock_side(true, (1788, 200), actor, 1.0, area, 16),
            Some("right")
        );
        assert_eq!(
            scene_dock_side(true, (600, -4), actor, 1.0, area, 16),
            Some("top")
        );
        assert_eq!(
            scene_dock_side(true, (-12, -4), actor, 1.0, area, 16),
            Some("top")
        );
        assert_eq!(
            scene_dock_side(true, (1788, -4), actor, 1.0, area, 16),
            Some("top")
        );
        assert_eq!(
            scene_dock_side(false, (-12, -4), actor, 1.0, area, 16),
            None
        );
        assert_eq!(
            scene_dock_side(true, (800, 200), actor, 1.0, area, 16),
            None
        );
        assert_eq!(
            scene_dock_side(true, (-1944, 100), actor, 2.0, (-1920, -200, 0, 1000), 32),
            Some("left")
        );
        assert_eq!(
            scene_dock_side(true, (-264, 100), actor, 2.0, (-1920, -200, 0, 1000), 32),
            Some("right")
        );
        assert_eq!(
            scene_dock_side(true, (-1200, -208), actor, 2.0, (-1920, -200, 0, 1000), 32),
            Some("top")
        );
    }

    /// 卡片右侧触边即可收纳；角色未到边缘、DPI、负工作区与旧前端均有独立回归。
    #[test]
    fn card_touching_right_edge_detects_before_actor_reaches_it() {
        let mut scene: SceneRequest = serde_json::from_value(serde_json::json!({
            "session":"test","revision":1,"width":435,"height":300,
            "anchor":[96,180],"actorBounds":[24,16,144,176],
            "dockBounds":[24,16,409,280],
            "regions":[[24,16,144,176],[73,150,360,146]]
        }))
        .unwrap();
        assert!(validate_scene(&scene).is_ok());
        for (scale, area) in [
            (1.0, (0, 0, 1920, 1040)),
            (1.5, (-2880, -300, 0, 1260)),
            (2.0, (0, 0, 3840, 2080)),
        ] {
            let position = (area.2 - (433.0 * scale) as i32, area.1 + 200);
            let margin = (16.0 * scale) as i32;
            assert_eq!(
                scene_dock_side(true, position, actor_bounds(&scene), scale, area, margin),
                None
            );
            assert_eq!(
                scene_dock_side(true, position, dock_bounds(&scene), scale, area, margin),
                Some("right")
            );
            assert_eq!(
                scene_dock_side(false, position, dock_bounds(&scene), scale, area, margin),
                None
            );
        }
        scene.dock_bounds = None;
        assert_eq!(dock_bounds(&scene), actor_bounds(&scene));
        for invalid in [
            [0.0, 0.0, 436.1, 200.0],
            [0.0, 0.0, 0.0, 200.0],
            [f64::NAN, 0.0, 100.0, 200.0],
        ] {
            scene.dock_bounds = Some(invalid);
            assert!(validate_scene(&scene).is_err());
        }
    }

    /// 三方向均按实际变换轮廓对齐，同时保留云朵；切向靠角落时也不裁状态文字。
    #[test]
    fn three_sided_peek_positions_keep_actor_edges_and_clouds_inside_work_area() {
        let mut scene: SceneRequest = serde_json::from_value(serde_json::json!({
            "session":"test","revision":1,"width":199,"height":96,
            "anchor":[48,88],"actorBounds":[0,0,63,94],
            "regions":[[0,0,63,94],[67,16,132,50]],"docking":"left"
        }))
        .unwrap();
        let area = (0, 0, 1920, 1040);
        assert_eq!(
            scene_position((40, 1000), &scene, 1.0, area, Some("left")),
            (0, 946)
        );
        scene.width = 230.0;
        scene.actor_bounds = Some([167.0, 0.0, 63.0, 94.0]);
        scene.regions = vec![[167.0, 0.0, 63.0, 94.0], [0.0, 16.0, 132.0, 50.0]];
        assert_eq!(
            scene_position((40, 1000), &scene, 1.0, area, Some("right")),
            (1690, 946)
        );
        scene.width = 232.0;
        scene.height = 63.0;
        scene.actor_bounds = Some([0.0, 0.0, 94.0, 63.0]);
        scene.regions = vec![[0.0, 0.0, 94.0, 63.0], [100.0, 10.0, 132.0, 50.0]];
        assert_eq!(
            scene_position((1900, 100), &scene, 1.0, area, Some("top")),
            (1688, 0)
        );
        assert_eq!(
            scene_position(
                (-1900, -100),
                &scene,
                2.0,
                (-1920, -200, 0, 1000),
                Some("top")
            ),
            (-1900, -200)
        );
        // 旧左布局刚检测到right时云朵仍在右边；先夹紧可见内容，待宿主反射几何。
        scene.actor_bounds = Some([0.0, 0.0, 63.0, 63.0]);
        assert_eq!(
            scene_position((1857, 100), &scene, 1.0, area, Some("right")),
            (1688, 100)
        );
    }

    /// 顶部收纳的切向位置跟随角色，而非卡片左上角；角落夹紧后也从可见探头处恢复。
    #[test]
    fn right_actor_top_peek_keeps_horizontal_anchor_through_roundtrip() {
        let normal: SceneRequest = serde_json::from_value(serde_json::json!({
            "session":"test","revision":1,"width":416,"height":660,
            "anchor":[342,136],"actorBounds":[280,20,120,120],
            "regions":[[280,20,120,120],[0,113,360,546]]
        }))
        .unwrap();
        let peek: SceneRequest = serde_json::from_value(serde_json::json!({
            "session":"test","revision":2,"width":234,"height":67,
            "anchor":[48,88],"actorBounds":[0,0,96,63],
            "regions":[[0,0,96,63],[100,8,132,50]],"docking":"top"
        }))
        .unwrap();
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let area = (-1920, -200, 1920, 1500);
            let origin = (-900, -200);
            let target = transition_position(origin, Some(&normal), &peek, None, None, scale);
            let tucked = scene_position(target, &peek, scale, area, Some("top"));
            assert!(
                ((f64::from(origin.0) + normal.anchor[0] * scale)
                    - (f64::from(tucked.0) + peek.anchor[0] * scale))
                    .abs()
                    <= 1.0
            );
            let restored = transition_position(
                tucked,
                Some(&peek),
                &normal,
                Some(origin),
                Some(normal.anchor),
                scale,
            );
            assert_eq!(restored.0, origin.0);
            // 状态云朵导致探头在右上角向左夹紧时，恢复也必须以当前探头为准，不能跳回旧卡片原点。
            let corner = scene_position((area.2, area.1), &peek, scale, area, Some("top"));
            let recovered = transition_position(
                corner,
                Some(&peek),
                &normal,
                Some(origin),
                Some(normal.anchor),
                scale,
            );
            assert!(
                ((f64::from(corner.0) + peek.anchor[0] * scale)
                    - (f64::from(recovered.0) + normal.anchor[0] * scale))
                    .abs()
                    <= 1.0
            );
        }
    }

    /// right旧左布局保云朵而夹紧时不落盘；反射后只由同一真实释放意图准入。
    #[test]
    fn release_position_waits_for_final_geometry_and_rejects_transient_or_stale_owners() {
        let mut scene: SceneRequest = serde_json::from_value(serde_json::json!({
            "session":"test","revision":1,"width":232,"height":144,
            "anchor":[72,132],"actorBounds":[12,4,120,140],
            "regions":[[12,4,120,140],[100,10,132,50]]
        }))
        .unwrap();
        let area = (0, 0, 1920, 1040);
        let pending = PendingPositionSave {
            session: "test".into(),
            generation: 7,
            side: Some("right".into()),
        };
        // 旧布局窗口为保云朵夹在1688；角色右缘只有1820，不能持久化这中间位置。
        assert!(!position_save_ready(
            &pending,
            &scene,
            7,
            false,
            (1688, 100),
            1.0,
            area
        ));
        scene.actor_bounds = Some([100.0, 4.0, 120.0, 140.0]);
        scene.anchor = [160.0, 132.0];
        assert!(position_save_ready(
            &pending,
            &scene,
            7,
            false,
            (1700, 100),
            1.0,
            area
        ));
        assert!(!position_save_ready(
            &pending,
            &scene,
            8,
            false,
            (1700, 100),
            1.0,
            area
        ));
        assert!(!position_save_ready(
            &pending,
            &scene,
            7,
            true,
            (1700, 100),
            1.0,
            area
        ));
        scene.docking = Some("right".into());
        assert!(!position_save_ready(
            &pending,
            &scene,
            7,
            false,
            (1700, 100),
            1.0,
            area
        ));
        scene.docking = None;
        scene.session = "new-skin".into();
        assert!(!position_save_ready(
            &pending,
            &scene,
            7,
            false,
            (1700, 100),
            1.0,
            area
        ));
        let free = PendingPositionSave {
            session: scene.session.clone(),
            generation: 7,
            side: None,
        };
        assert!(position_save_ready(
            &free,
            &scene,
            7,
            false,
            (800, 100),
            1.0,
            area
        ));
        let left = PendingPositionSave {
            side: Some("left".into()),
            ..free.clone()
        };
        assert!(position_save_ready(
            &left,
            &scene,
            7,
            false,
            (-100, 100),
            1.0,
            area
        ));
        let top = PendingPositionSave {
            side: Some("top".into()),
            ..free
        };
        assert!(position_save_ready(
            &top,
            &scene,
            7,
            false,
            (800, -4),
            1.0,
            area
        ));
    }

    /// 同一右边角色在默认与反射布局间往返只变换原点，反复重启不积累偏移。
    #[test]
    fn canonical_anchor_keeps_right_restart_detection_and_repeated_roundtrips_stable() {
        let mut normal: SceneRequest = serde_json::from_value(serde_json::json!({
            "session":"start","revision":1,"width":232,"height":144,
            "anchor":[72,132],"canonicalAnchor":[72,132],"actorBounds":[12,4,120,140],
            "regions":[[12,4,120,140],[100,10,132,50]]
        }))
        .unwrap();
        let mut reflected = normal.clone();
        reflected.anchor = [160.0, 132.0];
        reflected.actor_bounds = Some([100.0, 4.0, 120.0, 140.0]);
        reflected.regions = vec![[100.0, 4.0, 120.0, 140.0], [0.0, 10.0, 96.0, 50.0]];
        let area = (0, 0, 1920, 1040);
        let mut final_origin = (1700, 100);
        for _ in 0..5 {
            let saved = canonical_position(final_origin, &reflected, 1.0);
            assert_eq!(saved, (1788, 100));
            // 首次raw检测先于默认云朵夹紧，right不会在重启中丢失。
            let side = scene_dock_side(true, saved, actor_bounds(&normal), 1.0, area, 16);
            assert_eq!(side, Some("right"));
            let before_reflect = scene_position(saved, &normal, 1.0, area, side);
            assert_eq!(before_reflect, (1688, 100));
            final_origin = scene_position(before_reflect, &reflected, 1.0, area, side);
            assert_eq!(final_origin, (1700, 100));
        }
        reflected.canonical_anchor = None;
        assert_eq!(
            canonical_position(final_origin, &reflected, 1.0),
            final_origin
        );
        normal.canonical_anchor = Some([f64::NAN, 132.0]);
        assert!(validate_scene(&normal).is_err());
        normal.canonical_anchor = Some([72.0, 1601.0]);
        assert!(validate_scene(&normal).is_err());
        normal.canonical_anchor = Some([-1.0, 132.0]);
        assert!(validate_scene(&normal).is_err());
    }

    /// 旧flush不写新世代也不删除新pending；同一有效释放只写一次，失败保留以供重试。
    #[test]
    fn guarded_position_save_never_consumes_new_intents_and_retries_failed_write_once() {
        let scene: SceneRequest = serde_json::from_value(serde_json::json!({
            "session":"test","revision":1,"width":200,"height":200,
            "anchor":[100,180],"regions":[[0,0,100,100]]
        }))
        .unwrap();
        let old = PendingPositionSave {
            session: scene.session.clone(),
            generation: 7,
            side: None,
        };
        let newer = PendingPositionSave {
            generation: 8,
            ..old.clone()
        };
        let mut state = SceneState {
            active: true,
            scene: Some(scene.clone()),
            pending_position_save: Some(newer.clone()),
            ..Default::default()
        };
        let sample = PositionSaveSample {
            pending: old,
            scene,
            position: (300, 200),
            scale: 1.0,
            area: (0, 0, 1920, 1040),
        };
        let gate = crate::lock(&crate::DRAG_STATE_GATE).unwrap();
        let writes = std::cell::Cell::new(0);
        commit_position_save(&mut state, &sample, 8, false, &gate, |_| {
            writes.set(writes.get() + 1);
            Ok(())
        })
        .unwrap();
        assert_eq!(writes.get(), 0);
        assert!(state.pending_position_save.as_ref() == Some(&newer));
        let sample = PositionSaveSample {
            pending: newer.clone(),
            ..sample
        };
        assert!(
            commit_position_save(&mut state, &sample, 8, false, &gate, |_| Err(
                "模拟写盘失败".into()
            ))
            .is_err()
        );
        assert!(state.pending_position_save.as_ref() == Some(&newer));
        commit_position_save(&mut state, &sample, 8, false, &gate, |_| {
            writes.set(writes.get() + 1);
            Ok(())
        })
        .unwrap();
        assert_eq!(writes.get(), 1);
        assert!(state.pending_position_save.is_none());
        commit_position_save(&mut state, &sample, 8, false, &gate, |_| {
            writes.set(writes.get() + 1);
            Ok(())
        })
        .unwrap();
        assert_eq!(writes.get(), 1);
    }

    /// 新拖动在旧flush采样后先拿门禁，最终提交必须观察新世代，不能写或清新意图。
    #[test]
    fn concurrent_generation_change_before_save_commit_preserves_new_pending() {
        use std::sync::{
            atomic::{AtomicU64, Ordering},
            Arc,
        };
        let scene: SceneRequest = serde_json::from_value(serde_json::json!({
            "session":"test","revision":1,"width":200,"height":200,
            "anchor":[100,180],"regions":[[0,0,100,100]]
        }))
        .unwrap();
        let old = PendingPositionSave {
            session: scene.session.clone(),
            generation: 7,
            side: None,
        };
        let sample = PositionSaveSample {
            pending: old.clone(),
            scene: scene.clone(),
            position: (300, 200),
            scale: 1.0,
            area: (0, 0, 1920, 1040),
        };
        let shared = Arc::new(Mutex::new(SceneState {
            active: true,
            scene: Some(scene),
            pending_position_save: Some(old),
            ..Default::default()
        }));
        let generation = Arc::new(AtomicU64::new(7));
        let (send, receive) = std::sync::mpsc::channel();
        let target = shared.clone();
        let next_generation = generation.clone();
        let worker = std::thread::spawn(move || {
            let _gate = crate::lock(&crate::DRAG_STATE_GATE).unwrap();
            next_generation.store(8, Ordering::Relaxed);
            target
                .lock()
                .unwrap()
                .pending_position_save
                .as_mut()
                .unwrap()
                .generation = 8;
            send.send(()).unwrap();
        });
        // 明确确保新生命周期先提交，而不是靠sleep碰运气制造竞争。
        receive.recv().unwrap();
        let gate = crate::lock(&crate::DRAG_STATE_GATE).unwrap();
        let mut state = shared.lock().unwrap();
        let mut writes = 0;
        commit_position_save(
            &mut state,
            &sample,
            generation.load(Ordering::Relaxed),
            false,
            &gate,
            |_| {
                writes += 1;
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(writes, 0);
        assert_eq!(state.pending_position_save.as_ref().unwrap().generation, 8);
        drop(state);
        drop(gate);
        worker.join().unwrap();
    }

    /// 预览跨屏后按恢复完成的DPI记账；迟到的旧scene不能掩盖新scene尚未应用的Region。
    #[test]
    fn preview_region_cache_tracks_removal_final_dpi_and_scene_identity() {
        let scene: SceneRequest = serde_json::from_value(serde_json::json!({
            "session":"test","revision":1,"width":200,"height":200,
            "anchor":[100,180],"regions":[[0,0,100,100]]
        }))
        .unwrap();
        let mut state = SceneState {
            active: true,
            scene: Some(scene.clone()),
            region_scale: 1.0,
            ..Default::default()
        };
        record_region_application(&mut state, &scene, None);
        assert_eq!(state.region_scale, 0.0);
        assert!(region_needs_update(
            state.scene.as_ref(),
            &scene,
            state.region_scale,
            1.0
        ));
        record_region_application(&mut state, &scene, Some(1.5));
        assert_eq!(state.region_scale, 1.5);
        assert!(!region_needs_update(
            state.scene.as_ref(),
            &scene,
            state.region_scale,
            1.5
        ));
        state.scene.as_mut().unwrap().revision = 2;
        state.region_scale = 0.0;
        record_region_application(&mut state, &scene, Some(2.0));
        assert_eq!(state.region_scale, 0.0);
    }

    /// 资源包只含一个实际 32px PNG；测试跨层约束，不依赖本机皮肤目录或用户数据。
    fn sample_pack() -> Value {
        serde_json::json!({
            "manifest": { "schemaVersion":1, "id":"test-pet", "name":"合成角色", "author":"ahui", "description":"测试",
                "tokens":{"panelBackground":"#112233","panelBorder":"#445566","primary":"#2288ff","textPrimary":"#ffffff"},
                "actions":{"idle":{"atlas":"idle.png","poster":"idle.png","frameSize":32,"frames":1,"columns":1,"fps":8,"loop":true,"hitBands":[[0,0,32,32]]}}
            },
            "assets":{"idle.png":"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAAAN0lEQVR4nO3QQREAMAgDQYokdKK4nzJVwWdj4DJ7qu+LxeVm3AECBAgQIECAAAECBAgQIEDgCwzVbwL3oaLV6gAAAABJRU5ErkJggg=="}
        })
    }

    #[test]
    fn pack_checks_real_dimensions_colors_paths_and_hit_bounds() {
        let valid = sample_pack();
        assert_eq!(validate_pack(&valid).unwrap(), "test-pet");
        for (path, invalid) in [
            ("/manifest/id", serde_json::json!("classic")),
            ("/manifest/tokens/primary", serde_json::json!("url(remote)")),
            ("/manifest/actions/idle/frameSize", serde_json::json!(64)),
            ("/manifest/actions/idle/fps", serde_json::json!(31)),
            (
                "/manifest/actions/idle/hitBands",
                serde_json::json!([[0, 0, 33, 32]]),
            ),
            (
                "/manifest/actions/idle/atlas",
                serde_json::json!("../idle.png"),
            ),
        ] {
            let mut pack = valid.clone();
            *pack.pointer_mut(path).unwrap() = invalid;
            assert!(validate_pack(&pack).is_err(), "{path}");
        }
        let mut extra = valid;
        extra["assets"]["unreferenced.png"] = serde_json::json!("data:image/png;base64,AAAA");
        assert!(validate_pack(&extra).is_err());
    }

    /// 新动作不提高图片预算或接受未知名称；旧包缺少自主动作仍可正常导入。
    #[test]
    fn optional_activities_and_invalid_blink_are_checked() {
        let mut pack = sample_pack();
        let idle = pack["manifest"]["actions"]["idle"].clone();
        for action in [
            "petting", "dragging", "working", "success", "seated", "wave", "stretch", "landing",
            "bow", "failed", "error",
        ] {
            pack["manifest"]["actions"][action] = idle.clone();
        }
        assert!(validate_pack(&pack).is_ok());
        for blink in [
            Value::Null,
            serde_json::json!({"start":0,"frames":2,"fps":24,"minDelayMs":3500,"maxDelayMs":7500}),
            serde_json::json!({"start":1,"frames":2,"fps":24,"minDelayMs":8000,"maxDelayMs":3500}),
        ] {
            let mut invalid = pack.clone();
            invalid["manifest"]["actions"]["idle"]["blink"] = blink;
            assert!(validate_pack(&invalid).is_err());
        }
        // 升级前的配置缺少 autonomous 字段时，保留经典外观并启用自主动作缺省值。
        let old: crate::model::SkinPreferences =
            serde_json::from_str(r#"{"id":"classic","size":144,"animation":true,"bubbles":true}"#)
                .unwrap();
        assert!(old.autonomous);
        assert_eq!(old.idle_action, "idle");
        assert_eq!(old.id, "classic");
    }

    /// 探头复用动作安全限制且仅准入left单帧；名称不能借此绕过动作白名单。
    #[test]
    fn optional_docking_pack_validates_resources_and_static_left_contract() {
        let mut pack = sample_pack();
        let idle = pack["manifest"]["actions"]["idle"].clone();
        pack["manifest"]["docking"] = serde_json::json!({"left":idle});
        assert!(validate_pack(&pack).is_ok());
        for (path, value) in [
            ("/manifest/docking", Value::Null),
            ("/manifest/docking", serde_json::json!({})),
            ("/manifest/docking", serde_json::json!({"right":idle})),
            ("/manifest/docking/left/frames", serde_json::json!(2)),
            ("/manifest/docking/left/columns", serde_json::json!(2)),
            (
                "/manifest/docking/left/atlas",
                serde_json::json!("../escape.png"),
            ),
            (
                "/manifest/docking/left/hitBands",
                serde_json::json!([[0, 0, 33, 32]]),
            ),
        ] {
            let mut invalid = pack.clone();
            *invalid.pointer_mut(path).unwrap() = value;
            assert!(validate_pack(&invalid).is_err(), "{path}");
        }
        pack["manifest"]["actions"]["left"] = idle;
        assert!(validate_pack(&pack).is_err());
    }

    /// 边饰仅是宿主预设选择，不能借新字段绕过资源包的无脚本边界；旧包仍有效。
    #[test]
    fn decorations_accept_only_trusted_presets_and_keep_legacy_packs() {
        let mut pack = sample_pack();
        assert!(validate_pack(&pack).is_ok());
        for preset in ["none", "ocean", "stars"] {
            pack["manifest"]["decorations"] = serde_json::json!(preset);
            assert!(validate_pack(&pack).is_ok());
        }
        for invalid in [
            Value::Null,
            serde_json::json!({}),
            serde_json::json!("url(remote)"),
        ] {
            pack["manifest"]["decorations"] = invalid;
            assert!(validate_pack(&pack).is_err());
        }
    }
}
