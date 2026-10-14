<p align="center"><img src="assets/logo.png" width="160" alt="minnamate logo"></p>

# minnamate

An open-source, clean-room alternative to Adobe Animate, written in Rust. It opens Animate's `.fla` files in an editor laid out like Animate's, and publishes SWF movies that play in Ruffle.

It's early. Expect gaps; the Output panel (F2) lists anything a document uses that minnamate can't show yet.

## Run the editor

On Windows, with [Rust](https://rustup.rs) installed:

```powershell
cargo run -p minnamate-editor --release                    # the Home screen: new document or recent files
cargo run -p minnamate-editor --release -- path\to\movie.fla
```

After the first build you can start `target\release\minnamate.exe` directly, and drag `.fla` files onto its window.

The editor follows Animate's shortcuts: Enter plays, `,` and `.` step frames, F5/F6/F7 insert frames and keyframes, F8 converts the selection to a symbol, Ctrl+Enter tests the movie, Ctrl+Z undoes, and the tool letters (V, R, O, N, Y, L, H, Z...) pick tools.

## Command line

```sh
cargo run -p minnamate-cli -- render movie.fla --frame 12 -o frame.png   # one frame as PNG; frames start at 1
cargo run -p minnamate-cli -- swf movie.fla -o movie.swf                 # SWF, plays in Ruffle
```

Both commands take a `.fla` file or an unpacked XFL folder. They print `warning: skipped …` for anything they can't handle yet.

## Status

- Works: shapes with solid fills and strokes, groups, graphic symbols, movie clips (in SWF), classic tweens, color effects (alpha, tint, brightness), multiple scenes; in the editor, drawing (rectangle, oval, line, polystar, pencil, brush), selecting and moving, lasso, Convert to Symbol, frame editing, undo, Test Movie.
- Not yet: masks, gradients and bitmaps, shape and motion tweens, button states beyond Up, ActionScript.

## Development

Run `cargo test`. Contributors should read the clean-room rules in [CLAUDE.md](CLAUDE.md) first.
