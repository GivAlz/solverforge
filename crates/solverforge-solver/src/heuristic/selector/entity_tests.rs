use super::*;
use crate::test_utils::create_simple_nqueens_director;

#[test]
fn test_from_solution_entity_selector() {
    let director = create_simple_nqueens_director(4);

    let solution = director.working_solution();
    for (i, queen) in solution.queens.iter().enumerate() {
        assert_eq!(queen.column, i as i64);
    }

    let selector = FromSolutionEntitySelector::new(0);

    let refs: Vec<_> = selector.iter(&director).collect();
    assert_eq!(refs.len(), 4);
    assert_eq!(refs[0], EntityReference::new(0, 0));
    assert_eq!(refs[1], EntityReference::new(0, 1));
    assert_eq!(refs[2], EntityReference::new(0, 2));
    assert_eq!(refs[3], EntityReference::new(0, 3));

    assert_eq!(selector.size(&director), 4);
}

#[test]
fn test_all_entities_selector() {
    let director = create_simple_nqueens_director(3);

    let selector = AllEntitiesSelector::new();

    let refs: Vec<_> = selector.iter(&director).collect();
    assert_eq!(refs.len(), 3);
    assert_eq!(selector.size(&director), 3);
}

fn pinned_even_queens_director(
    n: usize,
) -> solverforge_scoring::ScoreDirector<crate::test_utils::NQueensSolution, ()> {
    let mut descriptor = crate::test_utils::create_nqueens_descriptor();
    let queen = descriptor
        .entity_descriptors
        .remove(0)
        .with_pin_predicate(|entity| {
            entity
                .downcast_ref::<crate::test_utils::Queen>()
                .expect("queen entity")
                .id
                % 2
                == 0
        });
    descriptor.entity_descriptors.push(queen);
    solverforge_scoring::ScoreDirector::simple(
        crate::test_utils::NQueensSolution::uninitialized(n),
        descriptor,
        |s, _| s.queens.len(),
    )
}

#[test]
fn stock_entity_selectors_skip_pinned_entities() {
    let director = pinned_even_queens_director(5);

    let from_solution = FromSolutionEntitySelector::new(0);
    let refs: Vec<_> = from_solution.iter(&director).collect();
    assert_eq!(
        refs,
        vec![EntityReference::new(0, 1), EntityReference::new(0, 3)]
    );
    assert_eq!(from_solution.size(&director), 2);

    let all = AllEntitiesSelector::new();
    assert_eq!(all.iter(&director).collect::<Vec<_>>(), refs);
    assert_eq!(all.size(&director), 2);
}
