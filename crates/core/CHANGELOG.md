# Changelog

## [0.3.1](https://github.com/naporin0624/gemelli/compare/gemelli-core-v0.3.0...gemelli-core-v0.3.1) (2026-07-14)


### Performance Improvements

* **core:** negotiate high-fps MJPEG + fast RGB-&gt;BGRA (~5 -&gt; ~50fps) ([5fd5e45](https://github.com/naporin0624/gemelli/commit/5fd5e45e5dfbc29d31b2835f27aae1cc9b3fb7b9))
* **core:** negotiate high-fps MJPEG + fast RGB→BGRA on main (~5 → ~50fps) ([492e4b4](https://github.com/naporin0624/gemelli/commit/492e4b4db392c51cbf375d2c323aa8a66c3f650e))

## [0.3.0](https://github.com/naporin0624/gemelli/compare/gemelli-core-v0.2.0...gemelli-core-v0.3.0) (2026-07-14)


### Features

* select capture devices by name or unique ID ([ac397c9](https://github.com/naporin0624/gemelli/commit/ac397c909eb6fe151514c4be0ac857a91764c98a))
* select capture devices by name or unique ID ([adb217e](https://github.com/naporin0624/gemelli/commit/adb217e51a4c27330919bfa6734849df5aca1633))


### Bug Fixes

* **gui:** keep the saved camera pin when falling back at launch ([c120fc3](https://github.com/naporin0624/gemelli/commit/c120fc3625a903f2708e7e878e25fd195170e48f))

## [0.2.0](https://github.com/naporin0624/gemelli/compare/gemelli-core-v0.1.0...gemelli-core-v0.2.0) (2026-07-08)


### Features

* **core:** add clockwise rotate (R0/R90/R180/R270) ([36b287c](https://github.com/naporin0624/gemelli/commit/36b287c16977116f56b4264344319da949b95ac1))
* **core:** add crop happy path ([c0dbaeb](https://github.com/naporin0624/gemelli/commit/c0dbaeb58572c590bc2380398cc1653c02b5adba))
* **core:** add Frame with BGRA8 length validation ([37591a3](https://github.com/naporin0624/gemelli/commit/37591a35ddaa54da9545849b58e8e6b18992cdad))
* **core:** add Frame::from_validated for transform-internal construction ([6f88ce3](https://github.com/naporin0624/gemelli/commit/6f88ce34f90e311acb74c199af9378f1f486bda9))
* **core:** add Frame::pixel BGRA8 accessor ([1ee6757](https://github.com/naporin0624/gemelli/commit/1ee6757503ad6724cbeb5dd59dc788b62b6a7f2a))
* **core:** add mirror flip (Keep/Horizontal/Vertical/Both) ([ef1c02f](https://github.com/naporin0624/gemelli/commit/ef1c02f027b4f2d28bd207ad1126c0a50b288683))
* **core:** add nearest-neighbor scale (Exact/Factor) ([07f8bc9](https://github.com/naporin0624/gemelli/commit/07f8bc917f28a8ac1dc9ad145425b03a199245b6))
* **core:** add nokhwa error-mapping and format-request helpers ([2a1e296](https://github.com/naporin0624/gemelli/commit/2a1e296995bef1a2c9b588bcee890af02498202e))
* **core:** add pure device-listing helpers for nokhwa CameraInfo ([37a7b3a](https://github.com/naporin0624/gemelli/commit/37a7b3a88db4d982371e3c41410a07205b2f6bf9))
* **core:** add rgb_to_bgra pixel swizzle helper ([7066bbd](https://github.com/naporin0624/gemelli/commit/7066bbdbeeafce6f3d963b9bd5bc7e400554579d))
* **core:** add run_pipeline orchestrating capture, transform, publish ([4659349](https://github.com/naporin0624/gemelli/commit/4659349fee1cc5c0061bb6a290f20750a48d49aa))
* **core:** add transform config types (Rotation, Flip, CropRect, ScaleSpec, TransformConfig, TransformError) ([bc33512](https://github.com/naporin0624/gemelli/commit/bc33512af4827d87786fc6b5b41ef40bd113e7c3))
* **core:** add TransformConfig::apply composition (crop -&gt; rotate -&gt; flip -&gt; scale) ([923b5cb](https://github.com/naporin0624/gemelli/commit/923b5cba7435dba5583459f0e543f893ae28e867))
* **core:** define CaptureSource trait, CaptureError, DeviceInfo ([1300386](https://github.com/naporin0624/gemelli/commit/1300386644b9899a167598159ab47d4eb0ea6140))
* **core:** define TexturePublisher trait and PublishError ([e4698fa](https://github.com/naporin0624/gemelli/commit/e4698fae68559ed92bbd2f7326842a01684dce2c))
* **core:** implement list_devices and NokhwaSource via nokhwa ([3ae38af](https://github.com/naporin0624/gemelli/commit/3ae38afcb02089136d75743a86e1d5377ac9f62f))
* **core:** validate crop bounds (CropZeroSize, CropOutOfBounds) ([0b9a229](https://github.com/naporin0624/gemelli/commit/0b9a22992e5127415672a41969ab0dff0fca2396))
* **core:** validate scale target (ScaleToZero, ScaleFactorInvalid) ([12c61f5](https://github.com/naporin0624/gemelli/commit/12c61f592be6df07910eba36835d166eb0075c22))


### Bug Fixes

* **core:** best-effort camera format selection with resolution-priority default ([c713719](https://github.com/naporin0624/gemelli/commit/c7137193e0a195ff409e21efbdf8871045637e6a))
* **core:** report device-query failures with cause instead of NoDevices ([e08825e](https://github.com/naporin0624/gemelli/commit/e08825ed677019d39d17348dab3373164652f73f))
