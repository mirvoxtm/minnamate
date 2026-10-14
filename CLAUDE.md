# minnamate

Clean-room, open-source alternative to Adobe Animate, in Rust.

## Clean-room rules
- Allowed sources: public specs (Adobe's SWF File Format Specification v19), XFL files we saved ourselves, and observing what Animate does and outputs.
- MIT/Apache code (Ruffle, storytold/*craft) may be reused, with attribution.
- Don't brand anything "Flash" or "Animate".

## Layout
- `crates/core` (minnamate-core): the document model and everything that reads, plays, draws or writes it.
  - `doc.rs`: the model. Geometry is in pixels; XFL edge strings are in twips (1/20 px).
  - `xfl.rs` / `xfl_write.rs`: .fla and XFL folders. Saving keeps every attribute and element the model doesn't read (`Extra`).
  - `display.rs`: playback rules (keyframes, tweens, graphic symbols, masks). The renderer, hit testing and the SWF export all go through it.
  - Tweens: `motion.rs` (motion tweens and their XFL motion XML), `shape_tween.rs` (shape tween in-betweens and hints); classic tweens are in display.rs.
  - `layer_tree.rs`: how layers link (folders, masks, guides via `parentLayerIndex`; layer parenting) and the commands that change it; `layer_motion.rs`: what guides and parenting do to motion, which display.rs places through.
  - `render.rs` (tiny-skia), `swf_export.rs` (Ruffle's `swf` crate).
  - `gradient.rs`: linear and radial gradients (`Paint::Gradient`, XFL's ±819.2 px square): fitting, colours, the tiny-skia paint.
  - `filters.rs`: filters, blend modes and the other Display options, kept in the element's `Extra`; render.rs draws them, swf_export.rs publishes them (PlaceObject3).
  - Editing operations: `edit.rs` (frames, layers), `draw.rs` (shape tools), `hit.rs` (hit testing, bounds), `symbols.rs`, `arrange.rs` (clipboard, group, arrange, align), `transform.rs`, `paint.rs` (bucket, ink), `erase.rs`, `pen.rs`.
  - Text: `text.rs` (layout, glyph outlines), `fonts.rs` (installed fonts).
- `apps/cli` (minnamate-cli): `render <fla|folder> --frame N` and `swf`. The render command is the oracle loop: diff its PNG against Animate's PNG export of the same frame. `swf` output should play in Ruffle; the tests parse it back with the `swf` crate.
- `apps/editor` (binary `minnamate`), egui 0.36 / eframe, laid out like Animate 2024's Essentials workspace:
  - `app.rs`: state, actions, undo, keyboard routing, layout.
  - The window's parts:
    - `home.rs`: Home screen, New Document, Document Settings.
    - `menu.rs`: app bar and menus; the menu table also defines the shortcuts.
    - `tools.rs`: Tools column.
    - `stage.rs`: stage view, selection, rulers, guides.
    - `timeline.rs`, with `layer_tree.rs` (folder tree, mask and guide commands, parenting view).
    - `panels.rs`: Properties and the icon strip; `filters.rs`: its Filters section.
    - `library.rs`.
    - `arrange_panels.rs`: Align, Transform, Info.
    - `color.rs`: swatch popup, Color and Swatches panels.
  - Tools with their own modules: `free_transform.rs`, `paint_tools.rs`, `eraser.rs`, `pen.rs`, `text_tool.rs`.
  - `gradient_transform.rs`: the Gradient Transform tool (F); the gradient bar and swatches are in `color.rs`.
  - `tweens.rs`: tween commands, auto-keyframing, the motion path and shape hints on stage.
  - Shared UI: `theme.rs` (palette, metrics, menus), `widgets.rs` (Properties controls), `icons.rs`, `cursors.rs`.
- `docs/animate-ui.md` and `docs/animate-ui-interaction.md`: the UI reference the editor is built from.

## Editor rules
- The GUI must look like Adobe Animate. Icons are our own SVG-path drawings in `icons.rs`, rasterized with tiny-skia; never copy Adobe artwork or fonts.
- Every document change goes through `Editor::checkpoint()` (or `checkpoint_gesture` for drags) so undo works.
- Menus and context menus use the light style: `theme::context_menu`, `theme::menu_item`.
- See the UI: `cargo test -p minnamate-editor -- --ignored screenshot` writes PNGs to target/screenshots (headless wgpu).
- eframe's `links` feature is off: its `webbrowser` dep links shlwapi, which the bundled MinGW linker (GNU toolchain) lacks.

## Check
`cargo test --workspace -- --include-ignored` (the ignored tests are the GPU screenshots).
