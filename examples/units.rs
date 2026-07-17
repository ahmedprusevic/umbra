//! Milestone 6 capstone: drag-select units, right-click group move.
//! Tile reservation keeps units from stacking; blocked units re-path.

use umbra::app::{run, AppConfig, EngineApp, FrameCtx};
use umbra::fixed::{Fixed, FixedVec2};
use umbra::gfx::atlas::{dev_atlas, Atlas};
use umbra::gfx::batch::SpriteBatch;
use umbra::gfx::camera::Camera;
use umbra::iso::IsoProj;
use umbra::path::{astar, Grid, Mover, MoveResult, Occupancy, Tile};
use umbra::select::{drag_box, pick, Rect};
use umbra::sim::{Envelope, Game, Sim, TickTimer};
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

const MAP: i32 = 20;
const ISO: IsoProj = IsoProj { tile_w: 128.0, tile_h: 64.0 };
const REPATH_AFTER: u32 = 2; // ticks spent Blocked before re-pathing

fn tile_center(t: Tile) -> FixedVec2 {
    FixedVec2::new(
        Fixed::from_int(t.0) + Fixed(128),
        Fixed::from_int(t.1) + Fixed(128),
    )
}

#[derive(Clone)]
struct Unit {
    id: u32,
    mover: Mover,
    blocked_for: u32,
    goal: Option<Tile>,
}

#[derive(Clone)]
struct UnitsGame {
    grid: Grid,
    occ: Occupancy,
    units: Vec<Unit>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Cmd {
    Move { ids: Vec<u32>, to: Tile },
}

impl UnitsGame {
    fn new() -> Self {
        let mut grid = Grid::new(MAP, MAP);
        for i in 0..MAP {
            if i != 4 && i != 15 {
                grid.set_blocked((10, i), true);
            }
        }
        let mut occ = Occupancy::new();
        let mut units = Vec::new();
        for (i, t) in [(2, 2), (3, 2), (2, 3), (3, 3), (2, 4), (3, 4)].into_iter().enumerate() {
            let id = i as u32;
            occ.claim(t, id);
            units.push(Unit {
                id,
                mover: Mover::new(tile_center(t), Fixed(32)),
                blocked_for: 0,
                goal: None,
            });
        }
        UnitsGame { grid, occ, units }
    }

    fn repath(grid: &Grid, occ: &Occupancy, unit: &mut Unit) {
        let Some(goal) = unit.goal else { return };
        let id = unit.id;
        let path = astar(grid, unit.mover.current_tile(), goal, &|t| {
            occ.holder(t).is_some_and(|h| h != id)
        })
        .or_else(|| astar(grid, unit.mover.current_tile(), goal, &|_| false));
        if let Some(p) = path {
            unit.mover.set_path(p);
        } else {
            unit.goal = None; // truly unreachable: give up
        }
    }
}

impl Game for UnitsGame {
    type Cmd = Cmd;

    fn apply(&mut self, env: &Envelope<Cmd>) {
        let Cmd::Move { ids, to } = &env.cmd;
        for id in ids {
            if let Some(unit) = self.units.iter_mut().find(|u| u.id == *id) {
                unit.goal = Some(*to);
                unit.blocked_for = 0;
                Self::repath(&self.grid, &self.occ, unit);
            }
        }
    }

    fn tick(&mut self) {
        // iterate in id order — determinism depends on a fixed unit order
        for i in 0..self.units.len() {
            let mut unit = self.units[i].clone();
            match unit.mover.step(unit.id, &mut self.occ) {
                MoveResult::Blocked => {
                    unit.blocked_for += 1;
                    if unit.blocked_for >= REPATH_AFTER {
                        unit.blocked_for = 0;
                        Self::repath(&self.grid, &self.occ, &mut unit);
                    }
                }
                MoveResult::Moving => unit.blocked_for = 0,
                MoveResult::Arrived => {
                    unit.blocked_for = 0;
                    unit.goal = None;
                }
            }
            self.units[i] = unit;
        }
    }
}

struct Demo {
    atlas: Option<Atlas>,
    batch: SpriteBatch,
    camera: Camera,
    sim: Sim<UnitsGame>,
    timer: TickTimer,
    seq: u32,
    selected: Vec<u32>,
    drag_from: Option<(f32, f32)>,
}

impl Default for Demo {
    fn default() -> Self {
        Demo {
            atlas: None,
            batch: SpriteBatch::default(),
            camera: Camera::default(),
            sim: Sim::new(UnitsGame::new()),
            timer: TickTimer::new(20.0),
            seq: 0,
            selected: Vec::new(),
            drag_from: None,
        }
    }
}

impl Demo {
    /// Units projected to actual screen px: (id, screen pos, pick radius).
    fn unit_screen_items(&self, viewport: (u32, u32)) -> Vec<(u32, (f32, f32), f32)> {
        self.sim
            .curr()
            .units
            .iter()
            .map(|u| {
                let w = (u.mover.pos.x.to_f32(), u.mover.pos.y.to_f32());
                let wp = ISO.world_to_screen(w.0, w.1);
                let sp = self.camera.world_to_screen(wp, viewport);
                (u.id, sp, 24.0 * self.camera.zoom)
            })
            .collect()
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
        let viewport = ctx.gfx.viewport();
        let pan = 600.0 * ctx.dt / self.camera.zoom;
        if ctx.input.is_down(KeyCode::KeyW) { self.camera.center.1 -= pan; }
        if ctx.input.is_down(KeyCode::KeyS) { self.camera.center.1 += pan; }
        if ctx.input.is_down(KeyCode::KeyA) { self.camera.center.0 -= pan; }
        if ctx.input.is_down(KeyCode::KeyD) { self.camera.center.0 += pan; }
        self.camera.zoom = (self.camera.zoom * (1.0 + ctx.input.scroll() * 0.1)).clamp(0.25, 4.0);

        // --- selection (render-side, not a sim command) ---
        if ctx.input.mouse_pressed(MouseButton::Left) {
            self.drag_from = Some(ctx.input.mouse());
        }
        if ctx.input.mouse_released(MouseButton::Left) {
            if let Some(from) = self.drag_from.take() {
                let to = ctx.input.mouse();
                let items = self.unit_screen_items(viewport);
                let dragged = (to.0 - from.0).abs() + (to.1 - from.1).abs() > 6.0;
                self.selected = if dragged {
                    drag_box(Rect::from_corners(from, to), items)
                } else {
                    pick(to, items).into_iter().collect()
                };
            }
        }
        // --- orders go through the command spine ---
        if ctx.input.mouse_pressed(MouseButton::Right) && !self.selected.is_empty() {
            let wp = self.camera.screen_to_world(ctx.input.mouse(), viewport);
            let tile = ISO.screen_to_tile(wp.0, wp.1);
            let on_map = (0..MAP).contains(&tile.0) && (0..MAP).contains(&tile.1);
            if on_map && !self.sim.curr().grid.is_blocked(tile) {
                self.seq += 1;
                self.sim.enqueue(Envelope {
                    tick: self.sim.tick(),
                    player: 0,
                    seq: self.seq,
                    cmd: Cmd::Move { ids: self.selected.clone(), to: tile },
                });
            }
        }

        for _ in 0..self.timer.advance(ctx.dt) {
            self.sim.step();
        }

        // --- render ---
        let atlas = self.atlas.as_ref().unwrap();
        self.batch.clear();
        for ty in 0..MAP {
            for tx in 0..MAP {
                let name = if self.sim.curr().grid.is_blocked((tx, ty)) { "block" } else { "grass" };
                let (sx, sy) = ISO.world_to_screen(tx as f32 + 0.5, ty as f32 + 0.5);
                self.batch.push(atlas.region(name), (sx, sy + 32.0), 0);
            }
        }
        let alpha = self.timer.alpha();
        for (pu, cu) in self.sim.prev().units.iter().zip(self.sim.curr().units.iter()) {
            let lerp = |a: Fixed, b: Fixed| a.to_f32() + (b.to_f32() - a.to_f32()) * alpha;
            let (wx, wy) = (lerp(pu.mover.pos.x, cu.mover.pos.x), lerp(pu.mover.pos.y, cu.mover.pos.y));
            let (sx, sy) = ISO.world_to_screen(wx, wy);
            if self.selected.contains(&cu.id) {
                self.batch.push(atlas.region("ring"), (sx, sy + 8.0), 1); // decal layer
            }
            self.batch.push(atlas.region("unit"), (sx, sy), 2); // units above decals
        }

        let Some(mut frame) = ctx.gfx.frame() else { return };
        frame.clear([0.08, 0.08, 0.10, 1.0]);
        frame.draw_sprites(&self.batch, atlas, &self.camera);
        frame.finish();
    }
}

fn main() -> anyhow::Result<()> {
    run(AppConfig { title: "umbra — units".into(), ..Default::default() }, Demo::default())
}
