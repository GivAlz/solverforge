use solverforge::prelude::*;

/* Stock CVRP pieces spelled out explicitly so Clarke-Wright construction can
use the opt-in capacity-aware savings bundle instead of the relaxed stock
`savings_hooks` that `domain = "cvrp"` wires. */
#[planning_entity]
pub struct Route {
    #[planning_id]
    pub id: usize,

    #[planning_list_variable(
        element_collection = "customer_values",
        solution_trait = "::solverforge::cvrp::VrpSolution",
        distance_meter = "::solverforge::cvrp::MatrixDistanceMeter",
        intra_distance_meter = "::solverforge::cvrp::MatrixIntraDistanceMeter",
        route_hooks = "::solverforge::cvrp::route_hooks",
        savings_hooks = "::solverforge::cvrp::capacity_savings_hooks",
        savings_metric_class_fn = "::solverforge::cvrp::savings_metric_class"
    )]
    pub visits: Vec<usize>,
}
