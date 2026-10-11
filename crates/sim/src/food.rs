//! Food: produced every tick, eaten by the population at the end of each year, with leftovers
//! stored up to a cap.

use serde::{Deserialize, Serialize};

/// Food each person eats per year.
pub const FOOD_PER_PERSON: f64 = 1.0;
/// Most food that carries over from one year to the next. Food can pile up past this during the
/// year; the cap applies after the population eats.
pub const FOOD_STORAGE: f64 = 500.0;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Food {
    stock: f64,
}

impl Food {
    pub fn stock(&self) -> f64 {
        self.stock
    }

    pub fn produce(&mut self, amount: f64) {
        self.stock += amount.max(0.0);
    }

    /// Feeds `population` people from the stock and keeps what's left up to `FOOD_STORAGE`.
    /// Returns the share of the population that was fed (`0..=1`).
    pub fn end_year(&mut self, population: u64) -> f64 {
        let need = population as f64 * FOOD_PER_PERSON;
        let eaten = need.min(self.stock);
        self.stock = (self.stock - eaten).min(FOOD_STORAGE);
        if need > 0.0 { eaten / need } else { 1.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surplus_is_stored_up_to_the_cap() {
        let mut food = Food::default();
        food.produce(100.0);
        assert_eq!(food.end_year(40), 1.0);
        assert_eq!(food.stock(), 60.0);
        food.produce(10_000.0);
        food.end_year(0);
        assert_eq!(food.stock(), FOOD_STORAGE);
    }

    #[test]
    fn shortage_feeds_a_share_and_empties_the_stock() {
        let mut food = Food::default();
        food.produce(30.0);
        assert_eq!(food.end_year(120), 0.25);
        assert_eq!(food.stock(), 0.0);
    }
}
