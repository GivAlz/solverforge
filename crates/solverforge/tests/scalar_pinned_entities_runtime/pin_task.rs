use solverforge::prelude::*;

use super::PinPlan;

#[planning_entity]
pub struct PinTask {
    #[planning_id]
    pub id: usize,
    pub target: usize,
    #[planning_pin]
    pub pinned: bool,

    #[planning_variable(
        value_range_provider = "values",
        allows_unassigned = true,
        nearby_value_candidates = "all_values",
        nearby_entity_candidates = "all_tasks",
        nearby_value_distance_meter = "value_distance",
        nearby_entity_distance_meter = "task_distance"
    )]
    pub value_idx: Option<usize>,
}

pub(super) fn value_distance(_plan: &PinPlan, task: &PinTask, value: usize) -> f64 {
    task.target.abs_diff(value) as f64
}

pub(super) fn task_distance(_plan: &PinPlan, left: &PinTask, right: &PinTask) -> f64 {
    left.id.abs_diff(right.id) as f64
}

pub(super) fn all_values(plan: &PinPlan, _entity_index: usize, _variable_index: usize) -> &[usize] {
    &plan.value_indices
}

pub(super) fn all_tasks(plan: &PinPlan, _entity_index: usize, _variable_index: usize) -> &[usize] {
    &plan.task_indices
}
