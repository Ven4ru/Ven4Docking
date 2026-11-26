//! layouts/<phone>.json: schema and validation (bounds, overlap, screen count).

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

pub const BLOCK_TYPES: &[&str] = &[
    "clock", "weather", "cpu", "gpu", "ram", "disks", "net", "ai", "apps", "media", "servers", "phone", "power", "keys", "mixer", "obs",
    "notify",
];

/// Buttons of the `power` block when its opts do not list them.
pub const POWER_BUTTONS: &[&str] = &["lock", "sleep", "restart", "shutdown"];

/// At most this many program screens (`forApp`), within the overall limit of 10 screens.
pub const MAX_APP_SCREENS: usize = 5;

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
    /// Program screen: exe name without a path ("obs64.exe", any case). Opened by the bar while the program
    /// runs, not part of the usual paging.
    #[serde(rename = "forApp", default, skip_serializing_if = "Option::is_none")]
    pub for_app: Option<String>,
    #[serde(default)]
    pub blocks: Vec<Block>,
}

/// Exe name without a path.
pub fn valid_exe_name(n: &str) -> bool {
    let n = n.trim();
    !n.is_empty() && n.len() <= 260 && !n.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|'])
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
    /// more than 5 program screens, or no usual screen left
    AppScreens(usize),
    /// `forApp` is not an exe name
    ForApp(String),
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
            LayoutError::AppScreens(n) => json!({"error": "appScreens", "count": n}),
            LayoutError::ForApp(s) => json!({"error": "forApp", "screens": [s]}),
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
        let apps = self.screens.iter().filter(|s| s.for_app.is_some()).count();
        if apps > MAX_APP_SCREENS || apps == self.screens.len() {
            return Err(LayoutError::AppScreens(apps));
        }
        if let Some(s) = self.screens.iter().find(|s| s.for_app.as_deref().is_some_and(|n| !valid_exe_name(n))) {
            return Err(LayoutError::ForApp(s.id.clone()));
        }
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

    pub fn blocks_of<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a Block> + 'a {
        self.screens.iter().flat_map(|s| s.blocks.iter()).filter(move |b| b.kind == kind)
    }

    pub fn has_block(&self, kind: &str) -> bool {
        self.blocks_of(kind).next().is_some()
    }

    /// A power button is allowed when some `power` block shows it (`opts.buttons`, all four by default);
    /// `cancel` — when restart or shutdown is allowed.
    pub fn power_allowed(&self, action: &str) -> bool {
        if action == "cancel" {
            return self.power_allowed("restart") || self.power_allowed("shutdown");
        }
        self.blocks_of("power").any(|b| match b.opts.get("buttons").and_then(Value::as_array) {
            Some(list) => list.iter().any(|x| x.as_str() == Some(action)),
            None => POWER_BUTTONS.contains(&action),
        })
    }

    /// An action is allowed when some `keys` block lists its id in `opts.actions`.
    pub fn action_allowed(&self, id: &str) -> bool {
        self.blocks_of("keys").any(|b| b.opts.get("actions").and_then(Value::as_array).is_some_and(|l| l.iter().any(|x| x.as_str() == Some(id))))
    }

    /// `forApp` of the program screens, as written in the layout.
    pub fn app_screens(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for n in self.screens.iter().filter_map(|s| s.for_app.as_deref()).map(str::trim) {
            if !v.iter().any(|x| x.eq_ignore_ascii_case(n)) {
                v.push(n.to_string());
            }
        }
        v
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
