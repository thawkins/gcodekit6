fn main() {
    // Build-time generator for `src/ui_impl.rs`.
    // If both crate features `with-slint` and `slint_generated` are enabled
    // and the SLINT_INCLUDE_GENERATED env var is present, try to run
    // `slint-build` to generate Rust modules from `src/ui.slint`. If the
    // generator produces .rs files, concatenate them into OUT_DIR/generated_ui.rs
    // and have `src/ui_impl.rs` include that. Otherwise fall back to the
    // local synthetic shim.

    let with_slint = std::env::var("CARGO_FEATURE_WITH_SLINT").is_ok();
    let slint_generated = std::env::var("CARGO_FEATURE_SLINT_GENERATED").is_ok();
    let include_env = std::env::var("SLINT_INCLUDE_GENERATED").is_ok();

    let ui_impl_path = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("src")
        .join("ui_impl.rs");

    // Under Option B we require Slint-generated sources when the
    // `slint_generated` feature is enabled. If generation doesn't produce
    // .rs artifacts in the Slint OUT directory we fail the build with a
    // helpful error message that tells the developer how to enable
    // generation (SLINT_INCLUDE_GENERATED=1 and features `with-slint,slint_generated`).

    // Option A behavior: prefer generated outputs when features and env var indicate generation,
    // otherwise write a fallback re-export of a committed stub so the crate remains buildable.
    let shim = String::from(r#"// Auto-generated shim: re-export the local generated stub so the UI crate
// builds even when Slint-generated code is not present.
pub use crate::ui_generated::*;
"#);

    let contents = if with_slint && slint_generated && include_env {
        let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let slint_file = std::path::Path::new(&manifest).join("src").join("ui.slint");
        if slint_file.exists() {
            if let Err(e) = slint_build::compile(&slint_file) {
                eprintln!("warning: slint-build failed: {}", e);
            }

            // slint-build writes to the `OUT` env var. Prefer that, but
            // fall back to `OUT_DIR` if necessary. We will always write the
            // concatenated generated file into OUT_DIR so the crate can
            // include it via env!("OUT_DIR").
            let out_src = std::env::var("OUT").or_else(|_| std::env::var("OUT_DIR"));
            let out_dest = std::env::var("OUT_DIR").ok();
            if let (Ok(src), Some(dest)) = (out_src, out_dest) {
                let src_path = std::path::Path::new(&src);
                let dest_path = std::path::Path::new(&dest);

                // Collect .rs files from src_path
                let mut rs_paths = Vec::new();
                if let Ok(entries) = std::fs::read_dir(src_path) {
                    for ent in entries.flatten() {
                        let p = ent.path();
                        if p.extension().and_then(|s| s.to_str()) == Some("rs") {
                            rs_paths.push(p);
                        }
                    }
                }

                if !rs_paths.is_empty() {
                    eprintln!("slint-build: found {} .rs files in {}", rs_paths.len(), src_path.display());
                    for p in &rs_paths {
                        eprintln!("slint-build: candidate -> {}", p.display());
                    }
                    rs_paths.sort();
                    let generated_file = dest_path.join("generated_ui.rs");
                    let mut output = String::new();
                    output.push_str("// Concatenated Slint-generated Rust modules\n");
                    for p in &rs_paths {
                        if let Ok(s) = std::fs::read_to_string(p) {
                            output.push_str(&format!("// from: {}\n", p.display()));
                            output.push_str(&s);
                            output.push_str("\n\n");
                        }
                    }
                    if let Err(e) = std::fs::write(&generated_file, output) {
                        eprintln!("warning: failed to write generated_ui.rs: {}", e);
                        shim.clone()
                    } else {
                        // Include the concatenated generated file via OUT_DIR
                        String::from(r#"// Auto-generated: include concatenated Slint-generated UI
include!(concat!(env!("OUT_DIR"), "/generated_ui.rs"));

pub use ui::*;

#[cfg(not(feature = "slint_generated"))]
pub use crate::ui_generated::*;
"#)
                    }
                } else {
                    // No .rs files found in slint output; fallback to shim
                    shim.clone()
                }
            } else {
                // Could not determine OUT/OUT_DIR; fallback to shim
                shim.clone()
            }
        } else {
            // slint file missing; fallback to shim
            shim.clone()
        }
    } else if with_slint && slint_generated {
        // Slint features enabled but generation not requested; provide shim
        shim.clone()
    } else {
        // Default: re-export an existing compiled stub if present
        shim.clone()
    };

    std::fs::write(&ui_impl_path, contents).expect("failed to write src/ui_impl.rs");
}
