//! The desktop bridge between the Tauri shell and the Engine.
//!
//! This library has no dependency on Tauri, so the frame format and the simulation
//! timing rules can be tested without a window. `main.rs` wires it to Tauri commands.

pub mod frame;
pub mod project;
pub mod simulation;
