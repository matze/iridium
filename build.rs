use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    let out_path = Path::new(&env::var_os("OUT_DIR").unwrap()).join("resources.gresource");

    let args = [
        format!("--target={}", out_path.display()),
        "data/resources.gresource.xml".to_string(),
    ];

    Command::new("glib-compile-resources")
        .args(&args)
        .output()
        .expect("failure");

    println!("cargo:rerun-if-changed=data/resources.gresource.xml");
    println!("cargo:rerun-if-changed=data/resources/ui/window.ui");
}
