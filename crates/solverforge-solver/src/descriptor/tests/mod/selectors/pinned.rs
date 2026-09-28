/// Tasks on worker 2 are pinned; they must never reach a descriptor cursor.
fn pinned_on_worker_two(entity: &dyn Any) -> bool {
    entity
        .downcast_ref::<Task>()
        .expect("task entity")
        .worker_idx
        == Some(2)
}

fn pinned_descriptor() -> SolutionDescriptor {
    let mut descriptor = descriptor_with_allows_unassigned(true);
    let task = descriptor
        .entity_descriptors
        .remove(0)
        .with_pin_predicate(pinned_on_worker_two);
    descriptor.entity_descriptors.insert(0, task);
    descriptor
}

#[test]
fn descriptor_selectors_skip_pinned_entities() {
    let descriptor = pinned_descriptor();
    let plan = Plan {
        workers: vec![Worker, Worker, Worker],
        tasks: [Some(0), Some(2), Some(0), Some(2), Some(1)]
            .into_iter()
            .map(|worker_idx| Task { worker_idx })
            .collect(),
        score: None,
    };
    let director = ScoreDirector::simple(plan, descriptor.clone(), |s, _| s.tasks.len());
    let target = VariableTargetConfig::default;
    let configs = [
        MoveSelectorConfig::ChangeMoveSelector(ChangeMoveConfig {
            selection_order: None,
            selection_metric: None,
            value_candidate_limit: None,
            target: target(),
        }),
        MoveSelectorConfig::SwapMoveSelector(SwapMoveConfig {
            selection_order: None,
            selection_metric: None,
            target: target(),
        }),
        MoveSelectorConfig::PillarChangeMoveSelector(PillarChangeMoveConfig {
            selection_order: None,
            selection_metric: None,
            minimum_sub_pillar_size: 0,
            maximum_sub_pillar_size: 0,
            value_candidate_limit: None,
            target: target(),
        }),
        MoveSelectorConfig::PillarSwapMoveSelector(PillarSwapMoveConfig {
            selection_order: None,
            selection_metric: None,
            minimum_sub_pillar_size: 0,
            maximum_sub_pillar_size: 0,
            target: target(),
        }),
        MoveSelectorConfig::RuinRecreateMoveSelector(RuinRecreateMoveSelectorConfig {
            selection_order: None,
            selection_metric: None,
            min_ruin_count: 1,
            max_ruin_count: 2,
            moves_per_step: Some(8),
            value_candidate_limit: None,
            recreate_heuristic_type: RecreateHeuristicType::FirstFit,
            target: target(),
        }),
    ];

    for config in configs {
        let selector = build_descriptor_move_selector::<Plan>(Some(&config), &descriptor, Some(7));
        let moves: Vec<_> = selector.iter_moves(&director).collect();
        assert!(
            moves
                .iter()
                .all(|mov| mov.entity_indices().iter().all(|&entity| entity != 1 && entity != 3)),
            "{config:?} generated a move on a pinned task"
        );
        if !matches!(config, MoveSelectorConfig::RuinRecreateMoveSelector(_)) {
            assert_eq!(selector.size(&director), moves.len(), "{config:?}");
        }
    }
}
