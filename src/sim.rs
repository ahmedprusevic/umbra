//! Fixed-tick deterministic simulation driver.

/// A command stamped for a specific tick and ordered canonically by (player, seq).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Envelope<C> {
    pub tick: u64,
    pub player: u8,
    pub seq: u32,
    pub cmd: C,
}

/// The game state the sim drives. Clone is required for prev/curr interpolation.
pub trait Game: Clone {
    type Cmd;
    /// Execute one command against the state.
    fn apply(&mut self, env: &Envelope<Self::Cmd>);
    /// Advance the simulation one tick.
    fn tick(&mut self);
}

/// Deterministic fixed-tick driver over a [`Game`].
pub struct Sim<G: Game> {
    prev: G,
    curr: G,
    tick: u64,
    queue: Vec<Envelope<G::Cmd>>,
}

impl<G: Game> Sim<G> {
    /// Start at tick 0 with `prev == curr == initial`.
    pub fn new(initial: G) -> Self {
        Sim {
            prev: initial.clone(),
            curr: initial,
            tick: 0,
            queue: Vec::new(),
        }
    }

    /// Queue a command. Its tick must be at or after the current tick.
    pub fn enqueue(&mut self, env: Envelope<G::Cmd>) {
        debug_assert!(
            env.tick >= self.tick,
            "command for past tick {} enqueued at tick {}",
            env.tick,
            self.tick
        );
        self.queue.push(env);
    }

    /// Advance one tick: snapshot prev, apply this tick's commands in canonical
    /// (player, seq) order, then tick the game.
    pub fn step(&mut self) {
        self.prev = self.curr.clone();
        let tick = self.tick;
        let (mut ready, rest): (Vec<_>, Vec<_>) =
            self.queue.drain(..).partition(|e| e.tick == tick);
        self.queue = rest;
        ready.sort_by_key(|e| (e.player, e.seq));
        for e in &ready {
            self.curr.apply(e);
        }
        self.curr.tick();
        self.tick += 1;
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// State before the last `step`, for render interpolation.
    pub fn prev(&self) -> &G {
        &self.prev
    }

    /// State after the last `step`.
    pub fn curr(&self) -> &G {
        &self.curr
    }
}

/// Render-side frame pacing. Turns real wall-clock `dt` into a count of sim
/// ticks to run, and exposes interpolation `alpha`. Float math here never
/// touches simulation state, so it cannot affect determinism.
pub struct TickTimer {
    step: f32,
    acc: f32,
}

impl TickTimer {
    /// Spiral-of-death guard: a single frame never runs more than this many
    /// ticks, so a long stall (breakpoint, GC pause) can't snowball into an
    /// ever-growing backlog that starves rendering.
    const MAX_TICKS: u32 = 8;

    pub fn new(tick_hz: f32) -> Self {
        TickTimer {
            step: 1.0 / tick_hz,
            acc: 0.0,
        }
    }

    /// Accumulate `dt` seconds and return how many ticks to run now (capped at
    /// `MAX_TICKS`).
    pub fn advance(&mut self, dt: f32) -> u32 {
        self.acc += dt;
        let mut n = 0;
        while self.acc >= self.step && n < Self::MAX_TICKS {
            self.acc -= self.step;
            n += 1;
        }
        // Hit the cap with backlog remaining: drop it so alpha stays in [0,1)
        // and we don't accumulate unbounded lag.
        if n == Self::MAX_TICKS {
            self.acc %= self.step;
        }
        n
    }

    /// Progress into the current tick, in [0, 1), for interpolation.
    pub fn alpha(&self) -> f32 {
        (self.acc / self.step).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Toy game: an order-sensitive accumulator. `Mix` folds n into acc with a
    // multiply, so applying commands in a different order changes the result —
    // exactly what the canonical-ordering guarantee must defend against.
    #[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
    struct Counter {
        acc: i64,
    }

    #[derive(Clone, Copy, Debug)]
    enum Op {
        Mix(i64),
    }

    impl Game for Counter {
        type Cmd = Op;
        fn apply(&mut self, env: &Envelope<Op>) {
            let Op::Mix(n) = env.cmd;
            self.acc = self.acc.wrapping_mul(31).wrapping_add(n);
        }
        fn tick(&mut self) {
            self.acc = self.acc.wrapping_add(1);
        }
    }

    fn env(tick: u64, player: u8, seq: u32, n: i64) -> Envelope<Op> {
        Envelope { tick, player, seq, cmd: Op::Mix(n) }
    }

    #[test]
    fn new_sim_starts_at_tick_zero_prev_eq_curr() {
        let sim = Sim::new(Counter { acc: 7 });
        assert_eq!(sim.tick(), 0);
        assert_eq!(sim.prev(), sim.curr());
        assert_eq!(sim.curr().acc, 7);
    }

    #[test]
    fn empty_step_advances_one_tick() {
        let mut sim = Sim::new(Counter::default());
        sim.step();
        assert_eq!(sim.tick(), 1);
        // prev is the pre-step state, curr the post-step state: differ by one tick().
        assert_eq!(sim.prev().acc, 0);
        assert_eq!(sim.curr().acc, 1);
    }

    #[test]
    fn insertion_order_does_not_change_result() {
        // Same three commands for tick 0, enqueued in two different orders.
        let a = env(0, 0, 0, 3);
        let b = env(0, 1, 0, 5);
        let c = env(0, 1, 1, 9);

        let mut s1 = Sim::new(Counter::default());
        s1.enqueue(a.clone());
        s1.enqueue(b.clone());
        s1.enqueue(c.clone());
        s1.step();

        let mut s2 = Sim::new(Counter::default());
        s2.enqueue(c);
        s2.enqueue(a);
        s2.enqueue(b);
        s2.step();

        assert_eq!(s1.curr(), s2.curr());
        // And it is genuinely order-sensitive: a straight non-canonical apply differs.
        let mut wrong = Counter::default();
        for e in [env(0, 1, 1, 9), env(0, 0, 0, 3), env(0, 1, 0, 5)] {
            wrong.apply(&e);
        }
        assert_ne!(wrong.acc, s1.curr().acc);
    }

    #[test]
    fn future_commands_do_not_run_early() {
        let mut sim = Sim::new(Counter::default());
        sim.enqueue(env(5, 0, 0, 100)); // stamped for tick 5
        sim.step(); // runs tick 0
        // Only tick() ran; the tick-5 command has not been applied.
        assert_eq!(sim.curr().acc, 1);
        for _ in 0..4 {
            sim.step();
        }
        // Now at tick 5: acc == 5 (five tick()s), command not yet consumed.
        assert_eq!(sim.tick(), 5);
        let before = sim.curr().acc;
        sim.step(); // this is the tick-5 step: command applies, then tick()
        assert_eq!(sim.curr().acc, before.wrapping_mul(31).wrapping_add(100).wrapping_add(1));
    }

    #[test]
    fn timer_accumulates_fractional_ticks() {
        // 20 Hz => 0.05 s per tick.
        let mut t = TickTimer::new(20.0);
        assert_eq!(t.advance(0.03), 0); // not enough yet
        assert!((t.alpha() - 0.6).abs() < 1e-4); // 0.03 / 0.05
        assert_eq!(t.advance(0.03), 1); // 0.06 total -> one tick, 0.01 left
        assert!((t.alpha() - 0.2).abs() < 1e-4); // 0.01 / 0.05
        assert!(t.alpha() >= 0.0 && t.alpha() < 1.0);
    }

    #[test]
    fn timer_runs_multiple_ticks_in_one_advance() {
        let mut t = TickTimer::new(20.0);
        assert_eq!(t.advance(0.16), 3); // 0.16 / 0.05 = 3 ticks + 0.01
        assert!((t.alpha() - 0.2).abs() < 1e-4);
    }

    #[test]
    fn timer_caps_at_eight_ticks() {
        let mut t = TickTimer::new(20.0);
        // A one-second stall would be 20 ticks; the guard caps it at 8.
        assert_eq!(t.advance(1.0), 8);
        // Backlog beyond the cap is discarded, so alpha stays in [0,1).
        assert!(t.alpha() >= 0.0 && t.alpha() < 1.0);
    }

    // Deterministic pseudo-random command script (no rng dependency): an LCG
    // drives command counts, players, and payloads over N ticks.
    fn script(ticks: u64) -> Vec<Envelope<Op>> {
        let mut v = Vec::new();
        let mut s: u64 = 0x1234_5678_9abc_def0;
        let mut next = || {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            s
        };
        for tick in 0..ticks {
            let count = (next() >> 33) % 3; // 0..=2 commands this tick
            for seq in 0..count as u32 {
                let r = next();
                let player = (r % 4) as u8;
                let n = (r >> 8) as i64;
                v.push(Envelope { tick, player, seq, cmd: Op::Mix(n) });
            }
        }
        v
    }

    fn run(script: &[Envelope<Op>], ticks: u64) -> u64 {
        let mut sim = Sim::new(Counter::default());
        for e in script {
            sim.enqueue(e.clone());
        }
        for _ in 0..ticks {
            sim.step();
        }
        // DefaultHasher is zero-keyed SipHash: deterministic within a build.
        let mut h = DefaultHasher::new();
        sim.curr().hash(&mut h);
        h.finish()
    }

    #[test]
    fn replay_is_deterministic() {
        let s = script(100);
        let mut rev = s.clone();
        rev.reverse(); // same commands, worst-case different insertion order
        let a = run(&s, 100);
        let b = run(&rev, 100);
        assert_eq!(a, b, "hash must not depend on enqueue order");
        assert_eq!(a, run(&s, 100), "same script + same state must hash equal");
    }
}
