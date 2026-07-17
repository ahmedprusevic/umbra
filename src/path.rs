use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

use crate::fixed::{Fixed, FixedVec2};

pub type Tile = (i32, i32);

#[derive(Clone)]
pub struct Grid {
    w: i32,
    h: i32,
    blocked: Vec<bool>,
}

impl Grid {
    pub fn new(w: i32, h: i32) -> Grid {
        Grid { w, h, blocked: vec![false; (w.max(0) * h.max(0)) as usize] }
    }
    pub fn in_bounds(&self, t: Tile) -> bool {
        t.0 >= 0 && t.1 >= 0 && t.0 < self.w && t.1 < self.h
    }
    fn idx(&self, t: Tile) -> usize { (t.1 * self.w + t.0) as usize }
    pub fn set_blocked(&mut self, t: Tile, blocked: bool) {
        if self.in_bounds(t) { let i = self.idx(t); self.blocked[i] = blocked; }
    }
    pub fn is_blocked(&self, t: Tile) -> bool {
        !self.in_bounds(t) || self.blocked[self.idx(t)]
    }
}

// Fixed neighbor order: row-major scan of the 3x3 minus center.
const DIRS: [(i32, i32); 8] = [
    (-1, -1), (0, -1), (1, -1),
    (-1,  0),          (1,  0),
    (-1,  1), (0,  1), (1,  1),
];

// Octile heuristic: 10*max + 4*min of |dx|,|dy|. Admissible for 10/14 costs.
fn heuristic(a: Tile, b: Tile) -> i32 {
    let dx = (a.0 - b.0).abs();
    let dy = (a.1 - b.1).abs();
    10 * dx.max(dy) + 4 * dx.min(dy)
}

/// 8-directional A*. Costs: 10 straight, 14 diagonal. No corner cutting.
/// Path excludes `from`, includes `to`. `None` if unreachable or `to` blocked.
/// Deterministic: frontier ties break on (f, then h, then row-major index);
/// neighbors are scanned in `DIRS` order; `g` is updated only on a strict
/// improvement, so equal-cost routes resolve identically every run.
pub fn astar(
    grid: &Grid,
    from: Tile,
    to: Tile,
    extra_blocked: &dyn Fn(Tile) -> bool,
) -> Option<Vec<Tile>> {
    if !grid.in_bounds(from) { return None; } // guard before indexing with `from`
    if grid.is_blocked(to) || extra_blocked(to) { return None; }
    if from == to { return Some(Vec::new()); }

    let blocked = |t: Tile| grid.is_blocked(t) || extra_blocked(t);
    let w = grid.w;
    let n = (grid.w * grid.h) as usize;
    let ridx = |t: Tile| t.1 * w + t.0; // row-major, unique per in-bounds tile

    let mut g = vec![i32::MAX; n];
    let mut came = vec![-1i32; n]; // parent tile as row-major index, -1 = none
    // Reverse<(f, h, idx, tile)>: BinaryHeap pops the smallest tuple; `idx` is
    // unique so the ordering is total and stable.
    let mut heap: BinaryHeap<Reverse<(i32, i32, i32, Tile)>> = BinaryHeap::new();

    g[ridx(from) as usize] = 0;
    let h0 = heuristic(from, to);
    heap.push(Reverse((h0, h0, ridx(from), from)));

    while let Some(Reverse((_f, _h, _i, cur))) = heap.pop() {
        if cur == to {
            let mut path = Vec::new();
            let mut c = cur;
            while c != from {
                path.push(c);
                let p = came[ridx(c) as usize];
                c = (p % w, p / w);
            }
            path.reverse();
            return Some(path);
        }
        let cg = g[ridx(cur) as usize];
        for &(dx, dy) in DIRS.iter() {
            let nb = (cur.0 + dx, cur.1 + dy);
            if blocked(nb) { continue; }
            if dx != 0 && dy != 0
                && (blocked((cur.0 + dx, cur.1)) || blocked((cur.0, cur.1 + dy)))
            {
                continue; // corner cut: both orthogonals must be clear
            }
            let step = if dx != 0 && dy != 0 { 14 } else { 10 };
            let ng = cg + step;
            let ni = ridx(nb) as usize;
            if ng < g[ni] {
                g[ni] = ng;
                came[ni] = ridx(cur);
                let h = heuristic(nb, to);
                heap.push(Reverse((ng + h, h, ridx(nb), nb)));
            }
        }
    }
    None
}

/// Tile-ownership map. BTreeMap (not HashMap) so any future iteration is
/// deterministic. One tile is held by at most one id.
#[derive(Clone)]
pub struct Occupancy {
    map: BTreeMap<Tile, u32>,
}

impl Occupancy {
    pub fn new() -> Occupancy { Occupancy { map: BTreeMap::new() } }
    /// Claim `t` for `id`. False if another id holds it; re-claiming own -> true.
    pub fn claim(&mut self, t: Tile, id: u32) -> bool {
        match self.map.get(&t) {
            Some(&h) if h != id => false,
            _ => { self.map.insert(t, id); true }
        }
    }
    /// Release `t` — no-op unless `id` currently holds it.
    pub fn release(&mut self, t: Tile, id: u32) {
        if self.map.get(&t) == Some(&id) { self.map.remove(&t); }
    }
    pub fn holder(&self, t: Tile) -> Option<u32> { self.map.get(&t).copied() }
}

impl Default for Occupancy {
    fn default() -> Occupancy { Occupancy::new() }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MoveResult { Moving, Blocked, Arrived }

fn center(t: Tile) -> FixedVec2 {
    FixedVec2::new(
        Fixed::from_int(t.0) + Fixed(128),
        Fixed::from_int(t.1) + Fixed(128),
    )
}

#[derive(Clone)]
pub struct Mover {
    pub pos: FixedVec2,
    pub speed: Fixed,
    path: Vec<Tile>,
    idx: usize,           // index of the next waypoint in `path`
    held: Tile,           // the tile this mover occupies and owns right now
    blocked_ticks: u32,
}

impl Mover {
    pub fn new(pos: FixedVec2, speed: Fixed) -> Mover {
        let held = (pos.x.0 >> 8, pos.y.0 >> 8); // floor(pos) == current_tile
        Mover { pos, speed, path: Vec::new(), idx: 0, held, blocked_ticks: 0 }
    }
    pub fn set_path(&mut self, path: Vec<Tile>) {
        self.path = path;
        self.idx = 0;
        self.blocked_ticks = 0;
    }
    pub fn goal(&self) -> Option<Tile> { self.path.last().copied() }
    pub fn current_tile(&self) -> Tile { (self.pos.x.0 >> 8, self.pos.y.0 >> 8) }
    pub fn blocked_ticks(&self) -> u32 { self.blocked_ticks }

    /// Advance one tick. Reserves the next waypoint before moving into it; if
    /// that tile is held by another id, waits (Blocked). `held` tracks the tile
    /// containing us: the moment the move carries us into a new tile we take it
    /// and release `held`, so exactly one standing-tile claim survives — whether
    /// the crossing lands on the waypoint center (full-tile speeds) or partway
    /// across it (sub-tile speeds). All arithmetic is Fixed — deterministic.
    pub fn step(&mut self, id: u32, occ: &mut Occupancy) -> MoveResult {
        if self.idx >= self.path.len() { return MoveResult::Arrived; }
        let cur = self.current_tile();
        occ.claim(cur, id); // hold our own tile (idempotent)

        let target = self.path[self.idx];
        if target == cur {
            self.idx += 1;
            return if self.idx >= self.path.len() {
                MoveResult::Arrived
            } else {
                MoveResult::Moving
            };
        }
        if !occ.claim(target, id) {
            self.blocked_ticks += 1;
            return MoveResult::Blocked;
        }
        self.blocked_ticks = 0;

        let c = center(target);
        let dx = c.x - self.pos.x;
        let dy = c.y - self.pos.y;
        let diag = dx != Fixed::ZERO && dy != Fixed::ZERO;
        let s = if diag { self.speed.mul(Fixed::INV_SQRT2) } else { self.speed };
        let rx = dx.abs() <= s;
        let ry = dy.abs() <= s;
        self.pos.x = if rx { c.x } else if dx > Fixed::ZERO { self.pos.x + s } else { self.pos.x - s };
        self.pos.y = if ry { c.y } else if dy > Fixed::ZERO { self.pos.y + s } else { self.pos.y - s };

        // Crossed into a new tile? Take it, drop the one behind us. This runs on
        // every crossing, not just exact-center arrivals — releasing only in the
        // `rx && ry` branch (as before) leaked a claim on every tile a
        // sub-tile-per-tick mover passed through, deadlocking shared corridors.
        let now = self.current_tile();
        if now != self.held {
            occ.claim(now, id);
            occ.release(self.held, id);
            self.held = now;
        }

        if rx && ry {
            self.idx += 1;
            if self.idx >= self.path.len() { return MoveResult::Arrived; }
        }
        MoveResult::Moving
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_extra(_: Tile) -> bool { false }

    #[test]
    fn straight_line() {
        let g = Grid::new(5, 5);
        let path = astar(&g, (0, 0), (4, 0), &no_extra).unwrap();
        assert_eq!(path, vec![(1, 0), (2, 0), (3, 0), (4, 0)]);
    }

    #[test]
    fn wall_detour() {
        let mut g = Grid::new(5, 5);
        for y in 0..4 { g.set_blocked((2, y), true); } // wall open only at y=4
        let path = astar(&g, (0, 0), (4, 0), &no_extra).unwrap();
        assert_eq!(*path.last().unwrap(), (4, 0));
        assert!(!path.contains(&(0, 0)));               // excludes `from`
        for t in &path { assert!(!g.is_blocked(*t)); }  // never steps on the wall
    }

    #[test]
    fn unreachable_is_none() {
        let mut g = Grid::new(5, 5);
        for y in 0..5 { g.set_blocked((2, y), true); }   // full wall, no gap
        assert_eq!(astar(&g, (0, 0), (4, 0), &no_extra), None);
    }

    #[test]
    fn blocked_target_is_none() {
        let mut g = Grid::new(5, 5);
        g.set_blocked((4, 0), true);
        assert_eq!(astar(&g, (0, 0), (4, 0), &no_extra), None);
    }

    #[test]
    fn no_corner_cut() {
        // (1,1) is reachable diagonally ONLY by cutting the corner past the
        // two orthogonal walls. With corner-cutting forbidden it is unreachable.
        let mut g = Grid::new(3, 3);
        g.set_blocked((1, 0), true);
        g.set_blocked((0, 1), true);
        assert_eq!(astar(&g, (0, 0), (1, 1), &no_extra), None);
    }

    #[test]
    fn no_corner_cut_detours_with_one_wall() {
        // Only (1,0) blocked: the diagonal is still illegal (needs both
        // orthogonals clear), so the first step must be the orthogonal (0,1).
        let mut g = Grid::new(3, 3);
        g.set_blocked((1, 0), true);
        let path = astar(&g, (0, 0), (1, 1), &no_extra).unwrap();
        assert_ne!(path[0], (1, 1));
        assert_eq!(path[0], (0, 1));
    }

    #[test]
    fn extra_blocked_is_honored() {
        let g = Grid::new(3, 1);
        let closed = |t: Tile| t == (1, 0);
        assert_eq!(astar(&g, (0, 0), (2, 0), &closed), None);
    }

    #[test]
    fn deterministic_across_runs() {
        // Open 5x5: many equal-cost diagonal routes to the far corner.
        let g = Grid::new(5, 5);
        let a = astar(&g, (0, 0), (4, 4), &no_extra).unwrap();
        let b = astar(&g, (0, 0), (4, 4), &no_extra).unwrap();
        let c = astar(&g, (0, 0), (4, 4), &no_extra).unwrap();
        assert_eq!(a, b);
        assert_eq!(b, c);
        assert_eq!(*a.last().unwrap(), (4, 4));
    }

    #[test]
    fn occupancy_claim_conflict_release() {
        let mut occ = Occupancy::new();
        assert!(occ.claim((3, 4), 1));            // free -> ok
        assert_eq!(occ.holder((3, 4)), Some(1));
        assert!(occ.claim((3, 4), 1));            // own tile again -> ok
        assert!(!occ.claim((3, 4), 2));           // held by 1 -> conflict
        assert_eq!(occ.holder((3, 4)), Some(1));  // unchanged after conflict
        occ.release((3, 4), 2);                    // wrong id -> no-op
        assert_eq!(occ.holder((3, 4)), Some(1));
        occ.release((3, 4), 1);                    // holder releases
        assert_eq!(occ.holder((3, 4)), None);
        assert!(occ.claim((3, 4), 2));            // now free for 2
    }

    fn raw(v: FixedVec2) -> (i32, i32) { (v.x.0, v.y.0) }

    #[test]
    fn mover_basics() {
        let m = Mover::new(center((0, 0)), Fixed(64));
        assert_eq!(m.current_tile(), (0, 0));
        assert_eq!(m.goal(), None);
        let mut m = m;
        m.set_path(vec![(1, 0), (2, 0)]);
        assert_eq!(m.goal(), Some((2, 0)));
    }

    #[test]
    fn mover_empty_path_arrives() {
        let mut occ = Occupancy::new();
        let mut m = Mover::new(center((0, 0)), Fixed(64));
        assert_eq!(m.step(1, &mut occ), MoveResult::Arrived);
    }

    #[test]
    fn mover_reaches_goal_and_frees_tiles() {
        let mut occ = Occupancy::new();
        // speed = ONE = one tile per tick: each straight step reaches the next.
        let mut m = Mover::new(center((0, 0)), Fixed(256));
        m.set_path(vec![(1, 0), (2, 0)]);
        assert_eq!(m.step(7, &mut occ), MoveResult::Moving);  // -> (1,0)
        assert_eq!(m.current_tile(), (1, 0));
        assert_eq!(occ.holder((0, 0)), None);                 // left tile freed
        assert_eq!(occ.holder((1, 0)), Some(7));
        assert_eq!(m.step(7, &mut occ), MoveResult::Arrived); // -> (2,0)
        assert_eq!(m.current_tile(), (2, 0));
        assert_eq!(occ.holder((1, 0)), None);
        assert_eq!(occ.holder((2, 0)), Some(7));
    }

    #[test]
    fn corridor_one_waits_then_proceeds() {
        // 1-wide corridor. A holds (1,0) while transiting; B must pass through
        // (1,0) and is Blocked until A vacates it.
        let mut occ = Occupancy::new();
        let mut a = Mover::new(center((1, 0)), Fixed(64)); // 4 ticks per tile
        a.set_path(vec![(2, 0), (3, 0)]);
        let mut b = Mover::new(center((0, 0)), Fixed(64));
        b.set_path(vec![(1, 0), (2, 0)]);

        assert_eq!(a.step(1, &mut occ), MoveResult::Moving); // A reserves (1,0)+(2,0)
        assert_eq!(b.step(2, &mut occ), MoveResult::Blocked); // (1,0) held by A
        assert_eq!(b.blocked_ticks(), 1);
        assert_eq!(b.current_tile(), (0, 0));                 // B did not move

        // Advance A until it leaves (1,0).
        for _ in 0..4 { a.step(1, &mut occ); }
        assert_eq!(a.current_tile(), (2, 0));
        assert_eq!(occ.holder((1, 0)), None);

        // Now B can proceed.
        assert_eq!(b.step(2, &mut occ), MoveResult::Moving);
        assert_eq!(b.blocked_ticks(), 0);
        assert_eq!(occ.holder((1, 0)), Some(2));
    }

    #[test]
    fn mover_positions_reproducible() {
        // Straight + diagonal leg exercises the INV_SQRT2 per-axis scaling.
        let run = || {
            let mut occ = Occupancy::new();
            let mut m = Mover::new(center((0, 0)), Fixed(100));
            m.set_path(vec![(1, 1), (2, 1)]);
            for _ in 0..10 { m.step(1, &mut occ); }
            raw(m.pos)
        };
        assert_eq!(run(), run());
    }
}
