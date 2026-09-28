//! Score-trial and selection primitives for canonical regret insertion.

use std::time::Instant;

use solverforge_core::domain::PlanningSolution;
use solverforge_scoring::Director;

use super::{candidate_entities, RegretAccess, RegretEvaluation, RegretValue};
use crate::builder::context::SourceElement;
use crate::phase::construction::record_construction_candidate;
use crate::scope::{PhaseScope, ProgressCallback, StepControlPolicy};
use crate::stats::{
    CandidateTraceConstructionTarget, CandidateTraceDisposition, CandidateTracePullToken,
    CandidateTraceSource,
};

pub(super) fn eval_insertion<S, A, D, BestCb>(
    access: &A,
    entry: &SourceElement<A::Element>,
    entity_index: usize,
    position: usize,
    phase_scope: &mut PhaseScope<'_, '_, S, D, BestCb>,
) -> S::Score
where
    S: PlanningSolution,
    A: RegretAccess<S>,
    D: Director<S>,
    BestCb: ProgressCallback<S>,
{
    let evaluation_started = Instant::now();
    let descriptor_index = access.descriptor_index();
    let score_state = phase_scope.score_director().snapshot_score_state();
    phase_scope
        .score_director_mut()
        .before_variable_changed(descriptor_index, entity_index);
    access.insert_element(
        phase_scope.score_director_mut().working_solution_mut(),
        entity_index,
        position,
        entry.element.clone(),
    );
    phase_scope
        .score_director_mut()
        .after_variable_changed(descriptor_index, entity_index);
    let score = phase_scope.score_director_mut().calculate_score();
    phase_scope
        .score_director_mut()
        .before_variable_changed(descriptor_index, entity_index);
    access.remove_element(
        phase_scope.score_director_mut().working_solution_mut(),
        entity_index,
        position,
    );
    phase_scope
        .score_director_mut()
        .after_variable_changed(descriptor_index, entity_index);
    phase_scope
        .score_director_mut()
        .restore_score_state(score_state);
    phase_scope.record_score_calculation();
    record_construction_candidate(
        phase_scope,
        std::time::Duration::ZERO,
        evaluation_started.elapsed(),
    );
    score
}

pub(super) fn apply_insertion<S, A, D>(
    access: &A,
    entry: &SourceElement<A::Element>,
    entity_index: usize,
    position: usize,
    score_director: &mut D,
) where
    S: PlanningSolution,
    A: RegretAccess<S>,
    D: Director<S>,
{
    let descriptor_index = access.descriptor_index();
    score_director.before_variable_changed(descriptor_index, entity_index);
    access.insert_element(
        score_director.working_solution_mut(),
        entity_index,
        position,
        entry.element.clone(),
    );
    score_director.after_variable_changed(descriptor_index, entity_index);
}

pub(super) fn record_insertion_trial<S, A, D, BestCb>(
    access: &A,
    phase_scope: &mut PhaseScope<'_, '_, S, D, BestCb>,
    source: CandidateTraceSource,
    candidate_index: usize,
    entry: &SourceElement<A::Element>,
    entity_index: usize,
    insertion_index: usize,
) -> Option<CandidateTracePullToken>
where
    S: PlanningSolution,
    A: RegretAccess<S>,
    D: Director<S>,
    BestCb: ProgressCallback<S>,
{
    let descriptor_index = access.descriptor_index();
    phase_scope.record_candidate_operation(
        source,
        None,
        candidate_index,
        Some(CandidateTraceConstructionTarget {
            descriptor_index,
            entity_index,
        }),
        descriptor_index,
        "list_insertion_trial",
        [
            entry.source_index as u64,
            entity_index as u64,
            insertion_index as u64,
        ],
    )
}

/// Best insertion of one element together with its regret-2 across owners.
///
/// Regret is measured over routes, not over positions: each candidate owner
/// contributes its single best insertion score, and the regret is the gap
/// between the best and the second-best owner.  An element with fewer than
/// two feasible owners (a fixed owner restriction, pinned alternatives, or a
/// single owner overall) has `RegretValue::Forced`, so it is placed before
/// every element that still has a finite regret; among forced elements the
/// choice degenerates to cheapest insertion.
pub(super) type RegretCandidate<Sc> = (
    RegretValue<Sc>,
    usize,
    usize,
    Sc,
    Option<CandidateTracePullToken>,
);

pub(super) fn evaluate_regret<S, A, D, BestCb>(
    access: &A,
    entry: &SourceElement<A::Element>,
    entity_count: usize,
    control_policy: StepControlPolicy,
    phase_scope: &mut PhaseScope<'_, '_, S, D, BestCb>,
) -> RegretEvaluation<RegretCandidate<S::Score>>
where
    S: PlanningSolution,
    A: RegretAccess<S>,
    D: Director<S>,
    BestCb: ProgressCallback<S>,
{
    let restriction = access.owner_restriction(
        phase_scope.score_director().working_solution(),
        entity_count,
        &entry.element,
    );
    evaluate_regret_over_owners(
        access,
        entry,
        candidate_entities(restriction, entity_count),
        control_policy,
        phase_scope,
    )
}

/// Scores every insertion position of `entry` in each non-pinned owner of
/// `owners`, in canonical owner-then-position order.  Ties keep the first
/// evaluated insertion, both for the selected position and for the owner
/// that supplies the best score.
pub(super) fn evaluate_regret_over_owners<S, A, D, BestCb>(
    access: &A,
    entry: &SourceElement<A::Element>,
    owners: impl Iterator<Item = usize>,
    control_policy: StepControlPolicy,
    phase_scope: &mut PhaseScope<'_, '_, S, D, BestCb>,
) -> RegretEvaluation<RegretCandidate<S::Score>>
where
    S: PlanningSolution,
    A: RegretAccess<S>,
    D: Director<S>,
    BestCb: ProgressCallback<S>,
{
    let mut best: Option<(usize, usize, S::Score, Option<CandidateTracePullToken>)> = None;
    let mut second_owner_best: Option<S::Score> = None;
    for entity_index in owners {
        if crate::pinning::entity_is_pinned(
            phase_scope.score_director(),
            access.descriptor_index(),
            entity_index,
        ) {
            continue;
        }
        let len = access.list_len(
            phase_scope.score_director().working_solution(),
            entity_index,
        );
        let previous_best_score = best.map(|(_, _, score, _)| score);
        let mut owner_best: Option<S::Score> = None;
        for position in 0..=len {
            if control_policy.should_terminate_construction(phase_scope.solver_scope_mut()) {
                if let Some((_, _, _, Some(token))) = best.take() {
                    phase_scope.record_candidate_trace_disposition(
                        token,
                        CandidateTraceDisposition::ForagerIgnored,
                    );
                }
                return RegretEvaluation::Interrupted;
            }
            let trace_token = record_insertion_trial(
                access,
                phase_scope,
                CandidateTraceSource::ListRegretInsertionTrial,
                position,
                entry,
                entity_index,
                position,
            );
            let score = eval_insertion(access, entry, entity_index, position, phase_scope);
            if let Some(token) = trace_token {
                phase_scope.record_candidate_trace_disposition(
                    token,
                    CandidateTraceDisposition::Evaluated,
                );
            }
            owner_best = Some(owner_best.map_or(score, |current| current.max(score)));
            if best.is_none_or(|(_, _, best_score, _)| score > best_score) {
                if let Some((_, _, _, Some(token))) = best.take() {
                    phase_scope.record_candidate_trace_disposition(
                        token,
                        CandidateTraceDisposition::ForagerIgnored,
                    );
                }
                best = Some((entity_index, position, score, trace_token));
            } else if let Some(token) = trace_token {
                phase_scope.record_candidate_trace_disposition(
                    token,
                    CandidateTraceDisposition::ForagerIgnored,
                );
            }
        }
        // Keep the second-best owner-level score: either the previous overall
        // best was beaten by this owner, or this owner may be the runner-up.
        let Some(owner_best) = owner_best else {
            continue;
        };
        let runner_up = match previous_best_score {
            Some(previous) if owner_best > previous => previous,
            Some(_) => owner_best,
            None => continue,
        };
        if second_owner_best.is_none_or(|second| runner_up > second) {
            second_owner_best = Some(runner_up);
        }
    }

    let Some((entity_index, position, best_score, trace_token)) = best else {
        return RegretEvaluation::Complete(None);
    };
    let regret = second_owner_best.map_or(RegretValue::Forced, |second| {
        RegretValue::Finite(best_score - second)
    });
    RegretEvaluation::Complete(Some((
        regret,
        entity_index,
        position,
        best_score,
        trace_token,
    )))
}
