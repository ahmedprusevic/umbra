//! Milestone 2: atlas sprites with camera pan (WASD) and zoom (scroll).

use umbra::app::{run, AppConfig, EngineApp, FrameCtx};
use umbra::gfx::atlas::{dev_atlas, Atlas};
use umbra::gfx::batch::SpriteBatch;
use umbra::gfx::camera::Camera;
use umbra::iso::IsoProj;
use winit::keyboard::KeyCode;

#[derive(Default)]
struct Demo {
    atlas: Option<Atlas>,
    batch: SpriteBatch,
    camera: Camera,
}

impl EngineApp for Demo {
    fn init(&mut self, gfx: &mut umbra::gfx::Gfx) {
        self.atlas = Some(dev_atlas(gfx));
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

        let atlas = self.atlas.as_ref().unwrap();
        let iso = IsoProj { tile_w: 128.0, tile_h: 64.0 };
        self.batch.clear();
        for ty in 0..12 {
            for tx in 0..12 {
                let name = if (tx + ty) % 2 == 0 { "grass" } else { "sand" };
                let (sx, sy) = iso.world_to_screen(tx as f32 + 0.5, ty as f32 + 0.5);
                // bottom-center anchor: diamond's bottom tip is center + h/2
                self.batch.push(atlas.region(name), (sx, sy + 32.0), 0);
            }
        }
        // a few "units" standing on tiles
        for (tx, ty) in [(2, 3), (5, 5), (8, 2)] {
            let (sx, sy) = iso.world_to_screen(tx as f32 + 0.5, ty as f32 + 0.5);
            self.batch.push(atlas.region("unit"), (sx, sy), 1);
        }

        let Some(mut frame) = ctx.gfx.frame() else { return };
        frame.clear([0.08, 0.08, 0.10, 1.0]);
        frame.draw_sprites(&self.batch, atlas, &self.camera);
        frame.finish();
    }
}

fn main() -> anyhow::Result<()> {
    run(AppConfig { title: "umbra — sprites".into(), ..Default::default() }, Demo::default())
}
