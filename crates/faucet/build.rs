use std::env;
use std::fs::{self};
use std::io::{self};
use std::path::{Path, PathBuf};

use miden_client::transaction::TransactionKernel;
use miden_client::utils::Serializable;

const ASSETS_DIR: &str = "assets";
const ASM_DIR: &str = "asm";
const ASM_TX_SCRIPTS_DIR: &str = "tx_scripts";

/// Compile contents of asm directory into .masb files.
fn main() {
    // re-build when the MASM code changes
    println!("cargo::rerun-if-changed={ASM_DIR}/");

    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let source_dir = Path::new(&crate_dir).join(ASM_DIR);

    let build_dir = env::var("OUT_DIR").unwrap();
    let target_dir = Path::new(&build_dir).join(ASSETS_DIR);

    compile_transaction_scripts(
        &source_dir.join(ASM_TX_SCRIPTS_DIR),
        &target_dir.join(ASM_TX_SCRIPTS_DIR),
    );
}

/// Reads all MASM files from the `source_dir`, complies each file individually into a MASB
/// file, and stores the compiled files into the `target_dir`.
///
/// The source files are expected to contain executable programs.
fn compile_transaction_scripts(source_dir: &Path, target_dir: &Path) {
    fs::create_dir_all(target_dir).expect("should create target directory");
    let assembler = TransactionKernel::assembler();

    let masm_files = get_masm_files(source_dir).expect("should find MASM files");
    for masm_file_path in masm_files {
        // read the MASM file, parse it, and serialize the parsed AST to bytes
        let code = assembler
            .clone()
            .assemble_program(masm_file_path.clone())
            .expect("program should assemble correctly");

        let bytes = code.to_bytes();

        let masm_file_name = masm_file_path.file_name().expect("file name should exist");
        let mut masb_file_path = target_dir.join(masm_file_name);

        // write the binary MASB to the output dir
        masb_file_path.set_extension("masb");
        fs::write(masb_file_path, bytes).expect("should write .masb file");
    }
}

// HELPER FUNCTIONS
// ================================================================================================

/// Returns a vector with paths to all MASM files in the specified directory.
///
/// All non-MASM files are skipped.
fn get_masm_files(dir_path: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    let entries = fs::read_dir(dir_path)?;
    for entry in entries {
        let file_path = entry?.path();
        if is_masm_file(&file_path)? {
            files.push(file_path);
        }
    }

    Ok(files)
}

/// Returns true if the provided path resolves to a file with `.masm` extension.
///
/// # Errors
/// Returns an error if the path could not be converted to a UTF-8 string.
fn is_masm_file(path: &Path) -> io::Result<bool> {
    if let Some(extension) = path.extension() {
        let extension = extension
            .to_str()
            .ok_or_else(|| io::Error::other("invalid UTF-8 filename"))?
            .to_lowercase();
        Ok(extension == "masm")
    } else {
        Ok(false)
    }
}
