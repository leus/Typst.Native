//! Core rendering logic that walks Typst frames and emits SLA objects.

use std::collections::HashMap;

use typst_layout::Page;
use typst_library::layout::{
    Abs, Frame, FrameItem, Point, Size, Transform,
};
use typst_library::text::TextItem;
use typst_library::visualize::{
    CurveItem, Geometry, Image, Paint, Shape,
};
use xmlwriter::XmlWriter;

use crate::color::SlaColor;

/// Try to find and decode the name-table entry with the given id.
/// `Font::find_name` is no longer public in typst 0.15.
fn find_font_name(ttf: &ttf_parser::Face, name_id: u16) -> Option<String> {
    ttf.names()
        .into_iter()
        .find_map(|entry| if entry.name_id == name_id { entry.to_string() } else { None })
}

/// Tracks state while rendering to SLA.
pub struct SlaRenderer {
    obj_id: usize,
    colors: HashMap<String, SlaColor>,
}

impl SlaRenderer {
    pub fn new() -> Self {
        Self {
            obj_id: 1_000_000, // start high to avoid collisions
            colors: HashMap::new(),
        }
    }

    fn next_id(&mut self) -> usize {
        let id = self.obj_id;
        self.obj_id += 1;
        id
    }

    fn register_color(&mut self, color: &SlaColor) -> String {
        let name = color.name.clone();
        self.colors.entry(name.clone()).or_insert_with(|| color.clone());
        name
    }

    pub fn colors(&self) -> &HashMap<String, SlaColor> {
        &self.colors
    }

    pub fn collect_colors_page(&mut self, page: &Page) {
        if let Some(paint) = page.fill_or_transparent() {
            let color = SlaColor::from_paint(&paint);
            self.register_color(&color);
        }
        self.collect_colors_frame(&page.frame);
    }

    fn collect_colors_frame(&mut self, frame: &Frame) {
        for (_pos, item) in frame.items() {
            match item {
                FrameItem::Group(group) => {
                    self.collect_colors_frame(&group.frame);
                }
                FrameItem::Text(text) => {
                    let color = SlaColor::from_paint(&text.fill);
                    self.register_color(&color);
                }
                FrameItem::Shape(shape, _span) => {
                    if let Some(paint) = &shape.fill {
                        let color = SlaColor::from_paint(paint);
                        self.register_color(&color);
                    }
                    if let Some(stroke) = &shape.stroke {
                        let color = SlaColor::from_paint(&stroke.paint);
                        self.register_color(&color);
                    }
                }
                FrameItem::Image(_, _, _)
                | FrameItem::Link(_, _)
                | FrameItem::Tag(_) => {}
            }
        }
    }

    /// Render all objects on a page.
    pub fn render_page(
        &mut self,
        xml: &mut XmlWriter,
        page_idx: usize,
        page: &Page,
        page_ypos: f64,
    ) {
        let page_height = page.frame.height().to_pt();

        // If the page has a background fill, emit a rectangle for it.
        if let Some(paint) = page.fill_or_transparent() {
            let w = page.frame.width().to_pt();
            self.emit_rect_fill(xml, page_idx, 0.0, 0.0, w, page_height, &paint, page_ypos);
        }

        let origin = Point::new(Abs::pt(0.0), Abs::pt(0.0));
        self.render_frame(xml, page_idx, &page.frame, origin, Transform::identity(), page_ypos);
    }

    fn render_frame(
        &mut self,
        xml: &mut XmlWriter,
        page_idx: usize,
        frame: &Frame,
        origin: Point,
        transform: Transform,
        page_ypos: f64,
    ) {
        for (pos, item) in frame.items() {
            let abs_pos = apply_transform(&transform, &(*pos + origin));
            match item {
                FrameItem::Group(group) => {
                    let combined = combine_transforms(&transform, &group.transform);
                    self.render_frame(xml, page_idx, &group.frame, abs_pos, combined, page_ypos);
                }
                FrameItem::Text(text) => {
                    self.render_text(xml, page_idx, abs_pos, text, page_ypos);
                }
                FrameItem::Shape(shape, _span) => {
                    self.render_shape(xml, page_idx, abs_pos, shape, page_ypos);
                }
                FrameItem::Image(image, size, _span) => {
                    self.render_image(xml, page_idx, abs_pos, image, *size, page_ypos);
                }
                FrameItem::Link(_, _) | FrameItem::Tag(_) => {}
            }
        }
    }

    /// Convert page-local coordinates to canvas coordinates (adding scratch offset + page ypos).
    fn to_canvas(&self, x: f64, y: f64, page_ypos: f64) -> (f64, f64) {
        let scratch_left = crate::SCRATCH_LEFT;
        (scratch_left + x, page_ypos + y)
    }

    /// Write common PAGEOBJECT attributes for a text frame (PTYPE=4).
    fn write_text_frame_attrs(
        &self,
        xml: &mut XmlWriter,
        page_idx: usize,
        canvas_x: f64,
        canvas_y: f64,
        w: f64,
        h: f64,
        item_id: usize,
    ) {
        xml.write_attribute("XPOS", &format!("{canvas_x:.4}"));
        xml.write_attribute("YPOS", &format!("{canvas_y:.4}"));
        xml.write_attribute("OwnPage", &page_idx.to_string());
        xml.write_attribute("ItemID", &item_id.to_string());
        xml.write_attribute("PTYPE", "4");
        xml.write_attribute("WIDTH", &format!("{w:.4}"));
        xml.write_attribute("HEIGHT", &format!("{h:.4}"));
        xml.write_attribute("FRTYPE", "0");
        xml.write_attribute("CLIPEDIT", "0");
        xml.write_attribute("PWIDTH", "1");
        xml.write_attribute("PLINEART", "1");
        xml.write_attribute("LOCALSCX", "1");
        xml.write_attribute("LOCALSCY", "1");
        xml.write_attribute("LOCALX", "0");
        xml.write_attribute("LOCALY", "0");
        xml.write_attribute("LOCALROT", "0");
        xml.write_attribute("PICART", "1");
        xml.write_attribute("SCALETYPE", "1");
        xml.write_attribute("RATIO", "1");
        xml.write_attribute("COLUMNS", "1");
        xml.write_attribute("COLGAP", "0");
        xml.write_attribute("AUTOTEXT", "0");
        xml.write_attribute("EXTRA", "0");
        xml.write_attribute("TEXTRA", "0");
        xml.write_attribute("BEXTRA", "0");
        xml.write_attribute("REXTRA", "0");
        xml.write_attribute("VAlign", "0");
        xml.write_attribute("FLOP", "1");
        xml.write_attribute("PLTSHOW", "0");
        xml.write_attribute("BASEOF", "0");
        xml.write_attribute("textPathType", "0");
        xml.write_attribute("textPathFlipped", "0");
        let path = format!("M0 0 L{w:.4} 0 L{w:.4} {h:.4} L0 {h:.4} L0 0 Z");
        xml.write_attribute("path", &path);
        xml.write_attribute("copath", &path);
        xml.write_attribute("gXpos", &format!("{canvas_x:.4}"));
        xml.write_attribute("gYpos", &format!("{canvas_y:.4}"));
        xml.write_attribute("gWidth", "0");
        xml.write_attribute("gHeight", "0");
        xml.write_attribute("LAYER", "0");
        xml.write_attribute("NEXTITEM", "-1");
        xml.write_attribute("BACKITEM", "-1");
    }

    /// Write common PAGEOBJECT attributes for a polygon/shape (PTYPE=6).
    fn write_shape_attrs(
        &self,
        xml: &mut XmlWriter,
        page_idx: usize,
        canvas_x: f64,
        canvas_y: f64,
        w: f64,
        h: f64,
        item_id: usize,
        fill_name: &str,
        fill_shade: &str,
        stroke_name: &str,
        stroke_width: f64,
    ) {
        xml.write_attribute("XPOS", &format!("{canvas_x:.4}"));
        xml.write_attribute("YPOS", &format!("{canvas_y:.4}"));
        xml.write_attribute("OwnPage", &page_idx.to_string());
        xml.write_attribute("ItemID", &item_id.to_string());
        xml.write_attribute("PTYPE", "6");
        xml.write_attribute("WIDTH", &format!("{w:.4}"));
        xml.write_attribute("HEIGHT", &format!("{h:.4}"));
        xml.write_attribute("FRTYPE", "0");
        xml.write_attribute("CLIPEDIT", "0");
        xml.write_attribute("PWIDTH", &format!("{stroke_width:.4}"));
        xml.write_attribute("PLINEART", "1");
        xml.write_attribute("LOCALSCX", "1");
        xml.write_attribute("LOCALSCY", "1");
        xml.write_attribute("LOCALX", "0");
        xml.write_attribute("LOCALY", "0");
        xml.write_attribute("LOCALROT", "0");
        xml.write_attribute("PICART", "1");
        xml.write_attribute("SCALETYPE", "1");
        xml.write_attribute("RATIO", "1");
        xml.write_attribute("PCOLOR", fill_name);
        xml.write_attribute("SHADE", fill_shade);
        xml.write_attribute("PCOLOR2", stroke_name);
        let path = format!("M0 0 L{w:.4} 0 L{w:.4} {h:.4} L0 {h:.4} L0 0 Z");
        xml.write_attribute("path", &path);
        xml.write_attribute("copath", &path);
        xml.write_attribute("gXpos", &format!("{canvas_x:.4}"));
        xml.write_attribute("gYpos", &format!("{canvas_y:.4}"));
        xml.write_attribute("gWidth", "0");
        xml.write_attribute("gHeight", "0");
        xml.write_attribute("LAYER", "0");
        xml.write_attribute("NEXTITEM", "-1");
        xml.write_attribute("BACKITEM", "-1");
    }

    /// Emit a text frame for a shaped text run.
    fn render_text(
        &mut self,
        xml: &mut XmlWriter,
        page_idx: usize,
        pos: Point,
        text: &TextItem,
        page_ypos: f64,
    ) {
        let id = self.next_id();
        let x = pos.x.to_pt();
        let y = pos.y.to_pt();
        let font_size = text.size.to_pt();
        // Scribus re-shapes text with its own engine, so glyph advances may differ
        // slightly from Typst's. Add a small buffer to prevent clipping/wrapping.
        let width = text.width().to_pt() + 2.0;
        let height = font_size * 1.4;

        let frame_y = y - font_size;

        let fill_color = SlaColor::from_paint(&text.fill);
        let color_name = self.register_color(&fill_color);

        // Scribus names fonts as "Family Subfamily" using name table IDs 1 + 2
        // (e.g. "EB Garamond Medium Italic", "Arial Regular", "Adobe Caslon Pro Regular")
        let ttf = text.font.ttf();
        let font_family = match (find_font_name(ttf, 1), find_font_name(ttf, 2)) {
            (Some(fam), Some(sub)) => format!("{fam} {sub}"),
            (Some(fam), None) => fam,
            _ => text.font.font().info().family.to_string(),
        };

        let (cx, cy) = self.to_canvas(x, frame_y, page_ypos);

        xml.start_element("PAGEOBJECT");
        self.write_text_frame_attrs(xml, page_idx, cx, cy, width, height, id);

        // StoryText wrapper (required by Scribus 1.6.4)
        xml.start_element("StoryText");

        xml.start_element("DefaultStyle");
        xml.end_element();

        let plain_text: String = text.text.to_string();
        xml.start_element("ITEXT");
        xml.write_attribute("CH", &plain_text);
        xml.write_attribute("FONT", &font_family);
        xml.write_attribute("FONTSIZE", &format!("{font_size:.2}"));
        xml.write_attribute("FCOLOR", &color_name);
        xml.end_element(); // ITEXT

        xml.start_element("trail");
        xml.end_element();

        xml.end_element(); // StoryText
        xml.end_element(); // PAGEOBJECT
    }

    fn render_shape(
        &mut self,
        xml: &mut XmlWriter,
        page_idx: usize,
        pos: Point,
        shape: &Shape,
        page_ypos: f64,
    ) {
        match &shape.geometry {
            Geometry::Rect(size) => {
                self.emit_rect(xml, page_idx, pos, *size, shape, page_ypos);
            }
            Geometry::Line(end) => {
                self.emit_line(xml, page_idx, pos, *end, shape, page_ypos);
            }
            Geometry::Curve(curve) => {
                self.emit_curve(xml, page_idx, pos, &curve.0, shape, page_ypos);
            }
        }
    }

    fn emit_rect(
        &mut self,
        xml: &mut XmlWriter,
        page_idx: usize,
        pos: Point,
        size: Size,
        shape: &Shape,
        page_ypos: f64,
    ) {
        let id = self.next_id();
        let x = pos.x.to_pt();
        let y = pos.y.to_pt();
        let w = size.x.to_pt();
        let h = size.y.to_pt();

        let (fill_name, fill_shade) = self.resolve_fill(&shape.fill);
        let (stroke_name, stroke_width) = self.resolve_stroke(shape);

        let (cx, cy) = self.to_canvas(x, y, page_ypos);

        xml.start_element("PAGEOBJECT");
        self.write_shape_attrs(xml, page_idx, cx, cy, w, h, id, &fill_name, &fill_shade, &stroke_name, stroke_width);
        xml.end_element();
    }

    fn emit_rect_fill(
        &mut self,
        xml: &mut XmlWriter,
        page_idx: usize,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        paint: &Paint,
        page_ypos: f64,
    ) {
        let id = self.next_id();
        let color = SlaColor::from_paint(paint);
        let color_name = self.register_color(&color);

        let (cx, cy) = self.to_canvas(x, y, page_ypos);

        xml.start_element("PAGEOBJECT");
        self.write_shape_attrs(xml, page_idx, cx, cy, w, h, id, &color_name, "100", "None", 0.0);
        xml.end_element();
    }

    fn emit_line(
        &mut self,
        xml: &mut XmlWriter,
        page_idx: usize,
        pos: Point,
        end: Point,
        shape: &Shape,
        page_ypos: f64,
    ) {
        let id = self.next_id();
        let x = pos.x.to_pt();
        let y = pos.y.to_pt();
        let ex = end.x.to_pt();
        let ey = end.y.to_pt();

        let width = ex.abs().max(1.0);
        let height = ey.abs().max(1.0);

        let (stroke_name, stroke_width) = self.resolve_stroke(shape);
        let (cx, cy) = self.to_canvas(x, y, page_ypos);

        xml.start_element("PAGEOBJECT");
        xml.write_attribute("XPOS", &format!("{cx:.4}"));
        xml.write_attribute("YPOS", &format!("{cy:.4}"));
        xml.write_attribute("OwnPage", &page_idx.to_string());
        xml.write_attribute("ItemID", &id.to_string());
        xml.write_attribute("PTYPE", "5");
        xml.write_attribute("WIDTH", &format!("{width:.4}"));
        xml.write_attribute("HEIGHT", &format!("{height:.4}"));
        xml.write_attribute("FRTYPE", "0");
        xml.write_attribute("CLIPEDIT", "0");
        xml.write_attribute("PWIDTH", &format!("{stroke_width:.4}"));
        xml.write_attribute("PLINEART", "1");
        xml.write_attribute("LOCALSCX", "1");
        xml.write_attribute("LOCALSCY", "1");
        xml.write_attribute("LOCALX", "0");
        xml.write_attribute("LOCALY", "0");
        xml.write_attribute("LOCALROT", "0");
        xml.write_attribute("PICART", "1");
        xml.write_attribute("SCALETYPE", "1");
        xml.write_attribute("RATIO", "1");
        xml.write_attribute("PCOLOR", "None");
        xml.write_attribute("PCOLOR2", &stroke_name);
        let path = format!("M0 0 L{width:.4} {height:.4}");
        xml.write_attribute("path", &path);
        xml.write_attribute("copath", &path);
        xml.write_attribute("gXpos", &format!("{cx:.4}"));
        xml.write_attribute("gYpos", &format!("{cy:.4}"));
        xml.write_attribute("gWidth", "0");
        xml.write_attribute("gHeight", "0");
        xml.write_attribute("LAYER", "0");
        xml.write_attribute("NEXTITEM", "-1");
        xml.write_attribute("BACKITEM", "-1");
        xml.end_element();
    }

    fn emit_curve(
        &mut self,
        xml: &mut XmlWriter,
        page_idx: usize,
        pos: Point,
        items: &[CurveItem],
        shape: &Shape,
        page_ypos: f64,
    ) {
        if items.is_empty() {
            return;
        }

        let id = self.next_id();
        let x = pos.x.to_pt();
        let y = pos.y.to_pt();

        let mut coords: Vec<f64> = Vec::new();
        let mut _cursor = (0.0_f64, 0.0_f64);
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;

        for item in items {
            match item {
                CurveItem::Move(p) => {
                    let px = p.x.to_pt();
                    let py = p.y.to_pt();
                    _cursor = (px, py);
                    update_bounds(&mut min_x, &mut min_y, &mut max_x, &mut max_y, px, py);
                    coords.extend_from_slice(&[px, py, px, py]);
                }
                CurveItem::Line(p) => {
                    let px = p.x.to_pt();
                    let py = p.y.to_pt();
                    update_bounds(&mut min_x, &mut min_y, &mut max_x, &mut max_y, px, py);
                    coords.extend_from_slice(&[px, py, px, py]);
                    _cursor = (px, py);
                }
                CurveItem::Cubic(_c1, c2, end) => {
                    let c2x = c2.x.to_pt();
                    let c2y = c2.y.to_pt();
                    let ex = end.x.to_pt();
                    let ey = end.y.to_pt();
                    update_bounds(&mut min_x, &mut min_y, &mut max_x, &mut max_y, ex, ey);
                    coords.extend_from_slice(&[ex, ey, c2x, c2y]);
                    _cursor = (ex, ey);
                }
                CurveItem::Close => {}
            }
        }

        if coords.is_empty() {
            return;
        }

        if min_x.is_finite() && min_y.is_finite() {
            let ox = min_x;
            let oy = min_y;
            for i in (0..coords.len()).step_by(2) {
                coords[i] -= ox;
                coords[i + 1] -= oy;
            }
        }

        let w = if max_x > min_x { max_x - min_x } else { 1.0 };
        let h = if max_y > min_y { max_y - min_y } else { 1.0 };

        let (fill_name, fill_shade) = self.resolve_fill(&shape.fill);
        let (stroke_name, stroke_width) = self.resolve_stroke(shape);

        let path_str: String = coords
            .iter()
            .map(|v| format!("{v:.4}"))
            .collect::<Vec<_>>()
            .join(" ");

        let (cx, cy) = self.to_canvas(x + min_x.min(0.0), y + min_y.min(0.0), page_ypos);

        xml.start_element("PAGEOBJECT");
        xml.write_attribute("XPOS", &format!("{cx:.4}"));
        xml.write_attribute("YPOS", &format!("{cy:.4}"));
        xml.write_attribute("OwnPage", &page_idx.to_string());
        xml.write_attribute("ItemID", &id.to_string());
        xml.write_attribute("PTYPE", "6");
        xml.write_attribute("WIDTH", &format!("{w:.4}"));
        xml.write_attribute("HEIGHT", &format!("{h:.4}"));
        xml.write_attribute("FRTYPE", "0");
        xml.write_attribute("CLIPEDIT", "0");
        xml.write_attribute("PWIDTH", &format!("{stroke_width:.4}"));
        xml.write_attribute("PLINEART", "1");
        xml.write_attribute("LOCALSCX", "1");
        xml.write_attribute("LOCALSCY", "1");
        xml.write_attribute("LOCALX", "0");
        xml.write_attribute("LOCALY", "0");
        xml.write_attribute("LOCALROT", "0");
        xml.write_attribute("PICART", "1");
        xml.write_attribute("SCALETYPE", "1");
        xml.write_attribute("RATIO", "1");
        xml.write_attribute("PCOLOR", &fill_name);
        xml.write_attribute("SHADE", &fill_shade);
        xml.write_attribute("PCOLOR2", &stroke_name);
        xml.write_attribute("NUMPO", &coords.len().to_string());
        xml.write_attribute("POCOOR", &path_str);
        let svg_path = format!("M0 0 L{w:.4} 0 L{w:.4} {h:.4} L0 {h:.4} L0 0 Z");
        xml.write_attribute("path", &svg_path);
        xml.write_attribute("copath", &svg_path);
        xml.write_attribute("gXpos", &format!("{cx:.4}"));
        xml.write_attribute("gYpos", &format!("{cy:.4}"));
        xml.write_attribute("gWidth", "0");
        xml.write_attribute("gHeight", "0");
        xml.write_attribute("LAYER", "0");
        xml.write_attribute("NEXTITEM", "-1");
        xml.write_attribute("BACKITEM", "-1");
        xml.end_element();
    }

    fn render_image(
        &mut self,
        xml: &mut XmlWriter,
        page_idx: usize,
        pos: Point,
        image: &Image,
        size: Size,
        page_ypos: f64,
    ) {
        let id = self.next_id();
        let x = pos.x.to_pt();
        let y = pos.y.to_pt();
        let w = size.x.to_pt();
        let h = size.y.to_pt();

        let _image_info = crate::image::image_data_base64(image);

        let scale_x = if image.width() > 0.0 {
            w / (image.width() * 72.0 / image.dpi().unwrap_or(72.0))
        } else {
            1.0
        };
        let scale_y = if image.height() > 0.0 {
            h / (image.height() * 72.0 / image.dpi().unwrap_or(72.0))
        } else {
            1.0
        };

        let (cx, cy) = self.to_canvas(x, y, page_ypos);

        xml.start_element("PAGEOBJECT");
        xml.write_attribute("XPOS", &format!("{cx:.4}"));
        xml.write_attribute("YPOS", &format!("{cy:.4}"));
        xml.write_attribute("OwnPage", &page_idx.to_string());
        xml.write_attribute("ItemID", &id.to_string());
        xml.write_attribute("PTYPE", "2");
        xml.write_attribute("WIDTH", &format!("{w:.4}"));
        xml.write_attribute("HEIGHT", &format!("{h:.4}"));
        xml.write_attribute("FRTYPE", "0");
        xml.write_attribute("CLIPEDIT", "0");
        xml.write_attribute("PWIDTH", "1");
        xml.write_attribute("PLINEART", "1");
        xml.write_attribute("LOCALSCX", &format!("{scale_x:.6}"));
        xml.write_attribute("LOCALSCY", &format!("{scale_y:.6}"));
        xml.write_attribute("LOCALX", "0");
        xml.write_attribute("LOCALY", "0");
        xml.write_attribute("LOCALROT", "0");
        xml.write_attribute("PICART", "1");
        xml.write_attribute("SCALETYPE", "0");
        xml.write_attribute("RATIO", "1");
        xml.write_attribute("Pagenumber", "0");
        xml.write_attribute("PFILE", &format!("image_{id}.png"));
        let path = format!("M0 0 L{w:.4} 0 L{w:.4} {h:.4} L0 {h:.4} L0 0 Z");
        xml.write_attribute("path", &path);
        xml.write_attribute("copath", &path);
        xml.write_attribute("gXpos", &format!("{cx:.4}"));
        xml.write_attribute("gYpos", &format!("{cy:.4}"));
        xml.write_attribute("gWidth", "0");
        xml.write_attribute("gHeight", "0");
        xml.write_attribute("LAYER", "0");
        xml.write_attribute("NEXTITEM", "-1");
        xml.write_attribute("BACKITEM", "-1");
        xml.end_element();
    }

    fn resolve_fill(&mut self, fill: &Option<Paint>) -> (String, String) {
        match fill {
            Some(paint) => {
                let color = SlaColor::from_paint(paint);
                let name = self.register_color(&color);
                (name, String::from("100"))
            }
            None => (String::from("None"), String::from("100")),
        }
    }

    fn resolve_stroke(&mut self, shape: &Shape) -> (String, f64) {
        match &shape.stroke {
            Some(stroke) => {
                let color = SlaColor::from_paint(&stroke.paint);
                let name = self.register_color(&color);
                (name, stroke.thickness.to_pt())
            }
            None => (String::from("None"), 0.0),
        }
    }
}

fn apply_transform(transform: &Transform, point: &Point) -> Point {
    if transform.is_identity() {
        return *point;
    }
    let x = point.x.to_pt();
    let y = point.y.to_pt();
    let sx = transform.sx.get();
    let ky = transform.ky.get();
    let kx = transform.kx.get();
    let sy = transform.sy.get();
    let tx = transform.tx.to_pt();
    let ty = transform.ty.to_pt();
    let new_x = sx * x + kx * y + tx;
    let new_y = ky * x + sy * y + ty;
    Point::new(Abs::pt(new_x), Abs::pt(new_y))
}

fn combine_transforms(parent: &Transform, child: &Transform) -> Transform {
    if parent.is_identity() {
        return *child;
    }
    if child.is_identity() {
        return *parent;
    }
    child.pre_concat(*parent)
}

fn update_bounds(
    min_x: &mut f64,
    min_y: &mut f64,
    max_x: &mut f64,
    max_y: &mut f64,
    x: f64,
    y: f64,
) {
    if x < *min_x { *min_x = x; }
    if y < *min_y { *min_y = y; }
    if x > *max_x { *max_x = x; }
    if y > *max_y { *max_y = y; }
}
