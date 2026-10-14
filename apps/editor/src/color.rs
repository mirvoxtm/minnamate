//! Colour controls (docs/animate-ui-interaction.md §3): the swatch popup every
//! colour chip opens, the Color and Swatches panels, and applying a colour or
//! stroke setting to the tool defaults and the selected shapes.

use eframe::egui::{
    self, Area, Color32, CursorIcon, Frame, Id, LayerId, Mesh, Order, Popup, PopupCloseBehavior, Pos2, Rect, Response, Sense, Shape,
    Stroke, StrokeKind, TextEdit, Ui, Vec2, WidgetInfo, WidgetType, pos2, vec2,
};
use minnamate_core::doc::{Cap, Element, Join, Paint};
use minnamate_core::paint::{self, Picked};

use crate::app::{Editor, rgba};
use crate::icons::{self, Icon};
use crate::theme;
use crate::widgets::{self, Number};

/// The swatch grid's pitch: 10 px cells between 1 px black lines.
const PITCH: f32 = 11.0;
const COLS: usize = 20;

/// Web 216 as the swatch popup lays it out, row by row in 12 rows of 20: a
/// column of greys and primaries, a black spacer column, then the cube (§3.1).
pub fn web216() -> Vec<Color32> {
    const K: [u8; 6] = [0x00, 0x33, 0x66, 0x99, 0xCC, 0xFF];
    const COL0: [u32; 12] = [
        0x000000, 0x333333, 0x666666, 0x999999, 0xCCCCCC, 0xFFFFFF, 0xFF0000, 0x00FF00, 0x0000FF, 0xFFFF00, 0x00FFFF, 0xFF00FF,
    ];
    let mut out = Vec::with_capacity(12 * COLS);
    for (row, &c) in COL0.iter().enumerate() {
        out.push(Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8));
        out.push(Color32::BLACK);
        let b = K[row % 6];
        let reds = if row < 6 { &K[..3] } else { &K[3..] };
        for &r in reds {
            out.extend(K.iter().map(|&g| Color32::from_rgb(r, g, b)));
        }
    }
    out
}

/// `c` with alpha `a` (0..=255), from its straight colour.
fn with_alpha(c: Color32, a: u8) -> Color32 {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

fn hex(c: Color32) -> String {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    format!("#{r:02X}{g:02X}{b:02X}")
}

fn parse_hex(s: &str) -> Option<Color32> {
    let s = s.trim().trim_start_matches('#');
    let v = u32::from_str_radix(s, 16).ok().filter(|_| s.len() == 6)?;
    Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/// The swatch popup's own state while it's open.
#[derive(Clone, Default)]
struct PopupState {
    /// Alpha in percent, given to whatever is picked.
    alpha: f32,
    /// The colour picker shows in place of the grid.
    picker: bool,
    /// What the eyedropper was over on the stage last frame.
    seen: Option<Color32>,
}

/// The swatch popup for colour chip `chip`, which a click opens: the Web 216
/// grid, a hex field, alpha, No Color (when `no_color`) and a colour picker.
/// While it's open the pointer is an eyedropper, and a click on the stage
/// picks the colour there. Returns the colour picked this frame; Esc or a
/// click anywhere else closes it unchanged.
pub fn swatch_popup(ui: &Ui, ed: &Editor, chip: &Response, current: Color32, no_color: bool) -> Option<Color32> {
    let ctx = ui.ctx().clone();
    let id = Popup::default_response_id(chip);
    if chip.clicked() {
        let alpha = if widgets::is_none(current) { 100.0 } else { (current.a() as f32 / 2.55).round() };
        ctx.data_mut(|d| d.insert_temp(id, PopupState { alpha, ..Default::default() }));
    }
    let mut state: PopupState = ctx.data_mut(|d| d.get_temp(id)).unwrap_or_default();
    let mut picked = None;
    let mut close = false;
    let mut hover: Option<Color32> = None;
    let mut over_grid = false;
    let frame = Frame::popup(ui.style()).fill(theme::PANEL_BG).stroke(Stroke::new(1.0, theme::BORDER)).inner_margin(4);
    let shown = Popup::from_toggle_button_response(chip).id(id).close_behavior(PopupCloseBehavior::IgnoreClicks).frame(frame).show(|ui| {
        ui.set_width(4.0 + COLS as f32 * PITCH + 1.0);
        ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
        let alpha = (state.alpha * 2.55).round() as u8;
        // The grid first, so the header can show what the pointer is over.
        let header = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover()).0;
        if state.picker {
            let mut c = if widgets::is_none(current) { Color32::BLACK } else { current };
            if egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::OnlyBlend) {
                picked = Some(c);
                state.alpha = (c.a() as f32 / 2.55).round();
            }
        } else {
            let colors = web216();
            let rows = colors.len().div_ceil(COLS);
            let (grid, resp) = ui.allocate_exact_size(vec2(COLS as f32 * PITCH + 1.0, rows as f32 * PITCH + 1.0), Sense::click());
            resp.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, "Swatch grid"));
            let p = ui.painter();
            p.rect_filled(grid, 0.0, Color32::BLACK);
            let cell_at = |pos: Pos2| {
                let (col, row) = (((pos.x - grid.left()) / PITCH) as usize, ((pos.y - grid.top()) / PITCH) as usize);
                (grid.contains(pos) && col < COLS && row < rows).then(|| row * COLS + col).filter(|&i| i < colors.len())
            };
            for (i, &c) in colors.iter().enumerate() {
                let r = Rect::from_min_size(grid.min + vec2((i % COLS) as f32 * PITCH + 1.0, (i / COLS) as f32 * PITCH + 1.0), vec2(10.0, 10.0));
                p.rect_filled(r, 0.0, c);
            }
            if let Some(i) = resp.hover_pos().and_then(cell_at) {
                over_grid = true;
                hover = Some(colors[i]);
                let r = Rect::from_min_size(grid.min + vec2((i % COLS) as f32 * PITCH + 1.0, (i / COLS) as f32 * PITCH + 1.0), vec2(10.0, 10.0));
                p.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::WHITE), StrokeKind::Inside);
            }
            if let Some(i) = resp.interact_pointer_pos().filter(|_| resp.clicked()).and_then(cell_at) {
                picked = Some(with_alpha(colors[i], alpha));
                close = true;
            }
        }

        // Header: preview, hex, alpha, then No Color and the picker at the right.
        let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(header).layout(egui::Layout::left_to_right(egui::Align::Center)));
        ui.spacing_mut().item_spacing.x = 4.0;
        let shown = hover.or(state.seen).unwrap_or(current);
        let (preview, _) = ui.allocate_exact_size(vec2(30.0, 18.0), Sense::hover());
        widgets::paint_swatch(ui.painter(), preview, shown, 0.0);
        if let Some(c) = hex_field(&mut ui, id.with("hex"), shown, 58.0) {
            picked = Some(with_alpha(c, alpha));
            close = true;
        }
        widgets::label(&mut ui, "Alpha:", 32.0);
        let a = ui.add(Number::new("Alpha", &mut state.alpha, 0.0..=100.0).suffix(" %").width(36.0));
        if a.changed() && !widgets::is_none(current) {
            picked = Some(with_alpha(current, (state.alpha * 2.55).round() as u8));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if theme::icon_button(ui, 18.0, state.picker, true, "Color Picker", |p, r, _| hue_wheel(p, r.shrink(2.0))).clicked() {
                state.picker = !state.picker;
            }
            if no_color && theme::icon_button(ui, 18.0, false, true, "No Color", |p, r, _| widgets::paint_swatch(p, r.shrink(2.0), Color32::TRANSPARENT, 0.0)).clicked() {
                picked = Some(Color32::TRANSPARENT);
                close = true;
            }
        });
    });
    let open = shown.is_some() && !close;

    // The eyedropper: a pane over the rest of the window takes the pointer, so
    // a click there picks from the stage or just closes, and nothing under it acts.
    state.seen = None;
    if open && !chip.clicked() {
        let screen = ctx.content_rect();
        let blocker = Area::new(id.with("eyedropper")).order(Order::Middle).fixed_pos(screen.min).show(&ctx, |ui| ui.allocate_rect(screen, Sense::click()));
        let resp = blocker.inner;
        if resp.hovered() {
            over_grid = true;
            state.seen = resp.hover_pos().and_then(|p| sample(ed, p));
        }
        if resp.clicked() {
            picked = resp.interact_pointer_pos().and_then(|p| sample(ed, p)).map(|c| with_alpha(c, (state.alpha * 2.55).round() as u8));
            close = true;
        }
    }
    if over_grid && let Some(pos) = ctx.pointer_hover_pos() {
        ctx.set_cursor_icon(CursorIcon::None);
        eyedropper(&ctx.layer_painter(LayerId::new(Order::Tooltip, id.with("pointer"))), pos);
    }
    if close {
        Popup::close_id(&ctx, id);
    }
    ctx.data_mut(|d| d.insert_temp(id, state));
    picked
}

/// What the eyedropper sees at screen point `pos`: a fill or stroke on the
/// stage, else the stage colour; nothing off the stage.
// ponytail: colours as the shapes hold them, before color effects (paint::pick's limit),
// and only from the stage: egui can't read pixels of the rest of the window.
fn sample(ed: &Editor, pos: Pos2) -> Option<Color32> {
    let (x, y) = crate::stage::local_point(ed, ed.stage_rect, pos);
    let scene = ed.to_scene((x, y));
    let on_stage = (0.0..=ed.doc.width).contains(&scene.0) && (0.0..=ed.doc.height).contains(&scene.1);
    let picked = paint::pick(&ed.doc, ed.timeline(), ed.frame, (x, y), 1.0 / ed.zoom);
    let c = match picked {
        Some(Picked::Fill(Paint::Solid(c)) | Picked::Stroke(minnamate_core::doc::Stroke { paint: Paint::Solid(c), .. })) => c,
        _ if on_stage => ed.doc.background,
        _ => return None,
    };
    let [r, g, b, _] = c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
    Some(Color32::from_rgb(r, g, b))
}

/// The Eyedropper glyph as the pointer: black with a white rim, its tip at `pos`.
fn eyedropper(p: &egui::Painter, pos: Pos2) {
    let glyph = Rect::from_min_size(pos - vec2(3.0, 15.0), vec2(18.0, 18.0));
    for d in [vec2(-1.0, 0.0), vec2(1.0, 0.0), vec2(0.0, -1.0), vec2(0.0, 1.0)] {
        icons::paint(p, glyph.translate(d), Icon::Eyedropper, Color32::WHITE);
    }
    icons::paint(p, glyph, Icon::Eyedropper, Color32::BLACK);
}

/// The Color Picker button's glyph: a hue wheel.
fn hue_wheel(p: &egui::Painter, r: Rect) {
    let (c, radius) = (r.center(), r.width().min(r.height()) / 2.0);
    let mut mesh = Mesh::default();
    mesh.colored_vertex(c, Color32::WHITE);
    const N: u32 = 24;
    for i in 0..=N {
        let t = i as f32 / N as f32;
        let a = t * std::f32::consts::TAU;
        mesh.colored_vertex(c + radius * vec2(a.cos(), a.sin()), hsb(t * 360.0, 1.0, 1.0));
        if i > 0 {
            mesh.add_triangle(0, i, i + 1);
        }
    }
    p.add(Shape::mesh(mesh));
}

/// HSB (hue in degrees, saturation and brightness 0..=1) to an opaque colour, in sRGB as Animate shows it.
fn hsb(h: f32, s: f32, b: f32) -> Color32 {
    let h = h.rem_euclid(360.0) / 60.0;
    let f = h - h.floor();
    let (p, q, t) = (b * (1.0 - s), b * (1.0 - s * f), b * (1.0 - s * (1.0 - f)));
    let (r, g, bl) = match h as u32 {
        0 => (b, t, p),
        1 => (q, b, p),
        2 => (p, b, t),
        3 => (p, q, b),
        4 => (t, p, b),
        _ => (b, p, q),
    };
    let u = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgb(u(r), u(g), u(bl))
}

/// An opaque colour's hue (degrees), saturation and brightness (0..=1).
fn to_hsb(c: Color32) -> (f32, f32, f32) {
    let [r, g, b, _] = c.to_srgba_unmultiplied().map(|v| v as f32 / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (h, if max == 0.0 { 0.0 } else { d / max }, max)
}

/// A stroke or colour setting from Properties or the Color panels.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Style {
    Fill(Color32),
    Stroke(Color32),
    Width(f32),
    Cap(Cap),
    Join(Join),
}

/// Sets a drawing default and gives the selected shapes the same: their fills
/// or their strokes (no colour removes them; a shape without strokes gets
/// none). One drag, or one pick, is one undo step.
pub fn restyle(ed: &mut Editor, style: Style) {
    restyle_all(ed, &[style]);
}

/// `restyle` with several settings at once (Swap Colors, Black and White), as one undo step.
pub fn restyle_all(ed: &mut Editor, styles: &[Style]) {
    for &style in styles {
        match style {
            Style::Fill(c) => ed.fill_color = c,
            Style::Stroke(c) => ed.stroke_color = c,
            Style::Width(w) => ed.stroke_width = w,
            Style::Cap(c) => ed.stroke_cap = c,
            Style::Join(j) => ed.stroke_join = j,
        }
    }
    let shapes: Vec<_> = ed.selection.iter().copied().filter(|&r| matches!(ed.element(r), Some(Element::Shape(_)))).collect();
    let Some(&first) = styles.first().filter(|_| !shapes.is_empty()) else { return };
    ed.checkpoint_gesture(match first {
        _ if styles.len() > 1 => "colors",
        Style::Fill(_) => "fill-color",
        Style::Stroke(_) => "stroke-color",
        Style::Width(_) => "stroke-width",
        Style::Cap(_) | Style::Join(_) => "stroke-ends",
    });
    for r in shapes {
        let Some(Element::Shape(s)) = ed.element_mut(r) else { continue };
        for &style in styles {
            match style {
                Style::Fill(c) if widgets::is_none(c) => {
                    s.fills.clear();
                    s.edges.iter_mut().for_each(|e| (e.fill0, e.fill1) = (0, 0));
                }
                Style::Stroke(c) if widgets::is_none(c) => {
                    s.strokes.clear();
                    s.edges.iter_mut().for_each(|e| e.stroke = 0);
                }
                Style::Fill(c) => s.fills.iter_mut().for_each(|f| *f = Paint::Solid(rgba(c))),
                Style::Stroke(c) => s.strokes.iter_mut().for_each(|st| st.paint = Paint::Solid(rgba(c))),
                Style::Width(w) => s.strokes.iter_mut().for_each(|st| st.width = w),
                Style::Cap(c) => s.strokes.iter_mut().for_each(|st| st.cap = c),
                Style::Join(j) => s.strokes.iter_mut().for_each(|st| st.join = j),
            }
        }
    }
}

/// Which colour the Color and Swatches panels edit: the fill (true) or the stroke.
fn fill_active(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp(Id::new("color-fill-active"))).unwrap_or(true)
}

/// The Color panel's components; the selected one drives the slider.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
enum Comp {
    #[default]
    H,
    S,
    B,
    R,
    G,
    Bl,
}

/// Window > Color (§3.2): stroke and fill controls, colour type, the colour
/// square with its slider and fields, hex and a preview bar.
pub fn color_panel(ui: &mut Ui, ed: &mut Editor) {
    let ctx = ui.ctx().clone();
    let fill = fill_active(&ctx);
    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
    ui.horizontal(|ui| {
        for (is_fill, icon, tip) in [(false, Icon::Pencil, "Stroke color"), (true, Icon::PaintBucket, "Fill color")] {
            if theme::icon_button(ui, 22.0, fill == is_fill, true, tip, |p, r, c| icons::paint(p, r.shrink(4.0), icon, c)).clicked() {
                ctx.data_mut(|d| d.insert_temp(Id::new("color-fill-active"), is_fill));
            }
            let current = if is_fill { ed.fill_color } else { ed.stroke_color };
            let chip = widgets::chip(ui, current, tip);
            if let Some(c) = swatch_popup(ui, ed, &chip, current, true) {
                restyle(ed, if is_fill { Style::Fill(c) } else { Style::Stroke(c) });
            }
            ui.add_space(4.0);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if theme::icon_button(ui, 22.0, false, true, "Swap colors", |p, r, c| icons::paint(p, r.shrink(4.0), Icon::Swap, c)).clicked()
                && !widgets::is_none(ed.stroke_color)
                && !widgets::is_none(ed.fill_color)
            {
                let (s, f) = (ed.stroke_color, ed.fill_color);
                restyle_all(ed, &[Style::Stroke(f), Style::Fill(s)]);
            }
            if theme::icon_button(ui, 22.0, false, true, "No color", |p, r, _| widgets::paint_swatch(p, r.shrink(5.0), Color32::TRANSPARENT, 0.0)).clicked() {
                restyle(ed, if fill { Style::Fill(Color32::TRANSPARENT) } else { Style::Stroke(Color32::TRANSPARENT) });
            }
            if theme::icon_button(ui, 22.0, false, true, "Black and white", |p, r, _| black_and_white(p, r)).clicked() {
                restyle_all(ed, &[Style::Stroke(Color32::BLACK), Style::Fill(Color32::WHITE)]);
            }
        });
    });
    let current = if fill { ed.fill_color } else { ed.stroke_color };
    let set = |ed: &mut Editor, c: Color32| restyle(ed, if fill { Style::Fill(c) } else { Style::Stroke(c) });
    widgets::row(ui, |ui| {
        widgets::label(ui, "Color type:", 64.0);
        let kind = if widgets::is_none(current) { 0 } else { 1 };
        // ponytail: gradients and bitmap fills wait on the model (Paint::Unsupported keeps them as XML).
        let items = [(0, "None", true), (1, "Solid color", true), (2, "Linear gradient", false), (3, "Radial gradient", false), (4, "Bitmap fill", false)];
        match widgets::pick(ui, "Color type", kind, &items, 150.0) {
            Some(0) => set(ed, Color32::TRANSPARENT),
            Some(1) => set(ed, Color32::BLACK),
            _ => {}
        }
    });

    // The square and slider show the other components of the selected one (Photoshop's way).
    let base = if widgets::is_none(current) { Color32::BLACK } else { current };
    let alpha = if widgets::is_none(current) { 255 } else { current.a() };
    let (h, s, b) = to_hsb(base);
    let [r, g, bl, _] = base.to_srgba_unmultiplied().map(|v| v as f32 / 255.0);
    let comp_id = Id::new("color-panel-comp");
    let mut comp: Comp = ctx.data(|d| d.get_temp(comp_id)).unwrap_or_default();
    // (x, y) of the square and the slider's value, each 0..=1, for the selected component.
    let axes = |comp: Comp, x: f32, y: f32, z: f32| -> Color32 {
        let u = |v: f32| (v * 255.0).round() as u8;
        let rgb = |r: f32, g: f32, b: f32| Color32::from_rgb(u(r), u(g), u(b));
        match comp {
            Comp::H => hsb(z * 360.0, x, y),
            Comp::S => hsb(x * 360.0, z, y),
            Comp::B => hsb(x * 360.0, y, z),
            Comp::R => rgb(z, y, x),
            Comp::G => rgb(y, z, x),
            Comp::Bl => rgb(x, y, z),
        }
    };
    let (x, y, z) = match comp {
        Comp::H => (s, b, h / 360.0),
        Comp::S => (h / 360.0, b, s),
        Comp::B => (h / 360.0, s, b),
        Comp::R => (bl, g, r),
        Comp::G => (bl, r, g),
        Comp::Bl => (r, g, bl),
    };
    let (mut new, mut new_alpha) = (None, None);
    // A typed value is an undo step of its own.
    let mut let_go = false;
    ui.horizontal_top(|ui| {
        let (square, sq) = ui.allocate_exact_size(vec2(150.0, 120.0), Sense::click_and_drag());
        gradient(ui.painter(), square, |u, v| axes(comp, u, 1.0 - v, z));
        let (strip, st) = ui.allocate_exact_size(vec2(14.0, 120.0), Sense::click_and_drag());
        gradient(ui.painter(), strip, |_, v| axes(comp, x, y, 1.0 - v));
        let p = ui.painter();
        p.circle_stroke(pos2(square.left() + x * square.width(), square.bottom() - y * square.height()), 4.0, Stroke::new(1.5, Color32::WHITE));
        let sy = strip.bottom() - z * strip.height();
        p.hline(strip.left() - 2.0..=strip.right() + 2.0, sy, Stroke::new(2.0, Color32::WHITE));
        if let Some(pos) = sq.interact_pointer_pos().filter(|_| sq.dragged() || sq.clicked()) {
            let u = ((pos.x - square.left()) / square.width()).clamp(0.0, 1.0);
            let v = ((square.bottom() - pos.y) / square.height()).clamp(0.0, 1.0);
            new = Some(axes(comp, u, v, z));
        }
        if let Some(pos) = st.interact_pointer_pos().filter(|_| st.dragged() || st.clicked()) {
            new = Some(axes(comp, x, y, ((strip.bottom() - pos.y) / strip.height()).clamp(0.0, 1.0)));
        }
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let mut vals = [h, s * 100.0, b * 100.0, r * 255.0, g * 255.0, bl * 255.0];
            let rows = [(Comp::H, "H:", " °", 360.0), (Comp::S, "S:", " %", 100.0), (Comp::B, "B:", " %", 100.0), (Comp::R, "R:", "", 255.0), (Comp::G, "G:", "", 255.0), (Comp::Bl, "B:", "", 255.0)];
            for (i, (c, name, unit, max)) in rows.into_iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if radio(ui, comp == c).clicked() {
                        comp = c;
                    }
                    widgets::label(ui, name, 14.0);
                    let field = ui.add(Number::new(name, &mut vals[i], 0.0..=max).suffix(unit).width(46.0));
                    if field.changed() {
                        new = Some(if i < 3 {
                            hsb(vals[0], vals[1] / 100.0, vals[2] / 100.0)
                        } else {
                            Color32::from_rgb(vals[3].round() as u8, vals[4].round() as u8, vals[5].round() as u8)
                        });
                    }
                    let_go |= field.drag_stopped() || field.lost_focus();
                });
            }
            ui.horizontal(|ui| {
                ui.add_space(18.0);
                widgets::label(ui, "A:", 14.0);
                let mut a = (alpha as f32 / 2.55).round();
                let field = ui.add(Number::new("A:", &mut a, 0.0..=100.0).suffix(" %").width(46.0));
                if field.changed() {
                    new_alpha = Some((a * 2.55).round() as u8);
                }
                let_go |= field.drag_stopped() || field.lost_focus();
            });
        });
    });
    ctx.data_mut(|d| d.insert_temp(comp_id, comp));
    if new.is_some() || new_alpha.is_some() {
        set(ed, with_alpha(new.unwrap_or(base), new_alpha.unwrap_or(alpha)));
    }
    ui.horizontal(|ui| {
        if let Some(c) = hex_field(ui, Id::new("color-panel-hex"), current, 64.0) {
            set(ed, with_alpha(c, alpha));
            let_go = true;
        }
        let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::hover());
        widgets::paint_swatch(ui.painter(), bar, current, 0.0);
    });
    if let_go {
        ed.end_gesture();
    }
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
        // ponytail: swatches the user adds need a home in the document model.
        ui.add_enabled_ui(false, |ui| widgets::button(ui, "Add to Swatches", 110.0));
    });
}

/// A "#RRGGBB" field showing `shown`; returns what was typed once Enter or
/// leaving the field commits it (Esc reverts).
fn hex_field(ui: &mut Ui, id: Id, shown: Color32, width: f32) -> Option<Color32> {
    let typed: Option<String> = ui.data(|d| d.get_temp(id));
    let mut text = typed.unwrap_or_else(|| if widgets::is_none(shown) { String::new() } else { hex(shown) });
    let field = ui.add(TextEdit::singleline(&mut text).id(id).desired_width(width).char_limit(7));
    if field.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, text.clone()));
    }
    if !field.lost_focus() {
        return None;
    }
    ui.data_mut(|d| d.remove::<String>(id));
    parse_hex(&text).filter(|_| !ui.input(|i| i.key_pressed(egui::Key::Escape)))
}

/// A small radio button.
fn radio(ui: &mut Ui, on: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(14.0, 20.0), Sense::click());
    let c = rect.center();
    ui.painter().circle_stroke(c, 5.0, Stroke::new(1.2, theme::CHECKBOX_BORDER));
    if on {
        ui.painter().circle_filled(c, 2.5, theme::TEXT);
    }
    resp
}

/// `rect` filled with `color(u, v)`, u and v running 0..=1 left to right and top to bottom.
fn gradient(p: &egui::Painter, rect: Rect, color: impl Fn(f32, f32) -> Color32) {
    const N: u32 = 24;
    let mut mesh = Mesh::default();
    for j in 0..=N {
        for i in 0..=N {
            let (u, v) = (i as f32 / N as f32, j as f32 / N as f32);
            mesh.colored_vertex(rect.min + vec2(u * rect.width(), v * rect.height()), color(u, v));
            if i > 0 && j > 0 {
                let k = j * (N + 1) + i;
                mesh.add_triangle(k - N - 2, k - N - 1, k);
                mesh.add_triangle(k - N - 2, k - 1, k);
            }
        }
    }
    p.add(Shape::mesh(mesh));
}

/// Black and White's glyph: a black square over a white one, on the diagonal.
fn black_and_white(p: &egui::Painter, r: Rect) {
    let back = Rect::from_min_size(r.min + vec2(9.0, 9.0), vec2(8.0, 8.0));
    let front = Rect::from_min_size(r.min + vec2(5.0, 5.0), vec2(8.0, 8.0));
    p.rect_filled(back, 0.0, Color32::WHITE);
    p.rect_stroke(back, 0.0, Stroke::new(1.0, theme::TEXT_DIM), StrokeKind::Inside);
    p.rect_filled(front, 0.0, Color32::BLACK);
    p.rect_stroke(front, 0.0, Stroke::new(1.0, theme::TEXT_DIM), StrokeKind::Inside);
}

/// Window > Swatches (§3.3): the Web 216 grid reflowed to the panel's width;
/// a click gives the swatch to the colour the Color panel edits.
pub fn swatches_panel(ui: &mut Ui, ed: &mut Editor) {
    let fill = fill_active(ui.ctx());
    let colors = web216();
    let cols = ((ui.available_width() / PITCH) as usize).max(1);
    let rows = colors.len().div_ceil(cols);
    let (grid, resp) = ui.allocate_exact_size(vec2(cols as f32 * PITCH + 1.0, rows as f32 * PITCH + 1.0), Sense::click());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, "Swatches"));
    let p = ui.painter();
    let cell = |i: usize| Rect::from_min_size(grid.min + vec2((i % cols) as f32 * PITCH + 1.0, (i / cols) as f32 * PITCH + 1.0), vec2(10.0, 10.0));
    // Each swatch in its black frame; the last row stops at its last swatch.
    for i in 0..colors.len() {
        p.rect_filled(cell(i).expand(1.0), 0.0, Color32::BLACK);
    }
    for (i, &c) in colors.iter().enumerate() {
        p.rect_filled(cell(i), 0.0, c);
    }
    let at = |pos: Pos2| {
        let v: Vec2 = (pos - grid.min) / PITCH;
        let i = v.y as usize * cols + v.x as usize;
        (v.x < cols as f32 && i < colors.len()).then_some(i)
    };
    let current = if fill { ed.fill_color } else { ed.stroke_color };
    if let Some(i) = colors.iter().position(|&c| c == current) {
        p.rect_stroke(cell(i), 0.0, Stroke::new(1.0, Color32::WHITE), StrokeKind::Inside);
    }
    if let Some(i) = resp.interact_pointer_pos().filter(|_| resp.clicked()).and_then(at) {
        restyle(ed, if fill { Style::Fill(colors[i]) } else { Style::Stroke(colors[i]) });
    }
    ui.add_space(ui.available_height() - 26.0);
    // ponytail: tagged swatches need a home in the document model; the bar is greyed till then.
    ui.horizontal(|ui| {
        for (icon, tip) in [(Icon::Tag, "Convert to Tagged Swatch"), (Icon::Pencil, "Edit"), (Icon::Delete, "Delete")] {
            theme::icon_button(ui, theme::ICON_BUTTON, false, false, tip, |p, r, c| icons::paint(p, r.shrink(4.0), icon, c));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web216_layout() {
        let c = web216();
        assert_eq!(c.len(), 240);
        assert_eq!((c[0], c[1], c[COLS * 11 + 19]), (Color32::BLACK, Color32::BLACK, Color32::WHITE), "black at the top left, white at the bottom right");
        assert_eq!(c[COLS * 6 + 2], Color32::from_rgb(0x99, 0, 0), "the lower half starts from the reds");
        let mut cube: Vec<_> = c.iter().enumerate().filter(|(i, _)| i % COLS >= 2).map(|(_, c)| c.to_array()).collect();
        cube.sort();
        cube.dedup();
        assert_eq!(cube.len(), 216, "every web-safe colour once");
    }

    #[test]
    fn restyling_the_selection() {
        use crate::app::testkit::*;
        let mut h = blank();
        h.state_mut().tool = crate::tools::Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        let ed = h.state_mut();
        ed.selection = ed.all_elements();
        let shape = |ed: &Editor| match ed.element(ed.selection[0]) {
            Some(Element::Shape(s)) => s.clone(),
            _ => panic!("no shape"),
        };
        let before = shape(ed);
        restyle_all(ed, &[Style::Stroke(Color32::BLACK), Style::Fill(Color32::WHITE)]);
        assert_eq!(shape(ed).fills, [Paint::Solid([1.0; 4])]);
        ed.end_gesture();
        restyle(ed, Style::Fill(Color32::TRANSPARENT));
        let s = shape(ed);
        assert!(s.fills.is_empty() && s.edges.iter().all(|e| e.fill0 == 0 && e.fill1 == 0), "no colour removes the fill");
        let ctx = h.ctx.clone();
        h.state_mut().run(crate::app::Action::Undo, &ctx);
        h.state_mut().run(crate::app::Action::Undo, &ctx);
        assert_eq!(shape(h.state()), before, "black and white was one step");
    }

    #[test]
    fn hsb_round_trip() {
        for c in [Color32::from_rgb(0x00, 0x66, 0xCC), Color32::from_rgb(200, 30, 90), Color32::WHITE, Color32::BLACK] {
            let (h, s, b) = to_hsb(c);
            assert_eq!(hsb(h, s, b), c);
        }
        assert_eq!(parse_hex("#0066cc"), Some(Color32::from_rgb(0, 0x66, 0xCC)));
        assert_eq!(hex(Color32::from_rgb(0, 0x66, 0xCC)), "#0066CC");
    }
}
