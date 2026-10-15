<p align="center"><img src="assets/logo.png" width="160" alt="minnamate logo"></p>

# minnamate

An open-source, vibecoded clean-room alternative to Adobe Animate, written in Rust. It opens Animate's `.fla` files in an editor laid out like Animate's, and publishes SWF movies that play in Ruffle.

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
cargo run -p minnamate-cli -- html5 movie.fla -o movie-web               # HTML5 Canvas page
cargo run -p minnamate-cli -- gif movie.fla -o movie.gif                 # also: image, png-sequence, video (ffmpeg), publish
```

The commands take a `.fla` file or an unpacked XFL folder. They print `warning: skipped …` for anything they can't handle yet.

## Status

- Works: shapes with solid, gradient and bitmap fills, groups, symbols (graphic, movie clip, button), classic, motion and shape tweens, masks, guides, layer folders and parenting, filters and blend modes, sound, frame scripts (stop, play, goto, trace...), scenes; publishing to SWF, HTML5 Canvas, GIF, PNG, JPEG and video; in the editor, the drawing tools, selection and transforms, symbols and the Library, the timeline, Actions panel, Publish Settings, undo, Test Movie.
- Not yet: bones, 3D, Asset Warp, input text, several documents open at once, custom workspaces.

## Development

Run `cargo test`. Contributors should read the clean-room rules in [CLAUDE.md](CLAUDE.md) first.

## Note
Minnamate is not something I am proud of due to it's vibecoded nature, but hey, fuck big corpos.
