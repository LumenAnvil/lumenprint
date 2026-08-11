//! Tells Cargo to rebuild when a template changes.
//!
//! `include_dir!` reads `src/templates/` while the proc macro expands, but Cargo
//! only tracks `.rs` files it can see. Without this, adding or editing a template
//! leaves the previously embedded copy in the binary and the change appears to do
//! nothing — a confusing first experience for a contributor whose whole
//! contribution is template files.
//!
//! Cargo walks a watched directory recursively, so one line covers every layer,
//! manifest and template file.

fn main() {
    println!("cargo::rerun-if-changed=src/templates");
}
