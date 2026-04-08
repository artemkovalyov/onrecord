use cairo::Context;
use crate::state::{Color, PathTool, Stroke};

fn set_source(cr: &Context, color: &Color) {
    cr.set_source_rgba(color.r, color.g, color.b, color.a);
}

pub fn render_stroke(cr: &Context, stroke: &Stroke) {
    match stroke {
        Stroke::Path { points, color, width, tool } => {
            if points.len() < 2 {
                return;
            }
            let alpha = if *tool == PathTool::Highlighter { 0.4 } else { color.a };
            cr.set_source_rgba(color.r, color.g, color.b, alpha);
            cr.set_line_width(*width);
            cr.set_line_cap(cairo::LineCap::Round);
            cr.set_line_join(cairo::LineJoin::Round);
            cr.move_to(points[0].0, points[0].1);
            for pt in &points[1..] {
                cr.line_to(pt.0, pt.1);
            }
            let _ = cr.stroke();
        }
        Stroke::Line { start, end, color, width } => {
            set_source(cr, color);
            cr.set_line_width(*width);
            cr.set_line_cap(cairo::LineCap::Round);
            cr.move_to(start.0, start.1);
            cr.line_to(end.0, end.1);
            let _ = cr.stroke();
        }
        Stroke::Rect { origin, size, color, width } => {
            set_source(cr, color);
            cr.set_line_width(*width);
            cr.rectangle(origin.0, origin.1, size.0, size.1);
            let _ = cr.stroke();
        }
        Stroke::Ellipse { center, radii, color, width } => {
            if radii.0 < f64::EPSILON || radii.1 < f64::EPSILON {
                return;
            }
            set_source(cr, color);
            cr.save().unwrap();
            cr.translate(center.0, center.1);
            cr.scale(radii.0, radii.1);
            // Normalize line width against geometric mean of radii so it appears uniform
            let scale_mean = (radii.0 * radii.1).sqrt().max(f64::EPSILON);
            cr.set_line_width(*width / scale_mean);
            cr.arc(0.0, 0.0, 1.0, 0.0, std::f64::consts::TAU);
            let _ = cr.stroke(); // stroke inside save/restore so CTM is still active
            cr.restore().unwrap();
        }
        Stroke::Text { position, content, color, size } => {
            set_source(cr, color);
            cr.set_font_size(*size);
            cr.move_to(position.0, position.1);
            let _ = cr.show_text(content);
        }
    }
}

pub fn render_laser(cr: &Context, points: &[(f64, f64)], alpha: f64) {
    if points.len() < 2 {
        return;
    }
    cr.set_source_rgba(1.0, 0.1, 0.1, alpha);
    cr.set_line_width(4.0);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.set_line_join(cairo::LineJoin::Round);
    cr.move_to(points[0].0, points[0].1);
    for pt in &points[1..] {
        cr.line_to(pt.0, pt.1);
    }
    let _ = cr.stroke();
}
