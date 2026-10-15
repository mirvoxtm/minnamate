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
  - Frame scripts: `script.rs` (a keyframe's script in XFL's `<Actionscript>`; the subset that runs: stop, play, goto, next/prev frame and scene, trace, navigateToURL; `to_avm1` for the SWF export, `to_js` for HTML5 Canvas), `player.rs` (Test Movie's player: every movie clip on its own frames, running the scripts; render.rs `render_playing` draws it).
  - `html5.rs`: HTML5 Canvas documents (`Document::is_canvas`) and `publish(doc, dir, name)`: name.html, runtime.js (`html5_runtime.js`, our player), name.js (baked display lists, path-command shapes, scripts as JS) and images/; File > Publish uses it for Canvas documents.
  - `gradient.rs`: linear and radial gradients (`Paint::Gradient`, XFL's ±819.2 px square): fitting, colours, the tiny-skia paint.
  - `filters.rs`: filters, blend modes and the other Display options, kept in the element's `Extra`; render.rs draws them, swf_export.rs publishes them (PlaceObject3).
  - `bitmap.rs`: bitmaps: library items in `Document::media` (their image file kept as read or imported: PNG, JPEG, GIF, or Animate's lossless data in bin/), `Element::Bitmap` on stage, `Paint::Bitmap` fills; import, decoding, their XFL, drawing and SWF tags (DefineBitsLossless2, DefineBitsJPEG2). `trace.rs`: the Magic Wand's cut and Trace Bitmap.
  - Editing operations: `edit.rs` (frames, layers), `draw.rs` (shape tools), `hit.rs` (hit testing, bounds), `symbols.rs`, `arrange.rs` (clipboard, group, arrange, align), `transform.rs`, `paint.rs` (bucket, ink), `erase.rs`, `pen.rs`.
  - `library.rs`: library folders (empty ones kept as DOMDocument's `<folders>` in the document's `Extra`), renaming and moving items, Duplicate, linkage. `scenes.rs`: duplicate, delete, rename, reorder scenes.
  - `button.rs`: a button's Up/Over/Down/Hit frames and hit area; `render_live` draws buttons hovered or pressed (Test Movie, Enable Simple Buttons); swf_export.rs writes DefineButton2.
  - Publishing: `publish.rs` (PublishSettings.xml profiles, File > Publish), `export.rs` (image, animated GIF, PNG sequence, video through ffmpeg), `encode.rs` (JPEG and GIF encoders).
  - Text: `text.rs` (layout, glyph outlines), `fonts.rs` (installed fonts).
  - Sound: `sound.rs` (library sounds in `doc.sounds`, WAV and MP3 bytes as imported; a keyframe's sound via `Frame::sound`/`set_sound`, kept as its DOMFrame attributes; decoding; which sounds a timeline plays), `sound_swf.rs` (DefineSound, StartSound, a stream per timeline in ADPCM). `video.rs`: Import Video's frames through ffmpeg.
- `apps/cli` (minnamate-cli): `render <fla|folder> --frame N`, `swf`, `html5` (HTML5 Canvas publish), and the exports `image`, `gif`, `png-sequence`, `video` and `publish`. The render command is the oracle loop: diff its PNG against Animate's PNG export of the same frame. `swf` output should play in Ruffle; the tests parse it back with the `swf` crate.
- `apps/editor` (binary `minnamate`), egui 0.36 / eframe, laid out like Animate 2024's Essentials workspace:
  - `app.rs`: state, actions, undo, keyboard routing, layout.
  - The window's parts:
    - `home.rs`: Home screen, New Document, Document Settings.
    - `menu.rs`: app bar and menus; the menu table also defines the shortcuts.
    - `tools.rs`: Tools column.
    - `stage.rs`: stage view, selection, rulers, guides.
    - `timeline.rs`, with `layer_tree.rs` (folder tree, mask and guide commands, parenting view).
    - `panels.rs`: Properties and the icon strip; `filters.rs`: its Filters section.
    - `library.rs`; `symbol_dialogs.rs`: Symbol Properties, Duplicate Symbol, Swap Symbol.
    - `publish.rs`: Publish Settings, Publish, the Doc tab's Publish section, the Export dialogs.
    - `scene_panel.rs`: Window > Scene. `buttons.rs`: a button's timeline labels, Enable Simple Buttons.
    - `arrange_panels.rs`: Align, Transform, Info.
    - `color.rs`: swatch popup, Color and Swatches panels.
  - Tools with their own modules: `free_transform.rs`, `paint_tools.rs`, `eraser.rs`, `pen.rs`, `text_tool.rs`.
  - `gradient_transform.rs`: the Gradient Transform tool (F); the gradient bar and swatches are in `color.rs`.
  - `tweens.rs`: tween commands, auto-keyframing, the motion path and shape hints on stage.
  - `actions_panel.rs`: Window > Actions (F9), Control > Enable Simple Frame Actions; `test_movie.rs`: Control > Test.
  - `bitmaps.rs`: File > Import (and images dropped on the window), the Library's bitmap preview, Bitmap Properties and Update, the Color panel's Bitmap fill, the Magic Wand, Trace Bitmap.
  - `sound.rs`: importing sounds, Properties > Frame > Sound, waveforms (timeline, Library preview), playing with the playhead and in Test Movie; `audio.rs`: the output (tinyaudio) and its voices; `video.rs`: Import Video (each frame a bitmap on a keyframe of a new layer).
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
