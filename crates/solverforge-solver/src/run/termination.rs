/* Config-driven top-level termination for configured solves. */

use std::fmt;
use std::marker::PhantomData;
use std::time::Duration;

use solverforge_config::{SolverConfig, TerminationConfig};
use solverforge_core::domain::PlanningSolution;
use solverforge_core::score::{ParseableScore, Score};
use solverforge_scoring::{ConstraintSet, Director, ScoreDirector};

use crate::scope::{ProgressCallback, SolverScope};
use crate::stats::CandidateTraceExecutionPolicy;
use crate::termination::{
    BestScoreTermination, StepCountTermination, Termination, TimeTermination,
    UnimprovedStepCountTermination, UnimprovedTimeTermination,
};

/// Monomorphized top-level termination for config-driven solves.
///
/// Holds one optional child per `[termination]` criterion (time, best score,
/// step count, unimproved step count, unimproved time) and terminates as soon
/// as ANY configured child terminates. Every configured child also installs its
/// in-phase limit, so each criterion stops the active phase loop as well. With
/// no configured child it never terminates.
pub struct AnyTermination<S: PlanningSolution, D: Director<S>> {
    pub(super) time: Option<TimeTermination>,
    pub(super) best_score: Option<BestScoreTermination<S::Score>>,
    pub(super) step_count: Option<StepCountTermination>,
    pub(super) unimproved_step_count: Option<UnimprovedStepCountTermination<S>>,
    pub(super) unimproved_time: Option<UnimprovedTimeTermination<S>>,
    _phantom: PhantomData<fn() -> D>,
}

impl<S: PlanningSolution, D: Director<S>> fmt::Debug for AnyTermination<S, D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnyTermination")
            .field("time", &self.time)
            .field("best_score", &self.best_score)
            .field("step_count", &self.step_count)
            .field("unimproved_step_count", &self.unimproved_step_count)
            .field("unimproved_time", &self.unimproved_time)
            .finish()
    }
}

impl<S: PlanningSolution, D: Director<S>, ProgressCb: ProgressCallback<S>>
    Termination<S, D, ProgressCb> for AnyTermination<S, D>
where
    S::Score: Score,
{
    fn is_terminated(&self, solver_scope: &SolverScope<S, D, ProgressCb>) -> bool {
        self.time
            .as_ref()
            .is_some_and(|t| t.is_terminated(solver_scope))
            || self
                .best_score
                .as_ref()
                .is_some_and(|t| t.is_terminated(solver_scope))
            || self
                .step_count
                .as_ref()
                .is_some_and(|t| t.is_terminated(solver_scope))
            || self
                .unimproved_step_count
                .as_ref()
                .is_some_and(|t| t.is_terminated(solver_scope))
            || self
                .unimproved_time
                .as_ref()
                .is_some_and(|t| t.is_terminated(solver_scope))
    }

    fn install_inphase_limits(&self, solver_scope: &mut SolverScope<S, D, ProgressCb>) {
        if let Some(t) = &self.time {
            t.install_inphase_limits(solver_scope);
        }
        if let Some(t) = &self.best_score {
            t.install_inphase_limits(solver_scope);
        }
        if let Some(t) = &self.step_count {
            t.install_inphase_limits(solver_scope);
        }
        if let Some(t) = &self.unimproved_step_count {
            t.install_inphase_limits(solver_scope);
        }
        if let Some(t) = &self.unimproved_time {
            t.install_inphase_limits(solver_scope);
        }
    }
}

/// Parsed solver termination policy shared by runtime phase assembly and the
/// top-level termination builder.
///
/// Every configured criterion is kept; the solve stops when any one of them is
/// reached. A best-score target that does not parse as the solution's score
/// type is not a criterion. Keeping the parse here prevents phase assembly from
/// treating an empty or unparsable configuration as a finite solver boundary.
#[derive(Clone, Copy)]
pub(crate) struct ConfiguredTermination<Sc> {
    time_limit: Option<Duration>,
    best_score: Option<Sc>,
    step_count: Option<u64>,
    unimproved_step_count: Option<u64>,
    unimproved_time: Option<Duration>,
}

impl<Sc> ConfiguredTermination<Sc> {
    /// Whether any score/work criterion (anything but the time limit) is set.
    fn has_criterion(&self) -> bool {
        self.best_score.is_some()
            || self.step_count.is_some()
            || self.unimproved_step_count.is_some()
            || self.unimproved_time.is_some()
    }

    pub(crate) fn has_effective_limit(&self) -> bool {
        self.time_limit.is_some() || self.has_criterion()
    }
}

pub(crate) fn parse_configured_termination<S>(
    config: Option<&TerminationConfig>,
) -> ConfiguredTermination<S::Score>
where
    S: PlanningSolution,
    S::Score: ParseableScore,
{
    ConfiguredTermination {
        time_limit: config.and_then(TerminationConfig::time_limit),
        best_score: config
            .and_then(|config| config.best_score_limit.as_deref())
            .and_then(|score| S::Score::parse(score).ok()),
        step_count: config.and_then(|config| config.step_count_limit),
        unimproved_step_count: config.and_then(|config| config.unimproved_step_count_limit),
        unimproved_time: config.and_then(TerminationConfig::unimproved_time_limit),
    }
}

/// Builds a termination from config, returning both the termination and the time limit.
///
/// All configured criteria are combined with OR semantics. When a score/work
/// criterion is configured without a time limit, `default_secs` becomes the
/// time guard; with no criterion and no time limit the solve is unbounded.
pub fn build_termination<S, C>(
    config: &SolverConfig,
    default_secs: u64,
) -> (AnyTermination<S, ScoreDirector<S, C>>, Option<Duration>)
where
    S: PlanningSolution,
    S::Score: Score + ParseableScore,
    C: ConstraintSet<S, S::Score>,
{
    let configured = parse_configured_termination::<S>(config.termination.as_ref());
    let effective_time_limit = configured.time_limit.or_else(|| {
        configured
            .has_criterion()
            .then(|| Duration::from_secs(default_secs))
    });

    let termination = AnyTermination {
        time: effective_time_limit.map(TimeTermination::new),
        best_score: configured.best_score.map(BestScoreTermination::new),
        step_count: configured.step_count.map(StepCountTermination::new),
        unimproved_step_count: configured
            .unimproved_step_count
            .map(UnimprovedStepCountTermination::new),
        unimproved_time: configured
            .unimproved_time
            .map(UnimprovedTimeTermination::new),
        _phantom: PhantomData,
    };

    (termination, effective_time_limit)
}

/// Records the termination policy the configured runtime actually installed.
///
/// This deliberately derives its time guard from `build_termination`'s
/// returned effective limit rather than from the input TOML.  In particular,
/// a score/work criterion without an explicit time limit gets the configured
/// entrypoint's fallback guard, and that injected guard is material to both
/// bounded-work and fixed-budget comparisons.
pub(crate) fn configured_execution_policy<S>(
    config: &SolverConfig,
    default_secs: u64,
    effective_time_limit: Option<Duration>,
) -> CandidateTraceExecutionPolicy
where
    S: PlanningSolution,
    S::Score: ParseableScore + std::fmt::Display,
{
    let configured = parse_configured_termination::<S>(config.termination.as_ref());
    let configured_time_limit = configured.time_limit;
    let fallback_time_limit = Duration::from_secs(default_secs);

    let time_limit_source = match (configured_time_limit, effective_time_limit) {
        (Some(_), Some(_)) => "configured",
        (None, Some(_)) if configured.has_criterion() => "configured_entrypoint_fallback",
        (None, Some(_)) => "internal",
        (_, None) => "not_installed",
    };
    let mut attributes = vec![
        ("entrypoint".to_string(), "configured_runtime".to_string()),
        (
            "configured_time_limit_ns".to_string(),
            configured_time_limit.map_or_else(|| "none".to_string(), duration_nanos),
        ),
        (
            "configured_entrypoint_default_time_limit_ns".to_string(),
            duration_nanos(fallback_time_limit),
        ),
        (
            "effective_time_limit_ns".to_string(),
            effective_time_limit.map_or_else(|| "none".to_string(), duration_nanos),
        ),
        (
            "time_limit_source".to_string(),
            time_limit_source.to_string(),
        ),
    ];

    // Criteria in canonical order, each with its configured target.
    let mut criteria = Vec::new();
    if let Some(target) = configured.best_score {
        criteria.push("best_score");
        attributes.push(("best_score_target".to_string(), target.to_string()));
    }
    if let Some(limit) = configured.step_count {
        criteria.push("step_count");
        attributes.push(("step_count_target".to_string(), limit.to_string()));
    }
    if let Some(limit) = configured.unimproved_step_count {
        criteria.push("unimproved_step_count");
        attributes.push((
            "unimproved_step_count_target".to_string(),
            limit.to_string(),
        ));
    }
    if let Some(limit) = configured.unimproved_time {
        criteria.push("unimproved_time");
        attributes.push((
            "unimproved_time_target_ns".to_string(),
            duration_nanos(limit),
        ));
    }

    let composition = match (effective_time_limit.is_some(), criteria.is_empty()) {
        (_, false) => {
            let mut parts = Vec::with_capacity(criteria.len() + 1);
            if effective_time_limit.is_some() {
                parts.push("time");
            }
            parts.extend(criteria.iter().copied());
            parts.join("_or_")
        }
        (true, true) => "time_only".to_string(),
        (false, true) => "unbounded".to_string(),
    };
    let criteria = if criteria.is_empty() {
        "none".to_string()
    } else {
        criteria.join(",")
    };
    attributes.push(("criteria".to_string(), criteria));
    attributes.push(("termination_composition".to_string(), composition));

    CandidateTraceExecutionPolicy::known("solverforge.execution_policy", attributes)
}

fn duration_nanos(duration: Duration) -> String {
    duration.as_nanos().to_string()
}
