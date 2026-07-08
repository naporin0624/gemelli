use std::{
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use clap::{Parser, Subcommand};

mod license_entry;
mod merge;
mod normalize;
mod render;
mod sort;

use license_entry::LicenseEntry;

#[derive(Debug, thiserror::Error)]
enum XtaskError {
    #[error("failed to spawn `cargo bundle-licenses`: {0}")]
    Spawn(std::io::Error),
    #[error("`cargo bundle-licenses` exited with an error:\n{0}")]
    Subprocess(String),
    #[error("`cargo bundle-licenses` output was not valid UTF-8: {0}")]
    Utf8(std::string::FromUtf8Error),
    #[error("failed to parse JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("I/O error: {0}")]
    Io(std::io::Error),
    #[error(
        "license artifact is stale: {0} does not match freshly generated output; run `cargo xtask gen-licenses`"
    )]
    Stale(String),
}

#[derive(Parser)]
#[command(name = "xtask")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

// Named `Commands`, not `Command` — `Command` collides with `std::process::Command` imported
// above for the subprocess call, which fails to compile (E0255 name defined multiple times /
// E0117 orphan rule on the derive).
#[derive(Subcommand)]
enum Commands {
    /// Regenerate crates/gui/assets/third-party-licenses.json and THIRD-PARTY-NOTICES.
    GenLicenses {
        /// Regenerate into memory and byte-compare against the committed files instead of
        /// writing them; exits nonzero if either file is stale.
        #[arg(long)]
        check: bool,
    },
}

struct Artifacts {
    json: String,
    notices: String,
}

fn project_root() -> PathBuf {
    // crates/xtask -> crates -> repo root. Standard cargo-xtask pattern: CARGO_MANIFEST_DIR is
    // resolved at compile time so this works regardless of the caller's current directory.
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.pop();
    root.pop();
    root
}

fn run_cargo_bundle_licenses() -> Result<normalize::CargoBundleOutput, XtaskError> {
    let output = Command::new("cargo")
        .args(["bundle-licenses", "--format", "json"])
        .output()
        .map_err(XtaskError::Spawn)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(XtaskError::Subprocess(stderr));
    }

    let stdout = String::from_utf8(output.stdout).map_err(XtaskError::Utf8)?;
    serde_json::from_str(&stdout).map_err(XtaskError::Json)
}

fn read_appendix(root: &Path) -> Result<Vec<LicenseEntry>, XtaskError> {
    let path = root.join("licenses/appendix.json");
    let raw = std::fs::read_to_string(path).map_err(XtaskError::Io)?;
    serde_json::from_str(&raw).map_err(XtaskError::Json)
}

fn build_artifacts(root: &Path) -> Result<Artifacts, XtaskError> {
    let scanned = run_cargo_bundle_licenses()?;
    let scanner_entries = normalize::normalize(scanned);
    let appendix_entries = read_appendix(root)?;
    let merged = merge::merge(scanner_entries, appendix_entries);
    let sorted = sort::sort_entries(merged);

    let mut json = serde_json::to_string_pretty(&sorted).map_err(XtaskError::Json)?;
    json.push('\n');
    let notices = render::render_notices(&sorted);

    Ok(Artifacts { json, notices })
}

fn check_matches(path: &Path, expected: &str) -> Result<(), XtaskError> {
    let actual = std::fs::read_to_string(path).map_err(XtaskError::Io)?;
    if actual != expected {
        return Err(XtaskError::Stale(path.display().to_string()));
    }
    Ok(())
}

fn gen_licenses(check: bool) -> Result<(), XtaskError> {
    let root = project_root();
    let artifacts = build_artifacts(&root)?;
    let assets_path = root.join("crates/gui/assets/third-party-licenses.json");
    let notices_path = root.join("THIRD-PARTY-NOTICES");

    if check {
        check_matches(&assets_path, &artifacts.json)?;
        check_matches(&notices_path, &artifacts.notices)?;
        println!("license artifacts are up to date");
        return Ok(());
    }

    if let Some(parent) = assets_path.parent() {
        std::fs::create_dir_all(parent).map_err(XtaskError::Io)?;
    }
    std::fs::write(&assets_path, &artifacts.json).map_err(XtaskError::Io)?;
    std::fs::write(&notices_path, &artifacts.notices).map_err(XtaskError::Io)?;
    println!("wrote {}", assets_path.display());
    println!("wrote {}", notices_path.display());
    Ok(())
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::GenLicenses { check } => gen_licenses(check),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}
