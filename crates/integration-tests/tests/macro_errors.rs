//! Compile errors of the config macros, see `tests/ui`.

// r[verify macro.error.path]
// r[verify macro.error.duplicate-path]
// r[verify macro.error.attributes]
// r[verify macro.error.shape]
#[test]
fn macro_errors() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
