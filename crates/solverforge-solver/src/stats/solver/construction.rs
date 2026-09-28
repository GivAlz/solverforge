//! Construction versus search split of the aggregate evaluation counters.

use std::time::Duration;

use super::SolverStats;
use crate::stats::Throughput;

impl SolverStats {
    /// Marks one evaluation already recorded in `moves_evaluated` as
    /// construction-heuristic work.
    pub(crate) fn record_construction_move_evaluated(&mut self) {
        self.construction_moves_evaluated += 1;
    }

    /// Adds the solve-clock time spent inside one construction phase.
    pub(crate) fn record_construction_time(&mut self, duration: Duration) {
        self.construction_time += duration;
    }

    /// Solve-clock time spent inside completed construction phases.
    pub fn construction_time(&self) -> Duration {
        self.construction_time
    }

    /// Moves evaluated outside construction-heuristic phases, i.e.
    /// `moves_evaluated` minus `construction_moves_evaluated`.
    pub fn search_moves_evaluated(&self) -> u64 {
        self.moves_evaluated
            .saturating_sub(self.construction_moves_evaluated)
    }

    /// Search moves over the solve-clock time spent outside construction
    /// phases. This is the basis of the headline `moves/s` rate.
    pub fn search_evaluated_throughput(&self) -> Throughput {
        Throughput {
            count: self.search_moves_evaluated(),
            elapsed: self.elapsed().saturating_sub(self.construction_time),
        }
    }
}
