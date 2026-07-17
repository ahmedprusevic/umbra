//! Milestone 3: generated isometric tilemap + tile picking under the mouse.

use umbra::app::{run, AppConfig, EngineApp, FrameCtx};
use umbra::gfx::atlas::{dev_atlas, Atlas};
use umbra::gfx::batch::SpriteBatch;
use umbra::gfx::camera::Camera;
use umbra::iso::IsoProj;
use winit::keyboard::KeyCode;

const MAP: i32 = 16;
const ISO: IsoProj = IsoProj { tile_w: 128.0, tile_h: 64.0 };

/// Deterministic "terrain": rocks dotted around, sand near the diagonal.
fn tile_kind(tx: i32, ty: i32) -> &'static str {
    if (tx * 7 + ty * 13) % 11 == 0 {
        "block"
    } else if (tx + ty) % 5 < 2 {
        "sand"
    } else {
        "grass"
    }
}

#[derive(Default)]
struct Demo {
    atlas: Option<Atlas>,
    batch: SpriteBatch,
    camera: Camera,
}

impl EngineApp for Demo {
    fn init(&mut self, gfx: &mut umbra::gfx::Gfx) {
        self.atlas = Some(dev_atlas(gfx));
        // start centered on the map: middle tile in world px
        let (cx, cy) = ISO.world_to_screen(MAP as f32 / 2.0, MAP as f32 / 2.0);
        self.camera.center = (cx, cy);
    }

    fn frame(&mut self, ctx: &mut FrameCtx) {
        if ctx.input.pressed(KeyCode::Escape) {
            std::process::exit(0);
        }
        let pan = 600.0 * ctx.dt / self.camera.zoom;
        if ctx.input.is_down(KeyCode::KeyW) { self.camera.center.1 -= pan; }
        if ctx.input.is_down(KeyCode::KeyS) { self.camera.center.1 += pan; }
        if ctx.input.is_down(KeyCode::KeyA) { self.camera.center.0 -= pan; }
        if ctx.input.is_down(KeyCode::KeyD) { self.camera.center.0 += pan; }
        self.camera.zoom = (self.camera.zoom * (1.0 + ctx.input.scroll() * 0.1)).clamp(0.25, 4.0);

        let viewport = ctx.gfx.viewport();
        let world_px = self.camera.screen_to_world(ctx.input.mouse(), viewport);
        let hover = ISO.screen_to_tile(world_px.0, world_px.1);

        let atlas = self.atlas.as_ref().unwrap();
        self.batch.clear();
        for ty in 0..MAP {
            for tx in 0..MAP {
                let (sx, sy) = ISO.world_to_screen(tx as f32 + 0.5, ty as f32 + 0.5);
                self.batch.push(atlas.region(tile_kind(tx, ty)), (sx, sy + 32.0), 0);
            }
        }
        // hover marker: ring on the picked tile (only when it's on the map)
        if (0..MAP).contains(&hover.0) && (0..MAP).contains(&hover.1) {
            let (sx, sy) = ISO.world_to_screen(hover.0 as f32 + 0.5, hover.1 as f32 + 0.5);
            self.batch.push(atlas.region("ring"), (sx, sy + 12.0), 1);
        }

        let Some(mut frame) = ctx.gfx.frame() else { return };
        frame.clear([0.08, 0.08, 0.10, 1.0]);
        frame.draw_sprites(&self.batch, atlas, &self.camera);
        frame.finish();
    }
}

fn main() -> anyhow::Result<()> {
    run(AppConfig { title: "umbra — isomap".into(), ..Default::default() }, Demo::default())
}
