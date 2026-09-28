// Tests that selector keys with documented defaults may be omitted from TOML.

use super::*;

fn parse_local_search_selector(selector_body: &str) -> MoveSelectorConfig {
    let toml =
        format!("[[phases]]\ntype = \"local_search\"\n[phases.move_selector]\n{selector_body}\n");
    let config = SolverConfig::from_toml_str(&toml)
        .unwrap_or_else(|error| panic!("selector `{selector_body}` should parse: {error}"));
    let PhaseConfig::LocalSearch(local_search) = config.phases.into_iter().next().unwrap() else {
        panic!("phase should be local_search");
    };
    local_search
        .move_selector
        .expect("local search should have a move selector")
}

#[test]
fn test_leaf_selectors_with_all_optional_keys_omitted_use_documented_defaults() {
    let cases = [
        (
            "change_move_selector",
            MoveSelectorConfig::ChangeMoveSelector(ChangeMoveConfig::default()),
        ),
        (
            "swap_move_selector",
            MoveSelectorConfig::SwapMoveSelector(SwapMoveConfig::default()),
        ),
        (
            "nearby_change_move_selector",
            MoveSelectorConfig::NearbyChangeMoveSelector(NearbyChangeMoveConfig::default()),
        ),
        (
            "nearby_swap_move_selector",
            MoveSelectorConfig::NearbySwapMoveSelector(NearbySwapMoveConfig::default()),
        ),
        (
            "pillar_change_move_selector",
            MoveSelectorConfig::PillarChangeMoveSelector(PillarChangeMoveConfig::default()),
        ),
        (
            "pillar_swap_move_selector",
            MoveSelectorConfig::PillarSwapMoveSelector(PillarSwapMoveConfig::default()),
        ),
        (
            "ruin_recreate_move_selector",
            MoveSelectorConfig::RuinRecreateMoveSelector(RuinRecreateMoveSelectorConfig::default()),
        ),
        (
            "list_change_move_selector",
            MoveSelectorConfig::ListChangeMoveSelector(ListChangeMoveConfig::default()),
        ),
        (
            "nearby_list_change_move_selector",
            MoveSelectorConfig::NearbyListChangeMoveSelector(NearbyListChangeMoveConfig::default()),
        ),
        (
            "list_swap_move_selector",
            MoveSelectorConfig::ListSwapMoveSelector(ListSwapMoveConfig::default()),
        ),
        (
            "list_permute_move_selector",
            MoveSelectorConfig::ListPermuteMoveSelector(ListPermuteMoveConfig::default()),
        ),
        (
            "list_precedence_move_selector",
            MoveSelectorConfig::ListPrecedenceMoveSelector(ListPrecedenceMoveConfig::default()),
        ),
        (
            "nearby_list_swap_move_selector",
            MoveSelectorConfig::NearbyListSwapMoveSelector(NearbyListSwapMoveConfig::default()),
        ),
        (
            "sublist_change_move_selector",
            MoveSelectorConfig::SublistChangeMoveSelector(SublistChangeMoveConfig::default()),
        ),
        (
            "sublist_swap_move_selector",
            MoveSelectorConfig::SublistSwapMoveSelector(SublistSwapMoveConfig::default()),
        ),
        (
            "list_reverse_move_selector",
            MoveSelectorConfig::ListReverseMoveSelector(ListReverseMoveConfig::default()),
        ),
        (
            "k_opt_move_selector",
            MoveSelectorConfig::KOptMoveSelector(KOptMoveSelectorConfig::default()),
        ),
        (
            "list_ruin_move_selector",
            MoveSelectorConfig::ListRuinMoveSelector(ListRuinMoveSelectorConfig::default()),
        ),
    ];

    for (selector_type, expected) in cases {
        let parsed = parse_local_search_selector(&format!("type = \"{selector_type}\""));
        assert_eq!(
            format!("{parsed:?}"),
            format!("{expected:?}"),
            "`{selector_type}` should fall back to its documented defaults"
        );
    }
}

#[test]
fn test_k_opt_partial_keys_fill_remaining_documented_defaults() {
    let MoveSelectorConfig::KOptMoveSelector(k_only) =
        parse_local_search_selector("type = \"k_opt_move_selector\"\nk = 3")
    else {
        panic!("selector should be k_opt");
    };
    assert_eq!(k_only.k, 3);
    assert_eq!(k_only.min_segment_len, 1);
    assert_eq!(k_only.max_nearby, 0);

    let MoveSelectorConfig::KOptMoveSelector(nearby) = parse_local_search_selector(
        "type = \"k_opt_move_selector\"\nk = 2\nmax_nearby = 8\nentity_class = \"Route\"\nvariable_name = \"visits\"",
    ) else {
        panic!("selector should be k_opt");
    };
    assert_eq!(nearby.k, 2);
    assert_eq!(nearby.min_segment_len, 1);
    assert_eq!(nearby.max_nearby, 8);
    assert_eq!(nearby.target.entity_class.as_deref(), Some("Route"));
    assert_eq!(nearby.target.variable_name.as_deref(), Some("visits"));
}

#[test]
fn test_partial_range_keys_fill_remaining_documented_defaults() {
    let MoveSelectorConfig::SublistChangeMoveSelector(sublist) = parse_local_search_selector(
        "type = \"sublist_change_move_selector\"\nmax_sublist_size = 4",
    ) else {
        panic!("selector should be sublist_change");
    };
    assert_eq!(sublist.min_sublist_size, 1);
    assert_eq!(sublist.max_sublist_size, 4);

    let MoveSelectorConfig::RuinRecreateMoveSelector(ruin) = parse_local_search_selector(
        "type = \"ruin_recreate_move_selector\"\nrecreate_heuristic_type = \"cheapest_insertion\"",
    ) else {
        panic!("selector should be ruin_recreate");
    };
    assert_eq!(ruin.min_ruin_count, 2);
    assert_eq!(ruin.max_ruin_count, 5);
    assert_eq!(
        ruin.recreate_heuristic_type,
        RecreateHeuristicType::CheapestInsertion
    );
}

#[test]
fn test_selector_keys_without_documented_defaults_stay_required() {
    let missing_group = "[[phases]]\ntype = \"local_search\"\n[phases.move_selector]\ntype = \"grouped_scalar_move_selector\"\n";
    let error = SolverConfig::from_toml_str(missing_group)
        .expect_err("grouped_scalar_move_selector requires group_name");
    assert!(error.to_string().contains("group_name"), "{error}");

    let missing_limit = "[[phases]]\ntype = \"local_search\"\n[phases.move_selector]\ntype = \"limited_neighborhood\"\n[phases.move_selector.selector]\ntype = \"change_move_selector\"\n";
    let error = SolverConfig::from_toml_str(missing_limit)
        .expect_err("limited_neighborhood requires selected_count_limit");
    assert!(
        error.to_string().contains("selected_count_limit"),
        "{error}"
    );

    for selector_type in [
        "conflict_repair_move_selector",
        "compound_conflict_repair_move_selector",
    ] {
        let missing_constraints = format!(
            "[[phases]]\ntype = \"local_search\"\n[phases.move_selector]\ntype = \"{selector_type}\"\n"
        );
        let error = SolverConfig::from_toml_str(&missing_constraints)
            .expect_err("conflict repair selectors require constraints");
        assert!(error.to_string().contains("constraints"), "{error}");
    }
}

#[test]
fn test_conflict_repair_selectors_fill_documented_defaults_around_constraints() {
    let MoveSelectorConfig::ConflictRepairMoveSelector(repair) = parse_local_search_selector(
        "type = \"conflict_repair_move_selector\"\nconstraints = [\"overlap\"]",
    ) else {
        panic!("selector should be conflict_repair");
    };
    assert_eq!(
        repair,
        ConflictRepairMoveSelectorConfig {
            constraints: vec!["overlap".to_string()],
            ..ConflictRepairMoveSelectorConfig::default()
        }
    );

    let MoveSelectorConfig::CompoundConflictRepairMoveSelector(compound) =
        parse_local_search_selector(
            "type = \"compound_conflict_repair_move_selector\"\nconstraints = [\"overlap\"]",
        )
    else {
        panic!("selector should be compound_conflict_repair");
    };
    assert_eq!(
        compound,
        CompoundConflictRepairMoveSelectorConfig {
            constraints: vec!["overlap".to_string()],
            ..CompoundConflictRepairMoveSelectorConfig::default()
        }
    );
}
