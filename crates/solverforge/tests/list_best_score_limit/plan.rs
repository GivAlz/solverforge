use solverforge::prelude::*;
use solverforge::stream::ConstraintFactory;
use solverforge::SolverConfig;

use crate::{Route, Stop};

const EXPLICIT_PHASES: &str = r#"
[[phases]]
type = "construction_heuristic"
construction_heuristic_type = "list_cheapest_insertion"

[[phases]]
type = "local_search"
"#;

/* Unassigned stops are not penalized: the structural completion gate already
requires every stop to be routed. The empty starting solution therefore
scores `0hard/0soft`, which meets the configured best score limit. */
#[planning_solution(
    constraints = "constraints",
    config = "solver_config_for_plan",
    solver_toml = "../fixtures/list_best_score_limit_solver.toml"
)]
pub struct Plan {
    #[planning_entity_collection]
    pub routes: Vec<Route>,

    #[problem_fact_collection]
    pub stops: Vec<Stop>,

    #[planning_score]
    pub score: Option<HardSoftScore>,

    pub explicit_phases: bool,
}

fn constraints() -> impl ConstraintSet<Plan, HardSoftScore> {
    (ConstraintFactory::<Plan, HardSoftScore>::new()
        .for_each(Plan::routes())
        .penalize(|route: &Route| HardSoftScore::of_soft((route.visits.len() as i64).pow(2)))
        .named("balanced routes"),)
}

fn solver_config_for_plan(plan: &Plan, mut config: SolverConfig) -> SolverConfig {
    if plan.explicit_phases {
        config.phases = SolverConfig::from_toml_str(EXPLICIT_PHASES)
            .expect("explicit phases should parse")
            .phases;
    }
    config
}
