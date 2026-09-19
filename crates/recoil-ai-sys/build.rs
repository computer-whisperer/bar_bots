use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed=vendor");
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg("-Ivendor/rts")
        .allowlist_type("S.*(Event|Command)")
        .allowlist_type("SSkirmishAICallback")
        .allowlist_type("EventTopic|CommandTopic|UnitCommandOptions")
        .allowlist_var("COMMAND_.*|EVENT_.*|UNIT_COMMAND_.*")
        .prepend_enum_name(false)
        .layout_tests(false)
        .generate()
        .expect("bindgen failed on the vendored engine headers");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings.write_to_file(out.join("bindings.rs")).unwrap();
}
