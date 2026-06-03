//! Core library for llm-system-containers.
//!
//! Holds the logic shared by the `llmsc` / `llmsctl` CLIs and the Tauri GUI. External systems
//! (Incus, the VM driver) sit behind traits so this logic stays unit-testable with fakes.
//! See `planning/tech-stack.md` and `planning/buildout.md`.

pub mod config;
