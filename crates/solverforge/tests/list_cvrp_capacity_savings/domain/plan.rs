use std::sync::Arc;

use solverforge::cvrp::{ProblemData, VrpSolution};
use solverforge::prelude::*;
use solverforge::stream::ConstraintFactory;

use super::Route;

#[planning_solution(
    constraints = "define_constraints",
    solver_toml = "../../fixtures/list_cvrp_capacity_savings_solver.toml"
)]
pub struct Plan {
    #[planning_list_element_collection(owner = "routes")]
    pub customer_values: Vec<usize>,

    #[planning_entity_collection]
    pub routes: Vec<Route>,

    #[planning_score]
    pub score: Option<HardSoftScore>,

    pub data: Arc<ProblemData>,
}

impl VrpSolution for Plan {
    fn vehicle_data_ptr(&self, _entity_idx: usize) -> *const ProblemData {
        Arc::as_ptr(&self.data)
    }

    fn vehicle_visits(&self, entity_idx: usize) -> &[usize] {
        &self.routes[entity_idx].visits
    }

    fn vehicle_visits_mut(&mut self, entity_idx: usize) -> &mut Vec<usize> {
        &mut self.routes[entity_idx].visits
    }

    fn vehicle_count(&self) -> usize {
        self.routes.len()
    }
}

pub fn route_load(data: &ProblemData, visits: &[usize]) -> i64 {
    visits.iter().map(|&visit| i64::from(data.demands[visit])).sum()
}

fn define_constraints() -> impl ConstraintSet<Plan, HardSoftScore> {
    (ConstraintFactory::<Plan, HardSoftScore>::new()
        .for_each(Plan::routes())
        .penalize(|route: &Route| HardSoftScore::of_soft(route.visits.len() as i64))
        .named("visits"),)
}

/// Customers on a ring around the depot. Every pair of customers has a
/// positive saving, so capacity is the only reason to open another route.
pub fn build_plan(customer_count: usize, demand: i32, capacity: i64, vehicles: usize) -> Plan {
    let points: Vec<(f64, f64)> = std::iter::once((0.0, 0.0))
        .chain((0..customer_count).map(|i| {
            let angle = std::f64::consts::TAU * i as f64 / customer_count as f64;
            (100.0 * angle.cos(), 100.0 * angle.sin())
        }))
        .collect();
    let distance_matrix: Vec<Vec<i64>> = points
        .iter()
        .map(|a| {
            points
                .iter()
                .map(|b| (a.0 - b.0).hypot(a.1 - b.1).round() as i64)
                .collect()
        })
        .collect();
    let dimension = points.len();
    let data = Arc::new(ProblemData {
        capacity,
        depot: 0,
        demands: std::iter::once(0)
            .chain(std::iter::repeat_n(demand, customer_count))
            .collect(),
        travel_times: distance_matrix.clone(),
        distance_matrix,
        time_windows: vec![(0, i64::MAX / 4); dimension],
        service_durations: vec![0; dimension],
        vehicle_departure_time: 0,
    });

    Plan {
        customer_values: (1..=customer_count).collect(),
        routes: (0..vehicles)
            .map(|id| Route {
                id,
                visits: Vec::new(),
            })
            .collect(),
        score: None,
        data,
    }
}
