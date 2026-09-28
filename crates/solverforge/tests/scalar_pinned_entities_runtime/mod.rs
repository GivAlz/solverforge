solverforge::planning_model! {
    root = "crates/solverforge/tests/scalar_pinned_entities_runtime";

    mod pin_plan;
    mod pin_task;
    mod pin_value;

    pub use pin_plan::PinPlan;
    pub use pin_task::PinTask;
    pub use pin_value::PinValue;
}
