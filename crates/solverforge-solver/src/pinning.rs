use solverforge_core::domain::PlanningSolution;
use solverforge_scoring::Director;

use crate::heuristic::r#move::Move;

#[inline]
pub(crate) fn entity_is_pinned<S: PlanningSolution, D: Director<S>>(
    director: &D,
    descriptor_index: usize,
    entity_index: usize,
) -> bool {
    director
        .solution_descriptor()
        .entity_descriptors
        .get(descriptor_index)
        .is_some_and(|descriptor| descriptor.is_pinned(director.working_solution(), entity_index))
}

#[inline]
pub(crate) fn move_changes_pinned<S: PlanningSolution, D: Director<S>, M: Move<S>>(
    mov: &M,
    director: &D,
) -> bool {
    if !director
        .solution_descriptor()
        .entity_descriptors
        .iter()
        .any(|descriptor| descriptor.has_pin_predicate())
    {
        return false;
    }
    let mut pinned = false;
    mov.for_each_affected_entity(&mut |entity| {
        if !pinned {
            pinned = entity_is_pinned(director, entity.descriptor_index, entity.entity_index);
        }
    });
    pinned
}

/// Pin state of one descriptor's entities, frozen when a selector snapshots
/// its entities for one cursor.
///
/// Selectors consult it so pinned entities never reach a cursor: they are
/// neither generated nor evaluated. Descriptors without a pin predicate, and
/// snapshots where no entity is currently pinned, carry no mask, so unpinned
/// models pay one branch per entity and no per-entity predicate calls beyond
/// the capture scan.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PinnedEntities {
    mask: Option<Vec<bool>>,
}

impl PinnedEntities {
    /// Captures the pin state of `descriptor_index` from the director's
    /// current working solution.
    pub(crate) fn capture<S: PlanningSolution, D: Director<S>>(
        director: &D,
        descriptor_index: usize,
    ) -> Self {
        let Some(descriptor) = director
            .solution_descriptor()
            .entity_descriptors
            .get(descriptor_index)
            .filter(|descriptor| descriptor.has_pin_predicate())
        else {
            return Self::default();
        };
        let solution = director.working_solution();
        let entity_count = descriptor.entity_count(solution).unwrap_or(0);
        let mask = (0..entity_count)
            .map(|entity_index| descriptor.is_pinned(solution, entity_index))
            .collect::<Vec<_>>();
        Self {
            mask: mask.contains(&true).then_some(mask),
        }
    }

    /// Returns whether `entity_index` was pinned when this snapshot was taken.
    #[inline]
    pub(crate) fn is_pinned(&self, entity_index: usize) -> bool {
        self.mask
            .as_ref()
            .is_some_and(|mask| mask.get(entity_index).copied().unwrap_or(false))
    }

    /// Returns the unpinned indices of `0..entity_count` when at least one
    /// entity is pinned, and `None` when every index is free.
    pub(crate) fn free_list(&self, entity_count: usize) -> Option<Vec<usize>> {
        self.mask.as_ref().map(|_| {
            (0..entity_count)
                .filter(|&entity_index| !self.is_pinned(entity_index))
                .collect()
        })
    }

    /// Returns the number of entities in `0..entity_count` that are not pinned.
    pub(crate) fn unpinned_count(&self, entity_count: usize) -> usize {
        match &self.mask {
            None => entity_count,
            Some(_) => (0..entity_count)
                .filter(|&entity_index| !self.is_pinned(entity_index))
                .count(),
        }
    }
}

/// Unpinned indices of `0..entity_count` in canonical order.
///
/// The iterator is a mapped range, so collecting it allocates exactly once
/// and, without pinned entities, it needs no index list of its own.
pub(crate) fn unpinned_indices(
    pins: &PinnedEntities,
    entity_count: usize,
) -> impl ExactSizeIterator<Item = usize> + 'static {
    let free = pins.free_list(entity_count);
    let len = free.as_ref().map_or(entity_count, Vec::len);
    (0..len).map(move |offset| free.as_ref().map_or(offset, |free| free[offset]))
}
