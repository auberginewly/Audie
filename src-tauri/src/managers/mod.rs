// Manager registry. Each manager owns one chunk of the pipeline
// (PROJECT_SPEC.md §6.1) and is stashed on the Tauri state at startup.

pub mod audio;
pub mod inject;
pub mod transcription;
