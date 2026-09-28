/* Solver entry point. */

use std::fmt;
use std::hash::Hash;
use std::marker::PhantomData;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};

#[cfg(test)]
use std::path::Path;

use solverforge_config::SolverConfig;
use solverforge_core::domain::{PlanningSolution, SolutionDescriptor};
use solverforge_core::score::{ParseableScore, Score};
use solverforge_scoring::{ConstraintSet, ScoreDirector};
use tracing::info;

use crate::builder::{RuntimeExtensionRegistry, Search};
use crate::manager::{SolverRuntime, SolverTerminalReason};
use crate::phase::Phase;
use crate::runtime::compiler::executor::{
    take_runtime_execution_failure, CompiledRuntimePhaseRunner,
};
use crate::runtime::compiler::{compile_runtime_graph, CompiledRuntimeExecutor, RuntimeGraphInput};
use crate::runtime_build_error::{RuntimeBuildError, RuntimeBuildResult};
use crate::scope::{ProgressCallback, SolverProgressKind, SolverProgressRef};
use crate::solver::Solver;
use crate::stats::{format_duration, whole_units_per_second, QualifiedCandidateTraceRunProvenance};

mod termination;

use termination::configured_execution_policy;
pub(crate) use termination::parse_configured_termination;
pub use termination::{build_termination, AnyTermination};

#[derive(Clone)]
pub struct ChannelProgressCallback<S: PlanningSolution> {
    runtime: SolverRuntime<S>,
    _phantom: PhantomData<fn() -> S>,
}

impl<S: PlanningSolution> ChannelProgressCallback<S> {
    fn new(runtime: SolverRuntime<S>) -> Self {
        Self {
            runtime,
            _phantom: PhantomData,
        }
    }
}

impl<S: PlanningSolution> fmt::Debug for ChannelProgressCallback<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChannelProgressCallback").finish()
    }
}

impl<S: PlanningSolution> ProgressCallback<S> for ChannelProgressCallback<S> {
    fn invoke(&self, progress: SolverProgressRef<'_, S>) {
        match progress.kind {
            SolverProgressKind::Progress => {
                self.runtime.emit_progress(
                    progress.current_score.copied(),
                    progress.best_score.copied(),
                    progress.telemetry.clone(),
                );
            }
            SolverProgressKind::BestSolution => {
                if let (Some(solution), Some(score)) = (progress.solution, progress.best_score) {
                    self.runtime.emit_best_solution(
                        (*solution).clone(),
                        progress.current_score.copied(),
                        *score,
                        progress.telemetry.clone(),
                    );
                }
            }
        }
    }
}

pub fn log_solve_start(
    entity_count: usize,
    element_count: Option<usize>,
    candidate_count: Option<usize>,
) {
    match (element_count, candidate_count) {
        (Some(element_count), None) => {
            info!(
                event = "solve_start",
                entity_count = entity_count,
                element_count = element_count,
                solve_shape = "list",
            );
        }
        (None, Some(candidate_count)) => {
            info!(
                event = "solve_start",
                entity_count = entity_count,
                candidate_count = candidate_count,
                solve_shape = "scalar",
            );
        }
        _ => {
            panic!(
                "log_solve_start requires exactly one solve scale: list elements or scalar candidates"
            );
        }
    }
}

#[cfg(test)]
fn load_solver_config_from(path: impl AsRef<Path>) -> SolverConfig {
    SolverConfig::load(path).unwrap_or_default()
}

/// Runs one configured model through the immutable runtime graph compiler and
/// retained compiled runner.
///
/// `build_search` creates the one descriptor-resolved declaration consumed by
/// the compiled graph. Every model, including a zero-work model, follows this
/// one lifecycle; the public API never exposes a graph, prepared source
/// catalog, or phase-builder fallback.
#[allow(clippy::too_many_arguments)]
pub fn try_run_solver_with_config_and_search<S, C, V, DM, IDM, Declaration, BuildSearch>(
    solution: S,
    constraints: C,
    descriptor: SolutionDescriptor,
    entity_count_by_descriptor: fn(&S, usize) -> usize,
    runtime: SolverRuntime<S>,
    config: SolverConfig,
    default_time_limit_secs: u64,
    log_scale: fn(&S),
    qualified_candidate_trace_provenance: Option<QualifiedCandidateTraceRunProvenance>,
    build_search: BuildSearch,
) -> RuntimeBuildResult<S>
where
    S: PlanningSolution + Clone + Send + Sync + 'static,
    S::Score: Score + Copy + Ord + ParseableScore,
    C: ConstraintSet<S, S::Score>,
    V: Clone + Copy + PartialEq + Eq + Hash + Into<usize> + Send + Sync + fmt::Debug + 'static,
    DM: crate::heuristic::selector::nearby_list_change::CrossEntityDistanceMeter<S>
        + Clone
        + Send
        + Sync
        + fmt::Debug
        + 'static,
    IDM: crate::heuristic::selector::nearby_list_change::CrossEntityDistanceMeter<S>
        + Clone
        + Send
        + Sync
        + fmt::Debug
        + 'static,
    Declaration: Search<S, V, DM, IDM>,
    Declaration::Extensions: RuntimeExtensionRegistry<S, V, DM, IDM>,
    BuildSearch: FnOnce(&SolverConfig, SolutionDescriptor) -> RuntimeBuildResult<Declaration>,
{
    try_run_solver_with_config_and_search_request(
        solution,
        constraints,
        descriptor,
        entity_count_by_descriptor,
        runtime,
        config,
        default_time_limit_secs,
        log_scale,
        qualified_candidate_trace_provenance,
        build_search,
    )
}

#[allow(clippy::too_many_arguments)]
fn try_run_solver_with_config_and_search_request<S, C, V, DM, IDM, Declaration, BuildSearch>(
    solution: S,
    constraints: C,
    descriptor: SolutionDescriptor,
    entity_count_by_descriptor: fn(&S, usize) -> usize,
    runtime: SolverRuntime<S>,
    config: SolverConfig,
    default_time_limit_secs: u64,
    log_scale: fn(&S),
    qualified_candidate_trace_provenance: Option<QualifiedCandidateTraceRunProvenance>,
    build_search: BuildSearch,
) -> RuntimeBuildResult<S>
where
    S: PlanningSolution + Clone + Send + Sync + 'static,
    S::Score: Score + Copy + Ord + ParseableScore,
    C: ConstraintSet<S, S::Score>,
    V: Clone + Copy + PartialEq + Eq + Hash + Into<usize> + Send + Sync + fmt::Debug + 'static,
    DM: crate::heuristic::selector::nearby_list_change::CrossEntityDistanceMeter<S>
        + Clone
        + Send
        + Sync
        + fmt::Debug
        + 'static,
    IDM: crate::heuristic::selector::nearby_list_change::CrossEntityDistanceMeter<S>
        + Clone
        + Send
        + Sync
        + fmt::Debug
        + 'static,
    Declaration: Search<S, V, DM, IDM>,
    Declaration::Extensions: RuntimeExtensionRegistry<S, V, DM, IDM>,
    BuildSearch: FnOnce(&SolverConfig, SolutionDescriptor) -> RuntimeBuildResult<Declaration>,
{
    try_run_solver_with_candidate_trace_request(
        solution,
        constraints,
        descriptor,
        entity_count_by_descriptor,
        runtime,
        config,
        default_time_limit_secs,
        log_scale,
        qualified_candidate_trace_provenance,
        move |config, descriptor| {
            let declaration = build_search(config, descriptor.clone())?;
            let (context, extensions) = declaration.into_runtime_parts();
            let graph = compile_runtime_graph(config, RuntimeGraphInput::new(context, extensions))
                .map_err(|error| {
                    let message = error.to_string();
                    RuntimeBuildError::Compilation {
                        path: error.path,
                        message,
                    }
                })?;
            let executor = CompiledRuntimeExecutor::new(graph);
            CompiledRuntimePhaseRunner::try_new(&executor)
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn try_run_solver_with_candidate_trace_request<S, C, Runner, BuildRunner>(
    solution: S,
    constraints: C,
    descriptor: SolutionDescriptor,
    entity_count_by_descriptor: fn(&S, usize) -> usize,
    runtime: SolverRuntime<S>,
    config: SolverConfig,
    default_time_limit_secs: u64,
    log_scale: fn(&S),
    qualified_candidate_trace_provenance: Option<QualifiedCandidateTraceRunProvenance>,
    build_runner: BuildRunner,
) -> RuntimeBuildResult<S>
where
    S: PlanningSolution,
    S::Score: Score + ParseableScore,
    C: ConstraintSet<S, S::Score>,
    Runner: Phase<S, ScoreDirector<S, C>, ChannelProgressCallback<S>> + Send + std::fmt::Debug,
    BuildRunner: FnOnce(&SolverConfig, &SolutionDescriptor) -> RuntimeBuildResult<Runner>,
{
    log_scale(&solution);
    let director = ScoreDirector::with_descriptor(
        solution,
        constraints,
        descriptor.clone(),
        entity_count_by_descriptor,
    );

    let (termination, time_limit) = build_termination::<S, C>(&config, default_time_limit_secs);
    let execution_policy =
        configured_execution_policy::<S>(&config, default_time_limit_secs, time_limit);

    let callback = ChannelProgressCallback::new(runtime);

    let runner = match build_runner(&config, &descriptor) {
        Ok(runner) => runner,
        Err(error) => {
            runtime.emit_failed(error.to_string());
            return Err(error);
        }
    };
    let mut solver = Solver::new((runner,))
        .with_config(config.clone())
        .with_candidate_trace_execution_policy(execution_policy)
        .with_termination(termination)
        .with_runtime(runtime)
        .with_progress_callback(callback);
    if let Some(provenance) = qualified_candidate_trace_provenance {
        solver = solver.with_qualified_candidate_trace_run_provenance(provenance);
    }
    if let Some(time_limit) = time_limit {
        solver = solver.with_time_limit(time_limit);
    }

    let result = match catch_unwind(AssertUnwindSafe(|| {
        solver.with_terminate(runtime.cancel_flag()).solve(director)
    })) {
        Ok(result) => result,
        Err(payload) => match take_runtime_execution_failure(payload) {
            Ok(error) => {
                runtime.emit_failed(error.to_string());
                return Err(error);
            }
            Err(payload) => resume_unwind(payload),
        },
    };

    let crate::solver::SolveResult {
        solution,
        current_score,
        best_score: final_score,
        terminal_reason,
        stats,
    } = result;
    let final_telemetry = stats.snapshot();
    let final_move_speed = whole_units_per_second(stats.moves_evaluated, stats.elapsed());
    match terminal_reason {
        SolverTerminalReason::Completed | SolverTerminalReason::TerminatedByConfig => {
            runtime.emit_completed(
                solution.clone(),
                current_score,
                final_score,
                final_telemetry,
                terminal_reason,
            );
        }
        SolverTerminalReason::Cancelled => {
            runtime.emit_cancelled(current_score, Some(final_score), final_telemetry);
        }
        SolverTerminalReason::Failed => {
            let error = RuntimeBuildError::Execution {
                phase_index: 0,
                message: "configured solver reported a failed terminal state".to_string(),
            };
            runtime.emit_failed(error.to_string());
            return Err(error);
        }
    }

    info!(
        event = "solve_end",
        score = %final_score,
        steps = stats.step_count,
        moves_generated = stats.moves_generated,
        moves_evaluated = stats.moves_evaluated,
        moves_accepted = stats.moves_accepted,
        score_calculations = stats.score_calculations,
        generation_time = %format_duration(stats.generation_time()),
        evaluation_time = %format_duration(stats.evaluation_time()),
        moves_speed = final_move_speed,
        acceptance_rate = format!("{:.1}%", stats.acceptance_rate() * 100.0),
    );
    Ok(solution)
}

#[cfg(test)]
mod tests;
