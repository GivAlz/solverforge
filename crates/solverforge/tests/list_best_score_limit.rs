#[path = "list_best_score_limit/mod.rs"]
mod domain;

pub use domain::{Plan, Route, Stop};

use solverforge::{HardSoftScore, SolverEvent, SolverManager, SolverTerminalReason};

const STOP_COUNT: usize = 12;

fn empty_plan(explicit_phases: bool) -> Plan {
    Plan {
        routes: (0..3)
            .map(|id| Route {
                id,
                visits: Vec::new(),
            })
            .collect(),
        stops: (0..STOP_COUNT).map(|id| Stop { id }).collect(),
        score: None,
        explicit_phases,
    }
}

fn solve_to_completion(manager: &'static SolverManager<Plan>, plan: Plan) -> Plan {
    let (job_id, mut receiver) = manager.solve(plan).expect("job should start");
    let completed = loop {
        match receiver
            .blocking_recv()
            .expect("event stream should reach a terminal event")
        {
            SolverEvent::BestSolution { solution, .. } => {
                let routed: usize = solution.routes.iter().map(|r| r.visits.len()).sum();
                assert_eq!(routed, STOP_COUNT, "published best must be complete");
            }
            SolverEvent::Completed { metadata, solution } => {
                assert_eq!(
                    metadata.terminal_reason,
                    Some(SolverTerminalReason::TerminatedByConfig)
                );
                break solution;
            }
            SolverEvent::Failed { error, .. } => panic!("solve unexpectedly failed: {error}"),
            SolverEvent::Cancelled { .. } => panic!("solve was unexpectedly cancelled"),
            _ => {}
        }
    };
    manager.delete(job_id).expect("completed job should delete");
    completed
}

fn assert_complete_within_limit(solution: &Plan) {
    let mut routed: Vec<usize> = solution
        .routes
        .iter()
        .flat_map(|route| route.visits.iter().copied())
        .collect();
    routed.sort_unstable();
    assert_eq!(routed, (0..STOP_COUNT).collect::<Vec<_>>());
    let score = solution.score.expect("completed solution should be scored");
    assert!(score < HardSoftScore::ZERO, "construction should have run");
    assert!(score >= HardSoftScore::of(0, -1_000_000));
}

#[test]
fn best_score_limit_met_by_empty_solution_waits_for_default_construction() {
    static MANAGER: SolverManager<Plan> = SolverManager::new();

    let solution = solve_to_completion(&MANAGER, empty_plan(false));

    assert_complete_within_limit(&solution);
}

#[test]
fn best_score_limit_met_by_empty_solution_waits_for_configured_construction() {
    static MANAGER: SolverManager<Plan> = SolverManager::new();

    let solution = solve_to_completion(&MANAGER, empty_plan(true));

    assert_complete_within_limit(&solution);
}
