//! Milestone 5: right-click to move one unit via A* over a blocked grid.

use umbra::app::{run, AppConfig, EngineApp, FrameCtx};
use umbra::fixed::{Fixed, FixedVec2};
use umbra::gfx::atlas::{dev_atlas, Atlas};
use umbra::gfx::batch::SpriteBatch;
use umbra::gfx::camera::Camera;
use umbra::iso::IsoProj;
use umbra::path::{astar, Grid, Mover, Occupancy, Tile};
use umbra::sim::{Envelope, Game, Sim, TickTimer};
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

const MAP: i32 = 20;
const ISO: IsoProj = IsoProj { tile_w: 128.0, tile_h: 64.0 };

fn tile_center(t: Tile) -> FixedVec2 {
    FixedVec2::new(
        Fixed::from_int(t.0) + Fixed(128),
        Fixed::from_int(t.1) + Fixed(128),
    )
}

#[derive(Clone)]
struct PathGame {
    grid: Grid,
    occ: Occupancy,
    mover: Mover,
}

impl PathGame {
    fn new() -> Self {
        let mut grid = Grid::new(MAP, MAP);
        for i in 0..MAP {
            // two walls with gaps, so paths have to detour
            if i != 4 && i != 15 {
                grid.set_blocked((8, i), true);
                grid.set_blocked((13, i), true);
            }
        }
        let start: Tile = (2, 2);
        let mut occ = Occupancy::new();
        occ.claim(start, 0);
        PathGame { grid, occ, mover: Mover::new(tile_center(start), Fixed(32)) }
    }
}

impl Game for PathGame {
    type Cmd = Tile; // "move to this tile"

    fn apply(&mut self, env: &Envelope<Tile>) {
        if let Some(path) = astar(&self.grid, self.mover.current_tile(), env.cmd, &|_| false) {
            self.mover.set_path(path);
        }
    }

    fn tick(&mut self) {
        self.mover.step(0, &mut self.occ);
    }
}

struct Demo {
    atlas: Option<Atlas>,
    batch: SpriteBatch,
    camera: Camera,
    sim: Sim<PathGame>,
    timer: TickTimer,
    seq: u32,
}

impl Default for Demo {
    fn default() -> Self {
        Demo {
            atlas: None,
            batch: SpriteBatch::default(),
            camera: Camera::default(),
            sim: Sim::new(PathGame::new()),
            timer: TickTimer::new(20.0),
            seq: 0,
        }
    }
}

impl EngineApp for Demo {
    fn init(&mut self, gfx: &mut umbra::gfx::Gfx) {
        self.atlas = Some(dev_atlas(gfx));
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

        // right-click → command through the spine
        if ctx.input.mouse_pressed(MouseButton::Right) {
            let wp = self.camera.screen_to_world(ctx.input.mouse(), ctx.gfx.viewport());
            let tile = ISO.screen_to_tile(wp.0, wp.1);
            if (0..MAP).contains(&tile.0) && (0..MAP).contains(&tile.1) {
                self.seq += 1;
                self.sim.enqueue(Envelope {
                    tick: self.sim.tick(),
                    player: 0,
                    seq: self.seq,
                    cmd: tile,
                });
            }
        }

        for _ in 0..self.timer.advance(ctx.dt) {
            self.sim.step();
        }

        let atlas = self.atlas.as_ref().unwrap();
        self.batch.clear();
        for ty in 0..MAP {
            for tx in 0..MAP {
                let name = if self.sim.curr().grid.is_blocked((tx, ty)) { "block" } else { "grass" };
                let (sx, sy) = ISO.world_to_screen(tx as f32 + 0.5, ty as f32 + 0.5);
                self.batch.push(atlas.region(name), (sx, sy + 32.0), 0);
            }
        }
        // interpolated unit position: fixed tile-units → f32 → world px
        let alpha = self.timer.alpha();
        let (p, c) = (self.sim.prev().mover.pos, self.sim.curr().mover.pos);
        let wx = p.x.to_f32() + (c.x.to_f32() - p.x.to_f32()) * alpha;
        let wy = p.y.to_f32() + (c.y.to_f32() - p.y.to_f32()) * alpha;
        let (sx, sy) = ISO.world_to_screen(wx, wy);
        self.batch.push(atlas.region("unit"), (sx, sy), 1);

        let Some(mut frame) = ctx.gfx.frame() else { return };
        frame.clear([0.08, 0.08, 0.10, 1.0]);
        frame.draw_sprites(&self.batch, atlas, &self.camera);
        frame.finish();
    }
}

fn main() -> anyhow::Result<()> {
    run(AppConfig { title: "umbra — pathmove".into(), ..Default::default() }, Demo::default())
}
