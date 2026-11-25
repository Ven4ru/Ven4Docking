use serde_json::json;
use ven4docking_agent::layout::{Layout, LayoutError};

fn layout(blocks: serde_json::Value) -> Layout {
    serde_json::from_value(json!({
        "version": 3, "orientation": "landscape", "grid": {"cols": 8, "rows": 4},
        "screens": [{"id": "s1", "name": "Главный", "blocks": blocks}]
    }))
    .unwrap()
}

#[test]
fn valid_layout_passes() {
    let l = layout(json!([
        {"id": "b1", "type": "cpu", "x": 0, "y": 0, "w": 2, "h": 2},
        {"id": "b2", "type": "gpu", "x": 2, "y": 0, "w": 6, "h": 4},
        {"id": "b3", "type": "apps", "x": 0, "y": 2, "w": 2, "h": 2, "opts": {"apps": ["telegram"]}}
    ]));
    assert_eq!(l.validate(), Ok(()));
    assert!(Layout::classic().validate().is_ok());
}

#[test]
fn overlap_is_rejected_with_both_ids() {
    let l = layout(json!([
        {"id": "b1", "type": "cpu", "x": 0, "y": 0, "w": 3, "h": 2},
        {"id": "b2", "type": "ram", "x": 2, "y": 1, "w": 2, "h": 2}
    ]));
    let e = l.validate().unwrap_err();
    assert_eq!(e, LayoutError::Overlap("b1".into(), "b2".into()));
    assert_eq!(e.payload(), json!({"error": "overlap", "blocks": ["b1", "b2"]}));
}

#[test]
fn touching_edges_do_not_overlap() {
    let l = layout(json!([
        {"id": "b1", "type": "cpu", "x": 0, "y": 0, "w": 4, "h": 4},
        {"id": "b2", "type": "ram", "x": 4, "y": 0, "w": 4, "h": 4}
    ]));
    assert!(l.validate().is_ok());
}

#[test]
fn out_of_grid_is_rejected() {
    for b in [
        json!({"id": "bx", "type": "cpu", "x": 7, "y": 0, "w": 2, "h": 1}),
        json!({"id": "bx", "type": "cpu", "x": 0, "y": 3, "w": 1, "h": 2}),
        json!({"id": "bx", "type": "cpu", "x": -1, "y": 0, "w": 1, "h": 1}),
        json!({"id": "bx", "type": "cpu", "x": 0, "y": 0, "w": 0, "h": 1}),
    ] {
        let e = layout(json!([b])).validate().unwrap_err();
        assert_eq!(e.payload(), json!({"error": "bounds", "blocks": ["bx"]}));
    }
}

#[test]
fn screen_count_and_types() {
    let mut l = layout(json!([]));
    l.screens.clear();
    assert_eq!(l.validate(), Err(LayoutError::Screens(0)));
    let l = layout(json!([{"id": "b1", "type": "rocket", "x": 0, "y": 0, "w": 1, "h": 1}]));
    assert_eq!(l.validate(), Err(LayoutError::BadType("b1".into())));
    let l = layout(json!([
        {"id": "b1", "type": "cpu", "x": 0, "y": 0, "w": 1, "h": 1},
        {"id": "b1", "type": "cpu", "x": 1, "y": 0, "w": 1, "h": 1}
    ]));
    assert_eq!(l.validate(), Err(LayoutError::DuplicateId("b1".into())));
}

#[test]
fn blocks_on_different_screens_may_share_cells() {
    let l: Layout = serde_json::from_value(json!({
        "version": 1, "grid": {"cols": 8, "rows": 4},
        "screens": [
            {"id": "s1", "name": "a", "blocks": [{"id": "b1", "type": "cpu", "x": 0, "y": 0, "w": 8, "h": 4}]},
            {"id": "s2", "name": "b", "blocks": [{"id": "b2", "type": "ram", "x": 0, "y": 0, "w": 8, "h": 4}]}
        ]
    }))
    .unwrap();
    assert!(l.validate().is_ok());
    assert_eq!(l.orientation, "landscape");
}
