//! set_widget: a small box in this desk's rail that an agent keeps up to
//! date -- "deploy 3/5", a test run's count, a port that is up -- without
//! a turn of the user's to ask for it (`crate::widget`, docs/WIDGETS.md).
//! propose_widget: a widget file the user adds or not, for a status that
//! should outlive the session: snyvi runs its command on a timer, on the
//! agent's desk alone (1.32).

use super::{arg, said, Session};
use crate::client;
use serde_json::{json, Value};

const SET: &str = "Show a small status box in this snyvi desk's rail, kept up to date as the work goes. Only \
when the user asked for one. A box is for a figure that changes by itself and that they would otherwise go and \
check: CI 3/5 green, a deploy running, a dev server up. Never for what waits on them (that is hand_over), how to \
do something (say it in your reply), news (a document), or what changes only when they act. Put the figure in \
count and one short line in the body (\"**deploy** 3/5 green\"), no instructions. The first time, the user gets \
a card on their Your turn to allow the box, and nothing shows until they do; after that the same name updates it \
freely, and an empty body clears it. If they said not now, or removed it, leave it. Optional: for (today, week, \
panel: while this panel is open) suggests how long it lasts, tone (ok, warn, bad) colours the count, lines is its \
height (1-6, keep it 1), stale_after the seconds before it dims as old (default 1800).";

const PROPOSE: &str = "Propose a widget file to the user, for a status worth keeping after this session ends: \
snyvi runs its command on a timer while this desk is in view and shows what it prints (Markdown, or \
{\"body\",\"tone\",\"count\"} JSON). Only when the user asked for a widget. The same test as set_widget: a \
figure that changes by itself and that they would otherwise go and check, printed as count plus one short line, \
never a to-do of theirs or how to do it. The user gets a card on \
their Your turn showing the script, where it runs and how often, can try it once, and adds it or not; nothing \
runs before. It is this desk's own: it runs in this desk's folder and on no other desk, and only the user can \
share it. The script's folder is first on PATH and in $SNYVI_WIDGET_DIR, so name the script bare as the command \
(run.sh, not ./run.sh or sh run.sh), or write \"$SNYVI_WIDGET_DIR/run.sh\"; snyvi refuses a command that cannot \
reach it. Give the script whole (it gets the desk and settings as JSON on stdin). for suggests how long it lasts: \
today, week, panel (while this panel is open), or leave it out for until the user turns it off.";

pub(super) fn specs() -> Vec<Value> {
    vec![
        json!({
            "name": "propose_widget", "title": "Propose a widget", "description": PROPOSE,
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string", "description": "Lowercase letters, digits and dashes, at most 32." },
                "title": { "type": "string" },
                "command": { "type": "string", "description": "One line, run with sh -c (cmd /C on Windows) in the desk's folder. Name the script bare: run.sh. Its folder is first on PATH and in $SNYVI_WIDGET_DIR." },
                "every": { "type": "integer", "minimum": 5, "description": "Seconds between runs (60)." },
                "timeout": { "type": "integer", "minimum": 1, "maximum": 30 },
                "lines": { "type": "integer", "minimum": 1, "maximum": 6 },
                "script_name": { "type": "string", "description": "The script's file name, e.g. run.sh." },
                "script": { "type": "string", "description": "The script, whole, at most 16 KB." },
                "why": { "type": "string", "description": "One sentence." },
                "for": { "type": "string", "enum": ["today", "week", "panel", "always"], "description": "How long you suggest it lasts; the user picks on the card." }
            }, "required": ["name", "command", "why"], "additionalProperties": false },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false, "openWorldHint": false }
        }),
        json!({
            "name": "set_widget", "title": "Set a widget", "description": SET,
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string", "description": "Lowercase letters, digits and dashes, at most 32." },
                "body": { "type": "string", "description": "Markdown, at most 1500 characters. Empty clears." },
                "tone": { "type": "string", "enum": ["ok", "warn", "bad", "none"] },
                "count": { "type": "string", "description": "At most 8 characters: 3, !2, 3/5." },
                "lines": { "type": "integer", "minimum": 1, "maximum": 6 },
                "stale_after": { "type": "integer", "minimum": 0, "description": "Seconds; 0 is never." },
                "for": { "type": "string", "enum": ["today", "week", "panel", "always"], "description": "How long you suggest it lasts; the user picks on the card." }
            }, "required": ["name", "body"], "additionalProperties": false },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false }
        }),
    ]
}

impl Session {
    pub(super) fn propose_widget(&self, args: &Value) -> Value {
        let Some(p) = self.pane.as_deref() else {
            return said(
                "This session is not running in a snyvi desk, so there is no one to propose it to.",
                true,
            );
        };
        let mut body = args.clone();
        body["by"] = json!(self.by());
        match client::pane_thread(&self.paths, p, "propose-widget", body) {
            Ok(_) => said(
                format!("Proposed. The user has a card on their Your turn in snyvi with the script, where it runs and how often; nothing runs until they add {}. Carry on.", arg(args, "name")),
                false,
            ),
            Err(e) => said(format!("snyvi did not take it: {e}"), true),
        }
    }

    pub(super) fn set_widget(&self, args: &Value) -> Value {
        let Some(p) = self.pane.as_deref() else {
            return said(
                "This session is not running in a snyvi desk, so there is no rail to put it in.",
                true,
            );
        };
        let name = arg(args, "name");
        let mut body = json!({ "body": args.get("body").and_then(Value::as_str).unwrap_or("") });
        for k in ["tone", "count", "lines", "stale_after"] {
            if let Some(v) = args.get(k).filter(|v| !v.is_null()) {
                body[k] = v.clone();
            }
        }
        let cleared = body["body"].as_str().is_some_and(|b| b.trim().is_empty());
        let mut sent = json!({ "name": name, "body": body });
        if let Some(f) = args.get("for").and_then(Value::as_str) {
            sent["for"] = json!(f);
        }
        match client::pane_thread(&self.paths, p, "widget", sent) {
            Ok(v) if v.get("waiting").and_then(Value::as_bool) == Some(true) => said(
                format!("The user has a card on their Your turn to allow the box {name} on this desk; it shows once they do, with what you sent last. Carry on, and set it as the work goes: your updates wait behind the card."),
                false,
            ),
            Ok(_) if cleared => said(format!("Cleared the widget {name}."), false),
            Ok(_) => said(
                format!("The widget {name} is in this desk's rail. Set it again with the same name to update it; an empty body clears it."),
                false,
            ),
            Err(e) => said(format!("snyvi did not take it: {e}"), true),
        }
    }
}
