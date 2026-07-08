# Changelog

## [0.2.0](https://github.com/naporin0624/gemelli/compare/gemelli-gui-v0.1.0...gemelli-gui-v0.2.0) (2026-07-08)


### Features

* **gui:** add BGRA8 to RGBA8 pixel conversion ([9ccffe3](https://github.com/naporin0624/gemelli/commit/9ccffe308b55e1fe68ab799cc7daecbd649d338f))
* **gui:** add clamp_rect for min-size and bounds enforcement ([c2a306a](https://github.com/naporin0624/gemelli/commit/c2a306a7161cc5fbfa7d04c0298bcc544f961e7e))
* **gui:** add crop-edit interaction ([dd8d202](https://github.com/naporin0624/gemelli/commit/dd8d2022bb3333ff9d6461d124c10efb37f942bf))
* **gui:** add CropMapping::to_frame with bounds clamping ([f1b98c0](https://github.com/naporin0624/gemelli/commit/f1b98c0c6ba29f189a4b263afabbba8ea7f71d09))
* **gui:** add CropMapping::to_screen frame-&gt;screen mapping ([c3e80df](https://github.com/naporin0624/gemelli/commit/c3e80df97fcb26d4ee5cef0aaa787d04b4a48b7e))
* **gui:** add DragMode/DragState and apply_drag corner/move math ([28e7691](https://github.com/naporin0624/gemelli/commit/28e76919457fff7e76cb09dc502b2023d0f8213e))
* **gui:** add hit_test with corner-priority handle detection ([0325f4e](https://github.com/naporin0624/gemelli/commit/0325f4e14cc6049252786225bb1a955e24e65e22))
* **gui:** add run_capture core loop with raw/output frame capture ([6a5bf9d](https://github.com/naporin0624/gemelli/commit/6a5bf9dda4233d3d314866b64155ee24bd72a113))
* **gui:** add sanctioned f32&lt;-&gt;u32 coord cast pair ([04189e3](https://github.com/naporin0624/gemelli/commit/04189e358e2b3ee72a11b3f5ca9891c35efbd7ee))
* **gui:** add SharedState for capture-thread &lt;-&gt; GUI exchange ([a5f4925](https://github.com/naporin0624/gemelli/commit/a5f4925af891d230a51ee537e10c39b3fd6cd8af))
* **gui:** add sliding 1-second FpsMeter ([7935641](https://github.com/naporin0624/gemelli/commit/7935641905fd94cd7023a74f85e48f86f7ca6042))
* **gui:** add WCAG 2.1 contrast ratio calculation ([1a657dc](https://github.com/naporin0624/gemelli/commit/1a657dc5b0545caf192628c77c902bd267afc0a2))
* **gui:** add WCAG-verified dark theme color tokens ([89ba430](https://github.com/naporin0624/gemelli/commit/89ba430ac6ba20a681f2463450b03d197d260b3f))
* **gui:** add WorkerHandle with idempotent stop and Drop-triggered join ([dbfd92c](https://github.com/naporin0624/gemelli/commit/dbfd92cf95ed9e79761c3fc220df32c45505e528))
* **gui:** add WorkerSpec and spawn_worker with platform-gated publisher ([0cafbe3](https://github.com/naporin0624/gemelli/commit/0cafbe37d6962cb483885dd5f4248aa3b62e8d6b))
* **gui:** apply WCAG token palette to egui visuals at startup ([60bcdbb](https://github.com/naporin0624/gemelli/commit/60bcdbb549a893b9fad8bc5720c3b508d8141a29))
* **gui:** build egui::ColorImage from Frame ([2943f7e](https://github.com/naporin0624/gemelli/commit/2943f7ece7add71e52569d2394acd52ee828494f))
* **gui:** embed LINE Seed JP and switch UI to Japanese ([67ec48b](https://github.com/naporin0624/gemelli/commit/67ec48b1fd0b2efca3dea14b2ee19e37be3b3692))
* **gui:** letterbox-fit frame dimensions into the preview rect ([44fb71d](https://github.com/naporin0624/gemelli/commit/44fb71d7852979a25cfb715812e3c40604738c43))
* **gui:** scaffold eframe bootstrap and syphon rpath build script ([7784ad1](https://github.com/naporin0624/gemelli/commit/7784ad1c289a18181428fa4fe82f51822748c6e2))
* **gui:** wire app state, sidebar, and status bar ([08bb410](https://github.com/naporin0624/gemelli/commit/08bb4102a4cd3a6f15236494b0643ecd2d6bd471))


### Bug Fixes

* **build:** gate syphon inside the crate so release-please can parse manifests ([96df93f](https://github.com/naporin0624/gemelli/commit/96df93f0eae62b043047879ed50a4fc482ca8b44))
* **gui:** drain stale worker errors and refit crop on device switch ([46a741c](https://github.com/naporin0624/gemelli/commit/46a741cf7a65fd81a116d2b3a1b042f2580af2e4))
* **gui:** keep UI copy in English; LINE Seed renders Japanese device names ([12ffccf](https://github.com/naporin0624/gemelli/commit/12ffccfeb2d47fc0af8c072f259730f38ce300ce))
* **gui:** meet WCAG AA on selection and status colors ([d882279](https://github.com/naporin0624/gemelli/commit/d882279ba9ce0d8d267bba5cf7a610760bfb40f9))


### Performance Improvements

* **gui:** share frames via Arc and skip redundant texture uploads ([8b6e5b3](https://github.com/naporin0624/gemelli/commit/8b6e5b346e6925a7cc01b1dd258ed9f8173a92ba))
