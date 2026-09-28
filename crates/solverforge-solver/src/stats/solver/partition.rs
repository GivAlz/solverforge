//! Folding the statistics of partition child scopes into their parent.

use super::SolverStats;
use crate::stats::{MoveTelemetry, SelectorTelemetry};

impl SolverStats {
    /// Adds the work recorded by one finished partition child scope.
    ///
    /// Every work counter is summed, and selector and move telemetry are
    /// merged by selector index and move label, so the parent reports the
    /// work its partitions did. `generation_time` and `evaluation_time` are
    /// summed as well: they total time spent in those stages, which parallel
    /// children spend concurrently, while the parent's solve clock stays the
    /// wall-clock basis of throughput. The child's own clock, its bounded
    /// applied-move and candidate traces, and its required-assignment gauge
    /// describe only its partition and are not carried over.
    ///
    /// The destructuring is exhaustive on purpose: a new field must decide
    /// here whether it is child work to sum or partition-local state.
    pub(crate) fn absorb_partition_child(&mut self, child: SolverStats) {
        let SolverStats {
            start_time: _,
            pause_started_at: _,
            step_count,
            moves_generated,
            moves_evaluated,
            moves_accepted,
            moves_applied,
            moves_not_doable,
            moves_acceptor_rejected,
            moves_forager_ignored,
            moves_hard_improving,
            moves_hard_neutral,
            moves_hard_worse,
            conflict_repair_provider_generated,
            conflict_repair_duplicate_filtered,
            conflict_repair_illegal_filtered,
            conflict_repair_not_doable_filtered,
            conflict_repair_hard_improving,
            conflict_repair_exposed,
            score_calculations,
            construction_slots_assigned,
            construction_slots_kept,
            construction_slots_no_doable,
            scalar_assignment_required_remaining: _,
            scalar_assignment_required_remaining_by_group: _,
            generation_time,
            evaluation_time,
            selector_stats,
            move_stats,
            applied_move_trace: _,
            candidate_trace: _,
        } = child;

        self.step_count += step_count;
        self.moves_generated += moves_generated;
        self.moves_evaluated += moves_evaluated;
        self.moves_accepted += moves_accepted;
        self.moves_applied += moves_applied;
        self.moves_not_doable += moves_not_doable;
        self.moves_acceptor_rejected += moves_acceptor_rejected;
        self.moves_forager_ignored += moves_forager_ignored;
        self.moves_hard_improving += moves_hard_improving;
        self.moves_hard_neutral += moves_hard_neutral;
        self.moves_hard_worse += moves_hard_worse;
        self.conflict_repair_provider_generated += conflict_repair_provider_generated;
        self.conflict_repair_duplicate_filtered += conflict_repair_duplicate_filtered;
        self.conflict_repair_illegal_filtered += conflict_repair_illegal_filtered;
        self.conflict_repair_not_doable_filtered += conflict_repair_not_doable_filtered;
        self.conflict_repair_hard_improving += conflict_repair_hard_improving;
        self.conflict_repair_exposed += conflict_repair_exposed;
        self.score_calculations += score_calculations;
        self.construction_slots_assigned += construction_slots_assigned;
        self.construction_slots_kept += construction_slots_kept;
        self.construction_slots_no_doable += construction_slots_no_doable;
        self.generation_time += generation_time;
        self.evaluation_time += evaluation_time;

        for mut selector in selector_stats {
            let label = std::mem::take(&mut selector.selector_label);
            let entry = self.selector_stats_entry_with_label(selector.selector_index, label);
            add_selector_telemetry(entry, selector);
        }
        for (move_label, telemetry) in move_stats {
            add_move_telemetry(self.move_stats_entry(move_label), telemetry);
        }
    }
}

fn add_selector_telemetry(entry: &mut SelectorTelemetry, child: SelectorTelemetry) {
    let SelectorTelemetry {
        selector_index: _,
        selector_label: _,
        moves_generated,
        moves_evaluated,
        moves_accepted,
        moves_applied,
        moves_not_doable,
        moves_acceptor_rejected,
        moves_forager_ignored,
        moves_hard_improving,
        moves_hard_neutral,
        moves_hard_worse,
        conflict_repair_provider_generated,
        conflict_repair_duplicate_filtered,
        conflict_repair_illegal_filtered,
        conflict_repair_not_doable_filtered,
        conflict_repair_hard_improving,
        conflict_repair_exposed,
        generation_time,
        evaluation_time,
    } = child;
    entry.moves_generated += moves_generated;
    entry.moves_evaluated += moves_evaluated;
    entry.moves_accepted += moves_accepted;
    entry.moves_applied += moves_applied;
    entry.moves_not_doable += moves_not_doable;
    entry.moves_acceptor_rejected += moves_acceptor_rejected;
    entry.moves_forager_ignored += moves_forager_ignored;
    entry.moves_hard_improving += moves_hard_improving;
    entry.moves_hard_neutral += moves_hard_neutral;
    entry.moves_hard_worse += moves_hard_worse;
    entry.conflict_repair_provider_generated += conflict_repair_provider_generated;
    entry.conflict_repair_duplicate_filtered += conflict_repair_duplicate_filtered;
    entry.conflict_repair_illegal_filtered += conflict_repair_illegal_filtered;
    entry.conflict_repair_not_doable_filtered += conflict_repair_not_doable_filtered;
    entry.conflict_repair_hard_improving += conflict_repair_hard_improving;
    entry.conflict_repair_exposed += conflict_repair_exposed;
    entry.generation_time += generation_time;
    entry.evaluation_time += evaluation_time;
}

fn add_move_telemetry(entry: &mut MoveTelemetry, child: MoveTelemetry) {
    let MoveTelemetry {
        move_label: _,
        moves_generated,
        moves_evaluated,
        moves_accepted,
        moves_applied,
        moves_not_doable,
        moves_acceptor_rejected,
        moves_forager_ignored,
        moves_score_improving,
        moves_applied_improving,
        moves_score_equal,
        moves_score_worse,
        moves_rejected_improving,
        applied_score_improvement,
    } = child;
    entry.moves_generated += moves_generated;
    entry.moves_evaluated += moves_evaluated;
    entry.moves_accepted += moves_accepted;
    entry.moves_applied += moves_applied;
    entry.moves_not_doable += moves_not_doable;
    entry.moves_acceptor_rejected += moves_acceptor_rejected;
    entry.moves_forager_ignored += moves_forager_ignored;
    entry.moves_score_improving += moves_score_improving;
    entry.moves_applied_improving += moves_applied_improving;
    entry.moves_score_equal += moves_score_equal;
    entry.moves_score_worse += moves_score_worse;
    entry.moves_rejected_improving += moves_rejected_improving;
    entry.applied_score_improvement += applied_score_improvement;
}
