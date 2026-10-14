# minnamate

Clean-room, open-source alternative to Adobe Animate, in Rust.

## Clean-room rules
- Allowed sources: public specs (Adobe's SWF File Format Specification v19), XFL files we saved ourselves, and observing what Animate does and outputs.
- MIT/Apache code (Ruffle, storytold/*craft) may be reused, with attribution.
- Don't brand anything "Flash" or "Animate".

## Notes
- XFL edge strings are in twips (1/20 px); the doc model stores pixels.
- `minnamate-cli render <fla|folder> --frame N` is the oracle loop: diff its PNG against Animate's PNG export of the same frame.
- `display.rs` holds the playback rules (keyframes, tweens, graphic symbols); the renderer and the SWF export both go through it.
- `minnamate-cli swf` output should play in Ruffle; the tests parse it back with the `swf` crate.
- Check: `cargo test`

## Editor (apps/editor, binary `minnamate`)
- egui 0.36 / eframe, laid out like Animate's Essentials workspace: `app.rs` (state, actions, undo, layout), `timeline.rs`, `stage.rs`, `tools.rs`, `panels.rs` (Properties, Library), `menu.rs`, `theme.rs` (palette and metrics), `icons.rs`.
- The GUI must look like Adobe Animate. Icons are our own SVG-path drawings in `icons.rs`, rasterized with tiny-skia; never copy Adobe artwork or fonts.
- Every document change goes through `Editor::checkpoint()` (or `checkpoint_gesture` for drags) so undo works.
- See the UI: `cargo test -p minnamate-editor -- --ignored screenshot` writes target/screenshots/editor.png (headless wgpu).
- eframe's `links` feature is off: its `webbrowser` dep links shlwapi, which the bundled MinGW linker (GNU toolchain) lacks.
