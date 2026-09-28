use std::fmt::Debug;

use solverforge_core::domain::PlanningSolution;
use solverforge_core::score::Score;
use solverforge_scoring::Director;

use crate::heuristic::selector::nearby_list_change::CrossEntityDistanceMeter;
use crate::runtime_build_error::RuntimeBuildError;
use crate::scope::{ProgressCallback, SolverScope};

use super::super::{RuntimeInstantiationError, RuntimeInstantiationErrorKind};
use super::CompiledRuntimePhaseRunner;

/// A top-level phase that records a reached execution failure instead of
/// unwinding. The configured entrypoint takes it after the solver returns and
/// reports it through the `Failed` lifecycle.
pub(crate) trait ExecutionFailureSource {
    fn take_execution_failure(&mut self) -> Option<RuntimeBuildError>;
}

impl<S, V, DM, IDM, Extension> ExecutionFailureSource
    for CompiledRuntimePhaseRunner<S, V, DM, IDM, Extension>
where
    S: PlanningSolution + Clone + Send + Sync + 'static,
    S::Score: Score,
    V: Clone + PartialEq + Send + Sync + Debug + 'static,
    DM: Clone + Send + Sync + Debug + CrossEntityDistanceMeter<S> + 'static,
    IDM: Clone + Send + Sync + Debug + CrossEntityDistanceMeter<S> + 'static,
{
    fn take_execution_failure(&mut self) -> Option<RuntimeBuildError> {
        self.failure.take()
    }
}

impl<S, V, DM, IDM, Extension> CompiledRuntimePhaseRunner<S, V, DM, IDM, Extension>
where
    S: PlanningSolution + Clone + Send + Sync + 'static,
    S::Score: Score,
    V: Clone + PartialEq + Send + Sync + Debug + 'static,
    DM: Clone + Send + Sync + Debug + CrossEntityDistanceMeter<S> + 'static,
    IDM: Clone + Send + Sync + Debug + CrossEntityDistanceMeter<S> + 'static,
{
    /// Ends the solve as `Failed` and retains the error for the entrypoint.
    /// No best solution is published after this point.
    pub(super) fn record_failure<D, ProgressCb>(
        &mut self,
        error: RuntimeBuildError,
        solver_scope: &mut SolverScope<'_, S, D, ProgressCb>,
    ) where
        D: Director<S>,
        ProgressCb: ProgressCallback<S>,
    {
        solver_scope.mark_failed();
        self.failure = Some(error);
    }
}

pub(super) fn map_preparation_error(error: RuntimeInstantiationError) -> RuntimeBuildError {
    match error.kind {
        RuntimeInstantiationErrorKind::SourceBinding { .. }
        | RuntimeInstantiationErrorKind::SourceRefresh { .. } => RuntimeBuildError::Execution {
            phase_index: error.phase_index,
            message: error.to_string(),
        },
        _ => RuntimeBuildError::Preparation {
            phase_index: error.phase_index,
            message: error.to_string(),
        },
    }
}

pub(super) fn execution_error(error: RuntimeInstantiationError) -> RuntimeBuildError {
    RuntimeBuildError::Execution {
        phase_index: error.phase_index,
        message: error.to_string(),
    }
}
