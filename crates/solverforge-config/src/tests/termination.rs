// Tests for time-based termination limits.

use std::time::Duration;

use super::*;

fn local_search_termination(config: &SolverConfig) -> &TerminationConfig {
    let PhaseConfig::LocalSearch(local_search) = &config.phases[0] else {
        panic!("phase should be local search");
    };
    local_search
        .termination
        .as_ref()
        .expect("phase termination should be configured")
}

#[test]
fn toml_accepts_fractional_time_limits() {
    let config = SolverConfig::from_toml_str(
        r#"
        [termination]
        seconds_spent_limit = 1.5
        unimproved_seconds_spent_limit = 0.25
        "#,
    )
    .unwrap();

    let termination = config.termination.as_ref().unwrap();
    assert_eq!(termination.seconds_spent_limit, Some(1.5));
    assert_eq!(config.time_limit(), Some(Duration::from_millis(1500)));
    assert_eq!(
        termination.unimproved_time_limit(),
        Some(Duration::from_millis(250))
    );
}

#[test]
fn toml_accepts_whole_number_time_limits_as_integer_or_float() {
    for literal in ["30", "30.0"] {
        let config = SolverConfig::from_toml_str(&format!(
            "[termination]\nseconds_spent_limit = {literal}\nunimproved_seconds_spent_limit = 5\n"
        ))
        .unwrap();

        assert_eq!(config.time_limit(), Some(Duration::from_secs(30)));
        assert_eq!(
            config.termination.unwrap().unimproved_time_limit(),
            Some(Duration::from_secs(5))
        );
    }
}

#[test]
fn fractional_minutes_combine_with_fractional_seconds() {
    let config = SolverConfig::from_toml_str(
        r#"
        [termination]
        seconds_spent_limit = 1.5
        minutes_spent_limit = 0.5
        "#,
    )
    .unwrap();

    assert_eq!(config.time_limit(), Some(Duration::from_millis(31_500)));
}

#[test]
fn zero_time_limit_means_no_time_limit() {
    let config = SolverConfig::from_toml_str("[termination]\nseconds_spent_limit = 0.0\n").unwrap();

    assert_eq!(config.time_limit(), None);
}

#[test]
fn yaml_accepts_fractional_time_limits() {
    let config = SolverConfig::from_yaml_str(
        r#"
        termination:
          seconds_spent_limit: 0.75
          minutes_spent_limit: 1
          unimproved_seconds_spent_limit: 0.1
        "#,
    )
    .unwrap();

    assert_eq!(config.time_limit(), Some(Duration::from_millis(60_750)));
    assert_eq!(
        config.termination.unwrap().unimproved_time_limit(),
        Some(Duration::from_millis(100))
    );
}

#[test]
fn phase_termination_accepts_fractional_time_limits() {
    let toml_config = SolverConfig::from_toml_str(
        r#"
        [[phases]]
        type = "local_search"
        [phases.termination]
        seconds_spent_limit = 0.5
        unimproved_seconds_spent_limit = 0.125
        "#,
    )
    .unwrap();
    let yaml_config = SolverConfig::from_yaml_str(
        r#"
        phases:
          - type: local_search
            termination:
              seconds_spent_limit: 0.5
              unimproved_seconds_spent_limit: 0.125
        "#,
    )
    .unwrap();

    for config in [&toml_config, &yaml_config] {
        let termination = local_search_termination(config);
        assert_eq!(termination.time_limit(), Some(Duration::from_millis(500)));
        assert_eq!(
            termination.unimproved_time_limit(),
            Some(Duration::from_millis(125))
        );
    }
}

#[test]
fn toml_rejects_negative_and_non_finite_time_limits() {
    for key in [
        "seconds_spent_limit",
        "minutes_spent_limit",
        "unimproved_seconds_spent_limit",
    ] {
        for value in ["-1.5", "-1", "nan", "inf"] {
            let top_level = format!("[termination]\n{key} = {value}\n");
            let phase_level = format!(
                "[[phases]]\ntype = \"local_search\"\n[phases.termination]\n{key} = {value}\n"
            );

            for toml in [top_level, phase_level] {
                let error = SolverConfig::from_toml_str(&toml)
                    .expect_err("invalid time limit should be rejected")
                    .to_string();
                assert!(
                    error.contains(key) && error.contains("finite, non-negative"),
                    "unexpected error for {key} = {value}: {error}"
                );
            }
        }
    }
}

#[test]
fn yaml_rejects_negative_and_non_finite_time_limits() {
    for key in [
        "seconds_spent_limit",
        "minutes_spent_limit",
        "unimproved_seconds_spent_limit",
    ] {
        for value in ["-0.5", ".nan", ".inf"] {
            let yaml = format!("termination:\n  {key}: {value}\n");
            let error = SolverConfig::from_yaml_str(&yaml)
                .expect_err("invalid time limit should be rejected")
                .to_string();
            assert!(
                error.contains(key) && error.contains("finite, non-negative"),
                "unexpected error for {key}: {value}: {error}"
            );
        }
    }
}

#[test]
fn rejects_time_limits_that_overflow_duration() {
    let error = SolverConfig::from_toml_str("[termination]\nminutes_spent_limit = 1e300\n")
        .expect_err("overflowing time limit should be rejected")
        .to_string();

    assert!(error.contains("minutes_spent_limit"), "{error}");
}

#[test]
fn builder_sets_sub_second_time_limit() {
    let config = SolverConfig::new().with_termination_time_limit(Duration::from_millis(1500));

    assert_eq!(config.time_limit(), Some(Duration::from_millis(1500)));
    assert_eq!(config.termination.unwrap().seconds_spent_limit, Some(1.5));
}

#[test]
fn whole_second_builder_still_sets_seconds_limit() {
    let config = SolverConfig::new().with_termination_seconds(60);

    assert_eq!(config.time_limit(), Some(Duration::from_secs(60)));
}
