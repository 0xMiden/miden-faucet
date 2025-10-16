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

    assert!(npm_install.success(), "npm install failed");

    let npm_build = Command::new("npm")
        .arg("run")
        .arg("build")
        .current_dir("frontend")
        .status()
        .expect("Failed to execute npm run build");

    assert!(npm_build.success(), "npm run build failed");
}
