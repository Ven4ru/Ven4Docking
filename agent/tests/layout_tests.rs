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

// ---------- 1.1: new blocks, program screens, permissions ----------

fn screens(v: serde_json::Value) -> Layout {
    serde_json::from_value(json!({"version": 1, "grid": {"cols": 8, "rows": 4}, "screens": v})).unwrap()
}

#[test]
fn new_blocks_and_program_screens() {
    let l = screens(json!([
        {"id": "s1", "name": "Главный", "blocks": [
            {"id": "b1", "type": "power", "x": 0, "y": 0, "w": 4, "h": 1, "opts": {"buttons": ["lock", "restart"]}},
            {"id": "b2", "type": "keys", "x": 4, "y": 0, "w": 4, "h": 1, "opts": {"actions": ["mic", "sign"]}},
            {"id": "b3", "type": "mixer", "x": 0, "y": 1, "w": 3, "h": 2},
            {"id": "b4", "type": "notify", "x": 3, "y": 1, "w": 3, "h": 2, "opts": {"max": 5}}
        ]},
        {"id": "s2", "name": "OBS", "forApp": "OBS64.exe", "blocks": [
            {"id": "b5", "type": "obs", "x": 0, "y": 0, "w": 4, "h": 2}
        ]}
    ]));
    assert_eq!(l.validate(), Ok(()));
    assert!(l.power_allowed("lock") && l.power_allowed("restart") && l.power_allowed("cancel"));
    assert!(!l.power_allowed("sleep") && !l.power_allowed("shutdown") && !l.power_allowed("format"));
    assert!(l.action_allowed("mic") && l.action_allowed("sign") && !l.action_allowed("out"));
    assert!(l.has_block("obs") && l.has_block("mixer") && !l.has_block("media"));
    assert_eq!(l.app_screens(), vec!["OBS64.exe".to_string()]);
    let back = serde_json::to_value(&l).unwrap();
    assert_eq!(back["screens"][1]["forApp"], "OBS64.exe");
    assert!(back["screens"][0].get("forApp").is_none(), "usual screens have no forApp");

    // a power block without opts shows all four; cancel goes with restart/shutdown only
    let all = screens(json!([{"id": "s1", "blocks": [{"id": "b1", "type": "power", "x": 0, "y": 0, "w": 4, "h": 1}]}]));
    assert!(["lock", "sleep", "restart", "shutdown", "cancel"].iter().all(|a| all.power_allowed(a)));
    let lock_only = screens(json!([{"id": "s1", "blocks": [{"id": "b1", "type": "power", "x": 0, "y": 0, "w": 1, "h": 1, "opts": {"buttons": ["lock"]}}]}]));
    assert!(!lock_only.power_allowed("cancel"));
    // the classic template has none of the new blocks: nothing is allowed by default
    let c = Layout::classic();
    assert!(!c.power_allowed("lock") && !c.action_allowed("mic") && c.app_screens().is_empty());
}

#[test]
fn program_screen_limits() {
    let app = |i: usize| json!({"id": format!("a{i}"), "name": "x", "forApp": format!("app{i}.exe"), "blocks": []});
    let mut list = vec![json!({"id": "s1", "name": "Главный", "blocks": []})];
    list.extend((0..5).map(app));
    assert_eq!(screens(json!(list.clone())).validate(), Ok(()), "five program screens");
    list.push(app(5));
    let e = screens(json!(list)).validate().unwrap_err();
    assert_eq!(e.payload(), json!({"error": "appScreens", "count": 6}));
    assert_eq!(screens(json!([app(0)])).validate(), Err(LayoutError::AppScreens(1)), "a usual screen is needed");
    for bad in ["", " ", "C:\\obs\\obs64.exe", "bin/obs64.exe", "obs*.exe"] {
        let l = screens(json!([{"id": "s1", "blocks": []}, {"id": "s2", "forApp": bad, "blocks": []}]));
        assert_eq!(l.validate().unwrap_err().payload(), json!({"error": "forApp", "screens": ["s2"]}), "{bad:?}");
    }
}
