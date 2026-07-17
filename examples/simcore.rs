//! Milestone 4: fixed-tick sim + render interpolation. Hold I to see raw ticks.

use umbra::app::{run, AppConfig, EngineApp, FrameCtx};
use umbra::fixed::{Fixed, FixedVec2};
use umbra::gfx::atlas::{dev_atlas, Atlas};
use umbra::gfx::batch::SpriteBatch;
use umbra::gfx::camera::Camera;
use umbra::sim::{Envelope, Game, Sim, TickTimer};
use winit::keyboard::KeyCode;

/// A ball bouncing horizontally between two walls, in fixed-point.
#[derive(Clone)]
struct Bounce {
    pos: FixedVec2,
    vel: Fixed,
}

impl Game for Bounce {
    type Cmd = ();
    fn apply(&mut self, _env: &Envelope<()>) {}
    fn tick(&mut self) {
        self.pos.x = self.pos.x + self.vel;
        if self.pos.x > Fixed::from_int(6) || self.pos.x < Fixed::from_int(-6) {
            self.vel = -self.vel;
        }
    }
}

struct Demo {
    atlas: Option<Atlas>,
    batch: SpriteBatch,
    camera: Camera,
    sim: Sim<Bounce>,
    timer: TickTimer,
}

impl Default for Demo {
    fn default() -> Self {
        Demo {
            atlas: None,
            batch: SpriteBatch::default(),
            camera: Camera::default(),
            sim: Sim::new(Bounce {
                pos: FixedVec2::new(Fixed::ZERO, Fixed::ZERO),
                vel: Fixed(64), // a quarter tile per tick
            }),
            timer: TickTimer::new(20.0),
        }
    }
}

impl EngineApp for Demo {
    fn init(&mut self, gfx: &mut umbra::gfx::Gfx) {
        self.atlas = Some(dev_atlas(gfx));
    }

    fn frame(&mut self, ctx: &mut FrameCtx) {
        if ctx.input.pressed(KeyCode::Escape) {
            std::process::exit(0);
        }
        for _ in 0..self.timer.advance(ctx.dt) {
            self.sim.step();
        }
        // interpolate prev -> curr; 50 px per tile-unit for display
        let alpha = if ctx.input.is_down(KeyCode::KeyI) { 1.0 } else { self.timer.alpha() };
        let (x0, x1) = (self.sim.prev().pos.x.to_f32(), self.sim.curr().pos.x.to_f32());
        let x = (x0 + (x1 - x0) * alpha) * 50.0;

        let atlas = self.atlas.as_ref().unwrap();
        self.batch.clear();
        self.batch.push(atlas.region("unit"), (x, 18.0), 0);

        let Some(mut frame) = ctx.gfx.frame() else { return };
        frame.clear([0.08, 0.08, 0.10, 1.0]);
        frame.draw_sprites(&self.batch, atlas, &self.camera);
        frame.finish();
    }
}

fn main() -> anyhow::Result<()> {
    run(AppConfig { title: "umbra — simcore".into(), ..Default::default() }, Demo::default())
}
