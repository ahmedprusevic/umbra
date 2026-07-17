//! Milestone 1: a window cleared to a solid color. Esc quits.

use umbra::app::{run, AppConfig, EngineApp, FrameCtx};
use winit::keyboard::KeyCode;

struct Demo;

impl EngineApp for Demo {
    fn frame(&mut self, ctx: &mut FrameCtx) {
        if ctx.input.pressed(KeyCode::Escape) {
            std::process::exit(0);
        }
        let Some(mut frame) = ctx.gfx.frame() else { return };
        frame.clear([0.10, 0.16, 0.25, 1.0]);
        frame.finish();
    }
}

fn main() -> anyhow::Result<()> {
    run(AppConfig { title: "umbra — window".into(), ..Default::default() }, Demo)
}
