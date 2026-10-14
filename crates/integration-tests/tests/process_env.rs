//! Reads the real process environment.
//!
//! Kept in its own test binary with a single test, since setting environment variables
//! while other threads read them is unsound.

mod common;

use common::*;
use ordna_core::ConfigValueSource;
use ordna_source_env::EnvSource;

// r[verify env.naming]
// r[verify env.values]
#[test]
fn loads_from_the_process_environment() {
    let prefix = format!("ORDNA_IT_{}", std::process::id());
    // SAFETY: this is the only test in this binary, so no other thread reads the
    // environment concurrently.
    unsafe {
        std::env::set_var(format!("{prefix}__PORT"), "9090");
        std::env::set_var(format!("{prefix}__DEBUG"), "true");
        std::env::set_var(format!("{prefix}__LOG__LEVEL"), "debug");
    }

    let dir = TempDir::new("process-env");
    let loaded = load(
        &dir.write("defaults.styx", DEFAULTS),
        &dir.write("secrets.styx", SECRETS),
        EnvSource::with_prefix(&prefix),
    )
    .unwrap();

    assert_eq!(loaded.config(), &expected_app());
    assert_eq!(
        loaded.source(&path("port")).unwrap().winner(),
        &ConfigValueSource::Direct(Origin::Env {
            var: format!("{prefix}__PORT"),
            path: path("port"),
        })
    );
}
