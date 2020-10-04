extern crate gen_eval_table;

fn main() {
    // let static_asset_dir: &str = "static_asssets";

    // static asset dir
    // println!("cargo:rustc-env=STATIC_ASSET_DIR={}", static_asset_dir);

    // generate hand eval table
    gen_eval_table::gen_eval_table();
}
