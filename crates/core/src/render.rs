//! CPU reference renderer: draws one frame of a document into a pixmap.

use std::collections::BTreeSet;

use tiny_skia::{self as sk, FillRule, PathBuilder, Pixmap, Transform};

use crate::Result;
use crate::display::{Item, display_list, display_list_shown};
use crate::doc::*;
use crate::text;

/// Renders `frame` (0-based, counted across scenes) at `scale`. Also returns
/// the features it skipped, so gaps against Animate's own output are visible.
pub fn render(doc: &Document, frame: u32, scale: f32) -> Result<(Pixmap, BTreeSet<String>)> {
    let (mut scene, mut local) = (None, frame);
    for s in &doc.scenes {
        if local < s.frame_count() {
            scene = Some(s);
            break;
        }
        local -= s.frame_count();
    }
    let scene = scene.ok_or_else(|| format!("frame out of range: the document has {} frames", frame - local))?;

    let size = |v: f32| (v * scale).ceil() as u32;
    let mut pixmap = Pixmap::new(size(doc.width), size(doc.height)).ok_or("bad stage size")?;
    pixmap.fill(color(doc.background));
    let mut r = Renderer { doc, pixmap, skipped: BTreeSet::new(), shown_only: false };
    r.timeline(scene, local, Transform::from_scale(scale, scale), ColorTransform::IDENTITY, 0);
    Ok((r.pixmap, r.skipped))
}

/// Renders `frame` of any timeline of `doc` (a scene's or a symbol's), placed
/// by `matrix`, onto a transparent `width` × `height` canvas at `scale`. The editor
/// draws symbols being edited in place, and Library previews, with it.
pub fn render_timeline(
    doc: &Document,
    tl: &Timeline,
    frame: u32,
    matrix: &Matrix,
    (width, height): (f32, f32),
    scale: f32,
) -> Result<(Pixmap, BTreeSet<String>)> {
    let size = |v: f32| (v * scale).ceil() as u32;
    let pixmap = Pixmap::new(size(width), size(height)).ok_or("bad canvas size")?;
    // The editor draws with this, so the timeline's hidden layers stay hidden.
    let mut r = Renderer { doc, pixmap, skipped: BTreeSet::new(), shown_only: true };
    r.timeline(tl, frame, Transform::from_scale(scale, scale).pre_concat(transform(matrix)), ColorTransform::IDENTITY, 0);
    Ok((r.pixmap, r.skipped))
}

struct Renderer<'a> {
    doc: &'a Document,
    pixmap: Pixmap,
    skipped: BTreeSet<String>,
    /// Leave out the top timeline's hidden layers.
    shown_only: bool,
}

impl<'a> Renderer<'a> {
    fn timeline(&mut self, tl: &'a Timeline, frame: u32, ts: Transform, cx: ColorTransform, depth: u32) {
        if depth > 32 {
            self.skipped.insert("clips nested over 32 deep".into());
            return;
        }
        let list = if self.shown_only && depth == 0 { display_list_shown } else { display_list };
        for p in list(self.doc, tl, frame, &mut self.skipped) {
            let (ts, cx) = (ts.pre_concat(transform(&p.matrix)), cx * p.color);
            match p.item {
                Item::Shape(s) => self.shape(s, ts, cx),
                Item::Text(t) => self.shape(&text::to_shape(t), ts, cx),
                // ponytail: clips show frame 1, as on Animate's stage. Playing them
                // needs per-instance state carried between frames (the SWF export has it).
                Item::Clip(sym, _) => self.timeline(&sym.timeline, 0, ts, cx, depth + 1),
            }
        }
    }

    fn shape(&mut self, s: &Shape, ts: Transform, cx: ColorTransform) {
        for (i, fill) in s.fills.iter().enumerate() {
            let (Some(paint), Some(path)) = (self.paint(fill, cx), fill_path(&s.edges, i as u32 + 1)) else {
                continue;
            };
            self.pixmap.fill_path(&path, &paint, FillRule::EvenOdd, ts, None);
        }
        for e in &s.edges {
            let Some(style) = (e.stroke as usize).checked_sub(1).and_then(|i| s.strokes.get(i)) else { continue };
            let Some(paint) = self.paint(&style.paint, cx) else { continue };
            let mut pb = PathBuilder::new();
            for p in &e.paths {
                pb.move_to(p.start.0, p.start.1);
                p.segs.iter().for_each(|&seg| push(&mut pb, seg));
            }
            let Some(path) = pb.finish() else { continue };
            let stroke = sk::Stroke {
                width: style.width,
                miter_limit: 3.0,
                line_cap: match style.cap {
                    Cap::None => sk::LineCap::Butt,
                    Cap::Round => sk::LineCap::Round,
                    Cap::Square => sk::LineCap::Square,
                },
                line_join: match style.join {
                    Join::Miter => sk::LineJoin::Miter,
                    Join::Round => sk::LineJoin::Round,
                    Join::Bevel => sk::LineJoin::Bevel,
                },
                ..Default::default()
            };
            self.pixmap.stroke_path(&path, &paint, &stroke, ts, None);
        }
    }

    fn paint(&mut self, p: &Paint, cx: ColorTransform) -> Option<sk::Paint<'static>> {
        match p {
            Paint::Solid(c) => {
                let mut paint = sk::Paint::default();
                paint.set_color(color(cx.apply(*c)));
                Some(paint)
            }
            Paint::Unsupported { name, .. } => {
                self.skipped.insert(name.clone());
                None
            }
        }
    }
}

/// The outline of fill `id`: every edge with that fill on either side, chained
/// end to start into closed loops. Even-odd filling then ignores loop direction.
fn fill_path(edges: &[Edge], id: u32) -> Option<sk::Path> {
    let mut pieces = Vec::new();
    for e in edges.iter().filter(|e| e.fill0 == id || e.fill1 == id) {
        for p in &e.paths {
            let mut from = p.start;
            for &seg in &p.segs {
                // fill1 lies right of the edge's direction, fill0 left: flip fill0
                // pieces so the pieces around one region all run the same way.
                if e.fill1 == id {
                    pieces.push((from, seg));
                }
                if e.fill0 == id {
                    pieces.push(match seg {
                        Seg::Line(to) => (to, Seg::Line(from)),
                        Seg::Quad(c, to) => (to, Seg::Quad(c, from)),
                    });
                }
                from = seg.end();
            }
        }
    }
    let near = |a: Point, b: Point| (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3;
    let mut pb = PathBuilder::new();
    // ponytail: O(n²) endpoint search. It runs from the end, where the loop being chained sits
    // when edges come loop by loop (as glyph outlines do), so text costs per glyph, not per
    // text. Index endpoints in a HashMap if dense brush shapes get slow.
    while let Some((start, seg)) = pieces.pop() {
        pb.move_to(start.0, start.1);
        push(&mut pb, seg);
        let mut end = seg.end();
        while let Some(i) = pieces.iter().rposition(|(from, _)| near(*from, end)) {
            let (_, seg) = pieces.swap_remove(i);
            push(&mut pb, seg);
            end = seg.end();
        }
        pb.close();
    }
    pb.finish()
}

fn push(pb: &mut PathBuilder, seg: Seg) {
    match seg {
        Seg::Line((x, y)) => pb.line_to(x, y),
        Seg::Quad((cx, cy), (x, y)) => pb.quad_to(cx, cy, x, y),
    }
}

fn transform(m: &Matrix) -> Transform {
    Transform::from_row(m.a, m.b, m.c, m.d, m.tx, m.ty)
}

fn color([r, g, b, a]: Rgba) -> sk::Color {
    sk::Color::from_rgba(r, g, b, a).unwrap_or(sk::Color::BLACK)
}
