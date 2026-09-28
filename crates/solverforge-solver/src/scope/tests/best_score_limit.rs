// Best-score limits judge only complete solutions.

use super::*;
use crate::termination::{BestScoreTermination, Termination};
use crate::test_utils::NQueensSolution;

/* Any score of the two-queens fixture reaches this limit, so every check
below is decided by solution completeness alone. */
const REACHED_LIMIT: i64 = -1_000;

fn scope_with_incomplete_best(
) -> SolverScope<'static, NQueensSolution, ScoreDirector<NQueensSolution, ()>> {
    let director = create_simple_nqueens_director(2);
    let mut scope = SolverScope::new(director);
    scope.defer_best_solution_publication();
    scope.start_solving();
    scope.update_best_solution();
    assert!(scope.best_score().is_some());
    scope
}

#[test]
fn inphase_best_score_limit_ignores_incomplete_best_solution() {
    let mut scope = scope_with_incomplete_best();
    scope.install_inphase_best_score_limit(SoftScore::of(REACHED_LIMIT));

    assert_eq!(scope.pending_control(), PendingControl::Continue);
    assert!(!scope.work_should_stop());
    assert!(!scope.should_terminate_construction());
    assert!(!scope.should_terminate());
    assert_eq!(scope.terminal_reason(), SolverTerminalReason::Completed);

    scope.publish_current_solution_as_best();

    assert_eq!(
        scope.pending_control(),
        PendingControl::ConfigTerminationRequested
    );
    assert!(scope.should_terminate());
    assert_eq!(
        scope.terminal_reason(),
        SolverTerminalReason::TerminatedByConfig
    );
}

#[test]
fn best_score_termination_ignores_incomplete_best_solution() {
    let mut scope = scope_with_incomplete_best();
    let termination = BestScoreTermination::new(SoftScore::of(REACHED_LIMIT));

    assert!(!termination.is_terminated(&scope));

    scope.publish_current_solution_as_best();

    assert!(termination.is_terminated(&scope));
}

#[test]
fn phase_best_score_limit_ignores_incomplete_best_solution() {
    let mut scope = scope_with_incomplete_best();
    let termination = TerminationConfig {
        best_score_limit: Some(REACHED_LIMIT.to_string()),
        ..TerminationConfig::default()
    };

    scope.with_phase_termination(Some(&termination), |scope| {
        assert_eq!(scope.pending_control(), PendingControl::Continue);
        scope.update_best_solution();
        scope.observe_phase_step_score(SoftScore::of(0));
        assert_eq!(scope.pending_control(), PendingControl::Continue);
        assert!(!scope.should_terminate_construction());
    });

    scope.publish_current_solution_as_best();
    scope.with_phase_termination(Some(&termination), |scope| {
        assert_eq!(
            scope.pending_control(),
            PendingControl::ConfigTerminationRequested
        );
    });
    assert_eq!(scope.terminal_reason(), SolverTerminalReason::Completed);
}
