# gemelli

A small tool that captures a webcam feed and publishes it as a shared GPU texture
(Syphon on macOS, Spout on Windows), with rotate/flip/crop/scale transforms
configurable via CLI arguments. Sister tool of [ravioli](https://github.com/naporin0624/ravioli).

## Building the Syphon bridge (macOS)

`crates/syphon` links against Apple's Syphon.framework, which is not vendored/prebuilt —
it's built locally from a git submodule the first time you set up the repo.

```bash
# 1. fetch the Syphon Framework source (once, or after a fresh clone without --recursive)
git submodule update --init --recursive

# 2. build Syphon.framework
cd vendor/syphon-src
xcodebuild -project Syphon.xcodeproj \
  -scheme Syphon \
  -configuration Release \
  -derivedDataPath build \
  ONLY_ACTIVE_ARCH=NO \
  BUILD_LIBRARY_FOR_DISTRIBUTION=YES
cp -R build/Build/Products/Release/Syphon.framework ../Syphon.framework
cd ../..

# 3. build the workspace
cargo build --workspace
```

`vendor/syphon-src` (the submodule source) and `vendor/Syphon.framework` (the build output)
are both gitignored except for the submodule pointer in `.gitmodules` — every clone rebuilds
the framework locally rather than committing a binary.

If Gatekeeper quarantines the copied framework and Syphon servers silently fail to appear to
clients, clear it: `xattr -dr com.apple.quarantine vendor/Syphon.framework`.
