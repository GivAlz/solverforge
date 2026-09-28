// Solver-level configured termination reached inside partition children.

use std::any::TypeId;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use solverforge_core::domain::{PlanningSolution, SolutionDescriptor};
use solverforge_core::score::SoftScore;
use solverforge_scoring::Director;

use crate::manager::SolverTerminalReason;
use crate::phase::partitioned::{
    FunctionalPartitioner, PartitionedSearchConfig, PartitionedSearchPhase, ThreadCount,
};
use crate::phase::Phase;
use crate::scope::{ProgressCallback, SolverScope};

const SOLVER_TIME_LIMIT: Duration = Duration::from_millis(100);
const CHILD_SAFETY_LIMIT: Duration = Duration::from_secs(5);

// Each partition owns one slot of `values`; the score is their sum.
#[derive(Clone, Debug)]
struct SlotSolution {
    values: Vec<i64>,
    owner: Option<usize>,
    score: Option<SoftScore>,
}

impl PlanningSolution for SlotSolution {
    type Score = SoftScore;

    fn score(&self) -> Option<Self::Score> {
        self.score
    }

    fn set_score(&mut self, score: Option<Self::Score>) {
        self.score = score;
    }
}

#[derive(Clone, Debug)]
struct SlotDirector {
    solution: SlotSolution,
    descriptor: SolutionDescriptor,
}

impl SlotDirector {
    fn new(solution: SlotSolution) -> Self {
        Self {
            solution,
            descriptor: SolutionDescriptor::new("SlotSolution", TypeId::of::<SlotSolution>()),
        }
    }
}

impl Director<SlotSolution> for SlotDirector {
    fn working_solution(&self) -> &SlotSolution {
        &self.solution
    }

    fn working_solution_mut(&mut self) -> &mut SlotSolution {
        &mut self.solution
    }

    fn calculate_score(&mut self) -> SoftScore {
        let score = SoftScore::of(self.solution.values.iter().sum());
        self.solution.set_score(Some(score));
        score
    }

    fn solution_descriptor(&self) -> &SolutionDescriptor {
        &self.descriptor
    }

    fn clone_working_solution(&self) -> SlotSolution {
        self.solution.clone()
    }

    fn before_variable_changed(&mut self, _descriptor_index: usize, _entity_index: usize) {}

    fn after_variable_changed(&mut self, _descriptor_index: usize, _entity_index: usize) {}

    fn entity_count(&self, _descriptor_index: usize) -> Option<usize> {
        Some(self.solution.values.len())
    }

    fn total_entity_count(&self) -> Option<usize> {
        Some(self.solution.values.len())
    }

    fn constraint_metadata(&self) -> Vec<solverforge_scoring::ConstraintMetadata<'_>> {
        Vec::new()
    }
}

// Improves its own slot once, then keeps searching until the scope stops it.
#[derive(Debug)]
struct ImproveThenRunUntilTerminatedPhase;

impl<D, ProgressCb> Phase<SlotSolution, D, ProgressCb> for ImproveThenRunUntilTerminatedPhase
where
    D: Director<SlotSolution>,
    ProgressCb: ProgressCallback<SlotSolution>,
{
    fn solve(&mut self, solver_scope: &mut SolverScope<'_, SlotSolution, D, ProgressCb>) {
        if solver_scope.should_terminate() {
            return;
        }
        let owner = solver_scope
            .working_solution()
            .owner
            .expect("partition child owns one slot");
        solver_scope.mutate(|director| {
            director.working_solution_mut().values[owner] += 1;
            director.after_variable_changed(0, owner);
        });
        solver_scope.increment_step_count();
        solver_scope.update_best_solution();

        let started = Instant::now();
        while !solver_scope.should_terminate() && started.elapsed() < CHILD_SAFETY_LIMIT {
            std::hint::spin_loop();
        }
    }

    fn phase_type_name(&self) -> &'static str {
        "ImproveThenRunUntilTerminated"
    }
}

fn slot_partitioner(
    merges: Arc<AtomicUsize>,
) -> FunctionalPartitioner<
    SlotSolution,
    impl Fn(&SlotSolution) -> Vec<SlotSolution> + Send + Sync,
    impl Fn(&SlotSolution, Vec<SlotSolution>) -> SlotSolution + Send + Sync,
> {
    FunctionalPartitioner::new(
        |solution: &SlotSolution| {
            (0..solution.values.len())
                .map(|owner| SlotSolution {
                    owner: Some(owner),
                    ..solution.clone()
                })
                .collect()
        },
        move |original: &SlotSolution, partitions: Vec<SlotSolution>| {
            merges.fetch_add(1, Ordering::SeqCst);
            let mut merged = original.clone();
            for partition in partitions {
                let owner = partition.owner.expect("partition owns one slot");
                merged.values[owner] = partition.values[owner];
            }
            merged.score = None;
            merged
        },
    )
}

fn solve_with_solver_time_limit(
    thread_count: usize,
) -> (
    Vec<i64>,
    SolverScope<'static, SlotSolution, SlotDirector>,
    usize,
) {
    let mut solver_scope = SolverScope::new(SlotDirector::new(SlotSolution {
        values: vec![0, 0],
        owner: None,
        score: None,
    }));
    solver_scope.set_time_limit(SOLVER_TIME_LIMIT);
    solver_scope.start_solving();
    solver_scope.initialize_working_solution_as_best();

    let merges = Arc::new(AtomicUsize::new(0));
    let mut phase = PartitionedSearchPhase::with_config(
        slot_partitioner(Arc::clone(&merges)),
        SlotDirector::new,
        || (ImproveThenRunUntilTerminatedPhase,),
        PartitionedSearchConfig {
            thread_count: ThreadCount::Specific(thread_count),
            log_progress: false,
        },
    );

    let started = Instant::now();
    phase.solve(&mut solver_scope);
    let elapsed = started.elapsed();
    assert!(
        elapsed < SOLVER_TIME_LIMIT + Duration::from_secs(2),
        "partition children must stop at the solver time limit, took {elapsed:?}"
    );

    let values = solver_scope.working_solution().values.clone();
    let merges = merges.load(Ordering::SeqCst);
    (values, solver_scope, merges)
}

#[test]
fn solver_time_limit_merges_parallel_partition_children() {
    let (values, solver_scope, merges) = solve_with_solver_time_limit(2);

    assert_eq!(merges, 1);
    assert_eq!(values, vec![1, 1]);
    assert_eq!(solver_scope.best_score().copied(), Some(SoftScore::of(2)));
    assert_eq!(
        solver_scope.terminal_reason(),
        SolverTerminalReason::TerminatedByConfig
    );
}

// The second sequential child starts after the solver deadline and returns its
// partition unchanged; the first child's improvement is still merged.
#[test]
fn solver_time_limit_merges_sequential_partition_children() {
    let (values, solver_scope, merges) = solve_with_solver_time_limit(1);

    assert_eq!(merges, 1);
    assert_eq!(values, vec![1, 0]);
    assert_eq!(solver_scope.best_score().copied(), Some(SoftScore::of(1)));
    assert_eq!(
        solver_scope.terminal_reason(),
        SolverTerminalReason::TerminatedByConfig
    );
}

// Cancels the solve after improving its own slot.
#[derive(Debug)]
struct ImproveThenCancelPhase {
    cancel: &'static AtomicBool,
}

impl<D, ProgressCb> Phase<SlotSolution, D, ProgressCb> for ImproveThenCancelPhase
where
    D: Director<SlotSolution>,
    ProgressCb: ProgressCallback<SlotSolution>,
{
    fn solve(&mut self, solver_scope: &mut SolverScope<'_, SlotSolution, D, ProgressCb>) {
        let owner = solver_scope
            .working_solution()
            .owner
            .expect("partition child owns one slot");
        solver_scope.mutate(|director| {
            director.working_solution_mut().values[owner] += 1;
            director.after_variable_changed(0, owner);
        });
        solver_scope.update_best_solution();
        self.cancel.store(true, Ordering::SeqCst);
    }

    fn phase_type_name(&self) -> &'static str {
        "ImproveThenCancel"
    }
}

#[test]
fn cancellation_still_abandons_partition_children() {
    static CANCEL: AtomicBool = AtomicBool::new(false);

    let mut solver_scope = SolverScope::new(SlotDirector::new(SlotSolution {
        values: vec![0, 0],
        owner: None,
        score: None,
    }))
    .with_terminate(Some(&CANCEL));
    solver_scope.set_time_limit(SOLVER_TIME_LIMIT);
    solver_scope.start_solving();
    solver_scope.initialize_working_solution_as_best();

    let merges = Arc::new(AtomicUsize::new(0));
    let mut phase = PartitionedSearchPhase::with_config(
        slot_partitioner(Arc::clone(&merges)),
        SlotDirector::new,
        || (ImproveThenCancelPhase { cancel: &CANCEL },),
        PartitionedSearchConfig {
            thread_count: ThreadCount::Specific(1),
            log_progress: false,
        },
    );

    phase.solve(&mut solver_scope);

    assert_eq!(merges.load(Ordering::SeqCst), 0);
    assert_eq!(solver_scope.working_solution().values, vec![0, 0]);
    assert_eq!(
        solver_scope.terminal_reason(),
        SolverTerminalReason::Cancelled
    );
}
