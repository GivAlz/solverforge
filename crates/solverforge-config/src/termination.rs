use std::time::Duration;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

// Termination configuration.
//
// Time limits are `f64` quantities so configurations can express sub-second
// and fractional budgets (`seconds_spent_limit = 1.5`). Whole-number values
// written as integers (`30`) or floats (`30.0`) are both accepted. Loading a
// configuration rejects negative, NaN, infinite, and `Duration`-overflowing
// time limits.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct TerminationConfig {
    // Maximum seconds to spend solving (fractional values allowed).
    #[serde(default, deserialize_with = "deserialize_seconds_spent_limit")]
    pub seconds_spent_limit: Option<f64>,

    // Maximum minutes to spend solving (fractional values allowed).
    #[serde(default, deserialize_with = "deserialize_minutes_spent_limit")]
    pub minutes_spent_limit: Option<f64>,

    // Target best score to achieve (as string, e.g., "0hard/0soft").
    pub best_score_limit: Option<String>,

    // Maximum number of steps.
    pub step_count_limit: Option<u64>,

    // Maximum unimproved steps before terminating.
    pub unimproved_step_count_limit: Option<u64>,

    // Maximum seconds without improvement (fractional values allowed).
    #[serde(
        default,
        deserialize_with = "deserialize_unimproved_seconds_spent_limit"
    )]
    pub unimproved_seconds_spent_limit: Option<f64>,
}

impl TerminationConfig {
    /// Returns the combined `seconds_spent_limit` + `minutes_spent_limit`
    /// budget, or `None` when neither is set or the total is zero.
    ///
    /// # Panics
    ///
    /// Panics if a limit assigned in code is negative, NaN, infinite, or too
    /// large for [`Duration`]. Parsed configurations are validated at load
    /// time and never panic here.
    pub fn time_limit(&self) -> Option<Duration> {
        let seconds = self
            .seconds_spent_limit
            .map_or(Duration::ZERO, |value| SECONDS_SPENT.duration(value));
        let minutes = self
            .minutes_spent_limit
            .map_or(Duration::ZERO, |value| MINUTES_SPENT.duration(value));
        let total = seconds.saturating_add(minutes);
        (!total.is_zero()).then_some(total)
    }

    /// Returns the `unimproved_seconds_spent_limit` budget, if configured.
    ///
    /// # Panics
    ///
    /// Panics under the same conditions as [`TerminationConfig::time_limit`].
    pub fn unimproved_time_limit(&self) -> Option<Duration> {
        self.unimproved_seconds_spent_limit
            .map(|value| UNIMPROVED_SECONDS_SPENT.duration(value))
    }
}

struct TimeLimitField {
    name: &'static str,
    unit: &'static str,
    seconds_per_unit: f64,
}

const SECONDS_SPENT: TimeLimitField = TimeLimitField {
    name: "seconds_spent_limit",
    unit: "seconds",
    seconds_per_unit: 1.0,
};

const MINUTES_SPENT: TimeLimitField = TimeLimitField {
    name: "minutes_spent_limit",
    unit: "minutes",
    seconds_per_unit: 60.0,
};

const UNIMPROVED_SECONDS_SPENT: TimeLimitField = TimeLimitField {
    name: "unimproved_seconds_spent_limit",
    unit: "seconds",
    seconds_per_unit: 1.0,
};

impl TimeLimitField {
    fn try_duration(&self, value: f64) -> Result<Duration, String> {
        Duration::try_from_secs_f64(value * self.seconds_per_unit).map_err(|_| {
            format!(
                "{} must be a finite, non-negative number of {} that fits in a Duration, got {value}",
                self.name, self.unit
            )
        })
    }

    fn duration(&self, value: f64) -> Duration {
        self.try_duration(value)
            .unwrap_or_else(|message| panic!("invalid TerminationConfig: {message}"))
    }

    fn deserialize<'de, D>(&self, deserializer: D) -> Result<Option<f64>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Option::<f64>::deserialize(deserializer)?;
        if let Some(value) = value {
            self.try_duration(value).map_err(D::Error::custom)?;
        }
        Ok(value)
    }
}

fn deserialize_seconds_spent_limit<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    SECONDS_SPENT.deserialize(deserializer)
}

fn deserialize_minutes_spent_limit<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    MINUTES_SPENT.deserialize(deserializer)
}

fn deserialize_unimproved_seconds_spent_limit<'de, D>(
    deserializer: D,
) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    UNIMPROVED_SECONDS_SPENT.deserialize(deserializer)
}
