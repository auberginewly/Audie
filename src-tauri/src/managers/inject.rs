// InjectManager — text injection at the caret. PROJECT_SPEC.md §6.1.
//
// The clipboard-method system calls live in the platform layer (§6.3); this
// manager is the platform-agnostic entry point that owns the §3.7 fallback:
// if injection fails, the text is already on the clipboard, so the user can
// still paste it manually — we surface an `Inject` error rather than losing it.

use std::sync::Arc;

use tauri::AppHandle;

use crate::error::AppResult;
use crate::platform::Platform;

pub struct InjectManager {
    platform: Arc<dyn Platform>,
}

impl InjectManager {
    pub fn new(platform: Arc<dyn Platform>) -> Self {
        Self { platform }
    }

    pub fn inject(&self, app: &AppHandle, text: &str) -> AppResult<()> {
        self.platform.inject_text(app, text)
    }
}
