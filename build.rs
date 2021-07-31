use rustc_version::{version, Version};

fn main() {
    if unsafe_op_in_unsafe_fn_stable() {
        println!("cargo:rustc-cfg=unsafe_op_in_unsafe_fn_stable");
    }
}

fn unsafe_op_in_unsafe_fn_stable() -> bool {
    // A build script is needed for this because the `rustversion` crate has some weird problems
    // around being used as a crate-level inner attribute.
    version().unwrap() >= Version::new(1, 52, 0)
}
