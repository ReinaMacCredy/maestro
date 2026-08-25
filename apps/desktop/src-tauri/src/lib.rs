use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State};

const PILL_W: f64 = 260.0;
const PILL_H: f64 = 44.0;
const PANEL_W: f64 = 440.0;
const PANEL_H: f64 = 620.0;
const MARGIN_BOTTOM: f64 = 6.0;
const LEAVE_POLL: Duration = Duration::from_millis(120);

#[derive(Default)]
pub struct Ui {
    pinned: AtomicBool,
    expanded: AtomicBool,
}

#[cfg(target_os = "macos")]
mod mac {
    use super::expand;
    use tauri::{AppHandle, Manager};
    use tauri_nspanel::{
        tauri_panel, CollectionBehavior, PanelLevel, StyleMask, TrackingAreaOptions,
        WebviewWindowExt,
    };

    tauri_panel! {
        panel!(PillPanel {
            config: {
                can_become_key_window: false,
                can_become_main_window: false,
                is_floating_panel: true
            }
            with: {
                tracking_area: {
                    options: TrackingAreaOptions::new()
                        .active_always()
                        .mouse_entered_and_exited(),
                    auto_resize: true
                }
            }
        })
        panel_event!(PillEvents {})
    }

    // Non-activating panel: the app never becomes active, the terminal keeps focus.
    // Hover-in is tracked at the NSWindow level because a non-key WKWebView does not
    // get reliable mouse events; hover-out is polled (see `expand`) because the
    // tracking area is rebuilt on resize and stops reporting exits.
    pub fn make_panel(app: &AppHandle) -> tauri::Result<()> {
        let window = app.get_webview_window("main").expect("main window");
        let panel = window.to_panel::<PillPanel>()?;
        panel.set_level(PanelLevel::Floating.value());
        panel.set_style_mask(StyleMask::empty().nonactivating_panel().into());
        panel.set_collection_behavior(
            CollectionBehavior::new()
                .can_join_all_spaces()
                .stationary()
                .full_screen_auxiliary()
                .into(),
        );
        panel.set_hides_on_deactivate(false);

        let events = PillEvents::new();
        let h = app.clone();
        events.on_mouse_entered(move |_| expand(&h));
        panel.set_event_handler(Some(events.as_ref()));
        panel.order_front_regardless();
        Ok(())
    }
}

fn cursor_inside(app: &AppHandle) -> bool {
    let Some(window) = app.get_webview_window("main") else { return false };
    let (Ok(c), Ok(p), Ok(s)) = (app.cursor_position(), window.outer_position(), window.outer_size())
    else {
        return false;
    };
    let (x, y) = (c.x as i32, c.y as i32);
    x >= p.x && x < p.x + s.width as i32 && y >= p.y && y < p.y + s.height as i32
}

/// Expand once and watch the cursor until it leaves the frame (unless pinned).
fn expand(app: &AppHandle) {
    let ui = app.state::<Ui>();
    if ui.expanded.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = layout(app, true);
    let _ = app.emit("hover", true);
    let h = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(LEAVE_POLL);
        let ui = h.state::<Ui>();
        if !ui.expanded.load(Ordering::SeqCst) {
            break;
        }
        if ui.pinned.load(Ordering::SeqCst) || cursor_inside(&h) {
            continue;
        }
        collapse(&h);
        break;
    });
}

fn collapse(app: &AppHandle) {
    app.state::<Ui>().expanded.store(false, Ordering::SeqCst);
    let _ = layout(app, false);
    let _ = app.emit("hover", false);
}

/// Anchor the window at the bottom-center of the monitor's work area (above the
/// Dock / taskbar) with its bottom edge fixed while it grows or shrinks.
fn layout(app: &AppHandle, expanded: bool) -> tauri::Result<()> {
    let window = app.get_webview_window("main").expect("main window");
    let monitor = window
        .current_monitor()?
        .or(app.primary_monitor()?)
        .expect("no monitor");
    let scale = monitor.scale_factor();
    let wa = monitor.work_area();
    let (w, h) = if expanded { (PANEL_W, PANEL_H) } else { (PILL_W, PILL_H) };
    let pw = (w * scale) as u32;
    let ph = (h * scale) as u32;
    let x = wa.position.x + ((wa.size.width as i32 - pw as i32) / 2);
    let y = wa.position.y + wa.size.height as i32 - ph as i32 - (MARGIN_BOTTOM * scale) as i32;
    window.set_size(PhysicalSize::new(pw, ph))?;
    window.set_position(PhysicalPosition::new(x, y))?;
    eprintln!("[layout] expanded={} x={} y={} w={} h={}", expanded, x, y, pw, ph);
    Ok(())
}

#[tauri::command]
fn set_pinned(app: AppHandle, ui: State<Ui>, value: bool) {
    ui.pinned.store(value, Ordering::SeqCst);
    if !value && !cursor_inside(&app) {
        collapse(&app);
    }
}

pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());
    builder
        .manage(Ui::default())
        .invoke_handler(tauri::generate_handler![set_pinned])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            layout(app.handle(), false)?;
            #[cfg(target_os = "macos")]
            mac::make_panel(app.handle())?;
            #[cfg(not(target_os = "macos"))]
            app.get_webview_window("main").expect("main window").show()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running maestro desktop");
}
