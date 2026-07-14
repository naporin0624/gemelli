# Changelog

## [0.5.0](https://github.com/naporin0624/gemelli/compare/gemelli-gui-v0.4.0...gemelli-gui-v0.5.0) (2026-07-14)


### Features

* **assets:** generate a multi-size windows icon.ico ([ffd1a61](https://github.com/naporin0624/gemelli/commit/ffd1a61fd0a2461909fb0cea6cd0244790c6301c))
* ship Windows installer and auto-attaching release builds ([c04ea33](https://github.com/naporin0624/gemelli/commit/c04ea3316e0da5d9dec38df5a0486e13845f05c0))

## [0.4.0](https://github.com/naporin0624/gemelli/compare/gemelli-gui-v0.3.0...gemelli-gui-v0.4.0) (2026-07-14)


### Features

* **gui:** tray residency & close-to-minimize ([22ae3ab](https://github.com/naporin0624/gemelli/commit/22ae3abd674e059182f977c9aa879ee363558322))
* publish to Spout on Windows ([ad5062f](https://github.com/naporin0624/gemelli/commit/ad5062fcb57969a2a383949002fbb4581bb8ef2a))
* select capture devices by name or unique ID ([ac397c9](https://github.com/naporin0624/gemelli/commit/ac397c909eb6fe151514c4be0ac857a91764c98a))
* select capture devices by name or unique ID ([adb217e](https://github.com/naporin0624/gemelli/commit/adb217e51a4c27330919bfa6734849df5aca1633))
* Spout (Windows) output ([013c10a](https://github.com/naporin0624/gemelli/commit/013c10afa66266599b3a8fce4cfe98f369c4885a))


### Bug Fixes

* **gui:** keep the saved camera pin when falling back at launch ([c120fc3](https://github.com/naporin0624/gemelli/commit/c120fc3625a903f2708e7e878e25fd195170e48f))
* **gui:** poll tray/menu events in App::logic so the tray works while minimized ([6dc2ec3](https://github.com/naporin0624/gemelli/commit/6dc2ec352fd77a9a4db799ed69efefa14d53d2e5))
* open BGRA-only cameras such as OBS Virtual Camera ([f0663f8](https://github.com/naporin0624/gemelli/commit/f0663f8566a1b63e2d543c85c281c1564583212b))

## [0.3.0](https://github.com/naporin0624/gemelli/compare/gemelli-gui-v0.2.0...gemelli-gui-v0.3.0) (2026-07-08)


### Features

* distribution prep — licenses, Cannelloni retheme, About menu, portrait UI ([5c52886](https://github.com/naporin0624/gemelli/commit/5c5288655017f4e110b086c730849c6559963fa3))
* **gui:** add cannelloni widget primitives ([50b03a0](https://github.com/naporin0624/gemelli/commit/50b03a01b2869a8a5e55e77eba568b24d47be3da))
* **gui:** add licenses data model ([a9ae7a1](https://github.com/naporin0624/gemelli/commit/a9ae7a1429ea1b025d22bd4f0ab85c38c8c52357))
* **gui:** add native app menu with About via muda ([4afdd07](https://github.com/naporin0624/gemelli/commit/4afdd07a007bf5707837ccb7acadc062ed87b18b))
* **gui:** add open-source licenses window ([56f483c](https://github.com/naporin0624/gemelli/commit/56f483c8b27bea85e252e9200c9c35448e27f6aa))
* **gui:** compact controls into a label-left grid ([51fadc8](https://github.com/naporin0624/gemelli/commit/51fadc808a72e682ffad76230e236140a4c1d645))
* **gui:** embed git build id via vergen-gix ([9e197c5](https://github.com/naporin0624/gemelli/commit/9e197c573b53115ea7b3e843f97c7dfa0f1146d4))
* **gui:** restructure to portrait controls-top layout ([8313677](https://github.com/naporin0624/gemelli/commit/83136775544aec7016b093ac447828b0506adb16))
* **gui:** set the app window icon ([2d9d055](https://github.com/naporin0624/gemelli/commit/2d9d0554c949c2b95f3085b521aa173bdd1ce2d3))
* **xtask:** add gen-licenses command and generated artifacts ([f0e5410](https://github.com/naporin0624/gemelli/commit/f0e54109e600c019761101d9e44f832c1ed35301))


### Bug Fixes

* **gui:** correct build-id and border token comments ([7481df2](https://github.com/naporin0624/gemelli/commit/7481df2e8ec88686a8e68bd0554a2a7859b66050))
* **gui:** describe theme tokens by current role only ([efe067b](https://github.com/naporin0624/gemelli/commit/efe067be33e5eab8b6964f169ed77312fce735dd))
* **gui:** keep menu comments to current design ([1c41c23](https://github.com/naporin0624/gemelli/commit/1c41c23d5fb331f24da13acb3441cff421b7ac6d))
* **gui:** make whole license row clickable and drop stale dead_code allow ([0b2838a](https://github.com/naporin0624/gemelli/commit/0b2838ac0c608765cdfc504ce0b0948713fb7c39))
* **gui:** render flip choices as text labels ([759b66e](https://github.com/naporin0624/gemelli/commit/759b66e5930e175a6592dc5b656cd660c8b2818f))
* **gui:** state window-size derivation without process context ([c02dbb8](https://github.com/naporin0624/gemelli/commit/c02dbb8fa822e79ebe461f66f56dd5ca88e50182))

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
