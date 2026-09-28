// Solver-level termination limits observed inside the local-search step loop.

macro_rules! climbing_local_search_phase {
    () => {{
        // Three improving steps (0 -> 1 -> 2 -> 3), then a plateau where no
        // move is accepted but every step still completes.
        let phase: LocalSearchPhase<_, ScoreFieldMove, _, _, _> = LocalSearchPhase::new(
            ScoreFieldSelector::new([1, 2, 3]),
            HillClimbingAcceptor::new(),
            AcceptedCountForager::new(1, false),
            None,
        );
        phase
    }};
}

#[test]
fn solver_unimproved_step_count_limit_stops_local_search_phase() {
    let result = crate::Solver::new((climbing_local_search_phase!(),))
        .with_termination(crate::termination::OrTermination::new((
            crate::termination::StepCountTermination::new(10_000),
            crate::termination::UnimprovedStepCountTermination::<TestSolution>::new(5),
        )))
        .solve(ScoreFieldDirector::new());

    assert_eq!(
        result.terminal_reason,
        crate::manager::SolverTerminalReason::TerminatedByConfig
    );
    assert_eq!(result.best_score, SoftScore::of(3));
    // Three improving steps followed by five unimproved steps.
    assert_eq!(result.stats.step_count, 8);
}

#[test]
fn solver_unimproved_time_limit_stops_local_search_phase() {
    let started = std::time::Instant::now();
    let result = crate::Solver::new((climbing_local_search_phase!(),))
        .with_termination(crate::termination::OrTermination::new((
            crate::termination::TimeTermination::seconds(3),
            crate::termination::UnimprovedTimeTermination::<TestSolution>::millis(20),
        )))
        .with_time_limit(Duration::from_secs(3))
        .solve(ScoreFieldDirector::new());

    assert_eq!(
        result.terminal_reason,
        crate::manager::SolverTerminalReason::TerminatedByConfig
    );
    assert_eq!(result.best_score, SoftScore::of(3));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "unimproved time limit was ignored inside local search: ran for {:?}",
        started.elapsed()
    );
}
