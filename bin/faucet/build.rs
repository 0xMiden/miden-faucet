use std::path::Path;
use std::process::Command;
use std::{env, fs};

fn main() {
    println!("cargo:rerun-if-changed=frontend/api.js");
    println!("cargo:rerun-if-changed=frontend/app.js");
    println!("cargo:rerun-if-changed=frontend/ui.js");
    println!("cargo:rerun-if-changed=frontend/utils.js");
    println!("cargo:rerun-if-changed=frontend/package.json");
    println!("cargo:rerun-if-changed=frontend/package-lock.json");

    let build_dir = env::var("OUT_DIR").expect("OUT_DIR should be set");
    let target_dir = Path::new(&build_dir).join("frontend");

    fs::create_dir_all(&target_dir).expect("target directory should be created");
    copy_dir_all(Path::new("frontend"), &target_dir)
        .expect("frontend directory should be copied to target directory");

    let npm_install = Command::new("npm")
        .arg("install")
        .current_dir(&target_dir)
        .status()
        .expect("npm install should succeed");

    assert!(npm_install.success(), "npm install failed");

    let npm_build = Command::new("npm")
        .arg("run")
        .arg("build")
        .current_dir(&target_dir)
        .status()
        .expect("npm run build should succeed");

    assert!(npm_build.success(), "npm run build failed");
}

/// Copy all files from source directory to destination directory. Skips inner directories.
fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}
