//! Build script for the torille-hunspell wrapper.
//!
//! cxx-build glues the Rust bridge (`src/lib.rs`'s `#[cxx::bridge]` module)
//! to the C++ side (`src/hunspell_compat.cc`). The C++ file is recompiled
//! whenever the bridge or the C++ source changes, and the resulting
//! staticlib is linked into every consumer of `torille_wrapper`.

fn main() {
    cxx_build::bridge("src/lib.rs")
        .file("src/hunspell_compat.cc")
        .flag_if_supported("-std=c++14")
        .include("include")
        .compile("torille_hunspell_compat");

    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/hunspell_compat.cc");
    println!("cargo:rerun-if-changed=include/torille_hunspell.h");
}