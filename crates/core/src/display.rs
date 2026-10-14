//! Playback rules: what is on stage at a given frame of a timeline. The
//! renderer and the exporters all read the document through this.

use std::collections::BTreeSet;
use std::f32::consts::{PI, TAU};

use crate::doc::*;
use crate::hit::ElementRef;

/// Something on stage, with its transform relative to the timeline's origin.
pub struct Placed<'a> {
    pub item: Item<'a>,
    pub matrix: Matrix,
    pub color: ColorTransform,
    /// The element of the listed timeline that put this here. Graphic symbols
    /// and groups are flattened in, so one element can place several items.
    pub source: ElementRef,
}

pub enum Item<'a> {
    Shape(&'a Shape),
    /// Drawn as its glyphs' shape (text.rs).
    Text(&'a Text),
    /// A movie clip or button: it runs its own timeline.
    Clip(&'a Symbol, SymbolKind),
}

/// Bottom-to-top contents of `frame` of `tl`. Graphic symbols and groups are
/// flattened in, since they play in step with their parent. Features that
/// can't be shown yet are added to `skipped`.
pub fn display_list<'a>(doc: &'a Document, tl: &'a Timeline, frame: u32, skipped: &mut BTreeSet<String>) -> Vec<Placed<'a>> {
    list(doc, tl, frame, skipped, false)
}

/// `display_list` without the layers hidden in the timeline (their eye turned
/// off), as the editor's stage shows them. Publishing still includes them.
pub fn display_list_shown<'a>(doc: &'a Document, tl: &'a Timeline, frame: u32, skipped: &mut BTreeSet<String>) -> Vec<Placed<'a>> {
    list(doc, tl, frame, skipped, true)
}

fn list<'a>(doc: &'a Document, tl: &'a Timeline, frame: u32, skipped: &mut BTreeSet<String>, shown_only: bool) -> Vec<Placed<'a>> {
    let source = ElementRef { layer: 0, keyframe: 0, element: 0 };
    let mut lister = Lister { doc, skipped, out: Vec::new(), source, shown_only };
    lister.timeline(tl, frame, Matrix::IDENTITY, ColorTransform::IDENTITY, 0);
    lister.out
}

struct Lister<'a, 's> {
    doc: &'a Document,
    skipped: &'s mut BTreeSet<String>,
    out: Vec<Placed<'a>>,
    /// The top-level element being listed.
    source: ElementRef,
    /// Skip the listed timeline's hidden layers.
    shown_only: bool,
}

impl<'a> Lister<'a, '_> {
    fn timeline(&mut self, tl: &'a Timeline, frame: u32, m: Matrix, cx: ColorTransform, depth: u32) {
        if depth > 32 {
            self.skipped.insert("graphics nested over 32 deep".into());
            return;
        }
        for (l, layer) in tl.layers.iter().enumerate().rev() {
            if self.shown_only && depth == 0 && !layer.visible() {
                continue;
            }
            match layer.kind {
                LayerKind::Normal => {}
                LayerKind::Guide | LayerKind::Folder => continue,
                // ponytail: masks aren't applied, so masked layers show unclipped. SWF has clip_depth for this.
                LayerKind::Mask => {
                    self.skipped.insert("mask layers".into());
                    continue;
                }
            }
            let Some(i) = layer.keyframe_at(frame) else { continue };
            let key = &layer.frames[i];
            let offset = frame - key.index;
            if let Tween::Unsupported(kind) = &key.tween {
                self.skipped.insert(format!("{kind} tweens"));
            }
            let tweened = tween_at(key, layer.frames.get(i + 1), offset);
            for (e, el) in key.elements.iter().enumerate() {
                if depth == 0 {
                    self.source = ElementRef { layer: l, keyframe: i, element: e };
                }
                // A tween only starts from a keyframe holding one instance, and replaces it.
                match &tweened {
                    Some(inst) => self.instance(inst, offset, m, cx, depth),
                    None => self.element(el, offset, m, cx, depth),
                }
            }
        }
    }

    fn element(&mut self, el: &'a Element, offset: u32, m: Matrix, cx: ColorTransform, depth: u32) {
        match el {
            Element::Shape(s) => {
                self.out.push(Placed { item: Item::Shape(s), matrix: m * s.matrix, color: cx, source: self.source })
            }
            Element::Text(t) => self.out.push(Placed { item: Item::Text(t), matrix: m * t.matrix, color: cx, source: self.source }),
            Element::Instance(i) => self.instance(i, offset, m, cx, depth),
            Element::Group(members) => members.iter().for_each(|el| self.element(el, offset, m, cx, depth)),
            Element::Unsupported { name, .. } => {
                self.skipped.insert(name.clone());
            }
        }
    }

    /// `offset` is how many frames ago the instance's keyframe started.
    fn instance(&mut self, inst: &Instance, offset: u32, m: Matrix, cx: ColorTransform, depth: u32) {
        let Some(sym) = self.doc.library.get(&inst.symbol) else {
            self.skipped.insert(format!("missing symbol {:?}", inst.symbol));
            return;
        };
        let (m, cx) = (m * inst.matrix, cx * inst.color);
        if inst.kind != SymbolKind::Graphic {
            self.out.push(Placed { item: Item::Clip(sym, inst.kind), matrix: m, color: cx, source: self.source });
            return;
        }
        self.timeline(&sym.timeline, graphic_frame(inst, offset, &sym.timeline), m, cx, depth + 1);
    }
}

/// The frame of `symbol` a graphic instance shows `offset` frames into its keyframe.
pub(crate) fn graphic_frame(inst: &Instance, offset: u32, symbol: &Timeline) -> u32 {
    let len = symbol.frame_count().max(1);
    match inst.looping {
        Loop::Loop => (inst.first_frame + offset) % len,
        Loop::PlayOnce => (inst.first_frame + offset).min(len - 1),
        Loop::SingleFrame => inst.first_frame,
    }
}

/// The in-between instance `offset` frames into `key`'s classic tween, or None
/// to show the keyframe as is.
pub fn tween_at(key: &Frame, next: Option<&Frame>, offset: u32) -> Option<Instance> {
    let Tween::Classic { ease, rotate } = key.tween else { return None };
    // Like Animate, a broken tween (no next keyframe, or not one matching symbol each side) holds.
    let next = next.filter(|n| n.index == key.index + key.duration)?;
    let ([Element::Instance(a)], [Element::Instance(b)]) = (&key.elements[..], &next.elements[..]) else {
        return None;
    };
    if a.symbol != b.symbol {
        return None;
    }
    let t = offset as f32 / key.duration as f32;
    Some(lerp_instance(a, b, t + ease * t * (1.0 - t), rotate))
}

/// Classic tween in-between: scale and skew angles interpolate separately and
/// the transformation point moves in a straight line, as in Animate.
fn lerp_instance(a: &Instance, b: &Instance, t: f32, rotate: Rotate) -> Instance {
    let lerp = |x: f32, y: f32| x + (y - x) * t;
    let turn = |from: f32, to: f32| {
        let d = (to - from).rem_euclid(TAU);
        from + t * match rotate {
            Rotate::Auto => if d > PI { d - TAU } else { d },
            Rotate::Cw(n) => d + TAU * n as f32,
            Rotate::Ccw(n) => (if d > 0.0 { d - TAU } else { 0.0 }) - TAU * n as f32,
        }
    };
    let decompose = |m: &Matrix| (m.a.hypot(m.b), m.c.hypot(m.d), (-m.c).atan2(m.d), m.b.atan2(m.a));
    let (sa, sb) = (decompose(&a.matrix), decompose(&b.matrix));
    let (sx, sy, skew_x, skew_y) = (lerp(sa.0, sb.0), lerp(sa.1, sb.1), turn(sa.2, sb.2), turn(sa.3, sb.3));

    let pivot = (lerp(a.pivot.0, b.pivot.0), lerp(a.pivot.1, b.pivot.1));
    let (pa, pb) = (a.matrix.apply(a.pivot), b.matrix.apply(b.pivot));
    let mut matrix = Matrix {
        a: sx * skew_y.cos(),
        b: sx * skew_y.sin(),
        c: -sy * skew_x.sin(),
        d: sy * skew_x.cos(),
        tx: 0.0,
        ty: 0.0,
    };
    let at = matrix.apply(pivot);
    (matrix.tx, matrix.ty) = (lerp(pa.0, pb.0) - at.0, lerp(pa.1, pb.1) - at.1);

    let color = ColorTransform {
        mult: std::array::from_fn(|i| lerp(a.color.mult[i], b.color.mult[i])),
        add: std::array::from_fn(|i| lerp(a.color.add[i], b.color.add[i])),
    };
    Instance { matrix, pivot, color, ..a.clone() }
}
