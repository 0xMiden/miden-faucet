use std::path::Path;

use anyhow::{Context, bail};
use cargo_miden::{OutputType, run};
use miden_client::Deserializable;
use miden_client::vm::Package;

/// Builds a Miden project in the specified directory
///
/// # Arguments
/// * `dir` - Path to the directory containing the Cargo.toml
/// * `release` - Whether to build in release mode
///
/// # Returns
/// The compiled `Package`
///
/// # Errors
/// Returns an error if compilation fails or if the output is not in the expected format
pub fn build_project_in_dir(dir: &Path, release: bool) -> anyhow::Result<Package> {
    let profile = if release { "--release" } else { "--debug" };
    let manifest_path = dir.join("Cargo.toml");
    let manifest_arg = manifest_path.to_string_lossy();

    let args = vec!["cargo", "miden", "build", profile, "--manifest-path", &manifest_arg];

    let output = run(args.into_iter().map(String::from), OutputType::Masm)
        .context("Failed to compile project")?
        .context("Cargo miden build returned None")?;

    let artifact_path = match output {
        cargo_miden::CommandOutput::BuildCommandOutput { output } => match output {
            cargo_miden::BuildOutput::Masm { artifact_path } => artifact_path,
            other @ cargo_miden::BuildOutput::Wasm { .. } => {
                bail!("Expected Masm output, got {other:?}")
            },
        },
        other @ cargo_miden::CommandOutput::NewCommandOutput { .. } => {
            bail!("Expected BuildCommandOutput, got {other:?}")
        },
    };

    let package_bytes = std::fs::read(&artifact_path)
        .context(format!("Failed to read compiled package from {}", artifact_path.display()))?;

    Package::read_from_bytes(&package_bytes).context("Failed to deserialize package from bytes")
}

/// Builds a Miden project and returns the intermediate MASM text.
///
/// This is useful for debugging the compiler output.
#[cfg(test)]
pub fn build_project_in_dir_masm(dir: &Path, release: bool) -> anyhow::Result<String> {
    let profile = if release { "--release" } else { "--debug" };
    let manifest_path = dir.join("Cargo.toml");
    let manifest_arg = manifest_path.to_string_lossy();

    // First build normally to produce the Wasm
    let args = vec!["cargo", "miden", "build", profile, "--manifest-path", &manifest_arg];
    let _ = run(args.into_iter().map(String::from), OutputType::Wasm)
        .context("Failed to compile project to Wasm")?
        .context("Cargo miden build returned None")?;

    // Now invoke midenc directly on the Wasm to get MASM text
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let wasm_path = workspace_root.join("target/wasm32-wasip2")
        .join(if release { "release" } else { "debug" })
        .join("miden_faucet_mint_tx.wasm");

    let out_dir = std::env::temp_dir().join("miden_masm_text");
    std::fs::create_dir_all(&out_dir)?;
    let output_file = out_dir.join("miden_faucet_mint_tx.masm");

    use std::rc::Rc;
    let input = midenc_session::InputFile::from_path(&wasm_path)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let output_file = out_dir.join("miden_faucet_mint_tx.wat");
    let args: Vec<String> = vec![
        "--output-dir".into(),
        out_dir.to_str().unwrap().into(),
        "-o".into(),
        output_file.to_str().unwrap().into(),
        "--emit".into(),
        "wat".into(),
    ];
    let session = Rc::new(midenc_compile::Compiler::new_session([input], None, args));
    let context = Rc::new(midenc_compile::Context::new(session));
    midenc_compile::compile(context).map_err(|e| anyhow::anyhow!("{e}"))?;

    std::fs::read_to_string(&output_file).context("Failed to read HIR output")
}
