//! Selected-owner snapshots for compiled list cursors.
//!
//! Pinned owners are removed here, after canonical ordering, so they never
//! reach a list cursor as a source or a destination while free owners keep
//! their seeded relative order.

use solverforge_core::domain::PlanningSolution;

use crate::builder::context::{list_access::ListAccess, RuntimeListSlot};
use crate::heuristic::selector::move_selector::MoveStreamContext;
use crate::heuristic::selector::nearby_list_change::CrossEntityDistanceMeter;
use crate::pinning::{unpinned_indices, PinnedEntities};

/// Free owners in canonical index order.
pub(super) fn free_entities<S, V, DM, IDM>(
    slot: &RuntimeListSlot<S, V, DM, IDM>,
    solution: &S,
    pins: &PinnedEntities,
) -> Vec<usize>
where
    S: PlanningSolution + Clone + Send + Sync + 'static,
    V: Clone + PartialEq + Send + Sync + std::fmt::Debug + 'static,
    DM: Clone + Send + Sync + std::fmt::Debug + CrossEntityDistanceMeter<S>,
    IDM: Clone + Send + Sync + std::fmt::Debug + CrossEntityDistanceMeter<S>,
{
    unpinned_indices(pins, ListAccess::entity_count(slot, solution)).collect()
}

/// Free owners in seeded stream order, with their route lengths.
pub(super) fn selected_entities<S, V, DM, IDM>(
    slot: &RuntimeListSlot<S, V, DM, IDM>,
    solution: &S,
    context: MoveStreamContext,
    pins: &PinnedEntities,
    rotation_salt: Option<u64>,
) -> (Vec<usize>, Vec<usize>)
where
    S: PlanningSolution + Clone + Send + Sync + 'static,
    V: Clone + PartialEq + Send + Sync + std::fmt::Debug + 'static,
    DM: Clone + Send + Sync + std::fmt::Debug + CrossEntityDistanceMeter<S>,
    IDM: Clone + Send + Sync + std::fmt::Debug + CrossEntityDistanceMeter<S>,
{
    let entity_count = ListAccess::entity_count(slot, solution);
    let mut entities = Vec::with_capacity(pins.unpinned_count(entity_count));
    entities.extend(
        (0..entity_count)
            .map(|offset| match rotation_salt {
                Some(salt) => {
                    context.selection_index_without_replacement(offset, entity_count, salt)
                }
                None => offset,
            })
            .filter(|&entity| !pins.is_pinned(entity)),
    );
    let route_lens = entities
        .iter()
        .map(|&entity| ListAccess::list_len(slot, solution, entity))
        .collect();
    (entities, route_lens)
}
