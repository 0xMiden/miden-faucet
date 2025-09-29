use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=frontend/index.js");
    println!("cargo:rerun-if-changed=frontend/package.json");
    println!("cargo:rerun-if-changed=frontend/package-lock.json");

    let npm_install = Command::new("npm")
        .arg("install")
        .current_dir("frontend")
        .status()
        .expect("Failed to execute npm install");

    if !npm_install.success() {
        panic!("npm install failed");
    }

    let npm_build = Command::new("npm")
        .arg("run")
        .arg("build")
        .current_dir("frontend")
        .status()
        .expect("Failed to execute npm run build");

    if !npm_build.success() {
        panic!("npm run build failed");
    }
}
