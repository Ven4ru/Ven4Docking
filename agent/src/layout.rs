//! layouts/<phone>.json: schema and validation (bounds, overlap, screen count).

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

pub const BLOCK_TYPES: &[&str] =
    &["clock", "weather", "cpu", "gpu", "ram", "disks", "net", "ai", "apps", "media", "servers", "phone"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Grid {
    pub cols: u32,
    pub rows: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Block {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
    #[serde(default = "empty_obj")]
    pub opts: Value,
}

fn empty_obj() -> Value {
    Value::Object(Map::new())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Screen {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Layout {
    #[serde(default)]
    pub version: u64,
    #[serde(default = "landscape")]
    pub orientation: String,
    pub grid: Grid,
    pub screens: Vec<Screen>,
}

fn landscape() -> String {
    "landscape".into()
}

/// Validation failure; `payload()` is the 400 body sent to the editor.
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutError {
    Overlap(String, String),
    Bounds(String),
    BadType(String),
    DuplicateId(String),
    Screens(usize),
    Grid,
    Orientation,
}

impl LayoutError {
    pub fn payload(&self) -> Value {
        match self {
            LayoutError::Overlap(a, b) => json!({"error": "overlap", "blocks": [a, b]}),
            LayoutError::Bounds(a) => json!({"error": "bounds", "blocks": [a]}),
            LayoutError::BadType(a) => json!({"error": "badType", "blocks": [a]}),
            LayoutError::DuplicateId(a) => json!({"error": "duplicateId", "blocks": [a]}),
            LayoutError::Screens(n) => json!({"error": "screens", "count": n}),
            LayoutError::Grid => json!({"error": "grid"}),
            LayoutError::Orientation => json!({"error": "orientation"}),
        }
    }
}

fn intersects(a: &Block, b: &Block) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

impl Layout {
    pub fn validate(&self) -> Result<(), LayoutError> {
        if !(1..=10).contains(&self.screens.len()) {
            return Err(LayoutError::Screens(self.screens.len()));
        }
        if !(1..=24).contains(&self.grid.cols) || !(1..=24).contains(&self.grid.rows) {
            return Err(LayoutError::Grid);
        }
        // orientation: 1.0 is landscape only; the value is stored as is and not checked
        let (cols, rows) = (self.grid.cols as i64, self.grid.rows as i64);
        let mut ids = std::collections::HashSet::new();
        for s in &self.screens {
            for (i, b) in s.blocks.iter().enumerate() {
                if !ids.insert(b.id.as_str()) || b.id.is_empty() {
                    return Err(LayoutError::DuplicateId(b.id.clone()));
                }
                if !BLOCK_TYPES.contains(&b.kind.as_str()) {
                    return Err(LayoutError::BadType(b.id.clone()));
                }
                if b.x < 0 || b.y < 0 || b.w < 1 || b.h < 1 || b.x + b.w > cols || b.y + b.h > rows {
                    return Err(LayoutError::Bounds(b.id.clone()));
                }
                for o in &s.blocks[..i] {
                    if intersects(o, b) {
                        return Err(LayoutError::Overlap(o.id.clone(), b.id.clone()));
                    }
                }
            }
        }
        Ok(())
    }

    /// Block types used on any screen.
    pub fn block_types(&self) -> impl Iterator<Item = &str> {
        self.screens.iter().flat_map(|s| s.blocks.iter().map(|b| b.kind.as_str()))
    }

    /// First-run template «Классика» (like the prototype panel).
    pub fn classic() -> Layout {
        serde_json::from_value(json!({
            "version": 1, "orientation": "landscape", "grid": {"cols": 8, "rows": 4},
            "screens": [
                {"id": "s1", "name": "Главный", "blocks": [
                    {"id": "b1", "type": "clock", "x": 0, "y": 0, "w": 3, "h": 2, "opts": {"weather": true}},
                    {"id": "b2", "type": "cpu", "x": 3, "y": 0, "w": 2, "h": 2, "opts": {}},
                    {"id": "b3", "type": "gpu", "x": 5, "y": 0, "w": 3, "h": 2, "opts": {}},
                    {"id": "b4", "type": "ram", "x": 0, "y": 2, "w": 2, "h": 1, "opts": {}},
                    {"id": "b5", "type": "net", "x": 2, "y": 2, "w": 3, "h": 1, "opts": {}},
                    {"id": "b6", "type": "media", "x": 5, "y": 2, "w": 3, "h": 1, "opts": {}},
                    {"id": "b7", "type": "apps", "x": 0, "y": 3, "w": 8, "h": 1, "opts": {}}
                ]},
                {"id": "s2", "name": "Серверы и нейросети", "blocks": [
                    {"id": "b8", "type": "servers", "x": 0, "y": 0, "w": 5, "h": 4, "opts": {}},
                    {"id": "b9", "type": "ai", "x": 5, "y": 0, "w": 3, "h": 3, "opts": {}},
                    {"id": "b10", "type": "disks", "x": 5, "y": 3, "w": 3, "h": 1, "opts": {}}
                ]}
            ]
        }))
        .expect("classic layout")
    }
}
