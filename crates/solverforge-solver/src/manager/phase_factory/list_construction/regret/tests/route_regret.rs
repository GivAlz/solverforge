//! Regret-2 semantics: regret is measured across owners (routes), not across
//! insertion positions inside one owner.

use super::*;

/// Every owner holds at most two elements without a hard penalty; each
/// element pays `cost(element, owner)` wherever it sits inside that owner, so
/// all positions inside one owner tie for every element.
fn two_slot_score(plan: &Plan, cost: fn(usize, usize) -> i64) -> HardSoftScore {
    let mut hard = 0;
    let mut soft = 0;
    for (owner, route) in plan.routes.iter().enumerate() {
        hard -= (route.len() as i64 - 2).max(0);
        soft -= route
            .iter()
            .map(|&element| cost(element, owner))
            .sum::<i64>();
    }
    HardSoftScore::of(hard, soft)
}

fn contested_cost(element: usize, owner: usize) -> i64 {
    match (element, owner) {
        (2, 0) => 1,
        (2, 1) => 10,
        (3, 1) => 2,
        _ => 0,
    }
}

fn contested_score(plan: &Plan) -> HardSoftScore {
    two_slot_score(plan, contested_cost)
}

#[test]
fn regret_compares_best_insertion_per_owner_not_adjacent_positions() {
    // Owners 0 and 1 each have one free slot. Element 2 loses 9 if it misses
    // owner 0, element 3 only loses 2. Regret-2 across owners inserts element
    // 2 first; a positional regret sees two tied positions inside owner 0 for
    // both elements (regret 0) and would greedily place element 3 there.
    let plan = Plan {
        elements: vec![0, 1, 2, 3],
        routes: vec![vec![0], vec![1]],
        score: None,
    };
    let mut scope = SolverScope::new(director(plan, contested_score));
    scope.start_solving();

    phase().solve(&mut scope);

    // Ties between equal positions keep the first (lowest) owner and position.
    assert_eq!(
        scope.working_solution().routes,
        vec![vec![2, 0], vec![3, 1]]
    );
    assert_eq!(
        contested_score(scope.working_solution()),
        HardSoftScore::of(0, -3)
    );
}

fn restricted_cost(element: usize, owner: usize) -> i64 {
    match (element, owner) {
        (2, 1) => 100,
        _ => 0,
    }
}

fn restricted_score(plan: &Plan) -> HardSoftScore {
    two_slot_score(plan, restricted_cost)
}

fn element_three_fixed_to_owner_zero(_: &Plan, element: &usize) -> Option<usize> {
    (*element == 3).then_some(0)
}

#[test]
fn element_with_a_single_feasible_owner_is_inserted_before_finite_regret() {
    // Element 3 may only use owner 0, so it has fewer than two feasible
    // owners and its regret-2 is unbounded. It must win over element 2, whose
    // finite regret across owners is 100, even though both have the same best
    // insertion score and element 2 comes first in construction order.
    let plan = Plan {
        elements: vec![0, 1, 2, 3],
        routes: vec![vec![0], vec![1]],
        score: None,
    };
    let mut scope = SolverScope::new(director(plan, restricted_score));
    scope.start_solving();
    let mut phase = phase().with_element_owner_fn(Some(element_three_fixed_to_owner_zero));

    phase.solve(&mut scope);

    assert_eq!(
        scope.working_solution().routes,
        vec![vec![3, 0], vec![2, 1]]
    );
    assert_eq!(
        restricted_score(scope.working_solution()),
        HardSoftScore::of(0, -100)
    );
}
