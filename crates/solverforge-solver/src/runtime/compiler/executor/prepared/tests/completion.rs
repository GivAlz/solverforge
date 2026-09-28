use super::*;

#[test]
fn cancellation_retains_complete_working_solution_over_better_incomplete_best() {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let config = SolverConfig {
        phases: vec![construction(ConstructionHeuristicType::ListClarkeWright)],
        ..SolverConfig::default()
    };
    let executor = executor(&config);
    let bindings = executor.graph().default_bindings().clone();
    let mut execution = executor
        .instantiate()
        .expect("Clarke-Wright runtime execution prepares");
    let best_solution_events = Arc::new(AtomicUsize::new(0));
    let observed_events = Arc::clone(&best_solution_events);
    let incomplete = plan(vec![1, 2, 3], vec![Vec::new()]);
    let director = ScoreDirector::simple(incomplete.clone(), descriptor(), |plan, _| {
        entity_count(plan)
    });
    let mut scope = SolverScope::new_with_callback(
        director,
        move |progress: SolverProgressRef<'_, Plan>| {
            if progress.kind == SolverProgressKind::BestSolution {
                observed_events.fetch_add(1, Ordering::SeqCst);
            }
        },
        None,
        None,
    );
    scope.defer_best_solution_publication();
    scope.set_best_solution(incomplete, SoftScore::of(1));
    scope.mutate(|director| director.working_solution_mut().routes = vec![vec![1, 2, 3]]);
    scope.mark_cancelled();
    let mut completion_published = false;

    assert!(!publish_if_mandatory_complete(
        &mut execution,
        &bindings,
        &mut completion_published,
        0,
        &mut scope,
    )
    .expect("cancellation completion check succeeds"));

    assert_eq!(
        scope.terminal_reason(),
        crate::manager::SolverTerminalReason::Cancelled
    );
    assert_eq!(
        scope
            .best_solution()
            .expect("complete working state replaces incomplete prior best")
            .routes,
        vec![vec![1, 2, 3]]
    );
    assert_eq!(best_solution_events.load(Ordering::SeqCst), 0);
}

thread_local! {
    static PANICS_ON_THIS_THREAD: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Counts panics raised on the calling thread while delegating reporting to
/// the previously installed hook, so concurrent tests keep their output.
fn install_thread_panic_counter() {
    static INSTALL: std::sync::Once = std::sync::Once::new();
    INSTALL.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            PANICS_ON_THIS_THREAD.with(|count| count.set(count.get() + 1));
            previous(info);
        }));
    });
}

fn panics_on_this_thread() -> usize {
    PANICS_ON_THIS_THREAD.with(std::cell::Cell::get)
}

#[test]
fn configured_termination_during_construction_fails_without_unwinding() {
    let _guard = TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    install_thread_panic_counter();
    let panics_before = panics_on_this_thread();
    let config = SolverConfig {
        termination: Some(TerminationConfig {
            step_count_limit: Some(3),
            ..TerminationConfig::default()
        }),
        phases: vec![
            construction(ConstructionHeuristicType::ListRoundRobin),
            PhaseConfig::LocalSearch(Default::default()),
        ],
        ..SolverConfig::default()
    };

    let result = crate::try_run_solver_with_config_and_search(
        plan((1..=8).collect(), vec![Vec::new(), Vec::new()]),
        (),
        descriptor(),
        |plan, _| entity_count(plan),
        crate::SolverRuntime::detached(),
        config,
        30,
        |_| {},
        None,
        |config, descriptor| {
            Ok(SearchContext::try_new(descriptor, model(), config.random_seed)?.defaults())
        },
    );

    let error = result.expect_err("construction cut by the step limit must fail");
    assert!(matches!(error, RuntimeBuildError::Execution { .. }));
    let message = error.to_string();
    assert!(
        message.contains("configured solve stopped with mandatory planning work incomplete"),
        "{message}"
    );
    assert!(message.contains("5 unassigned element(s)"), "{message}");
    assert!(message.contains("3 of 8 assigned"), "{message}");
    assert_eq!(
        panics_on_this_thread(),
        panics_before,
        "a configured-termination failure is a normal lifecycle outcome, not a panic"
    );
}
