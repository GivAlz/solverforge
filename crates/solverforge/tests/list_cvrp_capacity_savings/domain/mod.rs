solverforge::planning_model! {
    root = "crates/solverforge/tests/list_cvrp_capacity_savings/domain";

    mod plan;
    mod route;

    pub use plan::{build_plan, route_load, Plan};
    pub use route::Route;
}
