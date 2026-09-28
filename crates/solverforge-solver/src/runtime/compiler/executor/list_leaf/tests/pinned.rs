//! Pinned list owners never reach a compiled list cursor.

use solverforge_core::domain::SolutionDescriptor;
use solverforge_scoring::ScoreDirector;

use crate::heuristic::r#move::Move;
use crate::heuristic::selector::move_selector::{MoveCursor, MoveStreamContext};

use super::super::super::super::graph::ListLeafKind;
use super::support::{
    descriptor, dynamic_slot_for, initial_plan, selector, static_slot_for, ListPlan,
    PositionMetric, Slot, ALL_KINDS,
};

type RuntimeMove = super::super::RuntimeListMove<ListPlan, usize, PositionMetric, PositionMetric>;

/// The middle route is pinned; it is recognized by its first visit.
const PINNED_ROUTE: usize = 1;
const PINNED_ANCHOR: usize = 3;

fn routes() -> Vec<Vec<usize>> {
    vec![vec![0, 1, 2], vec![3, 4, 5], vec![6, 7]]
}

fn pinned_descriptor() -> SolutionDescriptor {
    let mut descriptor = descriptor();
    let vehicle = descriptor
        .entity_descriptors
        .remove(0)
        .with_pin_predicate(|entity| {
            entity
                .downcast_ref::<Vec<usize>>()
                .expect("vehicle entity is a route")
                .first()
                == Some(&PINNED_ANCHOR)
        });
    descriptor.entity_descriptors.push(vehicle);
    descriptor
}

fn director_with(descriptor: SolutionDescriptor) -> ScoreDirector<ListPlan, ()> {
    let mut plan = initial_plan();
    plan.routes = routes();
    ScoreDirector::simple(plan, descriptor, |plan, descriptor_index| {
        usize::from(descriptor_index == 0) * plan.routes.len()
    })
}

fn enumerate(
    kind: ListLeafKind,
    slot: Slot,
    director: &ScoreDirector<ListPlan, ()>,
) -> Vec<RuntimeMove> {
    let selector = selector(kind, slot, Some(71));
    let mut stream_state = selector.new_stream_state();
    let mut cursor = selector.open_cursor_with_stream_state(
        &mut stream_state,
        director,
        MoveStreamContext::default(),
    );
    std::iter::from_fn(|| cursor.next_owned_candidate()).collect()
}

fn touches_pinned_route(mov: &RuntimeMove) -> bool {
    let mut touches = false;
    mov.for_each_affected_entity(&mut |entity| touches |= entity.entity_index == PINNED_ROUTE);
    touches
}

fn recipe_key(mov: &RuntimeMove) -> String {
    format!("{:?}", mov.clone().into_recipe())
}

#[test]
fn every_list_family_skips_pinned_owners() {
    let pinned = director_with(pinned_descriptor());
    let unpinned = director_with(descriptor());
    for kind in ALL_KINDS {
        for (source, slot_for) in [
            ("typed", static_slot_for as fn(ListLeafKind) -> Slot),
            ("dynamic", dynamic_slot_for as fn(ListLeafKind) -> Slot),
        ] {
            let with_pins = enumerate(kind, slot_for(kind), &pinned);
            assert!(
                with_pins.iter().all(|mov| !touches_pinned_route(mov)),
                "{kind:?}/{source} generated a move on the pinned route"
            );
            assert!(
                with_pins
                    .iter()
                    .all(|mov| !crate::pinning::move_changes_pinned(mov, &pinned)),
                "{kind:?}/{source} generated a move rejected by pinning"
            );

            let without_pins = enumerate(kind, slot_for(kind), &unpinned);
            assert!(
                without_pins.iter().any(touches_pinned_route),
                "{kind:?}/{source} fixture must exercise the middle route"
            );
            // Nearby families refill their distance-ranked slots with free
            // candidates, and ruin draws its random subsets from free owners
            // only, so their streams are not a plain filter of the unpinned
            // stream.
            if matches!(
                kind,
                ListLeafKind::NearbyChange | ListLeafKind::NearbySwap | ListLeafKind::Ruin
            ) {
                assert!(!with_pins.is_empty(), "{kind:?}/{source} keeps free work");
                continue;
            }
            let expected = without_pins
                .iter()
                .filter(|mov| !touches_pinned_route(mov))
                .map(recipe_key)
                .collect::<Vec<_>>();
            let actual = with_pins.iter().map(recipe_key).collect::<Vec<_>>();
            assert_eq!(
                actual, expected,
                "{kind:?}/{source} must keep the canonical order of free candidates"
            );
        }
    }
}
