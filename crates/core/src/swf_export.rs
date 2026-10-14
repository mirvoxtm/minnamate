//! Exports a document as SWF, playable in Ruffle. Movie clips become sprites
//! with their own timelines. Graphic symbols and classic tweens are baked
//! frame by frame, as Animate's own SWF publish does. Shapes map one to one,
//! since SWF uses the same two-sided edge model as XFL.

use std::collections::{BTreeSet, HashMap};

use swf::{
    CharacterId, Color, Compression, Fixed8, Fixed16, FillStyle, Header, LineCapStyle, LineJoinStyle, LineStyle,
    PlaceObject, PlaceObjectAction, Point, PointDelta, Rectangle, RemoveObject, ShapeFlag, ShapeRecord, ShapeStyles,
    Sprite, StyleChangeData, Tag, Twips,
};

use crate::Result;
use crate::display::{Item, Placed, display_list};
use crate::doc::{self, Cap, Document, Join, Paint, Seg, Shape, Symbol, SymbolKind, Text};
use crate::text;

/// The SWF file, plus the features that couldn't be exported.
pub fn export(doc: &Document) -> Result<(Vec<u8>, BTreeSet<String>)> {
    let mut ex = Exporter { doc, defs: Vec::new(), ids: HashMap::new(), skipped: BTreeSet::new() };
    let mut lists = Vec::new();
    for scene in &doc.scenes {
        for f in 0..scene.frame_count() {
            lists.push(display_list(doc, scene, f, &mut ex.skipped));
        }
    }
    let main = ex.timeline(&lists)?;

    let [r, g, b, _] = rgba(doc.background);
    let mut tags = vec![Tag::SetBackgroundColor(Color { r, g, b, a: 255 })];
    tags.extend(ex.defs);
    tags.extend(main);
    let header = Header {
        compression: Compression::Zlib,
        version: 10,
        stage_size: Rectangle { x_min: Twips::ZERO, x_max: twips(doc.width), y_min: Twips::ZERO, y_max: twips(doc.height) },
        frame_rate: Fixed8::from_f32(doc.frame_rate),
        num_frames: lists.len().try_into().map_err(|_| "over 65535 frames")?,
    };
    let mut out = Vec::new();
    swf::write_swf(&header, &tags, &mut out)?;
    Ok((out, ex.skipped))
}

struct Exporter<'a> {
    doc: &'a Document,
    /// Character definitions, each after the ones it uses.
    defs: Vec<Tag<'static>>,
    /// Character ids by (address of the shape or symbol, exported as a button).
    ids: HashMap<(usize, bool), CharacterId>,
    skipped: BTreeSet<String>,
}

impl<'a> Exporter<'a> {
    /// Control tags that play `lists` (one display list per frame). Item `i`
    /// lives at depth `i + 1`; each frame only sends what changed.
    fn timeline(&mut self, lists: &[Vec<Placed<'a>>]) -> Result<Vec<Tag<'static>>> {
        let mut tags = Vec::new();
        let mut prev: Vec<(CharacterId, swf::Matrix, swf::ColorTransform)> = Vec::new();
        for list in lists {
            let cur = list.iter().map(|p| Ok((self.character(p)?, matrix(&p.matrix), color_transform(&p.color))));
            let cur = cur.collect::<Result<Vec<_>>>()?;
            // ponytail: depth is list position, so a clip that shifts position (something
            // added below it) restarts. Stable per-layer depths fix that when it matters.
            for (i, &(id, m, cx)) in cur.iter().enumerate() {
                let action = match prev.get(i) {
                    Some(&old) if old == (id, m, cx) => continue,
                    Some(&(old, ..)) if old == id => PlaceObjectAction::Modify,
                    Some(_) => PlaceObjectAction::Replace(id),
                    None => PlaceObjectAction::Place(id),
                };
                tags.push(Tag::PlaceObject(Box::new(PlaceObject {
                    version: 2,
                    action,
                    depth: i as u16 + 1,
                    matrix: Some(m),
                    color_transform: Some(cx),
                    ratio: None,
                    name: None,
                    clip_depth: None,
                    class_name: None,
                    filters: None,
                    background_color: None,
                    blend_mode: None,
                    clip_actions: None,
                    has_image: false,
                    is_bitmap_cached: None,
                    is_visible: None,
                    amf_data: None,
                })));
            }
            for depth in cur.len()..prev.len() {
                tags.push(Tag::RemoveObject(RemoveObject { depth: depth as u16 + 1, character_id: None }));
            }
            tags.push(Tag::ShowFrame);
            prev = cur;
        }
        Ok(tags)
    }

    fn character(&mut self, p: &Placed<'a>) -> Result<CharacterId> {
        match p.item {
            Item::Shape(s) => self.shape(s),
            Item::Text(t) => self.text(t),
            Item::Clip(sym, kind) => self.sprite(sym, kind == SymbolKind::Button),
        }
    }

    /// The id for `key`, and whether it still needs defining.
    fn id(&mut self, key: (usize, bool)) -> Result<(CharacterId, bool)> {
        if let Some(&id) = self.ids.get(&key) {
            return Ok((id, false));
        }
        let id = CharacterId::try_from(self.ids.len() + 1).map_err(|_| "over 65535 shapes and symbols")?;
        self.ids.insert(key, id);
        Ok((id, true))
    }

    fn sprite(&mut self, sym: &'a Symbol, button: bool) -> Result<CharacterId> {
        let (id, new) = self.id((sym as *const Symbol as usize, button))?;
        if new {
            // ponytail: buttons export their Up frame only; DefineButton2 states and hit areas come later.
            let frames = if button { 1 } else { sym.timeline.frame_count().max(1) };
            let lists: Vec<_> = (0..frames).map(|f| display_list(self.doc, &sym.timeline, f, &mut self.skipped)).collect();
            let tags = self.timeline(&lists)?;
            let num_frames = frames.try_into().map_err(|_| "symbol over 65535 frames")?;
            self.defs.push(Tag::DefineSprite(Sprite { id, num_frames, tags }));
        }
        Ok(id)
    }

    fn shape(&mut self, s: &Shape) -> Result<CharacterId> {
        let (id, new) = self.id((s as *const Shape as usize, false))?;
        if new {
            self.define_shape(id, s);
        }
        Ok(id)
    }

    /// Text goes out as the shape of its glyphs.
    // ponytail: so it can't be selected, and dynamic and input text are fixed. DefineFont3 and
    // DefineEditText would keep them live.
    fn text(&mut self, t: &Text) -> Result<CharacterId> {
        let (id, new) = self.id((t as *const Text as usize, false))?;
        if new {
            self.define_shape(id, &text::to_shape(t));
        }
        Ok(id)
    }

    fn define_shape(&mut self, id: CharacterId, s: &Shape) {
        let fill_styles = s.fills.iter().map(|f| FillStyle::Color(self.color(f))).collect();
        let line_styles = s.strokes.iter().map(|st| {
            let cap = match st.cap {
                Cap::None => LineCapStyle::None,
                Cap::Round => LineCapStyle::Round,
                Cap::Square => LineCapStyle::Square,
            };
            LineStyle::new()
                .with_width(twips(st.width))
                .with_color(self.color(&st.paint))
                .with_start_cap(cap)
                .with_end_cap(cap)
                .with_join_style(match st.join {
                    Join::Miter => LineJoinStyle::Miter(Fixed8::from_f32(3.0)),
                    Join::Round => LineJoinStyle::Round,
                    Join::Bevel => LineJoinStyle::Bevel,
                })
        });
        let line_styles: Vec<_> = line_styles.collect();

        // Deltas run between rounded absolute positions, so rounding never drifts.
        let tw = |(x, y): doc::Point| (twips(x), twips(y));
        let delta = |a: (Twips, Twips), b: (Twips, Twips)| PointDelta::new(b.0 - a.0, b.1 - a.1);
        let mut records = Vec::new();
        let mut points = Vec::new();
        for e in &s.edges {
            for path in &e.paths {
                let mut pen = tw(path.start);
                records.push(ShapeRecord::StyleChange(Box::new(StyleChangeData {
                    move_to: Some(Point::new(pen.0, pen.1)),
                    fill_style_0: Some(e.fill0),
                    fill_style_1: Some(e.fill1),
                    line_style: Some(e.stroke),
                    new_styles: None,
                })));
                points.push(pen);
                for &seg in &path.segs {
                    let to = tw(seg.end());
                    records.push(match seg {
                        Seg::Line(_) => ShapeRecord::StraightEdge { delta: delta(pen, to) },
                        Seg::Quad(c, _) => {
                            let c = tw(c);
                            points.push(c);
                            ShapeRecord::CurvedEdge { control_delta: delta(pen, c), anchor_delta: delta(c, to) }
                        }
                    });
                    points.push(to);
                    pen = to;
                }
            }
        }

        // Control points included: a slightly loose box, never a clipped one.
        let edge_bounds = match points.first() {
            None => Rectangle::default(),
            Some(&(x, y)) => points.iter().fold(Rectangle { x_min: x, x_max: x, y_min: y, y_max: y }, |r, &(x, y)| {
                Rectangle { x_min: r.x_min.min(x), x_max: r.x_max.max(x), y_min: r.y_min.min(y), y_max: r.y_max.max(y) }
            }),
        };
        let pad = twips(s.strokes.iter().map(|st| st.width / 2.0).fold(0.0, f32::max));
        let shape_bounds = Rectangle {
            x_min: edge_bounds.x_min - pad,
            x_max: edge_bounds.x_max + pad,
            y_min: edge_bounds.y_min - pad,
            y_max: edge_bounds.y_max + pad,
        };
        let flags = if s.strokes.is_empty() { ShapeFlag::empty() } else { ShapeFlag::HAS_SCALING_STROKES };
        self.defs.push(Tag::DefineShape(Box::new(swf::Shape {
            version: 4,
            id,
            shape_bounds,
            edge_bounds,
            flags,
            styles: ShapeStyles { fill_styles, line_styles },
            shape: records,
        })));
    }

    fn color(&mut self, p: &Paint) -> Color {
        match p {
            Paint::Solid(c) => {
                let [r, g, b, a] = rgba(*c);
                Color { r, g, b, a }
            }
            // Keeps the style's slot, since edges refer to styles by index.
            Paint::Unsupported { name, .. } => {
                self.skipped.insert(name.clone());
                Color { r: 0, g: 0, b: 0, a: 0 }
            }
        }
    }
}

fn twips(px: f32) -> Twips {
    Twips::new((px * 20.0).round() as i32)
}

fn rgba(c: doc::Rgba) -> [u8; 4] {
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn matrix(m: &doc::Matrix) -> swf::Matrix {
    swf::Matrix {
        a: Fixed16::from_f32(m.a),
        b: Fixed16::from_f32(m.b),
        c: Fixed16::from_f32(m.c),
        d: Fixed16::from_f32(m.d),
        tx: twips(m.tx),
        ty: twips(m.ty),
    }
}

fn color_transform(c: &doc::ColorTransform) -> swf::ColorTransform {
    let add = c.add.map(|v| (v * 255.0).round() as i16);
    swf::ColorTransform {
        r_multiply: Fixed8::from_f32(c.mult[0]),
        g_multiply: Fixed8::from_f32(c.mult[1]),
        b_multiply: Fixed8::from_f32(c.mult[2]),
        a_multiply: Fixed8::from_f32(c.mult[3]),
        r_add: add[0],
        g_add: add[1],
        b_add: add[2],
        a_add: add[3],
    }
}
