use std::{env, fs, path::Path};

use miden_client::Deserializable;
use miden_client::utils::Serializable;
use miden_client::vm::Package;

fn main() {
    // TODO: run cargo miden build --release and specify target directory as OUT_DIR
    let workspace_root = env::var("CARGO_MANIFEST_DIR")
        .map(|d| Path::new(&d).parent().unwrap().parent().unwrap().to_path_buf())
        .unwrap();

    // Rebuild when mint-script source changes
    println!("cargo:rerun-if-changed=../mint-script/src/lib.rs");

    // Look for compiled .masp file from mint-script
    let masp_path = workspace_root
        .join("target")
        .join("miden")
        .join("release")
        .join("mint_script.masp");

    let build_dir = env::var("OUT_DIR").unwrap();
    let target_dir = Path::new(&build_dir);
    fs::create_dir_all(&target_dir).expect("should create target directory");

    // Compile .masp to .txs
    let masp = fs::read(masp_path).expect("failed to read mint_script.masp");
    let package =
        Package::read_from_bytes(&masp).expect("failed to read mint_script.masp as package");
    let txs_path = target_dir.join("mint.txs");
    fs::write(txs_path, package.to_bytes()).expect("failed to write mint.txs");
}
