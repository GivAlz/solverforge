// Partition child work reported through the parent solver scope.

use std::cmp::Ordering as ScoreOrdering;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use solverforge_config::TerminationConfig;
use solverforge_scoring::Director;

use crate::manager::SolverTerminalReason;
use crate::phase::partitioned::{
    FunctionalPartitioner, PartitionedSearchConfig, PartitionedSearchPhase, ThreadCount,
};
use crate::phase::Phase;
use crate::scope::{ProgressCallback, SolverScope};
use crate::test_utils::{create_minimal_descriptor, create_scope, TestDirector, TestSolution};

const MOVES_PER_STEP: usize = 2;
const MOVE_LABEL: &str = "TestChange";

// Takes up to `max_steps` steps, evaluating `MOVES_PER_STEP` moves per step.
#[derive(Debug)]
struct WorkPhase {
    steps: Arc<AtomicUsize>,
    max_steps: usize,
}

impl<D, ProgressCb> Phase<TestSolution, D, ProgressCb> for WorkPhase
where
    D: Director<TestSolution>,
    ProgressCb: ProgressCallback<TestSolution>,
{
    fn solve(&mut self, solver_scope: &mut SolverScope<'_, TestSolution, D, ProgressCb>) {
        for _ in 0..self.max_steps {
            if solver_scope.should_terminate() {
                break;
            }
            for _ in 0..MOVES_PER_STEP {
                solver_scope.record_selector_evaluated(0, Duration::from_millis(1));
                solver_scope
                    .stats_mut()
                    .record_move_kind_evaluated(MOVE_LABEL, ScoreOrdering::Greater);
            }
            self.steps.fetch_add(1, Ordering::SeqCst);
            solver_scope.increment_step_count();
        }
        solver_scope.update_best_solution();
    }

    fn phase_type_name(&self) -> &'static str {
        "Work"
    }
}

fn child_director(solution: TestSolution) -> TestDirector {
    TestDirector::simple(solution, create_minimal_descriptor(), |_, _| 0)
}

fn two_way_partitioner(
    merges: Arc<AtomicUsize>,
) -> FunctionalPartitioner<
    TestSolution,
    impl Fn(&TestSolution) -> Vec<TestSolution> + Send + Sync,
    impl Fn(&TestSolution, Vec<TestSolution>) -> TestSolution + Send + Sync,
> {
    FunctionalPartitioner::new(
        |solution: &TestSolution| vec![solution.clone(), solution.clone()],
        move |original: &TestSolution, _partitions| {
            merges.fetch_add(1, Ordering::SeqCst);
            original.clone()
        },
    )
}

fn work_phase(
    thread_count: usize,
    steps: &Arc<AtomicUsize>,
    merges: &Arc<AtomicUsize>,
    max_steps: usize,
) -> impl Phase<TestSolution, TestDirector> {
    let child_steps = Arc::clone(steps);
    PartitionedSearchPhase::with_config(
        two_way_partitioner(Arc::clone(merges)),
        child_director,
        move || {
            (WorkPhase {
                steps: Arc::clone(&child_steps),
                max_steps,
            },)
        },
        PartitionedSearchConfig {
            thread_count: ThreadCount::Specific(thread_count),
            log_progress: false,
        },
    )
}

#[test]
fn sequential_partition_child_work_reaches_parent_telemetry() {
    assert_child_work_reaches_parent_telemetry(1);
}

#[test]
fn parallel_partition_child_work_reaches_parent_telemetry() {
    assert_child_work_reaches_parent_telemetry(2);
}

fn assert_child_work_reaches_parent_telemetry(thread_count: usize) {
    let mut solver_scope = create_scope();
    solver_scope.initialize_working_solution_as_best();
    solver_scope.increment_step_count();
    let parent_score_calculations = solver_scope.stats().score_calculations;
    let steps = Arc::new(AtomicUsize::new(0));
    let merges = Arc::new(AtomicUsize::new(0));
    let mut phase = work_phase(thread_count, &steps, &merges, 3);

    phase.solve(&mut solver_scope);

    let child_steps = 6;
    let child_moves = child_steps * MOVES_PER_STEP as u64;
    assert_eq!(steps.load(Ordering::SeqCst) as u64, child_steps);
    assert_eq!(merges.load(Ordering::SeqCst), 1);
    assert_eq!(solver_scope.total_step_count(), 1 + child_steps);
    let telemetry = solver_scope.stats().snapshot();
    assert_eq!(telemetry.step_count, 1 + child_steps);
    assert_eq!(telemetry.moves_evaluated, child_moves);
    assert_eq!(
        telemetry.evaluation_time,
        Duration::from_millis(child_moves)
    );
    // Each child bootstraps its own score before the parent rescoring after merge.
    assert!(telemetry.score_calculations >= parent_score_calculations + 3);
    assert_eq!(telemetry.selector_telemetry.len(), 1);
    assert_eq!(telemetry.selector_telemetry[0].moves_evaluated, child_moves);
    assert_eq!(telemetry.move_telemetry.len(), 1);
    assert_eq!(telemetry.move_telemetry[0].move_label, MOVE_LABEL);
    assert_eq!(telemetry.move_telemetry[0].moves_evaluated, child_moves);
    assert_eq!(
        telemetry.move_telemetry[0].moves_score_improving,
        child_moves
    );
}

#[test]
fn partition_child_work_is_reported_when_the_shared_budget_stops_children() {
    let mut solver_scope = create_scope();
    solver_scope.initialize_working_solution_as_best();
    solver_scope.inphase_step_count_limit = Some(4);
    let steps = Arc::new(AtomicUsize::new(0));
    let merges = Arc::new(AtomicUsize::new(0));
    let mut phase = work_phase(1, &steps, &merges, 3);

    phase.solve(&mut solver_scope);

    assert_eq!(steps.load(Ordering::SeqCst), 4);
    assert_eq!(solver_scope.stats().step_count, 4);
    assert_eq!(
        solver_scope.stats().moves_evaluated,
        4 * MOVES_PER_STEP as u64
    );
    assert_eq!(
        solver_scope.terminal_reason(),
        SolverTerminalReason::TerminatedByConfig
    );
}

// Child steps are consumed from the solver-level step budget, so the phases
// after a partitioned search only get what the children left over.
#[test]
fn phases_after_partitioned_search_see_the_budget_children_consumed() {
    let mut solver_scope = create_scope();
    solver_scope.initialize_working_solution_as_best();
    solver_scope.inphase_step_count_limit = Some(10);
    let steps = Arc::new(AtomicUsize::new(0));
    let merges = Arc::new(AtomicUsize::new(0));
    let mut phase = work_phase(2, &steps, &merges, 3);

    phase.solve(&mut solver_scope);

    assert_eq!(merges.load(Ordering::SeqCst), 1);
    assert_eq!(solver_scope.total_step_count(), 6);
    assert!(!solver_scope.should_terminate());

    let later_steps = Arc::new(AtomicUsize::new(0));
    let mut later_phase = WorkPhase {
        steps: Arc::clone(&later_steps),
        max_steps: 10,
    };
    later_phase.solve(&mut solver_scope);

    assert_eq!(later_steps.load(Ordering::SeqCst), 4);
    assert_eq!(solver_scope.total_step_count(), 10);
    assert_eq!(solver_scope.stats().step_count, 10);
}

// Child steps are counted only once the merge is decided, so they never make
// the partitioned phase's own step limit skip the merge.
#[test]
fn partitioned_phase_step_limit_still_merges_after_child_work() {
    let mut solver_scope = create_scope();
    solver_scope.initialize_working_solution_as_best();
    let steps = Arc::new(AtomicUsize::new(0));
    let merges = Arc::new(AtomicUsize::new(0));
    let mut phase = work_phase(1, &steps, &merges, 5);
    let termination = TerminationConfig {
        step_count_limit: Some(2),
        ..TerminationConfig::default()
    };

    solver_scope.with_phase_termination(Some(&termination), |solver_scope| {
        phase.solve(solver_scope);
    });

    let child_steps = steps.load(Ordering::SeqCst) as u64;
    assert!(child_steps > 0);
    assert_eq!(merges.load(Ordering::SeqCst), 1);
    assert_eq!(solver_scope.total_step_count(), child_steps);
    assert_eq!(solver_scope.stats().step_count, child_steps);
    assert_eq!(
        solver_scope.terminal_reason(),
        SolverTerminalReason::Completed
    );
}
