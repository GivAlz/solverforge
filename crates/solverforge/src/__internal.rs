use std::io::ErrorKind;
use std::path::Path;

use solverforge_config::{ConfigError, SolverConfig as FacadeSolverConfig};

// Initializes console output if the feature is enabled.
#[inline]
pub fn init_console() {
    #[cfg(feature = "console")]
    solverforge_console::init();
}

// Loads the runtime `solver.toml` from the working directory. Only a missing
// file falls back to `SolverConfig::default()`; an unreadable or invalid file
// panics so the solve fails instead of silently running with defaults.
pub fn load_solver_config() -> FacadeSolverConfig {
    let path = Path::new("solver.toml");
    match load_solver_config_or_default(path) {
        Ok(config) => config,
        Err(error) => panic!("invalid {}: {error}", path.display()),
    }
}

fn load_solver_config_or_default(path: &Path) -> Result<FacadeSolverConfig, ConfigError> {
    match FacadeSolverConfig::load(path) {
        Err(ConfigError::Io(error)) if error.kind() == ErrorKind::NotFound => {
            Ok(FacadeSolverConfig::default())
        }
        result => result,
    }
}

// Derive macros
pub use solverforge_macros::{PlanningEntityImpl, PlanningSolutionImpl, ProblemFactImpl};

// Domain types
pub use solverforge_core::domain::{
    EntityClassId, EntityCollectionExtractor, EntityDescriptor, PlanningEntity, PlanningId,
    PlanningSolution, ProblemFact, ProblemFactClassId, ProblemFactDescriptor, ShadowVariableKind,
    SolutionDescriptor, ValueRangeType, VariableDescriptor, VariableId,
};

// Scoring
pub use solverforge_scoring::api::{
    ConstraintSet, ConstraintSetChain, ConstraintSetSource, OrderedConstraintSetChain,
};
pub use solverforge_scoring::{Director, ScoreDirector, SolvableSolution};

// Solver infrastructure
pub use solverforge_solver::builder::{
    bind_scalar_groups, local_search, usize_element_source_key, CustomSearchPhase,
    ListVariableSlot, RuntimeModel, ScalarGroupBinding, ScalarGroupMemberBinding,
    ScalarVariableSlot, Search, SearchContext, ValueSource, VariableSlot,
};
pub use solverforge_solver::heuristic::selector::{
    DefaultCrossEntityDistanceMeter, DefaultDistanceMeter, FromSolutionEntitySelector,
};
pub use solverforge_solver::manager::{
    KOptPhaseBuilder, ListConstructionPhaseBuilder, PhaseFactory, SolverFactory,
};
pub use solverforge_solver::model_support::PlanningModelSupport;
pub use solverforge_solver::runtime::{ListVariableEntity, ListVariableMetadata};
pub use solverforge_solver::scope::{ProgressCallback, SolverScope};
pub use solverforge_solver::{
    log_solve_start, try_run_solver_with_config_and_search, RuntimeBuildResult, SolverEvent,
    SolverRuntime, SolverTelemetry,
};

// Config
pub use solverforge_config::{PhaseConfig, SolverConfig};

// Stream types needed for macro-generated source methods
pub use solverforge_core::{ConstraintRef, ImpactType};
pub use solverforge_scoring::stream::filter::{AndUniFilter, FnUniFilter, TrueFilter, UniFilter};
pub use solverforge_scoring::stream::ConstraintWeight;
pub use solverforge_scoring::stream::{
    source, ChangeSource, CollectionExtract, SourceExtract, UnassignedEntity, UniConstraintBuilder,
    UniConstraintStream,
};
