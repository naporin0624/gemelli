// The CLI shell layer (argument parsing, `cargo bundle-licenses` subprocess, file IO) that
// calls these pure functions from `gen-licenses` does not exist yet, so nothing in this crate
// has a caller. `dead_code` is allowed here so `cargo test -p xtask` can compile and run the
// unit tests in each module.
#![allow(dead_code)]

mod license_entry;
mod merge;
mod normalize;
mod render;
mod sort;

fn main() {}
