// Configured `partitioned_search` phase termination and child phases.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use solverforge_config::{PartitionedSearchConfig, PhaseConfig, SolverConfig, TerminationConfig};
use solverforge_core::domain::SolutionDescriptor;
use solverforge_scoring::{Director, ScoreDirector};

use super::super::executor::CompiledRuntimePhaseRunner;
use super::super::{
    compile_runtime_graph, CompiledRuntimeExecutor, RuntimeCompileErrorKind, RuntimeGraphInput,
};
use super::{context, custom, partitioned, Plan};
use crate::manager::SolverTerminalReason;
use crate::phase::partitioned::{
    FunctionalPartitioner, PartitionedSearchConfig as RuntimePartitionConfig,
    PartitionedSearchPhase,
};
use crate::phase::Phase;
use crate::scope::{ProgressCallback, SolverScope};

#[derive(Debug)]
struct CountStepsPhase {
    steps: Arc<AtomicUsize>,
    max_steps: usize,
}

impl<D, ProgressCb> Phase<Plan, D, ProgressCb> for CountStepsPhase
where
    D: Director<Plan>,
    ProgressCb: ProgressCallback<Plan>,
{
    fn solve(&mut self, solver_scope: &mut SolverScope<'_, Plan, D, ProgressCb>) {
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

fn descriptor() -> SolutionDescriptor {
    SolutionDescriptor::new("Plan", std::any::TypeId::of::<Plan>())
}

fn director(plan: Plan) -> ScoreDirector<Plan, ()> {
    ScoreDirector::simple(plan, descriptor(), |_, _| 0)
}

fn partitioned_with(
    termination: Option<TerminationConfig>,
    child_phases: Vec<PhaseConfig>,
) -> PhaseConfig {
    let PhaseConfig::PartitionedSearch(config) = partitioned(Some("halves")) else {
        unreachable!("partitioned helper must return a partitioned phase");
    };
    PhaseConfig::PartitionedSearch(PartitionedSearchConfig {
        termination,
        child_phases,
        ..config
    })
}

/* Solves `config` with one registered two-way partitioner whose children count
their steps, returning the total child step count and the merge count. */
fn solve_counting_child_steps(config: &SolverConfig) -> (usize, usize, SolverTerminalReason) {
    let steps = Arc::new(AtomicUsize::new(0));
    let merges = Arc::new(AtomicUsize::new(0));
    let builder_steps = Arc::clone(&steps);
    let builder_merges = Arc::clone(&merges);
    let declaration = context(config.random_seed).defaults().partitioned_phase(
        "halves",
        move |_context, partition_config| {
            let child_steps = Arc::clone(&builder_steps);
            let merges = Arc::clone(&builder_merges);
            PartitionedSearchPhase::with_config(
                FunctionalPartitioner::new(
                    |plan: &Plan| vec![plan.clone(), plan.clone()],
                    move |original: &Plan, _parts| {
                        merges.fetch_add(1, Ordering::SeqCst);
                        original.clone()
                    },
                ),
                director,
                move || {
                    (CountStepsPhase {
                        steps: Arc::clone(&child_steps),
                        max_steps: 5,
                    },)
                },
                RuntimePartitionConfig::from_serialized(partition_config),
            )
        },
    );
    let (context, extensions) = declaration.into_runtime_parts();
    let graph = compile_runtime_graph(config, RuntimeGraphInput::new(context, extensions))
        .expect("registered partitioner compiles");
    let executor = CompiledRuntimeExecutor::new(graph);
    let mut runner =
        CompiledRuntimePhaseRunner::try_new(&executor).expect("partitioned runtime prepares");
    let mut solver_scope = SolverScope::new(director(Plan { score: None }));
    solver_scope.initialize_working_solution_as_best();

    runner.solve(&mut solver_scope);

    (
        steps.load(Ordering::SeqCst),
        merges.load(Ordering::SeqCst),
        solver_scope.terminal_reason(),
    )
}

#[test]
fn configured_partitioned_phase_runs_children_without_phase_termination() {
    let config = SolverConfig {
        phases: vec![partitioned_with(None, Vec::new())],
        ..SolverConfig::default()
    };

    assert_eq!(
        solve_counting_child_steps(&config),
        (10, 1, SolverTerminalReason::Completed)
    );
}

#[test]
fn configured_partitioned_phase_termination_bounds_children() {
    let termination = TerminationConfig {
        step_count_limit: Some(2),
        ..TerminationConfig::default()
    };
    let config = SolverConfig {
        phases: vec![partitioned_with(Some(termination), Vec::new())],
        ..SolverConfig::default()
    };

    assert_eq!(
        solve_counting_child_steps(&config),
        (4, 1, SolverTerminalReason::Completed)
    );
}

#[test]
fn configured_partitioned_child_phases_are_rejected() {
    let config = SolverConfig {
        phases: vec![partitioned_with(None, vec![custom("child")])],
        ..SolverConfig::default()
    };
    let declaration = context(config.random_seed)
        .defaults()
        .partitioned_phase("halves", |_context, _config| super::MarkerExtension);
    let (context, extensions) = declaration.into_runtime_parts();

    let error = compile_runtime_graph(&config, RuntimeGraphInput::new(context, extensions))
        .expect_err("configured partition child phases must not be silently ignored");

    assert_eq!(error.path, "phases[0]");
    assert!(matches!(
        error.kind,
        RuntimeCompileErrorKind::UnsupportedPartitionedChildPhases { count: 1 }
    ));
    assert!(
        error.to_string().contains("child_phases"),
        "unexpected error message: {error}"
    );
}
