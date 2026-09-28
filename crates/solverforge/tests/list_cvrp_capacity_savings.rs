use solverforge::{SolverEvent, SolverManager};

#[path = "list_cvrp_capacity_savings/domain/mod.rs"]
mod domain;

use domain::{build_plan, route_load, Plan};

#[test]
fn capacity_savings_hooks_keep_clarke_wright_routes_within_capacity() {
    static MANAGER: SolverManager<Plan> = SolverManager::new();

    let (customers, demand, capacity, vehicles) = (12, 3, 10, 6);
    let plan = build_plan(customers, demand, capacity, vehicles);
    let data = plan.data.clone();
    let (job_id, mut receiver) = MANAGER.solve(plan).expect("solve should start");

    let completed = loop {
        match receiver
            .blocking_recv()
            .expect("event stream should reach a terminal event")
        {
            SolverEvent::Completed { solution, .. } => break solution,
            SolverEvent::Failed { error, .. } => panic!("solve unexpectedly failed: {error}"),
            SolverEvent::Cancelled { metadata } => {
                panic!(
                    "solve was unexpectedly cancelled: {:?}",
                    metadata.terminal_reason
                )
            }
            SolverEvent::BestSolution { .. }
            | SolverEvent::Progress { .. }
            | SolverEvent::PauseRequested { .. }
            | SolverEvent::Paused { .. }
            | SolverEvent::Resumed { .. } => {}
        }
    };

    MANAGER
        .delete(job_id)
        .expect("completed job should delete cleanly");

    let mut assigned: Vec<usize> = completed
        .routes
        .iter()
        .flat_map(|route| route.visits.iter().copied())
        .collect();
    assigned.sort_unstable();
    assert_eq!(assigned, (1..=customers).collect::<Vec<_>>());

    let loads: Vec<i64> = completed
        .routes
        .iter()
        .filter(|route| !route.visits.is_empty())
        .map(|route| route_load(&data, &route.visits))
        .collect();
    assert!(
        loads.iter().all(|&load| load <= capacity),
        "every constructed route must respect capacity, got loads {loads:?}"
    );
    let total_demand = i64::from(demand) * customers as i64;
    let min_routes = ((total_demand + capacity - 1) / capacity) as usize;
    assert!(
        loads.len() >= min_routes,
        "expected at least {min_routes} routes, got loads {loads:?}"
    );
}
