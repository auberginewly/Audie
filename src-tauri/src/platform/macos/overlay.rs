use crate::error::{AppError, AppResult};
use crate::platform::overlay::{OVERLAY_BOTTOM_MARGIN_PX, OVERLAY_WINDOW_LABEL};
use std::{thread, time::Duration};
use tauri::{AppHandle, Manager};
// Last bottom-center origin (x, y) the overlay panel was placed at. The follow
// loop compares the cursor-screen *target* to this — not the panel's live frame,
// which macOS animates during Space switches — so it only moves on a real screen
// change and never fights the swipe animation.
static OVERLAY_LAST_TARGET: parking_lot::Mutex<Option<(f64, f64)>> = parking_lot::Mutex::new(None);

/// Swizzle the overlay `NSWindow` into a non-activating `NSPanel`. Clicking the
/// capsule then never activates Audie, so clipboard injection keeps targeting
/// the user's frontmost app. `cocoa` is deprecated (→ objc2-app-kit) but the
/// crate's `set_collection_behaviour` still takes its type; the API works.
#[allow(deprecated)]
pub(super) fn convert_overlay_to_panel(app: &tauri::App) {
    use tauri_nspanel::cocoa::appkit::NSWindowCollectionBehavior;
    use tauri_nspanel::{WebviewPanelManager, WebviewWindowExt};

    app.manage(WebviewPanelManager::default());
    let overlay = match app.get_webview_window(OVERLAY_WINDOW_LABEL) {
        Some(overlay) => overlay,
        None => {
            log::error!("overlay window missing for panel conversion");
            return;
        }
    };
    let panel = match overlay.to_panel() {
        Ok(panel) => panel,
        Err(err) => {
            log::error!("overlay to_panel failed: {err:?}");
            return;
        }
    };
    // NSWindowStyleMaskNonactivatingPanel = 1 << 7.
    panel.set_style_mask(1 << 7);
    // RawNSPanel forces canBecomeKeyWindow = YES, so a button click would grab
    // keyboard focus and the synthesized Cmd+V would paste into the panel, not
    // the user's field. becomesKeyOnlyIfNeeded keeps key focus on their app.
    panel.set_becomes_key_only_if_needed(true);
    // Receive mouse-moved events so the ✕/✓ buttons get :hover states.
    panel.set_accepts_mouse_moved_events(true);
    // NSPanel hides on app deactivation by default. Our overlay is non-activating
    // so Audie is never the active app — without this it would hide+show (flicker)
    // on every Space/app switch.
    panel.set_hides_on_deactivate(false);
    // Above app windows; visible across spaces and over fullscreen apps.
    panel.set_level(25); // NSStatusWindowLevel
    panel.set_collection_behaviour(
        NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary
            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary,
    );
}

/// Move the overlay panel to the bottom-center of the screen the cursor is on,
/// so on a multi-display setup the capsule flies to the active screen. Done
/// natively (NSEvent.mouseLocation + NSScreen, all in points / one coordinate
/// space) to dodge winit's multi-scale physical-pixel pitfalls. Main thread only.
// `deprecated`: cocoa types. `unexpected_cfgs`: the old `objc` crate's msg_send!
// expands a `cargo-clippy` cfg that newer rustc flags.
#[allow(deprecated, unexpected_cfgs)]
pub(super) fn reposition_overlay_to_cursor_screen(panel: &tauri_nspanel::raw_nspanel::RawNSPanel) {
    use tauri_nspanel::cocoa::appkit::{NSEvent, NSScreen};
    use tauri_nspanel::cocoa::base::{id, nil};
    use tauri_nspanel::cocoa::foundation::{NSPoint, NSRect};
    use tauri_nspanel::objc::{msg_send, sel, sel_impl};

    unsafe {
        let mouse: NSPoint = NSEvent::mouseLocation(nil);
        let screens: id = NSScreen::screens(nil);
        if screens == nil {
            return;
        }
        let count: usize = msg_send![screens, count];
        // Find the screen whose frame contains the cursor; fall back to main.
        let mut target: id = nil;
        for i in 0..count {
            let screen: id = msg_send![screens, objectAtIndex: i];
            let f: NSRect = NSScreen::frame(screen);
            if mouse.x >= f.origin.x
                && mouse.x <= f.origin.x + f.size.width
                && mouse.y >= f.origin.y
                && mouse.y <= f.origin.y + f.size.height
            {
                target = screen;
                break;
            }
        }
        if target == nil {
            target = NSScreen::mainScreen(nil);
        }
        if target == nil {
            return;
        }
        // visibleFrame excludes the Dock/menu bar, so "贴底" sits above the Dock.
        let vf: NSRect = NSScreen::visibleFrame(target);
        let frame: NSRect = msg_send![panel, frame];
        let x = vf.origin.x + (vf.size.width - frame.size.width) / 2.0;
        let y = vf.origin.y + OVERLAY_BOTTOM_MARGIN_PX;
        // Move only when the *target* (cursor's screen) changed — never react to
        // the panel's live frame, which macOS animates mid-swipe. Comparing the
        // target keeps the follow loop from snapping the panel back during the
        // Space-switch animation (the flicker the user saw).
        let mut last = OVERLAY_LAST_TARGET.lock();
        let changed = last.is_none_or(|(lx, ly)| (lx - x).abs() > 1.0 || (ly - y).abs() > 1.0);
        if changed {
            *last = Some((x, y));
            drop(last);
            let _: () = msg_send![panel, setFrameOrigin: NSPoint { x, y }];
        }
    }
}

/// Live multi-display follow: while the capsule is visible, poll the cursor and
/// re-place the panel on its screen, so it flies to whatever display the user
/// moves to mid-recording. Re-placing to the same spot is a no-op, so the panel
/// only actually moves when the cursor crosses to another screen.
pub(super) fn spawn_overlay_follow_thread(app: AppHandle) {
    use tauri_nspanel::ManagerExt;
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(150));
        let app2 = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Ok(panel) = app2.get_webview_panel(OVERLAY_WINDOW_LABEL) {
                if panel.is_visible() {
                    reposition_overlay_to_cursor_screen(&panel);
                }
            }
        });
    });
}

pub(super) fn show_overlay(app: &AppHandle) -> AppResult<()> {
    let handle = app.clone();
    app.run_on_main_thread(move || {
        use tauri_nspanel::ManagerExt;
        match handle.get_webview_panel(OVERLAY_WINDOW_LABEL) {
            Ok(panel) => {
                // Fly to the screen the cursor is on, then show WITHOUT making it
                // key (so the user's app stays frontmost for injection).
                reposition_overlay_to_cursor_screen(&panel);
                panel.order_front_regardless();
            }
            Err(err) => log::error!("show_overlay: panel not found: {err:?}"),
        }
    })
    .map_err(|err| AppError::Internal(format!("run_on_main_thread: {err}")))
}

pub(super) fn hide_overlay(app: &AppHandle) -> AppResult<()> {
    // The pipeline dispatches this on the main thread after its ownership check.
    use tauri_nspanel::ManagerExt;
    match app.get_webview_panel(OVERLAY_WINDOW_LABEL) {
        Ok(panel) => panel.order_out(None),
        Err(err) => log::error!("hide_overlay: panel not found: {err:?}"),
    }
    Ok(())
}

pub(super) fn initialize_overlay(app: &tauri::App) -> AppResult<()> {
    crate::platform::overlay::position_overlay(app.handle())?;
    convert_overlay_to_panel(app);
    spawn_overlay_follow_thread(app.handle().clone());
    Ok(())
}
