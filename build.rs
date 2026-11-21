use std::{env, path::PathBuf, process::Command};

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let arch = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64",
        Ok("x86_64") => "x86_64",
        _ => panic!("Unsupported macOS architecture"),
    };
    let mut objects = Vec::new();
    for module in ["menu", "oled_overlay"] {
        let source = format!("src/{module}.m");
        let object = out.join(format!("{module}.o"));
        assert!(Command::new("xcrun")
            .args([
                "clang",
                "-arch",
                arch,
                "-mmacosx-version-min=12.0",
                "-fobjc-arc",
                "-c"
            ])
            .arg(&source)
            .arg("-o")
            .arg(&object)
            .status()
            .unwrap()
            .success());
        println!("cargo:rerun-if-changed={source}");
        objects.push(object);
    }
    assert!(Command::new("ar")
        .arg("crs")
        .arg(out.join("libmenu.a"))
        .args(objects)
        .status()
        .unwrap()
        .success());
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=menu");
    println!("cargo:rustc-link-lib=framework=AppKit");
}
