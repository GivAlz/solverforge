// Partition child work folded into the parent solver scope.

impl<'t, S: PlanningSolution, D: Director<S>, ProgressCb: ProgressCallback<S>>
    SolverScope<'t, S, D, ProgressCb>
{
    /// Adds the statistics of returned partition child scopes to this scope's
    /// telemetry, whatever their outcome, and returns the steps they took.
    ///
    /// The steps are not yet counted as this scope's own: pass them to
    /// `count_merged_partition_steps` when the partitions are merged.
    pub(crate) fn absorb_partition_children(
        &mut self,
        children: impl IntoIterator<Item = SolverStats>,
    ) -> u64 {
        let mut steps = 0;
        for child in children {
            steps += child.step_count;
            self.stats.absorb_partition_child(child);
        }
        steps
    }

    /// Counts the steps of merged partition children as solver steps.
    ///
    /// Children already share solver-level step, move, and score-calculation
    /// limits while they run; advancing the step count at the merge keeps
    /// later phases to the budget the children left over. Doing it only once
    /// the merge is decided keeps child steps out of the termination checks
    /// that decide whether to merge at all.
    pub(crate) fn count_merged_partition_steps(&mut self, steps: u64) {
        self.total_step_count += steps;
    }
}
