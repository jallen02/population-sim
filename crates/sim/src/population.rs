//! Age-structured population: one cohort per year of age, driven by birth and death rules.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

/// Extra yearly death rate for people who went unfed, scaled by the unfed share.
/// At 1.0, everyone the food couldn't cover dies.
pub const STARVATION_DEATH_RATE: f64 = 1.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Population {
    /// Head counts, indexed by age in years.
    counts: VecDeque<u64>,
    /// Fractional deaths not yet applied, parallel to `counts`.
    death_remainders: VecDeque<f64>,
    /// Fractional births not yet added to age 0.
    birth_remainder: f64,
    rate_rules: Vec<RateRule>,
}

/// Birth and/or death rates for ages in `start_age..end_age` (`end_age` exclusive, `None` = no limit).
/// Rates are per person per year; death rates are clamped to `0..=1` and birth rates to `>= 0`.
/// Where rules overlap, their rates are summed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateRule {
    start_age: u32,
    end_age: Option<u32>,
    birth_rate: Option<Rate>,
    death_rate: Option<Rate>,
    name: String,
}

/// A yearly rate as a function of `x`, the years since the rule's `start_age`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Rate {
    /// `value`
    Constant { value: f64 },
    /// `value + per_year * x`
    Linear { value: f64, per_year: f64 },
    /// `value * e^(growth * x)`
    Exponential { value: f64, growth: f64 },
}

impl Rate {
    fn at(&self, x: f64) -> f64 {
        match *self {
            Rate::Constant { value } => value,
            Rate::Linear { value, per_year } => value + per_year * x,
            Rate::Exponential { value, growth } => value * (growth * x).exp(),
        }
    }
}

impl RateRule {
    /// Panics if `end_age` is not greater than `start_age`.
    pub fn new(
        start_age: u32,
        end_age: Option<u32>,
        birth_rate: Option<Rate>,
        death_rate: Option<Rate>,
        name: impl Into<String>,
    ) -> Self {
        assert!(
            end_age.is_none_or(|end| start_age < end),
            "rate rule age range is empty: {start_age}..{end_age:?}"
        );
        RateRule {
            start_age,
            end_age,
            birth_rate,
            death_rate,
            name: name.into(),
        }
    }

    pub fn births(
        start_age: u32,
        end_age: Option<u32>,
        rate: Rate,
        name: impl Into<String>,
    ) -> Self {
        Self::new(start_age, end_age, Some(rate), None, name)
    }

    pub fn deaths(
        start_age: u32,
        end_age: Option<u32>,
        rate: Rate,
        name: impl Into<String>,
    ) -> Self {
        Self::new(start_age, end_age, None, Some(rate), name)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    fn covers(&self, age: u32) -> bool {
        age >= self.start_age && self.end_age.is_none_or(|end| age < end)
    }
}

impl Default for Population {
    fn default() -> Self {
        let mut counts = vec![0; 21];
        counts[20] = 100;
        Self::new(
            counts,
            vec![
                RateRule::births(18, Some(45), Rate::Constant { value: 0.1 }, "Fertility"),
                RateRule::deaths(
                    0,
                    None,
                    Rate::Exponential {
                        value: 0.0005,
                        growth: 0.085,
                    },
                    "Aging",
                ),
            ],
        )
    }
}

impl Population {
    pub fn new(counts: Vec<u64>, rate_rules: Vec<RateRule>) -> Self {
        let mut counts = VecDeque::from(counts);
        if counts.is_empty() {
            counts.push_back(0);
        }
        Self {
            death_remainders: VecDeque::from(vec![0.0; counts.len()]),
            counts,
            birth_remainder: 0.0,
            rate_rules,
        }
    }

    /// Head counts indexed by age in years.
    pub fn counts(&self) -> impl Iterator<Item = u64> + '_ {
        self.counts.iter().copied()
    }

    pub fn total(&self) -> u64 {
        self.counts
            .iter()
            .fold(0u64, |sum, &count| sum.saturating_add(count))
    }

    /// Applies the year's deaths, then births from the survivors, then ages everyone by one year
    /// and adds the newborns at age 0.
    ///
    /// `fed` is the share of the population that had food this year (`0..=1`). The unfed share
    /// adds starvation deaths and scales births down.
    pub fn end_year(&mut self, fed: f64) {
        let fed = fed.clamp(0.0, 1.0);
        let starvation = (1.0 - fed) * STARVATION_DEATH_RATE;
        let mut births = self.birth_remainder;
        for age in 0..self.counts.len() {
            let count = self.counts[age];
            if count == 0 {
                continue;
            }
            let age = age as u32;
            let death_rate = self
                .yearly_rate(age, |rule| rule.death_rate.as_ref())
                .max(0.0);
            let death_rate = (death_rate + starvation).min(1.0);
            let deaths = count as f64 * death_rate + self.death_remainders[age as usize];
            let whole = deaths.floor();
            self.death_remainders[age as usize] = deaths - whole;
            let count = count - (whole as u64).min(count);
            self.counts[age as usize] = count;

            let birth_rate = self
                .yearly_rate(age, |rule| rule.birth_rate.as_ref())
                .max(0.0);
            births += count as f64 * birth_rate * fed;
        }
        let whole = births.floor();
        self.birth_remainder = births - whole;

        self.age_one_year();
        self.counts[0] = whole as u64;
    }

    fn yearly_rate(&self, age: u32, rate: impl Fn(&RateRule) -> Option<&Rate>) -> f64 {
        self.rate_rules
            .iter()
            .filter(|rule| rule.covers(age))
            .filter_map(|rule| rate(rule).map(|r| r.at(f64::from(age - rule.start_age))))
            .sum()
    }

    fn age_one_year(&mut self) {
        self.counts.push_front(0);
        self.death_remainders.push_front(0.0);
        while self.counts.len() > 1 && self.counts.back() == Some(&0) {
            self.counts.pop_back();
            self.death_remainders.pop_back();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_years(population: &mut Population, years: u32) {
        for _ in 0..years {
            population.end_year(1.0);
        }
    }

    #[test]
    fn aging_shifts_cohorts() {
        let mut population = Population::new(vec![5], vec![]);
        run_years(&mut population, 2);
        assert_eq!(population.counts, [0, 0, 5]);
    }

    #[test]
    fn starvation_kills_the_unfed_share_and_slows_births() {
        let rule = RateRule::births(0, None, Rate::Constant { value: 1.0 }, "Births");
        let mut population = Population::new(vec![10], vec![rule]);
        population.end_year(0.5);
        // Half of 10 starve; the 5 survivors have 5 * 1.0 * 0.5 babies.
        assert_eq!(population.counts, [2, 5]);
    }

    #[test]
    fn births_accrue_for_small_cohorts() {
        // 10 people at 0.05/year is 0.5 births per year: truncation alone would give none.
        let rule = RateRule::births(0, None, Rate::Constant { value: 0.05 }, "Births");
        let mut population = Population::new(vec![10], vec![rule]);
        run_years(&mut population, 10);
        assert!(population.total() > 10);
    }

    #[test]
    fn death_rate_above_one_does_not_underflow() {
        let rule = RateRule::deaths(0, None, Rate::Constant { value: 5.0 }, "Plague");
        let mut population = Population::new(vec![7, 3], vec![rule]);
        run_years(&mut population, 2);
        assert_eq!(population.total(), 0);
    }

    #[test]
    fn negative_rates_are_ignored() {
        let rule = RateRule::new(
            0,
            None,
            Some(Rate::Constant { value: -1.0 }),
            Some(Rate::Constant { value: -1.0 }),
            "Bad",
        );
        let mut population = Population::new(vec![10], vec![rule]);
        run_years(&mut population, 1);
        assert_eq!(population.total(), 10);
    }

    #[test]
    fn end_age_is_exclusive() {
        let rule = RateRule::deaths(0, Some(1), Rate::Constant { value: 1.0 }, "Infant");
        let mut population = Population::new(vec![0, 4], vec![rule]);
        run_years(&mut population, 1);
        assert_eq!(population.total(), 4);
    }

    #[test]
    fn overlapping_rules_sum() {
        let half = || RateRule::deaths(0, None, Rate::Constant { value: 0.5 }, "Half");
        let mut population = Population::new(vec![10], vec![half(), half()]);
        run_years(&mut population, 1);
        assert_eq!(population.total(), 0);
    }

    #[test]
    #[should_panic(expected = "age range is empty")]
    fn empty_age_range_panics() {
        RateRule::births(5, Some(5), Rate::Constant { value: 0.1 }, "Empty");
    }

    #[test]
    fn total_saturates() {
        let population = Population::new(vec![u64::MAX, 1], vec![]);
        assert_eq!(population.total(), u64::MAX);
    }
}
