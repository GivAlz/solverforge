use solverforge::prelude::*;
use solverforge::stream::ConstraintFactory;
use solverforge::SolverConfig;

use super::{PinTask, PinValue};

#[planning_solution(constraints = "constraints", config = "solver_config_for_plan")]
pub struct PinPlan {
    #[problem_fact_collection]
    pub values: Vec<PinValue>,

    #[planning_entity_collection]
    pub tasks: Vec<PinTask>,

    #[planning_score]
    pub score: Option<HardSoftScore>,

    pub solver_toml: String,
    pub value_indices: Vec<usize>,
    pub task_indices: Vec<usize>,
}

fn constraints() -> impl ConstraintSet<PinPlan, HardSoftScore> {
    let unassigned = ConstraintFactory::<PinPlan, HardSoftScore>::new()
        .for_each(PinPlan::tasks())
        .unassigned()
        .penalize(HardSoftScore::ONE_HARD)
        .named("Unassigned");
    let distance = ConstraintFactory::<PinPlan, HardSoftScore>::new()
        .for_each(PinPlan::tasks())
        .penalize(|task: &PinTask| {
            HardSoftScore::of_soft(
                task.value_idx
                    .map_or(0, |value| value.abs_diff(task.target)) as i64,
            )
        })
        .named("Distance to target");
    (unassigned, distance)
}

fn solver_config_for_plan(plan: &PinPlan, _config: SolverConfig) -> SolverConfig {
    SolverConfig::from_toml_str(&plan.solver_toml).expect("test solver TOML parses")
}
