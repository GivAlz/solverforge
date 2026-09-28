use crate::{ProblemData, VrpSolution};

#[inline]
pub(crate) fn problem_data_for_entity<S: VrpSolution>(
    plan: &S,
    entity_idx: usize,
) -> Option<&ProblemData> {
    if entity_idx >= plan.vehicle_count() {
        return None;
    }
    let ptr = plan.vehicle_data_ptr(entity_idx);
    assert!(
        !ptr.is_null(),
        "VrpSolution::vehicle_data_ptr({entity_idx}) returned null for a non-empty fleet"
    );
    // SAFETY: VrpSolution implementors guarantee valid pointers for the duration
    // of the solve call; null for a non-empty fleet is rejected above.
    unsafe { ptr.as_ref() }
}

#[inline]
fn optional_problem_data_for_entity<S: VrpSolution>(
    plan: &S,
    entity_idx: usize,
) -> Option<&ProblemData> {
    if entity_idx >= plan.vehicle_count() {
        return None;
    }
    let ptr = plan.vehicle_data_ptr(entity_idx);
    if ptr.is_null() {
        return None;
    }
    // SAFETY: VrpSolution implementors guarantee valid pointers for the duration
    // of the solve call; this path treats null as non-admissible instead of
    // panicking because it is used by feasibility gates.
    unsafe { ptr.as_ref() }
}

pub fn depot_for_entity<S: VrpSolution>(plan: &S, entity_idx: usize) -> usize {
    problem_data_for_entity(plan, entity_idx).map_or(0, |data| data.depot)
}

/// Construction metric class for the route owner.
///
/// Owners that share the same `ProblemData` pointer share depot and distance
/// behavior, so Clarke-Wright can compute their savings rows once.
pub fn savings_metric_class<S: VrpSolution>(plan: &S, entity_idx: usize) -> usize {
    if entity_idx >= plan.vehicle_count() {
        return entity_idx;
    }

    let ptr = plan.vehicle_data_ptr(entity_idx);
    assert!(
        !ptr.is_null(),
        "VrpSolution::vehicle_data_ptr({entity_idx}) returned null for a non-empty fleet"
    );
    ptr as usize
}

/// Depot token used by Clarke-Wright construction.
pub fn savings_depot_for_entity<S: VrpSolution>(plan: &S, entity_idx: usize) -> usize {
    depot_for_entity(plan, entity_idx)
}

/// Construction distance used by Clarke-Wright for models that share CVRP route data.
pub fn savings_distance<S: VrpSolution>(
    plan: &S,
    entity_idx: usize,
    from: usize,
    to: usize,
) -> i64 {
    route_distance(plan, entity_idx, from, to)
}

/// Construction admissibility used by Clarke-Wright for stock CVRP route data.
///
/// This intentionally rejects only routes that cannot be evaluated safely.
/// Capacity and time-window violations remain scoreable during construction.
pub fn savings_feasible<S: VrpSolution>(plan: &S, entity_idx: usize, route: &[usize]) -> bool {
    if route.is_empty() {
        return true;
    }
    let Some(data) = optional_problem_data_for_entity(plan, entity_idx) else {
        return false;
    };
    route_is_structurally_valid(route, data)
}

/// Capacity-aware construction admissibility for Clarke-Wright.
///
/// Rejects everything [`savings_feasible`] rejects, plus any route with more
/// than one visit whose total demand exceeds the owner's capacity, so savings
/// merges stay capacity-bounded. A single visit is always admitted when it is
/// structurally valid: it cannot be split further, so its own over-capacity
/// demand stays scoreable. Time-window violations stay scoreable.
///
/// Unlike the stock [`savings_feasible`] gate, this gate can leave Clarke-Wright
/// without any complete admissible assignment when the fleet cannot hold all
/// demand; the solve then reports mandatory list work as incomplete.
pub fn savings_capacity_feasible<S: VrpSolution>(
    plan: &S,
    entity_idx: usize,
    route: &[usize],
) -> bool {
    if route.is_empty() {
        return true;
    }
    let Some(data) = optional_problem_data_for_entity(plan, entity_idx) else {
        return false;
    };
    route_is_structurally_valid(route, data)
        && (route.len() == 1 || route_is_capacity_feasible(route, data))
}

/// Distance between two element indices for the route owner.
pub fn route_distance<S: VrpSolution>(plan: &S, entity_idx: usize, from: usize, to: usize) -> i64 {
    problem_data_for_entity(plan, entity_idx).map_or(0, |data| data.distance_cost(from, to))
}

/// Replaces the current route for entity `entity_idx`.
///
/// Callers must pass a valid `entity_idx` for the current solution.
pub fn replace_route<S: VrpSolution>(plan: &mut S, entity_idx: usize, route: Vec<usize>) {
    *plan.vehicle_visits_mut(entity_idx) = route;
}

/// Returns a cloned snapshot of the route for entity `entity_idx`.
///
/// Callers must pass a valid `entity_idx` for the current solution.
pub fn get_route<S: VrpSolution>(plan: &S, entity_idx: usize) -> Vec<usize> {
    plan.vehicle_visits(entity_idx).to_vec()
}

/// Returns `true` if the route satisfies stock CVRP route-local feasibility.
pub fn route_feasible<S: VrpSolution>(plan: &S, entity_idx: usize, route: &[usize]) -> bool {
    if route.is_empty() {
        return true;
    }
    let Some(data) = optional_problem_data_for_entity(plan, entity_idx) else {
        return false;
    };
    route_is_structurally_valid(route, data)
        && route_is_capacity_feasible(route, data)
        && route_is_time_feasible(route, data)
}

/// Route-local hook bundle used by `#[planning_list_variable(domain = "cvrp")]`.
pub mod route_hooks {
    pub use super::depot_for_entity as depot;
    pub use super::get_route as get;
    pub use super::replace_route as set;
    pub use super::route_distance as distance;
    pub use super::route_feasible as feasible;
}

/// Clarke-Wright savings hook bundle used by `#[planning_list_variable(domain = "cvrp")]`.
///
/// Advanced users can wire this explicitly when custom macro attributes should
/// still share stock CVRP construction data.
pub mod savings_hooks {
    pub use super::savings_depot_for_entity as depot;
    pub use super::savings_distance as distance;
    pub use super::savings_feasible as feasible;
}

/// Capacity-aware Clarke-Wright savings hook bundle.
///
/// Opt-in alternative to [`savings_hooks`] for explicit macro declarations:
/// shares its depot and distance, but gates construction with
/// [`savings_capacity_feasible`].
pub mod capacity_savings_hooks {
    pub use super::savings_capacity_feasible as feasible;
    pub use super::savings_depot_for_entity as depot;
    pub use super::savings_distance as distance;
}

fn route_is_structurally_valid(route: &[usize], data: &ProblemData) -> bool {
    let Some(max_visit) = route.iter().copied().max() else {
        return true;
    };
    let max_node = max_visit.max(data.depot);

    if max_visit >= data.demands.len()
        || max_visit >= data.time_windows.len()
        || max_visit >= data.service_durations.len()
        || max_node >= data.distance_matrix.len()
        || max_node >= data.travel_times.len()
    {
        return false;
    }

    route
        .iter()
        .chain(std::iter::once(&data.depot))
        .all(|&node| {
            data.distance_matrix
                .get(node)
                .is_some_and(|row| row.len() > max_node)
                && data
                    .travel_times
                    .get(node)
                    .is_some_and(|row| row.len() > max_node)
        })
}

fn route_is_capacity_feasible(route: &[usize], data: &ProblemData) -> bool {
    let mut total = 0_i64;
    for &visit in route {
        let demand = i64::from(data.demands[visit]);
        total = match total.checked_add(demand) {
            Some(total) => total,
            None => return false,
        };
    }
    total <= data.capacity
}

fn route_is_time_feasible(route: &[usize], data: &ProblemData) -> bool {
    let mut current_time = data.vehicle_departure_time;
    let mut previous = data.depot;

    for &visit in route {
        let Some(travel_time) = data.travel_time(previous, visit) else {
            return false;
        };
        current_time = match current_time.checked_add(travel_time) {
            Some(current_time) => current_time,
            None => return false,
        };

        let (min_start, max_end) = data.time_windows[visit];
        if current_time < min_start {
            current_time = min_start;
        }

        let service_duration = data.service_durations[visit];
        if service_duration < 0 {
            return false;
        }
        current_time = match current_time.checked_add(service_duration) {
            Some(current_time) => current_time,
            None => return false,
        };
        if current_time > max_end {
            return false;
        }

        previous = visit;
    }

    let Some(return_time) = data.travel_time(previous, data.depot) else {
        return false;
    };
    current_time.checked_add(return_time).is_some()
}
