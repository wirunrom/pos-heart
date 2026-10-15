//! Line icons on a 24-unit grid, stroke 1.8 (design/docs/components/Icon.md).
//! Shapes follow Lucide's vocabulary but are drawn in code: no icon font or network asset.
use eframe::egui::{self, Color32, Pos2, Rect, Shape, Stroke, vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Home,
    Sell,
    Products,
    History,
    Shift,
    Reports,
    Settings,
    ChevronDown,
    ChevronUp,
    ChevronLeft,
    ChevronRight,
    Search,
    Close,
    Plus,
    Minus,
    Trash,
    Note,
    Pause,
    More,
    Check,
    Download,
    Upload,
    Tag,
    Lock,
    Unlock,
    Sliders,
    Banknote,
    Calendar,
    Alert,
    Info,
    CheckCircle,
    Undo,
    Options,
    Card,
    Transfer,
    Database,
    Pencil,
    Printer,
}

struct Pen<'a> {
    painter: &'a egui::Painter,
    origin: Pos2,
    scale: f32,
    stroke: Stroke,
}
impl Pen<'_> {
    fn p(&self, x: f32, y: f32) -> Pos2 {
        self.origin + vec2(x, y) * self.scale
    }
    fn line(&self, points: &[(f32, f32)]) {
        let points: Vec<Pos2> = points.iter().map(|&(x, y)| self.p(x, y)).collect();
        for &pt in [points.first(), points.last()].into_iter().flatten() {
            // Round caps keep short strokes from looking chopped.
            self.painter
                .circle_filled(pt, self.stroke.width / 2.0, self.stroke.color);
        }
        self.painter.add(Shape::line(points, self.stroke));
    }
    fn closed(&self, points: &[(f32, f32)]) {
        let points: Vec<Pos2> = points.iter().map(|&(x, y)| self.p(x, y)).collect();
        self.painter.add(Shape::closed_line(points, self.stroke));
    }
    fn rect(&self, x: f32, y: f32, w: f32, h: f32, r: f32) {
        self.painter.rect_stroke(
            Rect::from_min_size(self.p(x, y), vec2(w, h) * self.scale),
            r * self.scale,
            self.stroke,
            egui::StrokeKind::Middle,
        );
    }
    fn circle(&self, x: f32, y: f32, r: f32) {
        self.painter
            .circle_stroke(self.p(x, y), r * self.scale, self.stroke);
    }
    fn dot(&self, x: f32, y: f32, r: f32) {
        self.painter
            .circle_filled(self.p(x, y), r * self.scale, self.stroke.color);
    }
    /// Arc around (cx, cy) from `a0` to `a1` radians (0 = east, clockwise on screen).
    fn arc(&self, cx: f32, cy: f32, r: f32, a0: f32, a1: f32) {
        let steps = 18;
        let pts: Vec<(f32, f32)> = (0..=steps)
            .map(|i| {
                let a = a0 + (a1 - a0) * i as f32 / steps as f32;
                (cx + r * a.cos(), cy + r * a.sin())
            })
            .collect();
        self.line(&pts);
    }
}

pub fn paint(painter: &egui::Painter, icon: Icon, rect: Rect, color: Color32) {
    let size = rect.width().min(rect.height());
    let scale = size / 24.0;
    let pen = Pen {
        painter,
        origin: rect.center() - vec2(12.0, 12.0) * scale,
        scale,
        stroke: Stroke::new((1.8 * scale).max(1.3), color),
    };
    use std::f32::consts::PI;
    match icon {
        Icon::Home => {
            pen.line(&[(3., 10.), (12., 3.), (21., 10.)]);
            pen.line(&[(5., 9.), (5., 20.), (19., 20.), (19., 9.)]);
            pen.line(&[(10., 20.), (10., 14.), (14., 14.), (14., 20.)]);
        }
        Icon::Sell => {
            pen.rect(3., 4., 18., 16., 2.);
            pen.line(&[(3., 9.), (21., 9.)]);
            pen.line(&[(7., 13.), (11., 13.)]);
            pen.line(&[(7., 16.), (14., 16.)]);
        }
        Icon::Products => {
            pen.closed(&[
                (12., 2.5),
                (21., 7.),
                (21., 17.),
                (12., 21.5),
                (3., 17.),
                (3., 7.),
            ]);
            pen.line(&[(3., 7.), (12., 11.5), (21., 7.)]);
            pen.line(&[(12., 11.5), (12., 21.5)]);
        }
        Icon::History => {
            pen.line(&[
                (5., 3.),
                (5., 21.),
                (8., 19.),
                (11., 21.),
                (14., 19.),
                (17., 21.),
            ]);
            pen.line(&[(17., 21.), (19., 19.), (19., 3.), (5., 3.)]);
            pen.line(&[(9., 8.), (15., 8.)]);
            pen.line(&[(9., 12.), (15., 12.)]);
        }
        Icon::Shift => {
            pen.rect(2.5, 6., 19., 13., 2.);
            pen.line(&[(5., 6.), (16., 3.), (17., 6.)]);
            pen.rect(15., 11., 6.5, 4., 1.);
        }
        Icon::Reports => {
            pen.line(&[(3., 3.), (3., 21.), (21., 21.)]);
            pen.line(&[(8., 17.), (8., 12.)]);
            pen.line(&[(13., 17.), (13., 7.)]);
            pen.line(&[(18., 17.), (18., 10.)]);
        }
        Icon::Settings => {
            pen.circle(12., 12., 3.);
            let teeth: Vec<(f32, f32)> = (0..48)
                .map(|i| {
                    let a = i as f32 / 48.0 * 2.0 * PI;
                    let r = if (i / 3) % 2 == 0 { 9.0 } else { 7.2 };
                    (12. + r * a.cos(), 12. + r * a.sin())
                })
                .collect();
            pen.closed(&teeth);
        }
        Icon::ChevronDown => pen.line(&[(6., 9.), (12., 15.), (18., 9.)]),
        Icon::ChevronUp => pen.line(&[(6., 15.), (12., 9.), (18., 15.)]),
        Icon::ChevronLeft => pen.line(&[(15., 6.), (9., 12.), (15., 18.)]),
        Icon::ChevronRight => pen.line(&[(9., 6.), (15., 12.), (9., 18.)]),
        Icon::Search => {
            pen.circle(11., 11., 7.);
            pen.line(&[(16.2, 16.2), (21., 21.)]);
        }
        Icon::Close => {
            pen.line(&[(6., 6.), (18., 18.)]);
            pen.line(&[(18., 6.), (6., 18.)]);
        }
        Icon::Plus => {
            pen.line(&[(12., 5.), (12., 19.)]);
            pen.line(&[(5., 12.), (19., 12.)]);
        }
        Icon::Minus => pen.line(&[(5., 12.), (19., 12.)]),
        Icon::Trash => {
            pen.line(&[(3., 6.), (21., 6.)]);
            pen.line(&[(8., 6.), (8., 4.), (16., 4.), (16., 6.)]);
            pen.line(&[(5., 6.), (6., 20.), (18., 20.), (19., 6.)]);
            pen.line(&[(10., 10.), (10., 16.)]);
            pen.line(&[(14., 10.), (14., 16.)]);
        }
        Icon::Note => {
            pen.closed(&[
                (4., 4.),
                (20., 4.),
                (20., 16.),
                (9., 16.),
                (5., 20.),
                (5., 16.),
                (4., 16.),
            ]);
        }
        Icon::Pause => {
            pen.line(&[(9., 5.), (9., 19.)]);
            pen.line(&[(15., 5.), (15., 19.)]);
        }
        Icon::More => {
            for x in [5., 12., 19.] {
                pen.dot(x, 12., 1.6);
            }
        }
        Icon::Check => pen.line(&[(5., 12.5), (10., 17.5), (19., 7.)]),
        Icon::Printer => {
            // Lucide "printer": paper in, body, paper out.
            pen.line(&[(6., 9.), (6., 2.), (18., 2.), (18., 9.)]);
            pen.rect(2., 9., 20., 9., 2.);
            pen.rect(6., 14., 12., 8., 0.);
        }
        Icon::Download => {
            pen.line(&[(12., 3.), (12., 15.)]);
            pen.line(&[(7., 10.), (12., 15.), (17., 10.)]);
            pen.line(&[(4., 17.), (4., 20.), (20., 20.), (20., 17.)]);
        }
        Icon::Upload => {
            pen.line(&[(12., 15.), (12., 3.)]);
            pen.line(&[(7., 8.), (12., 3.), (17., 8.)]);
            pen.line(&[(4., 17.), (4., 20.), (20., 20.), (20., 17.)]);
        }
        Icon::Tag => {
            pen.closed(&[(3., 3.), (11., 3.), (21., 13.), (13., 21.), (3., 11.)]);
            pen.dot(7.5, 7.5, 1.4);
        }
        Icon::Lock | Icon::Unlock => {
            pen.rect(5., 11., 14., 10., 2.);
            if icon == Icon::Lock {
                pen.arc(12., 8., 4., PI, 2.0 * PI);
                pen.line(&[(8., 8.), (8., 11.)]);
                pen.line(&[(16., 8.), (16., 11.)]);
            } else {
                pen.arc(12., 8., 4., PI, 1.85 * PI);
                pen.line(&[(8., 8.), (8., 11.)]);
            }
        }
        Icon::Sliders => {
            for (y, x) in [(6., 15.), (12., 8.), (18., 13.)] {
                pen.line(&[(4., y), (20., y)]);
                pen.painter
                    .circle_filled(pen.p(x, y), 2.4 * pen.scale, pen.stroke.color);
            }
        }
        Icon::Banknote => {
            pen.rect(2.5, 6., 19., 12., 2.);
            pen.circle(12., 12., 2.5);
            pen.dot(6., 12., 0.9);
            pen.dot(18., 12., 0.9);
        }
        Icon::Calendar => {
            pen.rect(3., 5., 18., 16., 2.);
            pen.line(&[(3., 10.), (21., 10.)]);
            pen.line(&[(8., 3.), (8., 7.)]);
            pen.line(&[(16., 3.), (16., 7.)]);
        }
        Icon::Alert => {
            pen.closed(&[(12., 3.), (22., 20.), (2., 20.)]);
            pen.line(&[(12., 9.), (12., 14.)]);
            pen.dot(12., 17., 1.1);
        }
        Icon::Info => {
            pen.circle(12., 12., 9.);
            pen.line(&[(12., 11.), (12., 16.5)]);
            pen.dot(12., 7.8, 1.1);
        }
        Icon::CheckCircle => {
            pen.circle(12., 12., 9.);
            pen.line(&[(8., 12.5), (11., 15.5), (16., 9.5)]);
        }
        Icon::Undo => {
            pen.line(&[(9., 5.), (4., 10.), (9., 15.)]);
            pen.line(&[(4., 10.), (15., 10.)]);
            pen.arc(15., 14.5, 4.5, -0.5 * PI, 0.5 * PI);
            pen.line(&[(15., 19.), (8., 19.)]);
        }
        Icon::Options => {
            for y in [7., 12., 17.] {
                pen.line(&[(5., y), (19., y)]);
            }
        }
        Icon::Card => {
            pen.rect(2.5, 5., 19., 14., 2.);
            pen.line(&[(2.5, 10.), (21.5, 10.)]);
            pen.line(&[(6., 15.), (10., 15.)]);
        }
        Icon::Transfer => {
            pen.line(&[(4., 8.), (20., 8.)]);
            pen.line(&[(16., 4.), (20., 8.), (16., 12.)]);
            pen.line(&[(20., 16.), (4., 16.)]);
            pen.line(&[(8., 12.), (4., 16.), (8., 20.)]);
        }
        Icon::Database => {
            pen.painter.add(Shape::ellipse_stroke(
                pen.p(12., 6.),
                vec2(8., 3.) * pen.scale,
                pen.stroke,
            ));
            pen.line(&[(4., 6.), (4., 18.)]);
            pen.line(&[(20., 6.), (20., 18.)]);
            pen.arc(12., 12., 8., 0.0, PI);
            pen.arc(12., 15., 8., 0.0, PI);
        }
        Icon::Pencil => {
            pen.closed(&[(15., 4.), (20., 9.), (9., 20.), (4., 20.), (4., 15.)]);
            pen.line(&[(13., 6.), (18., 11.)]);
        }
    }
}

/// Paints an icon centred in `rect` at `size` points.
pub fn paint_sized(painter: &egui::Painter, icon: Icon, center: Pos2, size: f32, color: Color32) {
    paint(
        painter,
        icon,
        Rect::from_center_size(center, vec2(size, size)),
        color,
    );
}
/// Allocates space and draws an inline icon (for labels inside layouts).
pub fn show(ui: &mut egui::Ui, icon: Icon, size: f32, color: Color32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), egui::Sense::hover());
    paint(ui.painter(), icon, rect, color);
    response
}
