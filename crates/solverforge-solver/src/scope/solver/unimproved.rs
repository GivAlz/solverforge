/* Solver-wide unimproved window.

`UnimprovedStepCountTermination` and `UnimprovedTimeTermination` measure the
work done since the best solution last improved. The window restarts whenever
the best solution improves (or is replaced), and while construction work runs:
construction scores rarely improve the best score, and configured unimproved
limits never cut required construction short.
*/

#[derive(Debug, Clone, Copy, Default)]
struct UnimprovedWindow {
    step_count_limit: Option<u64>,
    time_limit: Option<Duration>,
    start_step_count: u64,
    start_elapsed: Duration,
}

impl UnimprovedWindow {
    fn reset(&mut self) {
        self.start_step_count = 0;
        self.start_elapsed = Duration::ZERO;
    }

    fn restart(&mut self, step_count: u64, elapsed: Duration) {
        // An improving step is observed before it completes, so a later
        // restart within the same step must not move the window backwards.
        self.start_step_count = self.start_step_count.max(step_count);
        self.start_elapsed = self.start_elapsed.max(elapsed);
    }

    fn has_limits(&self) -> bool {
        self.step_count_limit.is_some() || self.time_limit.is_some()
    }

    fn steps(&self, total_step_count: u64) -> u64 {
        total_step_count.saturating_sub(self.start_step_count)
    }

    fn time(&self, elapsed: Duration) -> Duration {
        elapsed.saturating_sub(self.start_elapsed)
    }

    fn limit_reached(&self, total_step_count: u64, elapsed: Duration) -> bool {
        self.step_count_limit
            .is_some_and(|limit| self.steps(total_step_count) >= limit)
            || self
                .time_limit
                .is_some_and(|limit| self.time(elapsed) >= limit)
    }
}

impl<'t, S: PlanningSolution, D: Director<S>, ProgressCb: ProgressCallback<S>>
    SolverScope<'t, S, D, ProgressCb>
{
    /// Steps completed since the best solution last improved.
    pub(crate) fn unimproved_step_count(&self) -> u64 {
        self.unimproved.steps(self.total_step_count)
    }

    /// Solver time spent since the best solution last improved.
    pub(crate) fn unimproved_time(&self) -> Duration {
        self.unimproved.time(self.elapsed().unwrap_or_default())
    }

    pub(crate) fn install_inphase_unimproved_step_count_limit(&mut self, limit: u64) {
        let limit = self
            .unimproved
            .step_count_limit
            .map_or(limit, |existing| existing.min(limit));
        self.unimproved.step_count_limit = Some(limit);
    }

    pub(crate) fn install_inphase_unimproved_time_limit(&mut self, limit: Duration) {
        let limit = self
            .unimproved
            .time_limit
            .map_or(limit, |existing| existing.min(limit));
        self.unimproved.time_limit = Some(limit);
    }

    fn restart_unimproved_window(&mut self, step_count: u64) {
        let elapsed = self.elapsed().unwrap_or_default();
        self.unimproved.restart(step_count, elapsed);
    }

    /// Construction work is not unimproved search: it restarts the window
    /// instead of being cut short by it.
    fn restart_unimproved_window_for_construction(&mut self) {
        if self.unimproved.has_limits() {
            self.restart_unimproved_window(self.total_step_count);
        }
    }

    fn observe_unimproved_step_score(&mut self, score: S::Score) {
        if self.best_score.is_none_or(|best| score > best) {
            self.restart_unimproved_window(self.total_step_count.saturating_add(1));
        }
    }

    fn inphase_unimproved_limit_reached(&self) -> bool {
        self.unimproved.has_limits()
            && self.best_score.is_some()
            && self
                .unimproved
                .limit_reached(self.total_step_count, self.elapsed().unwrap_or_default())
    }
}
