// State machine — single source of truth for the recording pipeline.
// Transitions: PROJECT_SPEC.md §3.3. Event payload: §3.6 `state-change`.
//
// P0.1 only exercises Idle ↔ Recording. The Processing / Success / Error /
// Cancel arms are wired so later slices (P0.4+) can extend without rework.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum AppState {
    Idle,
    Recording,
    Processing,
    Success,
    Error,
    Cancel,
}

#[derive(Debug, Clone, Serialize)]
pub struct StateChange {
    pub from: AppState,
    pub to: AppState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

const STATE_CHANGE_EVENT: &str = "state-change";

pub struct StateMachine {
    current: Mutex<AppState>,
}

impl StateMachine {
    pub fn new() -> Self {
        Self {
            current: Mutex::new(AppState::Idle),
        }
    }

    #[allow(dead_code)] // Used by commands and pipeline in later slices.
    pub fn current(&self) -> AppState {
        *self.current.lock()
    }

    /// Attempt a transition. Illegal transitions are ignored with a warn log
    /// (§3.3: "non-legal transitions ignored").
    /// Returns `true` if the transition was applied and emitted.
    pub fn transition(&self, app: &AppHandle, to: AppState, reason: Option<&str>) -> bool {
        let mut guard = self.current.lock();
        let from = *guard;

        if !is_legal(from, to) {
            log::warn!(
                "illegal state transition: {:?} -> {:?} (reason: {:?}), ignored",
                from,
                to,
                reason
            );
            return false;
        }

        *guard = to;
        // Drop the lock before emitting so subscribers can re-enter if needed.
        drop(guard);

        let payload = StateChange {
            from,
            to,
            reason: reason.map(str::to_string),
        };

        if let Err(err) = app.emit(STATE_CHANGE_EVENT, &payload) {
            log::error!("failed to emit state-change event: {err}");
        } else {
            log::info!("state {:?} -> {:?} (reason: {:?})", from, to, reason);
        }

        true
    }
}

impl Default for StateMachine {
    fn default() -> Self {
        Self::new()
    }
}

fn is_legal(from: AppState, to: AppState) -> bool {
    use AppState::*;
    match (from, to) {
        // Idle -> Error: permission denied at press time, before recording starts (P0.6).
        (Idle, Recording | Error) => true,
        // P0.1 short-circuits Recording -> Idle directly (no transcription yet).
        // P0.4+ will wire Recording -> Processing -> Success/Error -> Idle.
        (Recording, Processing | Idle | Cancel) => true,
        (Processing, Success | Error | Cancel) => true,
        (Success | Error | Cancel, Idle) => true,
        _ => false,
    }
}
