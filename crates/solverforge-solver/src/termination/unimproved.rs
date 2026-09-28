// Termination conditions based on lack of improvement.

use std::fmt::Debug;
use std::marker::PhantomData;
use std::time::Duration;

use solverforge_core::domain::PlanningSolution;
use solverforge_scoring::Director;

use super::Termination;
use crate::scope::ProgressCallback;
use crate::scope::SolverScope;

/// Terminates if no improvement occurs for a specified number of steps.
///
/// This is useful to avoid spending too much time when the solver has
/// plateaued and is unlikely to find better solutions. The count covers steps
/// completed since the best solution last improved. The limit is also
/// installed as an in-phase limit, so it stops search phases between steps;
/// construction work restarts the count instead of being cut short by it.
///
/// # Example
///
/// ```
/// use solverforge_solver::termination::UnimprovedStepCountTermination;
/// use solverforge_core::score::SoftScore;
/// use solverforge_core::domain::PlanningSolution;
///
/// #[derive(Clone)]
/// struct MySolution;
/// impl PlanningSolution for MySolution {
///     type Score = SoftScore;
///     fn score(&self) -> Option<Self::Score> { None }
///     fn set_score(&mut self, _: Option<Self::Score>) {}
/// }
///
/// // Terminate after 100 steps without improvement
/// let term = UnimprovedStepCountTermination::<MySolution>::new(100);
/// ```
pub struct UnimprovedStepCountTermination<S: PlanningSolution> {
    limit: u64,
    _phantom: PhantomData<fn() -> S>,
}

impl<S: PlanningSolution> Debug for UnimprovedStepCountTermination<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnimprovedStepCountTermination")
            .field("limit", &self.limit)
            .finish()
    }
}

impl<S: PlanningSolution> UnimprovedStepCountTermination<S> {
    pub fn new(limit: u64) -> Self {
        Self {
            limit,
            _phantom: PhantomData,
        }
    }
}

impl<S: PlanningSolution, D: Director<S>, BestCb: ProgressCallback<S>> Termination<S, D, BestCb>
    for UnimprovedStepCountTermination<S>
{
    fn is_terminated(&self, solver_scope: &SolverScope<S, D, BestCb>) -> bool {
        solver_scope.best_score().is_some() && solver_scope.unimproved_step_count() >= self.limit
    }

    fn install_inphase_limits(&self, solver_scope: &mut SolverScope<S, D, BestCb>) {
        solver_scope.install_inphase_unimproved_step_count_limit(self.limit);
    }
}

/// Terminates if no improvement occurs for a specified duration.
///
/// This is useful for time-boxed optimization where you want to ensure
/// progress is being made, but also allow more time if improvements are found.
/// The duration is pause-aware solver time since the best solution last
/// improved. Like [`UnimprovedStepCountTermination`], the limit is installed as
/// an in-phase limit checked between search steps, and construction work
/// restarts the window instead of being cut short by it.
///
/// # Example
///
/// ```
/// use std::time::Duration;
/// use solverforge_solver::termination::UnimprovedTimeTermination;
/// use solverforge_core::score::SoftScore;
/// use solverforge_core::domain::PlanningSolution;
///
/// #[derive(Clone)]
/// struct MySolution;
/// impl PlanningSolution for MySolution {
///     type Score = SoftScore;
///     fn score(&self) -> Option<Self::Score> { None }
///     fn set_score(&mut self, _: Option<Self::Score>) {}
/// }
///
/// // Terminate after 5 seconds without improvement
/// let term = UnimprovedTimeTermination::<MySolution>::seconds(5);
/// ```
pub struct UnimprovedTimeTermination<S: PlanningSolution> {
    limit: Duration,
    _phantom: PhantomData<fn() -> S>,
}

impl<S: PlanningSolution> Debug for UnimprovedTimeTermination<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnimprovedTimeTermination")
            .field("limit", &self.limit)
            .finish()
    }
}

impl<S: PlanningSolution> UnimprovedTimeTermination<S> {
    pub fn new(limit: Duration) -> Self {
        Self {
            limit,
            _phantom: PhantomData,
        }
    }

    pub fn millis(ms: u64) -> Self {
        Self::new(Duration::from_millis(ms))
    }

    pub fn seconds(secs: u64) -> Self {
        Self::new(Duration::from_secs(secs))
    }
}

impl<S: PlanningSolution, D: Director<S>, BestCb: ProgressCallback<S>> Termination<S, D, BestCb>
    for UnimprovedTimeTermination<S>
{
    fn is_terminated(&self, solver_scope: &SolverScope<S, D, BestCb>) -> bool {
        solver_scope.best_score().is_some() && solver_scope.unimproved_time() >= self.limit
    }

    fn install_inphase_limits(&self, solver_scope: &mut SolverScope<S, D, BestCb>) {
        solver_scope.install_inphase_unimproved_time_limit(self.limit);
    }
}
