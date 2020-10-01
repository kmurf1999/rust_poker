// extern crate gen_eval_table;
extern crate gen_eval_table;

use std::env;

fn main() {
    // copy out dir variable
    println!("cargo:rustc-env=OUT_DIR={}", env::var("OUT_DIR").unwrap());

    // generate hand eval table
    gen_eval_table::gen_eval_table();
}
