//! A new project desk's first panel and the folder Welcome makes: through
//! the routes, as the window calls them. Unix only, like `accounts`: a
//! panel here is a shell line.

use super::accounts::page;
use super::*;

/// The hold is the daemon's: a panel made held says so in every read of its
/// desk, the page's own start after a reload is refused, and the reader's
/// press starts it and ends the hold.
#[tokio::test]
async fn a_held_panel_starts_only_on_the_readers_press() {
    let mut d = 0;
    let (_tmp, app, router, l) = gated_app_with("snyvi-held", |store| {
        d = store.create_desk("/tmp", Some("held")).unwrap().id;
    });
    let (s, t) = page(
        &router,
        &l,
        "POST",
        &format!("/api/desks/{d}/panes"),
        Some(serde_json::json!({ "cmd": "true", "held": true })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{t}");
    let pane = serde_json::from_str::<serde_json::Value>(&t).unwrap()["pane"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(t.contains("\"held\":true"), "{t}");
    let (_, t) = page(&router, &l, "GET", "/api/desks", None).await;
    assert!(t.contains("\"held\":true"), "read back: {t}");

    let start_at = format!("/api/panes/{pane}/start");
    let (s, t) = page(
        &router,
        &l,
        "POST",
        &start_at,
        Some(serde_json::json!({ "marked": true })),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "{t}");
    assert!(!app.panes.status(&pane).running);
    assert!(app.store.pane(&pane).unwrap().unwrap().pane.held);

    let (s, t) = page(
        &router,
        &l,
        "POST",
        &start_at,
        Some(serde_json::json!({ "cmd": "true" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{t}");
    assert!(
        !app.store.pane(&pane).unwrap().unwrap().pane.held,
        "the press ends the hold"
    );

    // A shell is never held: there is nothing in it to wait for.
    let (_, t) = page(
        &router,
        &l,
        "POST",
        &format!("/api/desks/{d}/panes"),
        Some(serde_json::json!({ "held": true })),
    )
    .await;
    assert!(!t.contains("\"held\""), "{t}");
}

/// *Start something new* makes one folder, where the reader picked, and
/// opens it as the picker would; never one already there, never a path in
/// a name, never a hidden one.
#[tokio::test]
async fn start_something_new_makes_one_folder_and_nothing_else() {
    let (tmp, app, router, l) = gated_app_with("snyvi-new-folder", |_| {});
    let under = tmp.path.join("projects");
    std::fs::create_dir(&under).unwrap();
    let root = app.browse.open(&under).unwrap().id;
    let new = |name: &str| Some(serde_json::json!({ "name": name, "under": root }));

    let (s, t) = page(&router, &l, "POST", "/api/browse/new", new(" tide-clock ")).await;
    assert_eq!(s, StatusCode::CREATED, "{t}");
    assert!(under.join("tide-clock").is_dir());
    let made = serde_json::from_str::<serde_json::Value>(&t).unwrap()["root"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(app.browse.get(&made).is_some(), "opened, for its desk");

    let (s, _) = page(&router, &l, "POST", "/api/browse/new", new("tide-clock")).await;
    assert_eq!(s, StatusCode::CONFLICT, "never one already there");
    for bad in [
        "", "..", "../out", "a/b", "a\\b", ".hidden", "x\u{0}y", "con:",
    ] {
        let (s, t) = page(&router, &l, "POST", "/api/browse/new", new(bad)).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{bad:?} {t}");
    }
    assert!(!tmp.path.join("out").exists());
    let n = std::fs::read_dir(&under).unwrap().count();
    assert_eq!(n, 1, "one folder made, nothing else");
}
