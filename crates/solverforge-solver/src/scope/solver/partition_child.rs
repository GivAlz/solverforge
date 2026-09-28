// Phase-relative termination seen by partitioned search child scopes.

impl<S> ScopedPhaseTermination<S>
where
    S: PlanningSolution,
{
    /* Derives the overlay one partition child scope observes. The remaining
    phase time becomes an absolute deadline shared by every child; the
    remaining phase steps and the unimproved limits bound each child's own
    trajectory. A best score limit is judged only against the parent's full
    solution, never against a partition score. */
    fn for_partition_child(&self, total_step_count: u64, elapsed: Duration) -> Option<Self> {
        let phase_steps = total_step_count.saturating_sub(self.start_step_count);
        let remaining_time = self.time_limit.map(|limit| {
            Instant::now() + limit.saturating_sub(elapsed.saturating_sub(self.start_elapsed))
        });
        let deadline = match (self.deadline, remaining_time) {
            (Some(inherited), Some(own)) => Some(inherited.min(own)),
            (inherited, own) => inherited.or(own),
        };
        let step_count_limit = self
            .step_count_limit
            .map(|limit| limit.saturating_sub(phase_steps));
        if deadline.is_none()
            && step_count_limit.is_none()
            && self.unimproved_step_count_limit.is_none()
            && self.unimproved_time_limit.is_none()
        {
            return None;
        }
        Some(Self {
            start_step_count: 0,
            start_elapsed: Duration::ZERO,
            time_limit: None,
            deadline,
            step_count_limit,
            best_score_limit: None,
            unimproved_step_count_limit: self.unimproved_step_count_limit,
            unimproved_time_limit: self.unimproved_time_limit,
            best_score: None,
            improvement_score: None,
            last_improvement_step_count: 0,
            last_improvement_elapsed: Duration::ZERO,
        })
    }
}

impl<'t, S: PlanningSolution, D: Director<S>, ProgressCb: ProgressCallback<S>>
    SolverScope<'t, S, D, ProgressCb>
{
    /// Polls termination for a phase that still owns boundary work, such as a
    /// partition merge, when only its phase-relative overlay is reached.
    ///
    /// Returns `true` only when lifecycle control or solver-level configured
    /// termination requires stopping before that work.
    pub(crate) fn should_terminate_beyond_phase(&mut self) -> bool {
        self.should_terminate()
            && (self.terminal_reason.is_some()
                || self.yielded_to_parent
                || !self.phase_termination_reached())
    }
}
