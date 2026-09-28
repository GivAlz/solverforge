use super::{IncrementScorePhase, ScoreFromSolutionConstraints, TestSolution};
use crate::manager::SolverTerminalReason;
use crate::run::{build_termination, AnyTermination};
use crate::solver::{SolveResult, Solver};
use solverforge_config::{SolverConfig, TerminationConfig};
use solverforge_core::domain::SolutionDescriptor;
use solverforge_core::score::SoftScore;
use solverforge_scoring::{Director, ScoreDirector};
use std::any::TypeId;
use std::time::Duration;

fn assert_unbounded<D>(termination: &AnyTermination<TestSolution, D>)
where
    D: Director<TestSolution>,
{
    assert!(termination.time.is_none());
    assert!(termination.best_score.is_none());
    assert!(termination.step_count.is_none());
    assert!(termination.unimproved_step_count.is_none());
    assert!(termination.unimproved_time.is_none());
}

#[test]
fn build_termination_preserves_missing_time_limit_as_unlimited() {
    let config = SolverConfig::default();
    let (termination, time_limit) = build_termination::<TestSolution, ()>(&config, 180);

    assert_unbounded(&termination);
    assert_eq!(time_limit, None);
}

#[test]
fn build_termination_treats_empty_and_invalid_only_configs_as_unlimited() {
    for termination_config in [
        TerminationConfig::default(),
        TerminationConfig {
            best_score_limit: Some("not-a-score".to_string()),
            ..TerminationConfig::default()
        },
    ] {
        let config = SolverConfig {
            termination: Some(termination_config),
            ..SolverConfig::default()
        };
        let (termination, time_limit) = build_termination::<TestSolution, ()>(&config, 180);

        assert_unbounded(&termination);
        assert_eq!(time_limit, None);
    }
}

#[test]
fn build_termination_returns_fallback_time_for_best_score_limit() {
    let config = SolverConfig {
        termination: Some(TerminationConfig {
            best_score_limit: Some("0".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    };

    let (termination, time_limit) = build_termination::<TestSolution, ()>(&config, 180);

    assert!(termination.best_score.is_some());
    assert!(termination.time.is_some());
    assert_eq!(time_limit, Some(Duration::from_secs(180)));
}

fn solve_increment_score_with_termination(
    termination_config: TerminationConfig,
    max_score: i64,
) -> SolveResult<TestSolution> {
    let config = SolverConfig {
        termination: Some(termination_config),
        ..Default::default()
    };
    let (termination, time_limit) =
        build_termination::<TestSolution, ScoreFromSolutionConstraints>(&config, 180);
    let descriptor = SolutionDescriptor::new("TestSolution", TypeId::of::<TestSolution>());
    let director = ScoreDirector::with_descriptor(
        TestSolution {
            score: Some(SoftScore::of(0)),
        },
        ScoreFromSolutionConstraints,
        descriptor,
        |_, _| 1,
    );

    let mut solver = Solver::new((IncrementScorePhase { max_score },))
        .with_config(config)
        .with_termination(termination);
    if let Some(time_limit) = time_limit {
        solver = solver.with_time_limit(time_limit);
    }
    solver.solve(director)
}

#[test]
fn config_best_score_limit_stops_active_phase_loop() {
    let result = solve_increment_score_with_termination(
        TerminationConfig {
            best_score_limit: Some("2".to_string()),
            ..Default::default()
        },
        5,
    );

    assert_eq!(
        result.terminal_reason(),
        SolverTerminalReason::TerminatedByConfig
    );
    assert_eq!(*result.best_score(), SoftScore::of(2));
    assert_eq!(result.step_count(), 2);
}

#[test]
fn config_termination_stops_at_the_first_of_all_configured_criteria() {
    // An unreachable score target must not hide the step limit configured
    // alongside it.
    let result = solve_increment_score_with_termination(
        TerminationConfig {
            seconds_spent_limit: Some(60),
            best_score_limit: Some("100".to_string()),
            step_count_limit: Some(2),
            ..Default::default()
        },
        5,
    );
    assert_eq!(
        result.terminal_reason(),
        SolverTerminalReason::TerminatedByConfig
    );
    assert_eq!(result.step_count(), 2);
    assert_eq!(*result.best_score(), SoftScore::of(2));

    // Every criterion stays binding: here the score target is reached before
    // the step and unimproved-step limits.
    let result = solve_increment_score_with_termination(
        TerminationConfig {
            best_score_limit: Some("3".to_string()),
            step_count_limit: Some(4),
            unimproved_step_count_limit: Some(10),
            ..Default::default()
        },
        5,
    );
    assert_eq!(
        result.terminal_reason(),
        SolverTerminalReason::TerminatedByConfig
    );
    assert_eq!(result.step_count(), 3);
    assert_eq!(*result.best_score(), SoftScore::of(3));
}

#[test]
fn build_termination_returns_fallback_time_for_each_single_criterion() {
    for termination_config in [
        TerminationConfig {
            step_count_limit: Some(10),
            ..Default::default()
        },
        TerminationConfig {
            unimproved_step_count_limit: Some(10),
            ..Default::default()
        },
        TerminationConfig {
            unimproved_seconds_spent_limit: Some(10),
            ..Default::default()
        },
    ] {
        let config = SolverConfig {
            termination: Some(termination_config.clone()),
            ..Default::default()
        };

        let (termination, time_limit) = build_termination::<TestSolution, ()>(&config, 180);

        assert_eq!(
            termination.step_count.is_some(),
            termination_config.step_count_limit.is_some()
        );
        assert_eq!(
            termination.unimproved_step_count.is_some(),
            termination_config.unimproved_step_count_limit.is_some()
        );
        assert_eq!(
            termination.unimproved_time.is_some(),
            termination_config.unimproved_seconds_spent_limit.is_some()
        );
        assert!(termination.best_score.is_none());
        assert!(termination.time.is_some());
        assert_eq!(time_limit, Some(Duration::from_secs(180)));
    }
}

#[test]
fn build_termination_keeps_every_configured_criterion() {
    let config = SolverConfig {
        termination: Some(TerminationConfig {
            seconds_spent_limit: Some(30),
            best_score_limit: Some("0".to_string()),
            step_count_limit: Some(10),
            unimproved_step_count_limit: Some(20),
            unimproved_seconds_spent_limit: Some(5),
            ..Default::default()
        }),
        ..Default::default()
    };

    let (termination, time_limit) = build_termination::<TestSolution, ()>(&config, 180);

    assert!(termination.time.is_some());
    assert!(termination.best_score.is_some());
    assert!(termination.step_count.is_some());
    assert!(termination.unimproved_step_count.is_some());
    assert!(termination.unimproved_time.is_some());
    assert_eq!(time_limit, Some(Duration::from_secs(30)));
}

#[test]
fn build_termination_explicit_time_overrides_fallback() {
    let config = SolverConfig {
        termination: Some(TerminationConfig {
            step_count_limit: Some(10),
            seconds_spent_limit: Some(5),
            ..Default::default()
        }),
        ..Default::default()
    };

    let (termination, time_limit) = build_termination::<TestSolution, ()>(&config, 180);

    assert!(termination.step_count.is_some());
    assert_eq!(time_limit, Some(Duration::from_secs(5)));
}
