//! Umbra — a 2D isometric RTS engine.
//!
//! Sim-side modules (`fixed`, `sim`, `path`) are deterministic and
//! fixed-point: they never touch floats. Render-side modules (`app`,
//! `gfx`, `iso`, `select`) are f32 and feed the screen.

pub mod app;
pub mod fixed;
pub mod gfx;
pub mod input;
pub mod iso;
pub mod path;
pub mod select;
pub mod sim;
