# Installer Distribution (bucatini 方式) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Windows インストーラー (`gemelli-<ver>-windows-x64-setup.exe` + `.zip`) を `cargo xtask dist` で生成し、release-please とビルドジョブを単一 workflow に統合してリリース時に macOS/Windows の全成果物が自動で GitHub Release に添付されるようにする。

**Architecture:** 既存の `crates/xtask/src/bundle/` (macOS `.dmg`/`.tar.gz` 実装済み) に Windows パッケージングを追加する。純関数 (命名・引数組み立て・staging ペア) は `bundle/windows.rs` と `bundle/cmd.rs` に置き macOS 上でもユニットテストする。プロセス起動は既存 `run_checked` パターン。`bundle::dist` は `cfg!(windows)` / `cfg!(target_os = "macos")` の**実行時ディスパッチ**にし (cfg 属性ではなく)、両 OS のコードが常にコンパイル・lint・テストされるようにする。

**Tech Stack:** Rust (xtask), Inno Setup 6 (ISCC.exe), bsdtar (`tar -a`, Windows 10+ 標準), GitHub Actions (release-please-action v5, manifest/cargo-workspace 構成), Pillow (`scripts/gen-icon.py`)。

## Global Constraints

- ワークスペース lint: `unwrap_used = "deny"`, `expect_used = "deny"`, `as_conversions = "deny"` — unwrap/expect/`as` は書かない。
- Rust toolchain: CI は `dtolnay/rust-toolchain@1.96.1`。
- 成果物命名は小文字 `gemelli-` 始まり: `gemelli-<ver>-windows-x64.zip` / `gemelli-<ver>-windows-x64-setup.exe` (既存 `gemelli-<ver>-macos-universal.dmg` と揃える)。
- release-please は **crate ごと**にタグ/Release を作る (`gemelli-gui-v0.4.0` など)。配布物はすべて **`gemelli-gui-v*` の Release に添付**する。action v5 の per-path outputs は `steps.release.outputs['crates/gui--release_created']` / `['crates/gui--tag_name']`。
- gemelli は Spout/Syphon を同梱・静的リンクしており、外部ランタイム案内は書かない。
- コミットメッセージは conventional commits (release-please が読む)。コミットは worktree ブランチ `worktree-feat-installer-distribution` 上で行う。
- 各タスク完了時: `cargo fmt --all -- --check` && `cargo clippy --workspace --all-targets -- -D warnings` が通ること (pre-commit hook でも走る)。

---

### Task 1: cmd.rs — Windows 用コマンド引数ビルダー 3 種

**Files:**
- Modify: `crates/xtask/src/bundle/cmd.rs` (末尾の `#[cfg(test)] mod tests` の前に関数、tests 内にテスト追加)

**Interfaces:**
- Consumes: なし (葉モジュール)
- Produces:
  - `pub fn cargo_build_release_args(packages: &[&str]) -> Vec<OsString>`
  - `pub fn tar_zip_args(output: &Path, chdir: &Path, entry: &str) -> Vec<OsString>`
  - `pub fn iscc_args(version: &str, source_dir: &Path, output_dir: &Path, iss: &Path) -> Vec<OsString>`

- [ ] **Step 1: Write the failing tests**

`crates/xtask/src/bundle/cmd.rs` の `mod tests` 内に追加:

```rust
    #[test]
    fn cargo_build_release_args_lists_each_package_after_its_own_flag() {
        let args = cargo_build_release_args(&["gemelli-cli", "gemelli-gui"]);

        assert_eq!(
            args,
            vec![
                OsString::from("build"),
                OsString::from("--release"),
                OsString::from("-p"),
                OsString::from("gemelli-cli"),
                OsString::from("-p"),
                OsString::from("gemelli-gui"),
            ]
        );
    }

    #[test]
    fn tar_zip_args_uses_auto_format_and_chdirs_before_naming_the_entry() {
        let output = PathBuf::from("target/dist/gemelli-0.4.0-windows-x64.zip");
        let chdir = PathBuf::from("target/dist");

        let args = tar_zip_args(&output, &chdir, "gemelli-0.4.0-windows-x64");

        assert_eq!(
            args,
            vec![
                OsString::from("-a"),
                OsString::from("-c"),
                OsString::from("-f"),
                OsString::from("target/dist/gemelli-0.4.0-windows-x64.zip"),
                OsString::from("-C"),
                OsString::from("target/dist"),
                OsString::from("gemelli-0.4.0-windows-x64"),
            ]
        );
    }

    #[test]
    fn iscc_args_passes_defines_then_script_path() {
        let source_dir = PathBuf::from("/work/target/dist/gemelli-0.4.0-windows-x64");
        let output_dir = PathBuf::from("/work/target/dist");
        let iss = PathBuf::from("/work/packaging/windows/gemelli.iss");

        let args = iscc_args("0.4.0", &source_dir, &output_dir, &iss);

        assert_eq!(
            args,
            vec![
                OsString::from("/DMyAppVersion=0.4.0"),
                OsString::from("/DSourceDir=/work/target/dist/gemelli-0.4.0-windows-x64"),
                OsString::from("/DOutputDir=/work/target/dist"),
                OsString::from("/work/packaging/windows/gemelli.iss"),
            ]
        );
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p xtask cargo_build_release_args_lists -- --nocapture` (worktree ルートで)
Expected: コンパイルエラー `cannot find function cargo_build_release_args` (3 関数とも未定義)

- [ ] **Step 3: Write minimal implementation**

`crates/xtask/src/bundle/cmd.rs` の `tar_czf_args` の後、`mod tests` の前に追加:

```rust
/// `cargo build --release -p <pkg>...` — host-target release build of the given packages,
/// used on Windows where a single-arch build (no `--target`) is all that's needed.
pub fn cargo_build_release_args(packages: &[&str]) -> Vec<OsString> {
    let mut args = vec![OsString::from("build"), OsString::from("--release")];
    for package in packages {
        args.push(OsString::from("-p"));
        args.push(OsString::from(*package));
    }
    args
}

/// `tar -a -c -f <output> -C <chdir> <entry>` — bsdtar (bundled with Windows 10+) infers the
/// zip format from the `.zip` extension via `-a`, avoiding a zip crate dependency.
pub fn tar_zip_args(output: &Path, chdir: &Path, entry: &str) -> Vec<OsString> {
    vec![
        OsString::from("-a"),
        OsString::from("-c"),
        OsString::from("-f"),
        output.as_os_str().to_os_string(),
        OsString::from("-C"),
        chdir.as_os_str().to_os_string(),
        OsString::from(entry),
    ]
}

/// `ISCC.exe /DMyAppVersion=<v> /DSourceDir=<dir> /DOutputDir=<dir> <script.iss>` — ISCC
/// resolves relative paths against the .iss file's directory, not the CWD, so callers must
/// pass absolute staging/output paths.
pub fn iscc_args(version: &str, source_dir: &Path, output_dir: &Path, iss: &Path) -> Vec<OsString> {
    let mut source_define = OsString::from("/DSourceDir=");
    source_define.push(source_dir.as_os_str());
    let mut output_define = OsString::from("/DOutputDir=");
    output_define.push(output_dir.as_os_str());
    vec![
        OsString::from(format!("/DMyAppVersion={version}")),
        source_define,
        output_define,
        iss.as_os_str().to_os_string(),
    ]
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p xtask`
Expected: PASS (既存テスト含め全 green)

- [ ] **Step 5: Lint & Commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
git add crates/xtask/src/bundle/cmd.rs
git commit -m "feat(xtask): add windows packaging command builders"
```

---

### Task 2: windows.rs — 命名・staging・ISCC パス解決の純関数

**Files:**
- Create: `crates/xtask/src/bundle/windows.rs`
- Modify: `crates/xtask/src/bundle/mod.rs` (`pub mod windows;` を `pub mod readme;` の後に追加)

**Interfaces:**
- Consumes: なし (この段階では葉モジュール)
- Produces:
  - `pub fn stage_dir_name(version: &str) -> String` → `"gemelli-<ver>-windows-x64"`
  - `pub fn zip_name(version: &str) -> String` → `"gemelli-<ver>-windows-x64.zip"`
  - `pub fn staging_pairs(root: &Path, staging_dir: &Path) -> Vec<(PathBuf, PathBuf)>`
  - `pub fn iscc_path(env_override: Option<OsString>) -> PathBuf`

  (setup.exe の名前は ISCC が `.iss` の `OutputBaseFilename` から決めるので Rust 側に命名関数は不要)

- [ ] **Step 1: Write the failing tests**

`crates/xtask/src/bundle/windows.rs` を新規作成 (まずテストのみ意識した骨格。ファイルは実装込みで一度に書くが、先にテストを書いてから実装を埋める):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_dir_name_embeds_version_between_fixed_segments() {
        assert_eq!(stage_dir_name("0.4.0"), "gemelli-0.4.0-windows-x64");
    }

    #[test]
    fn zip_name_appends_zip_to_the_stage_dir_name() {
        assert_eq!(zip_name("0.4.0"), "gemelli-0.4.0-windows-x64.zip");
    }

    #[test]
    fn staging_pairs_maps_every_distribution_file_into_the_staging_dir() {
        let root = PathBuf::from("/repo");
        let staging_dir = PathBuf::from("/repo/target/dist/gemelli-0.4.0-windows-x64");

        let pairs = staging_pairs(&root, &staging_dir);

        assert_eq!(
            pairs,
            vec![
                (
                    PathBuf::from("/repo/target/release/gemelli.exe"),
                    PathBuf::from("/repo/target/dist/gemelli-0.4.0-windows-x64/gemelli.exe"),
                ),
                (
                    PathBuf::from("/repo/target/release/gemelli-gui.exe"),
                    PathBuf::from("/repo/target/dist/gemelli-0.4.0-windows-x64/gemelli-gui.exe"),
                ),
                (
                    PathBuf::from("/repo/crates/gui/assets/icon.ico"),
                    PathBuf::from("/repo/target/dist/gemelli-0.4.0-windows-x64/icon.ico"),
                ),
                (
                    PathBuf::from("/repo/README.md"),
                    PathBuf::from("/repo/target/dist/gemelli-0.4.0-windows-x64/README.md"),
                ),
                (
                    PathBuf::from("/repo/THIRD-PARTY-NOTICES"),
                    PathBuf::from("/repo/target/dist/gemelli-0.4.0-windows-x64/THIRD-PARTY-NOTICES"),
                ),
            ]
        );
    }

    #[test]
    fn iscc_path_prefers_the_env_override() {
        let path = iscc_path(Some(OsString::from(r"D:\tools\ISCC.exe")));

        assert_eq!(path, PathBuf::from(r"D:\tools\ISCC.exe"));
    }

    #[test]
    fn iscc_path_defaults_to_the_standard_inno_setup_6_install() {
        let path = iscc_path(None);

        assert_eq!(path, PathBuf::from(r"C:\Program Files (x86)\Inno Setup 6\ISCC.exe"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p xtask stage_dir_name`
Expected: コンパイルエラー (`stage_dir_name` 未定義。`mod windows;` 追加後なら E0425)

- [ ] **Step 3: Write minimal implementation**

`crates/xtask/src/bundle/windows.rs` の先頭 (tests の前) に:

```rust
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

/// Directory name for the Windows distribution staging dir, e.g. `gemelli-0.4.0-windows-x64`.
/// The `.zip` and the Inno Setup `OutputBaseFilename` both derive from this stem.
pub fn stage_dir_name(version: &str) -> String {
    format!("gemelli-{version}-windows-x64")
}

/// File name of the plain-archive artifact, e.g. `gemelli-0.4.0-windows-x64.zip`.
pub fn zip_name(version: &str) -> String {
    format!("{}.zip", stage_dir_name(version))
}

/// (source, destination) pairs copied into the Windows staging directory: both binaries, the
/// icon Inno Setup references at run time, and the docs bundled alongside them.
pub fn staging_pairs(root: &Path, staging_dir: &Path) -> Vec<(PathBuf, PathBuf)> {
    [
        ("target/release/gemelli.exe", "gemelli.exe"),
        ("target/release/gemelli-gui.exe", "gemelli-gui.exe"),
        ("crates/gui/assets/icon.ico", "icon.ico"),
        ("README.md", "README.md"),
        ("THIRD-PARTY-NOTICES", "THIRD-PARTY-NOTICES"),
    ]
    .into_iter()
    .map(|(source, destination)| (root.join(source), staging_dir.join(destination)))
    .collect()
}

/// Resolves `ISCC.exe`: the `ISCC_PATH` environment override if set, else the path the
/// stock Inno Setup 6 installer (and its chocolatey package) uses.
pub fn iscc_path(env_override: Option<OsString>) -> PathBuf {
    env_override
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)\Inno Setup 6\ISCC.exe"))
}
```

`crates/xtask/src/bundle/mod.rs` の module 宣言部を:

```rust
pub mod cmd;
pub mod layout;
pub mod plist;
pub mod readme;
pub mod windows;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p xtask`
Expected: PASS (新規 5 テスト含む)

- [ ] **Step 5: Lint & Commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
git add crates/xtask/src/bundle/windows.rs crates/xtask/src/bundle/mod.rs
git commit -m "feat(xtask): add windows dist naming and staging layout"
```

---

### Task 3: windows::dist オーケストレーション + OS ディスパッチ

**Files:**
- Modify: `crates/xtask/src/bundle/windows.rs` (末尾の tests の前に `dist` を追加)
- Modify: `crates/xtask/src/bundle/mod.rs` (`run_checked` の引数型変更、`dist` → `dist_macos` 改名 + 新 `dist` ディスパッチ)
- Modify: `crates/xtask/src/main.rs` (`XtaskError` に `UnsupportedHost` variant 追加)

**Interfaces:**
- Consumes: Task 1 の `cmd::cargo_build_release_args` / `cmd::tar_zip_args` / `cmd::iscc_args`、Task 2 の `stage_dir_name` / `zip_name` / `staging_pairs` / `iscc_path`、既存の `super::run_checked` / `super::cargo_metadata_json` / `layout::gui_package_version`
- Produces: `pub fn windows::dist(root: &Path) -> Result<(), crate::XtaskError>`。`bundle::dist` は従来シグネチャのまま OS ディスパッチになる (main.rs の呼び出しは無変更)。

- [ ] **Step 1: `XtaskError::UnsupportedHost` を追加**

`crates/xtask/src/main.rs` の `XtaskError` enum 末尾 (`PackageNotFound` の後) に:

```rust
    #[error("`cargo xtask dist` supports only macOS and Windows hosts")]
    UnsupportedHost,
}
```

- [ ] **Step 2: `run_checked` をコマンド名 `OsStr` 対応にする**

ISCC は `PathBuf` で解決されるため、`crates/xtask/src/bundle/mod.rs` の `run_checked` シグネチャを変更 (既存の `&str` 呼び出しはそのままコンパイルされる):

```rust
/// Runs `command` with `args` in `cwd`, mapping spawn failure and nonzero exit into `XtaskError`.
fn run_checked(
    command: impl AsRef<std::ffi::OsStr>,
    args: &[OsString],
    cwd: &Path,
) -> Result<(), crate::XtaskError> {
    let command_name = command.as_ref().to_string_lossy().into_owned();
    let output = Command::new(command.as_ref())
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|source| crate::XtaskError::Spawn { command: command_name.clone(), source })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(crate::XtaskError::Subprocess { command: command_name, stderr });
    }

    Ok(())
}
```

- [ ] **Step 3: `dist` を `dist_macos` に改名し、ディスパッチする `dist` を書く**

`crates/xtask/src/bundle/mod.rs` — 既存 `pub fn dist` を `fn dist_macos` に改名し (doc コメントは維持)、その直前に新しい public `dist` を置く:

```rust
/// Packages the host OS's distribution artifacts under `target/dist/`: `.dmg` + CLI `.tar.gz`
/// on macOS, `.zip` + Inno Setup installer on Windows. `cfg!` (not `#[cfg]`) keeps both
/// per-OS paths compiling — and their unit tests running — on every host.
pub fn dist(root: &Path) -> Result<(), crate::XtaskError> {
    if cfg!(windows) {
        windows::dist(root)
    } else if cfg!(target_os = "macos") {
        dist_macos(root)
    } else {
        Err(crate::XtaskError::UnsupportedHost)
    }
}

/// Assembles `target/dist/gemelli-<version>-macos-universal.dmg` (wrapping the `.app` from
/// [`bundle`]) and `target/dist/gemelli-<version>-macos-universal.tar.gz` (a universal2 CLI
/// binary plus its framework and docs).
fn dist_macos(root: &Path) -> Result<(), crate::XtaskError> {
    // (既存 dist の本体をそのまま — 変更なし)
```

- [ ] **Step 4: `windows::dist` を実装**

`crates/xtask/src/bundle/windows.rs` の `iscc_path` の後、`mod tests` の前に追加。冒頭の `use` に `std::fs` を足す:

```rust
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};
```

```rust
/// Assembles `target/dist/gemelli-<version>-windows-x64.zip` and
/// `target/dist/gemelli-<version>-windows-x64-setup.exe` (via Inno Setup's ISCC, resolved
/// through [`iscc_path`]). Builds both release binaries first, mirroring the macOS `dist`.
pub fn dist(root: &Path) -> Result<(), crate::XtaskError> {
    let build_args = super::cmd::cargo_build_release_args(&["gemelli-cli", "gemelli-gui"]);
    super::run_checked("cargo", &build_args, root)?;

    let metadata_json = super::cargo_metadata_json(root)?;
    let version = super::layout::gui_package_version(&metadata_json)?;

    let dist_dir = root.join("target/dist");
    let staging_dir = dist_dir.join(stage_dir_name(&version));
    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir).map_err(|source| super::io_error(&staging_dir, source))?;
    }
    fs::create_dir_all(&staging_dir).map_err(|source| super::io_error(&staging_dir, source))?;
    for (source, destination) in staging_pairs(root, &staging_dir) {
        fs::copy(&source, &destination).map_err(|error| super::io_error(&destination, error))?;
    }

    let zip_path = dist_dir.join(zip_name(&version));
    let tar_args = super::cmd::tar_zip_args(&zip_path, &dist_dir, &stage_dir_name(&version));
    super::run_checked("tar", &tar_args, root)?;

    let iss_path = root.join("packaging/windows/gemelli.iss");
    let iscc = iscc_path(std::env::var_os("ISCC_PATH"));
    let iscc_invocation_args = super::cmd::iscc_args(&version, &staging_dir, &dist_dir, &iss_path);
    super::run_checked(&iscc, &iscc_invocation_args, root)
}
```

注: `super::io_error` / `super::run_checked` / `super::cargo_metadata_json` は `bundle/mod.rs` の private アイテムだが、Rust では親モジュールの private アイテムは子モジュール (`bundle::windows`) から可視なのでそのまま呼べる。可視性変更は不要。

- [ ] **Step 5: Run tests / lint**

Run: `cargo test -p xtask && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: PASS。(`windows::dist` は macOS ではコンパイルのみ — 実行パスは CI の Windows ジョブで検証)

- [ ] **Step 6: `cargo run -p xtask -- dist` の macOS 経路が壊れていないことを確認 (smoke)**

Run: `cargo run -p xtask -- dist 2>&1 | tail -3`
Expected: `wrote dist artifacts to .../target/dist` (dmg/tar.gz 生成。数分かかる — universal2 ビルドを含むため)

- [ ] **Step 7: Commit**

```bash
git add crates/xtask/src/bundle/windows.rs crates/xtask/src/bundle/mod.rs crates/xtask/src/main.rs
git commit -m "feat(xtask): dispatch dist per host os and add windows zip + installer build"
```

---

### Task 4: packaging/windows/gemelli.iss

**Files:**
- Create: `packaging/windows/gemelli.iss`

**Interfaces:**
- Consumes: Task 3 の `cmd::iscc_args` が渡す `/DMyAppVersion` `/DSourceDir` `/DOutputDir`。SourceDir には Task 2 の `staging_pairs` が並べた 5 ファイルがある前提。
- Produces: `{#OutputDir}\gemelli-{#MyAppVersion}-windows-x64-setup.exe`

- [ ] **Step 1: `.iss` を書く**

```ini
; Inno Setup script for gemelli (Windows x64).
; Build with: ISCC.exe /DMyAppVersion=X.Y.Z /DSourceDir=<staged> /DOutputDir=<out> gemelli.iss
; Spout/Syphon are compiled in; no external runtime is required.

#ifndef MyAppVersion
  #define MyAppVersion "0.0.0"
#endif
#ifndef SourceDir
  #define SourceDir "."
#endif
#ifndef OutputDir
  #define OutputDir "dist"
#endif

#define MyAppName "gemelli"
#define MyAppPublisher "naporin0624"
#define MyAppExeName "gemelli-gui.exe"
#define MyAppIco SourceDir + "\icon.ico"

[Setup]
; A stable AppId keeps upgrades/uninstall consistent across versions.
AppId={{3FDAFA9C-6113-4555-8F3A-8FE7E9FFD465}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#OutputDir}
OutputBaseFilename=gemelli-{#MyAppVersion}-windows-x64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
SetupIconFile={#MyAppIco}
UninstallDisplayIcon={app}\icon.ico

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\gemelli.exe";          DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\gemelli-gui.exe";      DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\icon.ico";             DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\README.md";            DestDir: "{app}"; Flags: ignoreversion isreadme
Source: "{#SourceDir}\THIRD-PARTY-NOTICES";  DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}";           Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\icon.ico"
Name: "{group}\Uninstall {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}";     Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\icon.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; Flags: nowait postinstall skipifsilent
```

- [ ] **Step 2: 目視検証**

チェック: `[Files]` の 5 エントリが Task 2 `staging_pairs` の destination 名と 1:1 で一致していること。`OutputBaseFilename` が `gemelli-<ver>-windows-x64-setup` であること。

- [ ] **Step 3: Commit**

```bash
git add packaging/windows/gemelli.iss
git commit -m "feat(packaging): add inno setup script for the windows installer"
```

---

### Task 5: icon.ico の生成とコミット

**Files:**
- Modify: `scripts/gen-icon.py` (`main()` のみ)
- Create (生成物): `crates/gui/assets/icon.ico`

**Interfaces:**
- Consumes: 既存の `draw()` (1024×1024 RGBA を返す)
- Produces: `crates/gui/assets/icon.ico` (16–256px マルチサイズ)。Task 2 の `staging_pairs` と Task 4 の `.iss` がこのパスを参照する。

- [ ] **Step 1: `main()` に `.ico` 出力を追加**

`scripts/gen-icon.py` の `main()` を置き換え:

```python
def main() -> None:
    root = Path(__file__).resolve().parent.parent
    assets = root / "crates" / "gui" / "assets"
    assets.mkdir(parents=True, exist_ok=True)
    img = draw()

    png_dst = assets / "icon.png"
    img.save(png_dst)
    print(f"wrote {png_dst}")

    # Multi-size .ico for the Windows installer / Explorer; Pillow resamples
    # each size from the master render.
    ico_dst = assets / "icon.ico"
    img.save(ico_dst, sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    print(f"wrote {ico_dst}")
```

- [ ] **Step 2: 生成する**

Run: `python3 scripts/gen-icon.py` (Pillow 未導入なら `uv run --with pillow python scripts/gen-icon.py`)
Expected: `wrote .../icon.png` と `wrote .../icon.ico` の 2 行。

- [ ] **Step 3: 生成物を確認**

Run: `file crates/gui/assets/icon.ico && git status --short crates/gui/assets/`
Expected: `MS Windows icon resource - 7 icons`。`icon.png` が差分に出た場合は Pillow バージョン差による再エンコードなので一緒にコミットしてよい (幾何は決定的)。

- [ ] **Step 4: Commit**

```bash
git add scripts/gen-icon.py crates/gui/assets/icon.ico crates/gui/assets/icon.png
git commit -m "feat(assets): generate a multi-size windows icon.ico"
```

(icon.png に差分がなければ `git add` から外れていても `git add` は no-op なので問題ない)

---

### Task 6: release workflow 統合

**Files:**
- Modify: `.github/workflows/release.yml` (全置換)
- Delete: `.github/workflows/release-please.yml`

**Interfaces:**
- Consumes: Task 3 の `cargo xtask dist` (両 OS)、Task 4 の `.iss`、既存 `scripts/fetch-fonts.sh` / `scripts/fetch-spout.sh`
- Produces: リリース作成時に `gemelli-gui-v*` Release へ `.dmg` / `.tar.gz` / `.zip` / `-setup.exe` の 4 点が自動添付される

- [ ] **Step 1: `.github/workflows/release.yml` を全置換**

```yaml
name: release

# release-please manages versioning from conventional commits: it opens a release
# PR, and merging it creates the per-crate GitHub releases + tags. The build jobs
# run in this same workflow via `needs:`, so they fire from the default
# GITHUB_TOKEN — the token's event-cascade limitation only applies when one
# workflow tries to trigger another. All distribution artifacts attach to the
# `gemelli-gui-v*` release (its version is what the artifact names embed).

on:
  push:
    branches: [main]
  workflow_dispatch:
    inputs:
      tag:
        description: Existing gemelli-gui-v* release tag to build and upload artifacts for
        required: true
        type: string

permissions:
  contents: write
  pull-requests: write

jobs:
  release-please:
    if: github.event_name == 'push'
    runs-on: ubuntu-latest
    outputs:
      gui_release_created: ${{ steps.release.outputs['crates/gui--release_created'] }}
      gui_tag_name: ${{ steps.release.outputs['crates/gui--tag_name'] }}
    steps:
      - uses: googleapis/release-please-action@v5
        id: release
        with:
          config-file: release-please-config.json
          manifest-file: .release-please-manifest.json

  build-macos:
    needs: [release-please]
    # `!cancelled()` lets this run on workflow_dispatch, where release-please is skipped.
    if: >-
      !cancelled() && (
        needs.release-please.outputs.gui_release_created == 'true' ||
        github.event_name == 'workflow_dispatch'
      )
    runs-on: macos-15
    timeout-minutes: 60
    name: Build (macOS universal)
    env:
      TAG: ${{ github.event_name == 'workflow_dispatch' && github.event.inputs.tag || needs.release-please.outputs.gui_tag_name }}
      GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
    steps:
      - uses: actions/checkout@v7
        with:
          submodules: recursive

      - uses: dtolnay/rust-toolchain@1.96.1

      - name: Add universal2 targets
        run: rustup target add aarch64-apple-darwin x86_64-apple-darwin

      - uses: Swatinem/rust-cache@v2

      # gemelli-syphon links against Apple's Syphon.framework, built locally from the
      # vendor/syphon-src submodule — see README.md "Setup" for the source of these commands.
      - name: Build Syphon.framework
        run: |
          cd vendor/syphon-src
          xcodebuild -project Syphon.xcodeproj \
            -scheme Syphon \
            -configuration Release \
            -derivedDataPath build \
            ONLY_ACTIVE_ARCH=NO \
            BUILD_LIBRARY_FOR_DISTRIBUTION=YES
          cp -R build/Build/Products/Release/Syphon.framework ../Syphon.framework

      # gemelli-gui embeds LINE Seed JP via include_bytes! — see scripts/fetch-fonts.sh.
      - name: Fetch fonts
        run: ./scripts/fetch-fonts.sh

      - name: cargo xtask dist
        run: cargo xtask dist

      - name: Upload release artifacts
        run: gh release upload "$TAG" target/dist/*.dmg target/dist/*.tar.gz --clobber

  build-windows:
    needs: [release-please]
    if: >-
      !cancelled() && (
        needs.release-please.outputs.gui_release_created == 'true' ||
        github.event_name == 'workflow_dispatch'
      )
    runs-on: windows-latest
    timeout-minutes: 60
    name: Build (Windows x64)
    env:
      TAG: ${{ github.event_name == 'workflow_dispatch' && github.event.inputs.tag || needs.release-please.outputs.gui_tag_name }}
      GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
    steps:
      - uses: actions/checkout@v7
        with:
          submodules: recursive

      - uses: dtolnay/rust-toolchain@1.96.1

      - uses: Swatinem/rust-cache@v2

      # gemelli-gui embeds LINE Seed JP via include_bytes! — see scripts/fetch-fonts.sh.
      - name: Fetch fonts
        shell: bash
        run: ./scripts/fetch-fonts.sh

      # gemelli-spout compiles SpoutDX + SpoutGL from the vendored Spout2 SDK
      # — see scripts/fetch-spout.sh.
      - name: Fetch Spout2 SDK
        shell: bash
        run: ./scripts/fetch-spout.sh

      - name: Install Inno Setup
        run: choco install innosetup --no-progress -y

      # Builds both release binaries, stages them with the icon + docs, zips the
      # staging dir, and runs ISCC over packaging/windows/gemelli.iss.
      - name: cargo xtask dist
        run: cargo xtask dist

      - name: Upload release artifacts
        shell: bash
        run: gh release upload "$TAG" target/dist/*.zip target/dist/*-setup.exe --clobber
```

- [ ] **Step 2: 旧 workflow を削除**

```bash
git rm .github/workflows/release-please.yml
```

- [ ] **Step 3: YAML 検証**

Run: `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/release.yml')); print('yaml ok')"`
(PyYAML が無ければ `ruby -ryaml -e "YAML.load_file('.github/workflows/release.yml'); puts 'yaml ok'"`)
Expected: `yaml ok`

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci(release): chain macos and windows installer builds off release-please"
```

---

### Task 7: README と spec の更新

**Files:**
- Modify: `README.md` (147 行目 `## Install / 配布` セクション)
- Modify: `docs/superpowers/specs/2026-07-14-installer-distribution-design.md` (§4 の outputs 記述を実態に合わせる)

**Interfaces:**
- Consumes: Task 4/6 で確定した成果物名とインストール手順
- Produces: エンドユーザー向けドキュメント

- [ ] **Step 1: README の導入文を置き換え**

`## Install / 配布` 直下の段落 (「There is no packaged Windows release yet …」を含む 3 行) を:

```markdown
gemelli ships unsigned prebuilt binaries for both platforms from the
[GitHub Releases](../../releases) page — all artifacts are attached to the
`gemelli-gui-v*` release. macOS builds are **universal2** (Apple Silicon + Intel);
Windows builds are x64. Spout/Syphon support is compiled in — no separate runtime
install is required on either platform.
```

- [ ] **Step 2: `### CLI` セクションの後に Windows セクションを追加**

`### 開発者 (ローカルビルド)` の直前に挿入:

```markdown
### Windows

1. Download `gemelli-<version>-windows-x64-setup.exe` from the
   [GitHub Releases](../../releases) page and run it. It installs the GUI + CLI,
   creates Start Menu shortcuts, and offers an optional desktop icon.
2. The build is unsigned, so SmartScreen blocks the first run — dismiss it with
   **More info → Run anyway**.
3. Prefer not to install? `gemelli-<version>-windows-x64.zip` contains the same
   `gemelli.exe` / `gemelli-gui.exe`, runnable from any directory.
```

- [ ] **Step 3: `### 開発者 (ローカルビルド)` に Windows の一文を追記**

`cargo xtask dist` の説明段落の末尾に追加:

```markdown
On Windows, `cargo xtask dist` instead writes `gemelli-<version>-windows-x64.zip` and
`gemelli-<version>-windows-x64-setup.exe` (requires Inno Setup 6; override the compiler
location with the `ISCC_PATH` environment variable).
```

- [ ] **Step 4: spec §4 の outputs 記述を補正**

`docs/superpowers/specs/2026-07-14-installer-distribution-design.md` §4 の「`outputs` に `release_created` / `tag_name` を追加」を、per-crate outputs (`crates/gui--release_created` / `crates/gui--tag_name`) を使い **`gemelli-gui-v*` Release に添付**する旨に書き換える (release-please は crate ごとに Release を作るため)。

- [ ] **Step 5: Commit**

```bash
git add README.md docs/superpowers/specs/2026-07-14-installer-distribution-design.md
git commit -m "docs: document prebuilt windows installer distribution"
```

---

### Task 8: 最終ゲート

**Files:** なし (検証のみ)

- [ ] **Step 1: 全ゲート実行**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: すべて green (ベースライン 278 tests + 新規 8 tests)。

- [ ] **Step 2: 未コミット差分がないことを確認**

Run: `git status --short`
Expected: 出力なし (spec/plan ドキュメントのコミット漏れがあればここでコミット)

- [ ] **Step 3: マージ後の検証 (ユーザー側 / PR 後)**

- PR マージ後、`gh workflow run release.yml -f tag=gemelli-gui-v0.4.0` で手動実行し、Release に `.dmg` / `.tar.gz` / `.zip` / `-setup.exe` の 4 点が揃うことを確認。
- Windows 実機で `-setup.exe` を実行し、Start Menu から GUI 起動・アンインストール動作を確認。
