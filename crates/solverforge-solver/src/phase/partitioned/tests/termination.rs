// Phase-relative termination of a partitioned search phase.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use solverforge_config::TerminationConfig;
use solverforge_scoring::Director;

use crate::manager::SolverTerminalReason;
use crate::phase::partitioned::{
    FunctionalPartitioner, PartitionedSearchConfig, PartitionedSearchPhase, ThreadCount,
};
use crate::phase::Phase;
use crate::scope::{ProgressCallback, SolverScope};
use crate::test_utils::{create_minimal_descriptor, create_scope, TestDirector, TestSolution};

const CHILD_SAFETY_LIMIT: Duration = Duration::from_secs(5);

#[derive(Debug)]
struct CountStepsPhase {
    steps: Arc<AtomicUsize>,
    max_steps: usize,
}

impl<D, ProgressCb> Phase<TestSolution, D, ProgressCb> for CountStepsPhase
where
    D: Director<TestSolution>,
    ProgressCb: ProgressCallback<TestSolution>,
{
    fn solve(&mut self, solver_scope: &mut SolverScope<'_, TestSolution, D, ProgressCb>) {
        for _ in 0..self.max_steps {
            if solver_scope.should_terminate() {
                break;
            }
            self.steps.fetch_add(1, Ordering::SeqCst);
            solver_scope.increment_step_count();
        }
        solver_scope.update_best_solution();
    }

    fn phase_type_name(&self) -> &'static str {
        "CountSteps"
    }
}

// Runs until its scope asks it to stop; the safety limit only bounds a failing test.
#[derive(Debug)]
struct RunUntilTerminatedPhase;

impl<D, ProgressCb> Phase<TestSolution, D, ProgressCb> for RunUntilTerminatedPhase
where
    D: Director<TestSolution>,
    ProgressCb: ProgressCallback<TestSolution>,
{
    fn solve(&mut self, solver_scope: &mut SolverScope<'_, TestSolution, D, ProgressCb>) {
        let started = Instant::now();
        while !solver_scope.should_terminate() && started.elapsed() < CHILD_SAFETY_LIMIT {
            std::hint::spin_loop();
        }
        solver_scope.update_best_solution();
    }

    fn phase_type_name(&self) -> &'static str {
        "RunUntilTerminated"
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

fn config(thread_count: usize) -> PartitionedSearchConfig {
    PartitionedSearchConfig {
        thread_count: ThreadCount::Specific(thread_count),
        log_progress: false,
    }
}

#[test]
fn phase_step_limit_bounds_each_partition_child_and_still_merges() {
    let mut solver_scope = create_scope();
    solver_scope.initialize_working_solution_as_best();
    let steps = Arc::new(AtomicUsize::new(0));
    let merges = Arc::new(AtomicUsize::new(0));
    let child_steps = Arc::clone(&steps);
    let mut phase = PartitionedSearchPhase::with_config(
        two_way_partitioner(Arc::clone(&merges)),
        child_director,
        move || {
            (CountStepsPhase {
                steps: Arc::clone(&child_steps),
                max_steps: 5,
            },)
        },
        config(1),
    );
    let termination = TerminationConfig {
        step_count_limit: Some(2),
        ..TerminationConfig::default()
    };

    solver_scope.with_phase_termination(Some(&termination), |solver_scope| {
        phase.solve(solver_scope);
    });

    assert_eq!(steps.load(Ordering::SeqCst), 4);
    assert_eq!(merges.load(Ordering::SeqCst), 1);
    assert_eq!(
        solver_scope.terminal_reason(),
        SolverTerminalReason::Completed
    );
    assert!(!solver_scope.should_terminate());
}

#[test]
fn phase_time_limit_stops_parallel_partition_children_and_still_merges() {
    assert_phase_time_limit_stops_children_and_merges(2);
}

// The second sequential child starts after the shared phase deadline.
#[test]
fn phase_time_limit_stops_sequential_partition_children_and_still_merges() {
    assert_phase_time_limit_stops_children_and_merges(1);
}

fn assert_phase_time_limit_stops_children_and_merges(thread_count: usize) {
    let mut solver_scope = create_scope();
    solver_scope.initialize_working_solution_as_best();
    let merges = Arc::new(AtomicUsize::new(0));
    let mut phase = PartitionedSearchPhase::with_config(
        two_way_partitioner(Arc::clone(&merges)),
        child_director,
        || (RunUntilTerminatedPhase,),
        config(thread_count),
    );
    let termination = TerminationConfig {
        seconds_spent_limit: Some(1),
        ..TerminationConfig::default()
    };

    let started = Instant::now();
    solver_scope.with_phase_termination(Some(&termination), |solver_scope| {
        phase.solve(solver_scope);
    });
    let elapsed = started.elapsed();

    assert!(
        elapsed >= Duration::from_secs(1) && elapsed < Duration::from_secs(3),
        "partition children must stop at the phase time limit, took {elapsed:?}"
    );
    assert_eq!(merges.load(Ordering::SeqCst), 1);
    assert_eq!(
        solver_scope.terminal_reason(),
        SolverTerminalReason::Completed
    );
}

#[test]
fn phase_termination_already_reached_skips_partitioning() {
    let mut solver_scope = create_scope();
    solver_scope.initialize_working_solution_as_best();
    let steps = Arc::new(AtomicUsize::new(0));
    let merges = Arc::new(AtomicUsize::new(0));
    let child_steps = Arc::clone(&steps);
    let mut phase = PartitionedSearchPhase::with_config(
        two_way_partitioner(Arc::clone(&merges)),
        child_director,
        move || {
            (CountStepsPhase {
                steps: Arc::clone(&child_steps),
                max_steps: 5,
            },)
        },
        config(1),
    );
    let termination = TerminationConfig {
        best_score_limit: Some("0".to_string()),
        ..TerminationConfig::default()
    };

    solver_scope.with_phase_termination(Some(&termination), |solver_scope| {
        phase.solve(solver_scope);
    });

    assert_eq!(steps.load(Ordering::SeqCst), 0);
    assert_eq!(merges.load(Ordering::SeqCst), 0);
    assert_eq!(
        solver_scope.terminal_reason(),
        SolverTerminalReason::Completed
    );
}
