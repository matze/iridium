use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("resources.gresource");

    let args = [
        format!("--target={}", out_path.display()),
        "data/resources.gresource.xml".to_string(),
    ];

    Command::new("glib-compile-resources")
        .args(&args)
        .output()
        .unwrap();

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=data/resources.gresource.xml");
    println!("cargo:rerun-if-changed=data/resources/css/base.css");
    println!("cargo:rerun-if-changed=data/resources/ui/about.ui");
    println!("cargo:rerun-if-changed=data/resources/ui/import.ui");
    println!("cargo:rerun-if-changed=data/resources/ui/setup.ui");
    println!("cargo:rerun-if-changed=data/resources/ui/shortcuts.ui");
    println!("cargo:rerun-if-changed=data/resources/ui/window.ui");
}
