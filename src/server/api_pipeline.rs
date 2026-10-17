//! Pipelines over HTTP (`crate::pipeline`, plan 2a0edf418f): the window's,
//! and only the window's.
//!
//! Every route here is behind the desk's gate (`refuse_desk`): a page that
//! holds the capability. No route behind the token makes, edits, starts or
//! goes on with a pipeline -- an agent in a panel can no more start one than
//! it can start a command -- and the pipeline's panel is checked to be on
//! the desk the route names.

use super::*;
use crate::pipeline::{self, Edited, Made, New};

type Q = Query<std::collections::HashMap<String, String>>;

fn refused(code: StatusCode, error: impl Into<String>) -> Response {
    (code, Json(json!({ "error": error.into() }))).into_response()
}

/// A desk's pipelines, with their steps.
pub(crate) async fn desk_pipelines(
    State(app): S,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(q): Q,
) -> Response {
    if let Some(no) = refuse_desk(&app, &headers, &q) {
        return no;
    }
    match app.store.clocked(|c, now| pipeline::for_desk(c, id, now)) {
        Ok(ps) => Json(json!({ "pipelines": ps })).into_response(),
        Err(e) => err(e),
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct NewBody {
    pane: String,
    #[serde(flatten)]
    new: New,
}

/// The editor's Save on a new pipeline: a draft, on a panel of this desk.
pub(crate) async fn new_pipeline(
    State(app): S,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(q): Q,
    Json(b): Json<NewBody>,
) -> Response {
    if let Some(no) = refuse_desk(&app, &headers, &q) {
        return no;
    }
    match app.store.pane(&b.pane) {
        Ok(Some(placed)) if placed.desk_id == id => {}
        _ => return StatusCode::NOT_FOUND.into_response(),
    }
    match app
        .store
        .clocked(|c, now| pipeline::create(c, id, &b.pane, &b.new, now))
    {
        Ok(Made::New(p)) => {
            super::pipeline_run::changed(&app, id);
            (StatusCode::CREATED, Json(json!({ "pipeline": p }))).into_response()
        }
        Ok(Made::Bad(why)) => refused(StatusCode::BAD_REQUEST, why),
        Ok(Made::Busy) => refused(
            StatusCode::CONFLICT,
            "this panel is already working through a pipeline",
        ),
        Ok(Made::NoSuchDesk) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => err(e),
    }
}

/// The editor's Save on a pipeline that is not running.
pub(crate) async fn edit_pipeline(
    State(app): S,
    headers: HeaderMap,
    Path((id, row)): Path<(i64, i64)>,
    Query(q): Q,
    Json(n): Json<New>,
) -> Response {
    if let Some(no) = refuse_desk(&app, &headers, &q) {
        return no;
    }
    match app
        .store
        .clocked(|c, now| pipeline::edit(c, id, row, &n, now))
    {
        Ok(Edited::Saved(p)) => {
            super::pipeline_run::changed(&app, id);
            Json(json!({ "pipeline": p })).into_response()
        }
        Ok(Edited::Bad(why)) => refused(StatusCode::BAD_REQUEST, why),
        Ok(Edited::Running) => refused(
            StatusCode::CONFLICT,
            "pause it first: a running stage is not edited",
        ),
        Ok(Edited::NotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => err(e),
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct ActBody {
    /// `go` and `skip`: the step.
    step: i64,
}

/// Start, pause, resume, stop, put away and bring back a pipeline; Go or
/// Skip a step the planner proposed (or one still queued).
pub(crate) async fn pipeline_act(
    State(app): S,
    headers: HeaderMap,
    Path((id, row, act)): Path<(i64, i64, String)>,
    Query(q): Q,
    body: Option<Json<ActBody>>,
) -> Response {
    if let Some(no) = refuse_desk(&app, &headers, &q) {
        return no;
    }
    let step = body.map(|Json(b)| b.step).unwrap_or(0);
    let Ok(Some(p)) = app.store.clocked(|c, _| pipeline::get(c, id, row)) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let state = p.state.as_str();
    let ok: anyhow::Result<bool> = match act.as_str() {
        "start" | "resume" => {
            let other = app
                .store
                .clocked(|c, _| pipeline::of_pane(c, &p.pane))
                .ok()
                .flatten()
                .filter(|o| o.id != p.id);
            if other.is_some() {
                return refused(
                    StatusCode::CONFLICT,
                    "this panel is already working through another pipeline",
                );
            }
            if !matches!(state, "draft" | "paused" | "stopped" | "done" | "waiting")
                || (act == "start" && state != "draft")
            {
                return refused(StatusCode::CONFLICT, format!("it is {state}"));
            }
            app.store
                .clocked(|c, now| pipeline::set_state(c, p.id, "running", "", now).map(|()| true))
        }
        "pause" if matches!(state, "running" | "waiting") => app.store.clocked(|c, now| {
            pipeline::set_state(
                c,
                p.id,
                "paused",
                "you paused it; the stage under way finishes",
                now,
            )
            .map(|()| true)
        }),
        "stop" if p.active() || state == "draft" => {
            if let Some(s) = p.running() {
                let _ = app.store.clocked(|c, now| {
                    pipeline::step_ended(
                        c,
                        s.id,
                        &pipeline::End {
                            state: "stopped",
                            result: &s.last_reply,
                            commits: "",
                            diff_lines: 0,
                        },
                        now,
                    )
                });
            }
            super::pipeline_run::end(&app, &p, "stopped", "you stopped it").await;
            Ok(true)
        }
        "hide" => app.store.clocked(|c, now| pipeline::hide(c, id, p.id, now)),
        "unhide" => app.store.clocked(|c, _| pipeline::unhide(c, id, p.id)),
        "go" | "skip" => app.store.clocked(|c, now| {
            let settled = pipeline::settle_step(c, id, p.id, step, act == "go", now)?;
            if settled && state == "waiting" {
                pipeline::set_state(c, p.id, "running", "", now)?;
            }
            Ok(settled)
        }),
        "pause" | "stop" => return refused(StatusCode::CONFLICT, format!("it is {state}")),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    match ok {
        Ok(true) => {
            super::pipeline_run::changed(&app, id);
            Json(json!({ "ok": true })).into_response()
        }
        Ok(false) => refused(StatusCode::CONFLICT, "nothing to do"),
        Err(e) => err(e),
    }
}
