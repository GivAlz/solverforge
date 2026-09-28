solverforge::planning_model! {
    root = "crates/solverforge/tests/list_best_score_limit";

    mod plan;
    mod route;
    mod stop;

    pub use plan::Plan;
    pub use route::Route;
    pub use stop::Stop;
}
