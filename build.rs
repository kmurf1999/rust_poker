extern crate bindgen;
extern crate gen_eval_table;

use cmake::Config;
use std::env;
use std::path::PathBuf;

fn main() {
    // copy out dir variable
    println!("cargo:rustc-env=OUT_DIR={}", env::var("OUT_DIR").unwrap());

    // build static library
    let dst = Config::new("hand_indexer").build();

    // link library
    // since this is a workspace, we are still searching relative to crate root
    println!("cargo:rustc-link-search={}", dst.display());
    println!("cargo:rustc-link-lib=static=handindexer");

    // Tell cargo to invalidate the built crate whenever the wrapper changes
    println!("cargo:rerun-if-changed=hand_indexer/wrapper.h");

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        .header("hand_indexer/wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    // generate hand eval table
    gen_eval_table::gen_eval_table();
}
