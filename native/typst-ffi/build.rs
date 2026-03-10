use std::env;
use std::path::PathBuf;

fn main() {
    // Generate C header for the FFI interface.
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let config = cbindgen::Config::from_file("cbindgen.toml")
        .expect("Unable to read cbindgen.toml");

    let header_path = PathBuf::from(&crate_dir)
        .join("include")
        .join("typst_ffi.h");

    // Ensure the include directory exists.
    std::fs::create_dir_all(
        header_path.parent().unwrap()
    ).ok();

    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(config)
        .generate()
        .expect("Unable to generate C bindings")
        .write_to_file(header_path);
}
