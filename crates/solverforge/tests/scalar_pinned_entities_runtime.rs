//! Pinned scalar entities never reach the local-search selectors: the work a
//! step does depends only on the free entities, not on how many are pinned.

#[path = "scalar_pinned_entities_runtime/mod.rs"]
mod domain;

use domain::{PinPlan, PinTask, PinValue};

use solverforge::{SolverEvent, SolverManager};

const VALUE_COUNT: usize = 12;
const FREE_TASKS: usize = 4;
const PINNED_TASKS: usize = 60;
const STEPS: u64 = 3;

/// Explicit selectors default to a seeded random order whose permutation
/// depends on the entity count, so the comparison uses canonical order.
fn solver_toml(move_selector: &str) -> String {
    format!(
        r#"
random_seed = 1

[termination]
step_count_limit = {STEPS}

[[phases]]
type = "local_search"

[phases.acceptor]
type = "hill_climbing"

[phases.forager]
type = "best_score"

[phases.move_selector]
selection_order = "original"
{move_selector}
"#
    )
}

/// Pinned tasks come first and already sit on their target; free tasks start
/// in pairs on values away from their targets, so every family has work. The
/// free tasks are identical with and without the pinned prefix.
fn plan(pinned: usize, move_selector: &str) -> PinPlan {
    let pinned_tasks = (0..pinned).map(|id| {
        let target = (id * 5) % VALUE_COUNT;
        PinTask {
            id,
            target,
            pinned: true,
            value_idx: Some(target),
        }
    });
    let free_tasks = (0..FREE_TASKS).map(|offset| PinTask {
        id: pinned + offset,
        target: (offset * 7 + 3) % VALUE_COUNT,
        pinned: false,
        value_idx: Some(offset / 2 + 1),
    });
    PinPlan {
        values: (0..VALUE_COUNT).map(|id| PinValue { id }).collect(),
        tasks: pinned_tasks.chain(free_tasks).collect(),
        score: None,
        solver_toml: solver_toml(move_selector),
        value_indices: (0..VALUE_COUNT).collect(),
        task_indices: (0..pinned + FREE_TASKS).collect(),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct RunSummary {
    steps: u64,
    generated: u64,
    evaluated: u64,
    not_doable: u64,
    free_values: Vec<Option<usize>>,
}

fn run(manager: &'static SolverManager<PinPlan>, pinned: usize, move_selector: &str) -> RunSummary {
    let (job_id, mut events) = manager
        .solve(plan(pinned, move_selector))
        .expect("job starts");
    let mut summary = None;
    while let Some(event) = events.blocking_recv() {
        match event {
            SolverEvent::Completed { metadata, solution } => {
                let telemetry = &metadata.telemetry;
                assert!(
                    solution.tasks[..pinned]
                        .iter()
                        .all(|task| task.value_idx == Some(task.target)),
                    "pinned tasks keep their input values"
                );
                summary = Some(RunSummary {
                    steps: telemetry.step_count,
                    generated: telemetry.moves_generated,
                    evaluated: telemetry.moves_evaluated,
                    not_doable: telemetry.moves_not_doable,
                    free_values: solution.tasks[pinned..]
                        .iter()
                        .map(|task| task.value_idx)
                        .collect(),
                });
                break;
            }
            SolverEvent::Failed { error, .. } => panic!("pinned solve failed: {error}"),
            _ => {}
        }
    }
    manager.delete(job_id).expect("delete completed job");
    summary.expect("solve completes")
}

fn assert_pinned_entities_are_not_enumerated(
    manager: &'static SolverManager<PinPlan>,
    move_selector: &str,
) {
    let free_only = run(manager, 0, move_selector);
    let with_pinned = run(manager, PINNED_TASKS, move_selector);

    assert_eq!(free_only.steps, STEPS, "{move_selector}");
    assert!(free_only.generated > 0, "{move_selector}");
    assert_eq!(
        with_pinned, free_only,
        "pinned entities must not add generated, evaluated, or rejected candidates for {move_selector}"
    );
}

#[test]
fn change_selector_skips_pinned_entities() {
    static MANAGER: SolverManager<PinPlan> = SolverManager::new();
    assert_pinned_entities_are_not_enumerated(&MANAGER, r#"type = "change_move_selector""#);
    // Every free task offers each value plus unassignment on every step.
    let summary = run(&MANAGER, PINNED_TASKS, r#"type = "change_move_selector""#);
    assert_eq!(
        summary.generated,
        STEPS * (FREE_TASKS * (VALUE_COUNT + 1)) as u64
    );
}

#[test]
fn swap_selector_skips_pinned_entities() {
    static MANAGER: SolverManager<PinPlan> = SolverManager::new();
    assert_pinned_entities_are_not_enumerated(&MANAGER, r#"type = "swap_move_selector""#);
}

#[test]
fn pillar_change_selector_skips_pinned_entities() {
    static MANAGER: SolverManager<PinPlan> = SolverManager::new();
    assert_pinned_entities_are_not_enumerated(
        &MANAGER,
        "type = \"pillar_change_move_selector\"\nminimum_sub_pillar_size = 0\nmaximum_sub_pillar_size = 0",
    );
}

#[test]
fn ruin_recreate_selector_skips_pinned_entities() {
    static MANAGER: SolverManager<PinPlan> = SolverManager::new();
    assert_pinned_entities_are_not_enumerated(
        &MANAGER,
        "type = \"ruin_recreate_move_selector\"\nmin_ruin_count = 1\nmax_ruin_count = 2\nmoves_per_step = 4\nrecreate_heuristic_type = \"first_fit\"",
    );
}

#[test]
fn nearby_change_selector_skips_pinned_entities() {
    static MANAGER: SolverManager<PinPlan> = SolverManager::new();
    assert_pinned_entities_are_not_enumerated(
        &MANAGER,
        "type = \"nearby_change_move_selector\"\nmax_nearby = 3",
    );
}

#[test]
fn nearby_swap_selector_skips_pinned_entities() {
    static MANAGER: SolverManager<PinPlan> = SolverManager::new();
    assert_pinned_entities_are_not_enumerated(
        &MANAGER,
        "type = \"nearby_swap_move_selector\"\nmax_nearby = 3",
    );
}

#[test]
fn pillar_swap_selector_skips_pinned_entities() {
    static MANAGER: SolverManager<PinPlan> = SolverManager::new();
    assert_pinned_entities_are_not_enumerated(
        &MANAGER,
        "type = \"pillar_swap_move_selector\"\nminimum_sub_pillar_size = 0\nmaximum_sub_pillar_size = 0",
    );
}
