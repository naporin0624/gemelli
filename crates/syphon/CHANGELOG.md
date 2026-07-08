# Changelog

## [0.2.0](https://github.com/naporin0624/gemelli/compare/gemelli-syphon-v0.1.0...gemelli-syphon-v0.2.0) (2026-07-08)


### Features

* **syphon:** compile syphon_bridge.mm via cc and link Syphon/Metal/IOSurface ([fd54025](https://github.com/naporin0624/gemelli/commit/fd54025b3182f829612c919cf6a0c2418fa4326c))
* **syphon:** implement SyphonPublisher over the FFI bridge with interior-NUL test ([a0e2e99](https://github.com/naporin0624/gemelli/commit/a0e2e998b801ff0df01d4cf71c6297342d1fa850))
* **syphon:** port sender-only Syphon Metal bridge from electron-texture-bridge ([89fafae](https://github.com/naporin0624/gemelli/commit/89fafae367e2e7cfd38ee130000518bc3b8c1d54))
* **syphon:** scaffold webcam-sharedtexture-syphon crate ([8189e5d](https://github.com/naporin0624/gemelli/commit/8189e5d81e9384e0efc703ebe98ef2913d019f22))


### Bug Fixes

* **build:** gate syphon inside the crate so release-please can parse manifests ([96df93f](https://github.com/naporin0624/gemelli/commit/96df93f0eae62b043047879ed50a4fc482ca8b44))
* **build:** single-source Syphon rpath list via links metadata ([8b5bc16](https://github.com/naporin0624/gemelli/commit/8b5bc1697bcfa3982d6cd59ec59b9f0baaf34676))
