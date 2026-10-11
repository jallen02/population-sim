//! Pure game simulation: no I/O, no browser APIs, deterministic.
//!
//! Front ends (the Leptos app, the CLI) drive it three ways:
//! - `advance(elapsed_ms)` turns wall-clock time into whole fixed-length ticks.
//! - `apply(command)` performs a player action.
//! - `snapshot()` returns what to render.
//!
//! `save()` / `load()` round-trip the full state as JSON.

use serde::{Deserialize, Serialize};

/// Length of one simulation tick.
pub const TICK_MS: f64 = 100.0;

/// Everything that persists in a save. Replace the example fields with your game's state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct State {
    pub tick: u64,
    pub points: f64,
    pub generators: u32,
}

/// A player action. Front ends send these as JSON, e.g. `{"type":"Click"}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Command {
    Click,
    BuyGenerator,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CommandError {
    CannotAfford { cost: f64 },
}

/// What the front end renders. Keep it flat and derived from `State`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub tick: u64,
    pub points: f64,
    pub points_per_sec: f64,
    pub generators: u32,
    pub generator_cost: f64,
}

#[derive(Debug, Clone, Default)]
pub struct Game {
    pub state: State,
    /// Leftover time shorter than one tick, carried to the next `advance`.
    accumulator_ms: f64,
}

impl Game {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(json: &str) -> serde_json::Result<Self> {
        Ok(Self { state: serde_json::from_str(json)?, accumulator_ms: 0.0 })
    }

    pub fn save(&self) -> String {
        serde_json::to_string(&self.state).expect("state serializes")
    }

    /// Runs as many whole ticks as fit in the elapsed time plus leftover. Returns ticks run.
    pub fn advance(&mut self, elapsed_ms: f64) -> u64 {
        self.accumulator_ms += elapsed_ms.max(0.0);
        let ticks = (self.accumulator_ms / TICK_MS).floor();
        self.accumulator_ms -= ticks * TICK_MS;
        let ticks = ticks as u64;
        for _ in 0..ticks {
            self.tick();
        }
        ticks
    }

    /// One fixed step of game rules.
    pub fn tick(&mut self) {
        self.state.points += self.points_per_sec() * TICK_MS / 1000.0;
        self.state.tick += 1;
    }

    pub fn apply(&mut self, command: Command) -> Result<(), CommandError> {
        match command {
            Command::Click => self.state.points += 1.0,
            Command::BuyGenerator => {
                let cost = self.generator_cost();
                if self.state.points < cost {
                    return Err(CommandError::CannotAfford { cost });
                }
                self.state.points -= cost;
                self.state.generators += 1;
            }
        }
        Ok(())
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            tick: self.state.tick,
            points: self.state.points,
            points_per_sec: self.points_per_sec(),
            generators: self.state.generators,
            generator_cost: self.generator_cost(),
        }
    }

    fn points_per_sec(&self) -> f64 {
        self.state.generators as f64
    }

    fn generator_cost(&self) -> f64 {
        10.0 * 1.15f64.powi(self.state.generators as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_carries_partial_ticks() {
        let mut game = Game::new();
        assert_eq!(game.advance(TICK_MS * 1.5), 1);
        assert_eq!(game.advance(TICK_MS * 0.5), 1);
        assert_eq!(game.state.tick, 2);
    }

    #[test]
    fn generators_produce_over_time() {
        let mut game = Game::new();
        game.state.points = 10.0;
        game.apply(Command::BuyGenerator).unwrap();
        game.advance(1000.0);
        assert!((game.state.points - 1.0).abs() < 1e-9);
    }

    #[test]
    fn cannot_buy_without_points() {
        let mut game = Game::new();
        assert!(matches!(game.apply(Command::BuyGenerator), Err(CommandError::CannotAfford { .. })));
    }

    #[test]
    fn save_round_trips() {
        let mut game = Game::new();
        game.apply(Command::Click).unwrap();
        let loaded = Game::load(&game.save()).unwrap();
        assert_eq!(loaded.state.points, 1.0);
    }

    #[test]
    fn commands_parse_from_json() {
        let command: Command = serde_json::from_str(r#"{"type":"BuyGenerator"}"#).unwrap();
        assert!(matches!(command, Command::BuyGenerator));
    }
}
