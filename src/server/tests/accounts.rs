//! Claude accounts through the routes (`crate::accounts`, `api_accounts`):
//! signed in or pasted, picked by a desk or a panel, and the token in no
//! answer on the way. Unix only: a sign-in is a script in a terminal and a
//! panel is a shell line that writes what it was given.

use super::*;

/// The page's call: its host, origin and capability, a JSON body or none.
pub(super) async fn page(
    router: &Router,
    l: &Leaves,
    method: &str,
    path: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, String) {
    let req = axum::http::Request::builder()
        .method(method)
        .uri(path)
        .header("host", l.host.as_str())
        .header("origin", l.origin.as_str())
        .header(CAPABILITY_HEADER, l.cap.as_str());
    let req = match body {
        Some(b) => req
            .header("content-type", "application/json")
            .body(Body::from(b.to_string()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

/// A desk on `/tmp` with one panel, and a token for an account on it.
fn desk_and_token(
    name: &str,
) -> (
    crate::store::tempdir::Dir,
    Arc<App>,
    Router,
    Leaves,
    (i64, String),
    String,
) {
    let mut ids = (0, String::new());
    let (tmp, app, router, l) = gated_app_with(name, |store| {
        let d = store.create_desk("/tmp", Some("a")).unwrap().id;
        let crate::desk::Opened::Pane(p) = store.open_pane(d, "/tmp", "", false).unwrap() else {
            panic!("no pane");
        };
        ids = (d, p.id);
    });
    let token = format!("sk-ant-oat01-{}", "Zq9_x-".repeat(16));
    (tmp, app, router, l, ids, token)
}

/// The account a pasted token makes, by its id.
async fn add(router: &Router, l: &Leaves, token: &str) -> (i64, String) {
    let (s, t) = page(
        router,
        l,
        "POST",
        "/api/accounts",
        Some(serde_json::json!({ "label": " Work ", "token": token })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{t}");
    let id = serde_json::from_str::<serde_json::Value>(&t).unwrap()["account"]["id"]
        .as_i64()
        .unwrap();
    (id, t)
}

/// *Sign in…*: `claude setup-token` in a hidden terminal (here a script that
/// draws what it draws), the sign-in page's address handed to the page, the
/// code the page shows typed back, and the token kept the moment the line
/// after it is on the screen -- with the token in no answer on the way.
#[tokio::test]
async fn a_sign_in_keeps_the_token_it_reads_and_shows_none_of_it() {
    let (_tmp, app, router, l) = gated_app_with("snyvi-signin", |_| {});
    let token = format!("sk-ant-oat01-{}", "Yw8_k-".repeat(16));
    *crate::accounts::signin::TEST_SCRIPT.lock().unwrap() = Some(format!(
        "printf '\\033[1mBrowser did not open? Use the url below to sign in\\033[0m\\r\\n  https://claude.ai/oauth/authorize?code=true&state=t\\r\\nPaste code here if prompted > '; \
         read c; [ \"$c\" = abc-123 ] || exit 3; \
         printf 'Your OAuth token (valid for 1 year):\\r\\n\\r\\n%s\\r\\n\\r\\nStore this token securely.\\r\\n' '{token}'; sleep 30"
    ));
    let mut seen = String::new();
    let mut until = async |want: &str| -> serde_json::Value {
        for _ in 0..100 {
            let (s, t) = page(&router, &l, "GET", "/api/accounts/signin", None).await;
            assert_eq!(s, StatusCode::OK, "{t}");
            seen.push_str(&t);
            let j: serde_json::Value = serde_json::from_str(&t).unwrap();
            if j["signin"]["phase"] == want {
                return j["signin"].clone();
            }
            assert_ne!(j["signin"]["phase"], "failed", "{t}");
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        panic!("never {want}");
    };
    let code = || Some(serde_json::json!({ "code": "abc-123" }));

    let (s, t) = page(
        &router,
        &l,
        "POST",
        "/api/accounts/signin",
        Some(serde_json::json!({ "label": "Work" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{t}");
    let waiting = until("waiting").await;
    assert_eq!(
        waiting["url"],
        "https://claude.ai/oauth/authorize?code=true&state=t"
    );
    let (s, t) = page(&router, &l, "POST", "/api/accounts/signin/code", code()).await;
    assert_eq!(s, StatusCode::OK, "{t}");
    let done = until("done").await;
    let id = done["account"].as_i64().unwrap();
    assert_eq!(app.store.claude_account(id).unwrap().unwrap().label, "Work");
    assert_eq!(
        app.secrets.claude_value(id).as_deref(),
        Some(token.as_str())
    );
    assert!(!seen.contains(&token[13..30]), "the token is in no answer");
    // Nothing open to cancel now; a code has nowhere to go.
    let (_, t) = page(
        &router,
        &l,
        "POST",
        "/api/accounts/signin/cancel",
        Some(serde_json::json!({})),
    )
    .await;
    assert!(t.contains("\"cancelled\":false"), "{t}");
    let (s, _) = page(&router, &l, "POST", "/api/accounts/signin/code", code()).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    *crate::accounts::signin::TEST_SCRIPT.lock().unwrap() = None;
}

/// A Claude account added from a pasted token is listed by its label only:
/// the token is in no answer, the desk and the panel take the account, and a
/// panel's own wins over its desk's.
#[tokio::test]
async fn an_account_token_goes_in_once_and_never_comes_back_out() {
    let (_tmp, app, router, l, (desk, pane), token) = desk_and_token("snyvi-accounts");
    let (id, t) = add(&router, &l, &token).await;
    assert!(
        !t.contains(&token[13..30]),
        "the token is not in the answer: {t}"
    );
    assert_eq!(
        app.secrets.claude_value(id).as_deref(),
        Some(token.as_str())
    );

    let api_key = Some(serde_json::json!({ "token": "sk-ant-api03-not-this" }));
    let (s, t) = page(&router, &l, "POST", "/api/accounts", api_key).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        t.contains("ANTHROPIC_API_KEY"),
        "an API key is sent to Keys: {t}"
    );

    let (s, t) = page(&router, &l, "GET", "/api/accounts", None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(t.contains("\"Work\"") && !t.contains(&token[13..30]), "{t}");

    let pick = |account: serde_json::Value| Some(serde_json::json!({ "account": account }));
    let desk_at = format!("/api/desks/{desk}/account");
    let pane_at = format!("/api/panes/{pane}/account");
    let (s, _) = page(&router, &l, "POST", &desk_at, pick(id.into())).await;
    assert_eq!(s, StatusCode::OK);
    let account = || app.store.pane(&pane).unwrap().unwrap().account();
    assert_eq!(account(), id, "the panel follows its desk");
    let (s, _) = page(&router, &l, "POST", &pane_at, pick(0.into())).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(account(), 0, "its own wins");
    let (s, _) = page(&router, &l, "POST", &desk_at, pick((id + 7).into())).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "no such account");

    let (s, t) = page(&router, &l, "GET", "/api/desks", None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(
        !t.contains(&token[13..30]),
        "the desks list has no token either"
    );
    assert!(t.contains("\"Work\""), "but it names the account: {t}");
}

/// Started, a panel has its desk's account's token, and only there; a token
/// that is nowhere starts it on `/login` and says so; and taking the account
/// away puts the desk back on `/login` and forgets the token.
#[tokio::test]
async fn a_panel_starts_with_its_accounts_token_and_without_one_that_is_gone() {
    let (tmp, app, router, l, (desk, pane), token) = desk_and_token("snyvi-account-start");
    let (id, _) = add(&router, &l, &token).await;
    let desk_at = format!("/api/desks/{desk}/account");
    let (s, _) = page(
        &router,
        &l,
        "POST",
        &desk_at,
        Some(serde_json::json!({ "account": id })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let seen = tmp.path.join("seen");
    let start = || {
        Some(serde_json::json!({
            "cmd": format!("printf %s \"$CLAUDE_CODE_OAUTH_TOKEN\" > '{}'", seen.display()),
        }))
    };
    let ran = async || {
        for _ in 0..200 {
            if !app.panes.status(&pane).running && seen.exists() {
                return std::fs::read_to_string(&seen).unwrap_or_default();
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        panic!("the panel did not run");
    };
    let start_at = format!("/api/panes/{pane}/start");

    let (s, t) = page(&router, &l, "POST", &start_at, start()).await;
    assert_eq!(s, StatusCode::OK, "{t}");
    assert!(
        !t.contains(&token[13..30]) && !t.contains("account_missing"),
        "{t}"
    );
    assert_eq!(
        ran().await,
        token,
        "the token is in the panel's environment"
    );
    assert_eq!(
        app.panes.status(&pane).account,
        id,
        "and the panel says as whom"
    );
    assert!(app.store.claude_account(id).unwrap().unwrap().used_at > 0);

    std::fs::remove_file(&seen).unwrap();
    app.secrets.forget_claude(id);
    let (s, t) = page(&router, &l, "POST", &start_at, start()).await;
    assert_eq!(s, StatusCode::OK, "{t}");
    assert!(t.contains("\"account_missing\":true"), "{t}");
    assert_ne!(ran().await, token, "no token, so /login");
    assert_eq!(app.panes.status(&pane).account, 0);

    let delete_at = format!("/api/accounts/{id}/delete");
    let (s, _) = page(&router, &l, "POST", &delete_at, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(app.secrets.claude_value(id), None, "the token is forgotten");
    assert_eq!(app.store.desk(desk).unwrap().unwrap().account, 0);
    let keys =
        std::fs::read_to_string(tmp.path.join("config").join("keys.json")).unwrap_or_default();
    assert!(!keys.contains(&token[13..30]));
}

/// What a panel's hook and status line post, with the agent's token.
async fn agent(router: &Router, l: &Leaves, pane: &str, body: serde_json::Value) -> StatusCode {
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/panes/{pane}/agent"))
        .header("host", l.host.as_str())
        .header("authorization", l.bearer.as_str())
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    router.clone().oneshot(req).await.unwrap().status()
}

/// Two panels on two accounts (plan 7ead8a629d): each status line's windows
/// are kept under the account its panel runs as, Home names both and its
/// one `quota` is the account with the least left, and a reading that moves
/// no whole percent is no news. A turn that ends because the sign-in no
/// longer works marks the account until it is renewed, or until a turn
/// ends with its answer; and the desks say how many panels run as each.
#[tokio::test]
async fn usage_is_kept_by_account_and_a_sign_in_that_stopped_waits_for_renew() {
    let (_tmp, app, router, l, (desk, pane), token) = desk_and_token("snyvi-account-usage");
    let (id, _) = add(&router, &l, &token).await;
    let crate::desk::Opened::Pane(p2) = app.store.open_pane(desk, "/tmp", "", false).unwrap()
    else {
        panic!("no second pane");
    };
    let p2 = p2.id;
    let pick = |account: i64| Some(serde_json::json!({ "account": account }));
    let (s, _) = page(
        &router,
        &l,
        "POST",
        &format!("/api/desks/{desk}/account"),
        pick(id),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = page(
        &router,
        &l,
        "POST",
        &format!("/api/panes/{p2}/account"),
        pick(0),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    for p in [&pane, &p2] {
        let at = format!("/api/panes/{p}/start");
        let (s, t) = page(
            &router,
            &l,
            "POST",
            &at,
            Some(serde_json::json!({ "cmd": "sleep 20" })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{t}");
    }
    assert_eq!(app.panes.status(&pane).account, id);
    assert_eq!(app.panes.status(&p2).account, 0);

    let at = crate::store::now() + 3600;
    let line = |used: f64| serde_json::json!({ "model": "Opus", "limits": { "five_hour": { "used": used, "resets_at": at } } });
    assert_eq!(
        agent(&router, &l, &pane, line(20.0)).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        agent(&router, &l, &p2, line(70.0)).await,
        StatusCode::NO_CONTENT
    );
    let (s, t) = page(&router, &l, "GET", "/api/home", None).await;
    assert_eq!(s, StatusCode::OK, "{t}");
    let j: serde_json::Value = serde_json::from_str(&t).unwrap();
    let qs = j["quotas"].as_array().unwrap();
    assert_eq!(qs.len(), 2, "{t}");
    assert_eq!(
        (qs[0]["account"].as_i64(), qs[0]["label"].as_str()),
        (Some(0), Some("your login"))
    );
    assert_eq!(qs[0]["five_hour"]["used"], 70.0);
    assert_eq!(
        (qs[1]["account"].as_i64(), qs[1]["label"].as_str()),
        (Some(id), Some("Work"))
    );
    assert_eq!(qs[1]["five_hour"]["used"], 20.0);
    assert_eq!(
        j["quota"]["five_hour"]["used"], 70.0,
        "the one with the least left"
    );
    let limit = |used: f64| {
        Some(crate::statusline::Limit {
            used,
            resets_at: at,
        })
    };
    let now = crate::store::now();
    assert!(
        !app.usage.heard(id, limit(20.3), None, now),
        "the same whole percent"
    );
    assert!(
        app.usage.heard(id, limit(21.0), None, now),
        "a percent more"
    );

    let desks = async || -> serde_json::Value {
        let (s, t) = page(&router, &l, "GET", "/api/desks", None).await;
        assert_eq!(s, StatusCode::OK, "{t}");
        serde_json::from_str(&t).unwrap()
    };
    let j = desks().await;
    assert_eq!(j["accounts"][0]["running"], 1, "one panel runs as Work");
    assert_eq!(j["quotas"][id.to_string()]["five_hour"]["used"], 21.0);
    assert!(j["broken"].as_object().unwrap().is_empty());

    let stopped = |why: &str| serde_json::json!({ "state": "done", "stopped": why });
    assert_eq!(
        agent(&router, &l, &pane, stopped("authentication_failed")).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        agent(&router, &l, &p2, stopped("rate_limit")).await,
        StatusCode::NO_CONTENT
    );
    let j = desks().await;
    assert_eq!(j["broken"][id.to_string()], "authentication_failed");
    assert!(
        j["broken"].get("0").is_none(),
        "a limit is not a broken sign-in"
    );
    let renew = Some(serde_json::json!({ "token": token }));
    let (s, _) = page(
        &router,
        &l,
        "POST",
        &format!("/api/accounts/{id}/renew"),
        renew,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert!(
        desks().await["broken"].as_object().unwrap().is_empty(),
        "renewed"
    );

    agent(&router, &l, &p2, stopped("billing_error")).await;
    assert_eq!(desks().await["broken"]["0"], "billing_error");
    agent(&router, &l, &p2, serde_json::json!({ "state": "done" })).await;
    assert!(
        desks().await["broken"].as_object().unwrap().is_empty(),
        "a turn that ends with its answer says it works"
    );
    for p in [&pane, &p2] {
        let _ = page(&router, &l, "POST", &format!("/api/panes/{p}/stop"), None).await;
    }
}
