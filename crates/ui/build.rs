fn main() {
    // Build-time generator for `src/ui_impl.rs`.
    // If both crate features `with-slint` and `slint_generated` are enabled and
    // the SLINT_INCLUDE_GENERATED env var is present, write a file that expands
    // `slint::include_modules!()` to include the real generated UI. Otherwise
    // write a shim that re-exports the `ui_generated` stub so the crate builds
    // cleanly under `--all-features` even when generated files are absent.

    let with_slint = std::env::var("CARGO_FEATURE_WITH_SLINT").is_ok();
    let slint_generated = std::env::var("CARGO_FEATURE_SLINT_GENERATED").is_ok();
    let include_env = std::env::var("SLINT_INCLUDE_GENERATED").is_ok();

    let out_path = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("src")
        .join("ui_impl.rs");

    let contents = if with_slint && slint_generated && include_env {
        r#"// Auto-generated: include the real Slint-generated modules
slint::include_modules!();
pub use ui::*;
"#
    } else {
        r#"// Auto-generated shim: re-export the local generated stub so the UI crate
// builds even when Slint-generated code is not present.
pub use gcodekit_ui::ui_generated::*;
"#
    };

    std::fs::write(&out_path, contents).expect("failed to write src/ui_impl.rs");
}
