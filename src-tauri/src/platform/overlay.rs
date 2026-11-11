use crate::error::{AppError, AppResult};
use tauri::{AppHandle, Manager, PhysicalPosition};
pub(super) const OVERLAY_WINDOW_LABEL: &str = "overlay";
pub(super) const OVERLAY_BOTTOM_MARGIN_PX: f64 = 4.0;
/// Place the capsule at the bottom-center of whichever monitor the cursor is on,
/// so on a multi-display setup it follows the user to the active screen. Called
/// at setup and again on every show. Interactivity is handled by the NSPanel
/// conversion in `setup`, not here.
pub(super) fn position_overlay(app: &AppHandle) -> AppResult<()> {
    let overlay = app
        .get_webview_window(OVERLAY_WINDOW_LABEL)
        .ok_or_else(|| AppError::Internal("overlay window not found".into()))?;

    let monitors = overlay
        .available_monitors()
        .map_err(|err| AppError::Internal(format!("available_monitors: {err}")))?;
    let cursor = app.cursor_position().ok();
    let monitor = cursor
        .and_then(|c| {
            monitors.iter().find(|m| {
                let p = m.position();
                let s = m.size();
                c.x >= p.x as f64
                    && c.x < (p.x + s.width as i32) as f64
                    && c.y >= p.y as f64
                    && c.y < (p.y + s.height as i32) as f64
            })
        })
        .or_else(|| monitors.first())
        .ok_or_else(|| AppError::Internal("no monitor available".into()))?;

    let m_pos = monitor.position();
    let m_size = monitor.size();
    let scale = monitor.scale_factor();
    let win_size = overlay
        .outer_size()
        .map_err(|err| AppError::Internal(format!("outer_size: {err}")))?;

    // Offset by the monitor origin so the position is correct in the global
    // multi-display coordinate space.
    let bottom_margin_px = (OVERLAY_BOTTOM_MARGIN_PX * scale).round() as i32;
    let x = m_pos.x + (m_size.width as i32 - win_size.width as i32) / 2;
    let y = m_pos.y + m_size.height as i32 - win_size.height as i32 - bottom_margin_px;

    overlay
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|err| AppError::Internal(format!("set_position: {err}")))?;
    Ok(())
}

pub(super) fn show_overlay(app: &AppHandle) -> AppResult<()> {
    app.get_webview_window(OVERLAY_WINDOW_LABEL)
        .ok_or_else(|| AppError::Internal("overlay window not found".into()))?
        .show()
        .map_err(|err| AppError::Internal(format!("show overlay: {err}")))
}

pub(super) fn hide_overlay(app: &AppHandle) -> AppResult<()> {
    app.get_webview_window(OVERLAY_WINDOW_LABEL)
        .ok_or_else(|| AppError::Internal("overlay window not found".into()))?
        .hide()
        .map_err(|err| AppError::Internal(format!("hide overlay: {err}")))
}
