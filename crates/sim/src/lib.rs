//! Pure game simulation: no I/O, no browser APIs, deterministic.
//!
//! Front ends (the Leptos app, the CLI) drive it two ways:
//! - `advance(elapsed_ms)` turns wall-clock time into whole fixed-length ticks.
//! - `snapshot()` returns what to render.
//!
//! `save()` / `load()` round-trip the full state as JSON.

mod food;
mod population;

pub use food::{FOOD_PER_PERSON, FOOD_STORAGE, Food};
pub use population::{Population, Rate, RateRule, STARVATION_DEATH_RATE};

use serde::{Deserialize, Serialize};

/// Length of one simulation tick.
pub const TICK_MS: f64 = 100.0;
/// Ticks per in-game year. With 100 ms ticks, one year passes per second.
/// Food is produced every tick; eating, births, deaths and aging happen when a year ends.
pub const TICKS_PER_YEAR: u64 = 10;
/// Food produced per year. Fixed for now; later it will come from people working at buildings.
pub const BASE_FOOD_PRODUCTION: f64 = 1000.0;

/// Everything that persists in a save. Replace the example fields with your game's state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct State {
    pub tick: u64,
    #[serde(default)]
    pub population: Population,
    #[serde(default)]
    pub food: Food,
}

/// What the front end renders. Keep it flat and derived from `State`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub tick: u64,
    pub population: u64,
    pub food: f64,
    /// Food produced per year, spread evenly across its ticks.
    pub food_production: f64,
    /// Food the current population eats per year.
    pub food_consumption: f64,
    /// Head counts indexed by age in years.
    pub ages: Vec<u64>,
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
        Ok(Self {
            state: serde_json::from_str(json)?,
            accumulator_ms: 0.0,
        })
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
        let production = self.food_production() / TICKS_PER_YEAR as f64;
        self.state.food.produce(production);
        self.state.tick += 1;
        if self.state.tick.is_multiple_of(TICKS_PER_YEAR) {
            self.end_year();
        }
    }

    /// Feeds the population from the stored food, then applies births, deaths and aging.
    fn end_year(&mut self) {
        let fed = self.state.food.end_year(self.state.population.total());
        self.state.population.end_year(fed);
    }

    fn food_production(&self) -> f64 {
        BASE_FOOD_PRODUCTION
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            tick: self.state.tick,
            population: self.state.population.total(),
            food: self.state.food.stock(),
            food_production: self.food_production(),
            food_consumption: self.state.population.total() as f64 * FOOD_PER_PERSON,
            ages: self.state.population.counts().collect(),
        }
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
    fn nothing_changes_until_the_year_ends() {
        let mut game = Game::new();
        let start = game.snapshot();
        for _ in 0..TICKS_PER_YEAR - 1 {
            game.tick();
        }
        assert_eq!(game.snapshot().ages, start.ages);
        game.tick();
        assert_ne!(game.snapshot().ages, start.ages);
    }

    #[test]
    fn food_grows_every_tick() {
        let mut game = Game::new();
        game.tick();
        let per_tick = BASE_FOOD_PRODUCTION / TICKS_PER_YEAR as f64;
        assert_eq!(game.snapshot().food, per_tick);
        game.tick();
        assert_eq!(game.snapshot().food, per_tick * 2.0);
    }

    #[test]
    fn food_caps_the_population() {
        let mut game = Game::new();
        for _ in 0..500 * TICKS_PER_YEAR {
            game.tick();
        }
        let cap = (BASE_FOOD_PRODUCTION + FOOD_STORAGE) / FOOD_PER_PERSON;
        assert!(game.snapshot().population as f64 <= cap * 1.1);
    }

    #[test]
    fn population_round_trips_and_loads_old_saves() {
        let mut game = Game::new();
        game.advance(TICK_MS * TICKS_PER_YEAR as f64 * 5.0);
        let loaded = Game::load(&game.save()).unwrap();
        assert_eq!(loaded.snapshot().population, game.snapshot().population);

        let old = Game::load(r#"{"tick":0,"points":0.0,"generators":0}"#).unwrap();
        assert_eq!(old.snapshot().population, Game::new().snapshot().population);
    }
}
