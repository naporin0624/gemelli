# Changelog

## [0.4.1](https://github.com/naporin0624/gemelli/compare/gemelli-syphon-v0.4.0...gemelli-syphon-v0.4.1) (2026-07-14)

## [0.4.0](https://github.com/naporin0624/gemelli/compare/gemelli-syphon-v0.3.0...gemelli-syphon-v0.4.0) (2026-07-14)


### Features

* **ci:** label-gated cross-platform benchmark workflow ([cecb2d6](https://github.com/naporin0624/gemelli/commit/cecb2d6665e1447b3ed285f27141d4105ac3b028))
* Spout (Windows) output ([013c10a](https://github.com/naporin0624/gemelli/commit/013c10afa66266599b3a8fce4cfe98f369c4885a))


### Bug Fixes

* **syphon:** wait for the last command buffer before stopping the server ([d27398c](https://github.com/naporin0624/gemelli/commit/d27398ce96e8b2343d59ba1aec3615665cb7f0d5))


### Performance Improvements

* **syphon:** reuse a cached IOSurface and Metal texture across frames ([2935584](https://github.com/naporin0624/gemelli/commit/2935584bddaa7566b61c012796a0b9c78fb152bd))
* **syphon:** reuse a cached IOSurface and Metal texture across frames ([858c990](https://github.com/naporin0624/gemelli/commit/858c9908d19d711d8e0a5e4db56159d32d366caf))

## [0.3.0](https://github.com/naporin0624/gemelli/compare/gemelli-syphon-v0.2.0...gemelli-syphon-v0.3.0) (2026-07-08)


### Features

* distribution prep — licenses, Cannelloni retheme, About menu, portrait UI ([5c52886](https://github.com/naporin0624/gemelli/commit/5c5288655017f4e110b086c730849c6559963fa3))


### Bug Fixes

* **syphon:** align IOSurface row stride to Metal's 16-byte requirement ([de00fb3](https://github.com/naporin0624/gemelli/commit/de00fb3d54917c097265241e2f106d20b8f5ddf2))

## [0.2.0](https://github.com/naporin0624/gemelli/compare/gemelli-syphon-v0.1.0...gemelli-syphon-v0.2.0) (2026-07-08)


### Features

* **syphon:** compile syphon_bridge.mm via cc and link Syphon/Metal/IOSurface ([fd54025](https://github.com/naporin0624/gemelli/commit/fd54025b3182f829612c919cf6a0c2418fa4326c))
* **syphon:** implement SyphonPublisher over the FFI bridge with interior-NUL test ([a0e2e99](https://github.com/naporin0624/gemelli/commit/a0e2e998b801ff0df01d4cf71c6297342d1fa850))
* **syphon:** port sender-only Syphon Metal bridge from electron-texture-bridge ([89fafae](https://github.com/naporin0624/gemelli/commit/89fafae367e2e7cfd38ee130000518bc3b8c1d54))
* **syphon:** scaffold webcam-sharedtexture-syphon crate ([8189e5d](https://github.com/naporin0624/gemelli/commit/8189e5d81e9384e0efc703ebe98ef2913d019f22))


### Bug Fixes

* **build:** gate syphon inside the crate so release-please can parse manifests ([96df93f](https://github.com/naporin0624/gemelli/commit/96df93f0eae62b043047879ed50a4fc482ca8b44))
* **build:** single-source Syphon rpath list via links metadata ([8b5bc16](https://github.com/naporin0624/gemelli/commit/8b5bc1697bcfa3982d6cd59ec59b9f0baaf34676))
