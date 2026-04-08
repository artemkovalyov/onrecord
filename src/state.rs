use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Color {
    pub fn new(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self { r, g, b, a }
    }
    pub fn red() -> Self     { Self::new(0.9, 0.1, 0.1, 1.0) }
    pub fn green() -> Self   { Self::new(0.1, 0.8, 0.1, 1.0) }
    pub fn blue() -> Self    { Self::new(0.1, 0.4, 0.9, 1.0) }
    pub fn yellow() -> Self  { Self::new(1.0, 0.9, 0.0, 1.0) }
    pub fn white() -> Self   { Self::new(1.0, 1.0, 1.0, 1.0) }
    pub fn black() -> Self   { Self::new(0.0, 0.0, 0.0, 1.0) }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathTool {
    Pen,
    Highlighter,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tool {
    Pen,
    Highlighter,
    Line,
    Rectangle,
    Ellipse,
    Text,
    Laser,
    Eraser,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StrokeWidth {
    Thin,
    Medium,
    Thick,
}

impl StrokeWidth {
    pub fn pixels(&self) -> f64 {
        match self {
            StrokeWidth::Thin   => 2.0,
            StrokeWidth::Medium => 5.0,
            StrokeWidth::Thick  => 12.0,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Stroke {
    Path {
        points: Vec<(f64, f64)>,
        color: Color,
        width: f64,
        tool: PathTool,
    },
    Line {
        start: (f64, f64),
        end: (f64, f64),
        color: Color,
        width: f64,
    },
    Rect {
        origin: (f64, f64),
        size: (f64, f64),
        color: Color,
        width: f64,
    },
    Ellipse {
        center: (f64, f64),
        radii: (f64, f64),
        color: Color,
        width: f64,
    },
    Text {
        position: (f64, f64),
        content: String,
        color: Color,
        size: f64,
    },
}

pub struct AppState {
    pub strokes: Vec<Stroke>,
    pub active_tool: Tool,
    pub active_color: Color,
    pub stroke_width: StrokeWidth,
    pub draw_mode: bool,
    pub toolbar_visible: bool,
    // In-progress stroke during mouse drag
    pub current_stroke: Option<Stroke>,
    // Laser pointer points (ephemeral, not stored in strokes)
    pub laser_points: Vec<(f64, f64)>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            strokes: Vec::new(),
            active_tool: Tool::Pen,
            active_color: Color::red(),
            stroke_width: StrokeWidth::Medium,
            draw_mode: false,
            toolbar_visible: true,
            current_stroke: None,
            laser_points: Vec::new(),
        }
    }

    pub fn undo(&mut self) {
        self.strokes.pop();
    }

    /// Clears all drawn content (strokes, in-progress stroke, laser trail).
    /// Tool, color, width, and mode settings are preserved.
    pub fn clear(&mut self) {
        self.strokes.clear();
        self.current_stroke = None;
        self.laser_points.clear();
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

pub type SharedState = Arc<Mutex<AppState>>;

pub fn new_shared_state() -> SharedState {
    Arc::new(Mutex::new(AppState::new()))
}
