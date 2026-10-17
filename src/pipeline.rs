//! Pipelines: a panel that works through a plan the reader can watch (plan
//! 2a0edf418f, 1.40).
//!
//! A **pipeline** belongs to one panel. The reader makes it in the window,
//! and only there: a list of steps they wrote, one step looped, or a goal a
//! planner works towards one step at a time. Each **stage** is a fresh
//! Claude Code session in that panel, started with the step as its first
//! prompt -- one argv entry, never through a shell (`pane::Start::args`) --
//! in a worktree of the desk's folder on a `pipeline/<name>` branch, so the
//! reader's own checkout and the other panels are never touched.
//!
//! This file is the store's side: the rows, what a reader may write in them,
//! and the report. It runs nothing. What starts a stage, reads its end and
//! asks the planner is `server::pipeline_run`; the planner's one call is
//! `planner`; the transcript is read by `transcript`.
//!
//! Nothing is deleted. A ✕ sets `hidden_at`, a skipped step stays in the
//! list as skipped, and every stage's session stays resumable.

pub mod planner;
#[cfg(test)]
mod tests;
pub mod transcript;

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS pipelines (
  id INTEGER PRIMARY KEY,
  desk_id INTEGER NOT NULL REFERENCES desks(id) ON DELETE CASCADE,
  pane TEXT NOT NULL,
  name TEXT NOT NULL,
  kind TEXT NOT NULL,
  goal TEXT NOT NULL DEFAULT '',
  rules TEXT NOT NULL DEFAULT '',
  autopilot INTEGER NOT NULL DEFAULT 0,
  planner_model TEXT NOT NULL DEFAULT 'sonnet',
  ctx_limit INTEGER NOT NULL DEFAULT 70,
  caps TEXT NOT NULL DEFAULT '{}',
  branch TEXT NOT NULL DEFAULT '',
  folder TEXT NOT NULL DEFAULT '',
  base TEXT NOT NULL DEFAULT '',
  state TEXT NOT NULL DEFAULT 'draft',
  why TEXT NOT NULL DEFAULT '',
  resume_at INTEGER NOT NULL DEFAULT 0,
  report TEXT NOT NULL DEFAULT '',
  created_at INTEGER NOT NULL,
  started_at INTEGER NOT NULL DEFAULT 0,
  ended_at INTEGER NOT NULL DEFAULT 0,
  hidden_at INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS pipelines_desk ON pipelines(desk_id, hidden_at, created_at);
CREATE INDEX IF NOT EXISTS pipelines_state ON pipelines(state);
CREATE TABLE IF NOT EXISTS pipeline_steps (
  id INTEGER PRIMARY KEY,
  pipeline_id INTEGER NOT NULL REFERENCES pipelines(id) ON DELETE CASCADE,
  n INTEGER NOT NULL,
  author TEXT NOT NULL,
  kind TEXT NOT NULL DEFAULT 'prompt',
  text TEXT NOT NULL,
  why TEXT NOT NULL DEFAULT '',
  state TEXT NOT NULL DEFAULT 'queued',
  session TEXT NOT NULL DEFAULT '',
  transcript TEXT NOT NULL DEFAULT '',
  base TEXT NOT NULL DEFAULT '',
  last_reply TEXT NOT NULL DEFAULT '',
  stopped_at INTEGER NOT NULL DEFAULT 0,
  handoff INTEGER NOT NULL DEFAULT 0,
  result TEXT NOT NULL DEFAULT '',
  ctx_peak INTEGER NOT NULL DEFAULT 0,
  commits TEXT NOT NULL DEFAULT '',
  diff_lines INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  started_at INTEGER NOT NULL DEFAULT 0,
  ended_at INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS pipeline_steps_of ON pipeline_steps(pipeline_id, n);
"#;

/// Where the next step comes from: the reader's list, in order; one step
/// again, each cycle; or the planner, reading what the last one did.
pub const KINDS: [&str; 3] = ["list", "loop", "goal"];
/// A step is a prompt, or a `/goal` condition Claude Code keeps working
/// towards with its own judge.
pub const STEP_KINDS: [&str; 2] = ["prompt", "goal"];
/// The planner's model, as `claude --model` takes it. Sonnet by default: on
/// the desk's subscription a call is a sliver of the five-hour window, and
/// it was the sharper of the two in the spikes.
pub const MODELS: [&str; 3] = ["sonnet", "haiku", "opus"];

/// A pipeline's state. `waiting` is a planner's step waiting for the
/// reader's Go (autopilot off); `paused` waits for Resume, or for
/// `resume_at` when a limit paused it.
pub const ACTIVE: [&str; 3] = ["running", "waiting", "paused"];

/// The most a step's text may be, in bytes: it is one argv entry, and a
/// stage's first prompt, not a document.
pub const STEP_BYTES: usize = 8000;
/// The goal and the rules, each.
pub const GOAL_BYTES: usize = 4000;
/// Steps a reader may write into a list at once.
pub const STEPS_MAX: usize = 40;
/// A name, in characters: it becomes the branch, so it is short.
pub const NAME_CHARS: usize = 60;
/// What a step's result keeps of the stage's last reply, in characters.
pub const RESULT_CHARS: usize = 6000;

/// The line a looped step ends its reply with when there is nothing left to
/// do. Two in a row end the loop: the guard against an improve loop that
/// churns once the work is done.
pub const NOTHING_LEFT: &str = "NOTHING LEFT";

/// What the reader caps a pipeline at. The first one met ends it, and the
/// report says which.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Caps {
    /// Stages run, in all.
    pub stages: u32,
    /// A loop's cycles.
    pub cycles: u32,
    /// Minutes from Start, counted at each stage's start.
    pub minutes: u32,
    /// No stage starts once the account's five-hour window is this full;
    /// the pipeline waits for it to reset instead.
    pub five_hour: u8,
    /// Lines a stage may change (added and removed). One over ends the
    /// pipeline: a step that rewrote half the tree is for the reader to look at.
    pub diff_lines: u32,
}

impl Default for Caps {
    fn default() -> Caps {
        Caps {
            stages: 12,
            cycles: 6,
            minutes: 240,
            five_hour: 80,
            diff_lines: 1500,
        }
    }
}

impl Caps {
    fn ok(&self) -> bool {
        (1..=100).contains(&self.stages)
            && (1..=100).contains(&self.cycles)
            && (5..=24 * 60).contains(&self.minutes)
            && (10..=100).contains(&self.five_hour)
            && (10..=100_000).contains(&self.diff_lines)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Step {
    pub id: i64,
    pub n: i64,
    /// `reader` or `planner`: every step says who wrote it.
    pub author: String,
    pub kind: String,
    pub text: String,
    /// The planner's reason for this step; empty for the reader's.
    pub why: String,
    /// proposed, queued, running, done, failed, skipped.
    pub state: String,
    pub session: String,
    #[serde(skip)]
    pub transcript: String,
    /// HEAD in the worktree when the stage started: its commits are the ones
    /// after it.
    #[serde(skip)]
    pub base: String,
    /// What the stage said last, at its last Stop.
    #[serde(skip)]
    pub last_reply: String,
    /// When the stage's last turn ended, or 0 before the first.
    pub stopped_at: i64,
    /// How many times the stage was handed to a fresh session past the
    /// pipeline's context limit; below 0 while one is asked for
    /// (`step_handoff`).
    pub handoff: i64,
    pub result: String,
    pub ctx_peak: i64,
    /// `abc1234 subject` per line, oldest first.
    pub commits: String,
    pub diff_lines: i64,
    pub created_at: i64,
    pub started_at: i64,
    pub ended_at: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Pipeline {
    pub id: i64,
    pub desk_id: i64,
    pub pane: String,
    pub name: String,
    pub kind: String,
    pub goal: String,
    pub rules: String,
    pub autopilot: bool,
    pub planner_model: String,
    pub ctx_limit: i64,
    pub caps: Caps,
    pub branch: String,
    pub folder: String,
    #[serde(skip)]
    pub base: String,
    pub state: String,
    pub why: String,
    pub resume_at: i64,
    pub report: String,
    pub created_at: i64,
    pub started_at: i64,
    pub ended_at: i64,
    pub hidden_at: i64,
    pub steps: Vec<Step>,
}

impl Pipeline {
    /// The step running now, if one is.
    pub fn running(&self) -> Option<&Step> {
        self.steps.iter().find(|s| s.state == "running")
    }
    /// The step the reader's Go starts, if the planner left one.
    pub fn proposed(&self) -> Option<&Step> {
        self.steps.iter().find(|s| s.state == "proposed")
    }
    /// The next step in line.
    pub fn queued(&self) -> Option<&Step> {
        self.steps.iter().find(|s| s.state == "queued")
    }
    /// Stages that ran, to their end or not.
    pub fn ran(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| matches!(s.state.as_str(), "done" | "failed" | "running"))
            .count()
    }
    pub fn active(&self) -> bool {
        ACTIVE.contains(&self.state.as_str())
    }
}

/// A step as the reader writes it. `id` names one already in the list, when
/// the editor sends the list back.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct NewStep {
    pub id: Option<i64>,
    pub kind: String,
    pub text: String,
}

/// What the editor sends.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct New {
    pub name: String,
    pub kind: String,
    /// The goal (goal), or the step that repeats (loop).
    pub goal: String,
    /// What every stage and the planner are told: "commit each stage, run
    /// the benches, never touch ui/vendor".
    pub rules: String,
    pub steps: Vec<NewStep>,
    pub autopilot: bool,
    pub planner_model: String,
    pub ctx_limit: Option<i64>,
    pub caps: Option<Caps>,
}

#[derive(Debug, PartialEq)]
pub enum Made {
    New(Box<Pipeline>),
    /// What is wrong, in a sentence for the editor.
    Bad(&'static str),
    /// The panel already has one running, waiting or paused.
    Busy,
    NoSuchDesk,
}

/// The branch a pipeline's name makes: lower case letters, digits and
/// single dashes, at most 40 of them. Empty when nothing of the name is
/// left, which is a name refused.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
        if out.len() >= 40 {
            break;
        }
    }
    out.trim_end_matches('-').to_string()
}

fn check(n: &New) -> Result<(), &'static str> {
    let name = n.name.trim();
    if name.is_empty() || name.chars().count() > NAME_CHARS || slug(name).is_empty() {
        return Err("a name is one to sixty characters, with a letter or a digit in it");
    }
    if !KINDS.contains(&n.kind.as_str()) {
        return Err("a pipeline is a list, a loop or a goal");
    }
    if n.goal.len() > GOAL_BYTES || n.rules.len() > GOAL_BYTES {
        return Err("the goal and the rules are 4,000 bytes each at most");
    }
    if matches!(n.kind.as_str(), "loop" | "goal") && n.goal.trim().is_empty() {
        return Err(if n.kind == "loop" {
            "a loop needs the step it repeats"
        } else {
            "a goal pipeline needs its goal"
        });
    }
    if n.kind == "list" && n.steps.iter().all(|s| s.text.trim().is_empty()) {
        return Err("a list needs a step");
    }
    if n.steps.len() > STEPS_MAX {
        return Err("forty steps at most");
    }
    for s in &n.steps {
        if s.text.len() > STEP_BYTES {
            return Err("a step is 8,000 bytes at most");
        }
        if !s.kind.is_empty() && !STEP_KINDS.contains(&s.kind.as_str()) {
            return Err("a step is a prompt or a goal");
        }
        if s.text.contains('\0') {
            return Err("a step cannot hold a NUL");
        }
    }
    if n.goal.contains('\0') {
        return Err("the goal cannot hold a NUL");
    }
    if !n.planner_model.is_empty() && !MODELS.contains(&n.planner_model.as_str()) {
        return Err("the planner is sonnet, haiku or opus");
    }
    if n.ctx_limit.is_some_and(|c| !(20..=95).contains(&c)) {
        return Err("the context limit is 20 to 95 percent");
    }
    if n.caps.is_some_and(|c| !c.ok()) {
        return Err("a cap is out of its range");
    }
    Ok(())
}

const COLS: &str = "id, desk_id, pane, name, kind, goal, rules, autopilot, planner_model, ctx_limit, caps, branch, folder, base, state, why, resume_at, report, created_at, started_at, ended_at, hidden_at";
const STEP_COLS: &str = "id, n, author, kind, text, why, state, session, transcript, base, last_reply, stopped_at, handoff, result, ctx_peak, commits, diff_lines, created_at, started_at, ended_at";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Pipeline> {
    let caps: String = r.get(10)?;
    Ok(Pipeline {
        id: r.get(0)?,
        desk_id: r.get(1)?,
        pane: r.get(2)?,
        name: r.get(3)?,
        kind: r.get(4)?,
        goal: r.get(5)?,
        rules: r.get(6)?,
        autopilot: r.get::<_, i64>(7)? != 0,
        planner_model: r.get(8)?,
        ctx_limit: r.get(9)?,
        caps: serde_json::from_str(&caps).unwrap_or_default(),
        branch: r.get(11)?,
        folder: r.get(12)?,
        base: r.get(13)?,
        state: r.get(14)?,
        why: r.get(15)?,
        resume_at: r.get(16)?,
        report: r.get(17)?,
        created_at: r.get(18)?,
        started_at: r.get(19)?,
        ended_at: r.get(20)?,
        hidden_at: r.get(21)?,
        steps: Vec::new(),
    })
}

fn step_row(r: &rusqlite::Row) -> rusqlite::Result<Step> {
    Ok(Step {
        id: r.get(0)?,
        n: r.get(1)?,
        author: r.get(2)?,
        kind: r.get(3)?,
        text: r.get(4)?,
        why: r.get(5)?,
        state: r.get(6)?,
        session: r.get(7)?,
        transcript: r.get(8)?,
        base: r.get(9)?,
        last_reply: r.get(10)?,
        stopped_at: r.get(11)?,
        handoff: r.get(12)?,
        result: r.get(13)?,
        ctx_peak: r.get(14)?,
        commits: r.get(15)?,
        diff_lines: r.get(16)?,
        created_at: r.get(17)?,
        started_at: r.get(18)?,
        ended_at: r.get(19)?,
    })
}

fn with_steps(conn: &Connection, mut p: Pipeline) -> Result<Pipeline> {
    let mut st = conn.prepare(&format!(
        "SELECT {STEP_COLS} FROM pipeline_steps WHERE pipeline_id = ?1 ORDER BY n, id"
    ))?;
    p.steps = st
        .query_map(params![p.id], step_row)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(p)
}

/// One pipeline of a desk, with its steps.
pub fn get(conn: &Connection, desk_id: i64, id: i64) -> Result<Option<Pipeline>> {
    let p = conn
        .query_row(
            &format!("SELECT {COLS} FROM pipelines WHERE id = ?1 AND desk_id = ?2"),
            params![id, desk_id],
            row,
        )
        .optional()?;
    p.map(|p| with_steps(conn, p)).transpose()
}

/// One pipeline by its id alone: the runner's, which has no desk to hand.
pub fn by_id(conn: &Connection, id: i64) -> Result<Option<Pipeline>> {
    let p = conn
        .query_row(
            &format!("SELECT {COLS} FROM pipelines WHERE id = ?1"),
            params![id],
            row,
        )
        .optional()?;
    p.map(|p| with_steps(conn, p)).transpose()
}

/// A desk's pipelines on view, newest first, and the ones put away in the
/// last day, for their Undo.
pub fn for_desk(conn: &Connection, desk_id: i64, now: i64) -> Result<Vec<Pipeline>> {
    let mut st = conn.prepare(&format!(
        "SELECT {COLS} FROM pipelines WHERE desk_id = ?1 AND (hidden_at = 0 OR hidden_at > ?2)
         ORDER BY created_at DESC, id DESC LIMIT 30"
    ))?;
    let ps: Vec<Pipeline> = st
        .query_map(params![desk_id, now - 86_400], row)?
        .collect::<rusqlite::Result<_>>()?;
    ps.into_iter().map(|p| with_steps(conn, p)).collect()
}

/// The pipeline a panel is working through now, if any.
pub fn of_pane(conn: &Connection, pane: &str) -> Result<Option<Pipeline>> {
    let p = conn
        .query_row(
            &format!(
                "SELECT {COLS} FROM pipelines WHERE pane = ?1 AND state IN ('running', 'waiting', 'paused')
                 ORDER BY id DESC LIMIT 1"
            ),
            params![pane],
            row,
        )
        .optional()?;
    p.map(|p| with_steps(conn, p)).transpose()
}

/// Every pipeline the runner looks at: running, or paused with a time to
/// go on at.
pub fn live(conn: &Connection) -> Result<Vec<Pipeline>> {
    let mut st = conn.prepare(&format!(
        "SELECT {COLS} FROM pipelines WHERE state = 'running' OR (state = 'paused' AND resume_at > 0)"
    ))?;
    let ps: Vec<Pipeline> = st.query_map([], row)?.collect::<rusqlite::Result<_>>()?;
    ps.into_iter().map(|p| with_steps(conn, p)).collect()
}

/// A daemon that stopped with pipelines running cannot say where their
/// stages got to: each is paused, and says why, for the reader's Resume.
pub fn pause_all(conn: &Connection) -> Result<usize> {
    Ok(conn.execute(
        "UPDATE pipelines SET state = 'paused', why = 'snyvi restarted', resume_at = 0
         WHERE state = 'running'",
        [],
    )?)
}

fn step_kind(k: &str) -> &str {
    if k == "goal" {
        "goal"
    } else {
        "prompt"
    }
}

/// A new pipeline on a panel, as a draft: nothing runs until Start.
pub fn create(conn: &mut Connection, desk_id: i64, pane: &str, n: &New, now: i64) -> Result<Made> {
    if let Err(why) = check(n) {
        return Ok(Made::Bad(why));
    }
    let tx = conn.transaction()?;
    let desk: Option<i64> = tx
        .query_row(
            "SELECT id FROM desks WHERE id = ?1",
            params![desk_id],
            |r| r.get(0),
        )
        .optional()?;
    if desk.is_none() {
        return Ok(Made::NoSuchDesk);
    }
    let busy: i64 = tx.query_row(
        "SELECT COUNT(*) FROM pipelines WHERE pane = ?1 AND state IN ('running', 'waiting', 'paused')",
        params![pane],
        |r| r.get(0),
    )?;
    if busy > 0 {
        return Ok(Made::Busy);
    }
    tx.execute(
        "INSERT INTO pipelines (desk_id, pane, name, kind, goal, rules, autopilot, planner_model, ctx_limit, caps, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            desk_id,
            pane,
            n.name.trim(),
            n.kind,
            n.goal.trim(),
            n.rules.trim(),
            n.autopilot as i64,
            if n.planner_model.is_empty() { "sonnet" } else { n.planner_model.as_str() },
            n.ctx_limit.unwrap_or(70),
            serde_json::to_string(&n.caps.unwrap_or_default())?,
            now,
        ],
    )?;
    let id = tx.last_insert_rowid();
    for s in n.steps.iter().filter(|s| !s.text.trim().is_empty()) {
        insert_step(
            &tx,
            id,
            "reader",
            step_kind(&s.kind),
            s.text.trim(),
            "",
            "queued",
            now,
        )?;
    }
    tx.commit()?;
    Ok(match by_id(conn, id)? {
        Some(p) => Made::New(Box::new(p)),
        None => Made::NoSuchDesk,
    })
}

#[derive(Debug, PartialEq)]
pub enum Edited {
    Saved(Box<Pipeline>),
    Bad(&'static str),
    /// Running: pause it first, so a stage is never edited out from under
    /// the session doing it.
    Running,
    NotFound,
}

/// The editor's Save on a pipeline that is not running. Its queued steps
/// become the list sent; one left out is skipped, never removed, and a step
/// that ran is as it was.
pub fn edit(conn: &mut Connection, desk_id: i64, id: i64, n: &New, now: i64) -> Result<Edited> {
    if let Err(why) = check(n) {
        return Ok(Edited::Bad(why));
    }
    let Some(p) = get(conn, desk_id, id)? else {
        return Ok(Edited::NotFound);
    };
    if p.state == "running" {
        return Ok(Edited::Running);
    }
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE pipelines SET name = ?2, kind = ?3, goal = ?4, rules = ?5, autopilot = ?6,
                planner_model = ?7, ctx_limit = ?8, caps = ?9
         WHERE id = ?1",
        params![
            id,
            n.name.trim(),
            n.kind,
            n.goal.trim(),
            n.rules.trim(),
            n.autopilot as i64,
            if n.planner_model.is_empty() {
                "sonnet"
            } else {
                n.planner_model.as_str()
            },
            n.ctx_limit.unwrap_or(p.ctx_limit),
            serde_json::to_string(&n.caps.unwrap_or(p.caps))?,
        ],
    )?;
    let kept: Vec<i64> = n.steps.iter().filter_map(|s| s.id).collect();
    for s in p
        .steps
        .iter()
        .filter(|s| s.state == "queued" && !kept.contains(&s.id))
    {
        tx.execute(
            "UPDATE pipeline_steps SET state = 'skipped', why = 'left out in an edit', ended_at = ?2 WHERE id = ?1",
            params![s.id, now],
        )?;
    }
    for s in n.steps.iter().filter(|s| !s.text.trim().is_empty()) {
        match s
            .id
            .filter(|i| p.steps.iter().any(|x| x.id == *i && x.state == "queued"))
        {
            Some(sid) => {
                tx.execute(
                    "UPDATE pipeline_steps SET text = ?2, kind = ?3 WHERE id = ?1",
                    params![sid, s.text.trim(), step_kind(&s.kind)],
                )?;
            }
            None if s.id.is_none() => {
                insert_step(
                    &tx,
                    id,
                    "reader",
                    step_kind(&s.kind),
                    s.text.trim(),
                    "",
                    "queued",
                    now,
                )?;
            }
            None => {}
        }
    }
    tx.commit()?;
    Ok(match get(conn, desk_id, id)? {
        Some(p) => Edited::Saved(Box::new(p)),
        None => Edited::NotFound,
    })
}

#[allow(clippy::too_many_arguments)]
fn insert_step(
    conn: &Connection,
    pipeline: i64,
    author: &str,
    kind: &str,
    text: &str,
    why: &str,
    state: &str,
    now: i64,
) -> Result<i64> {
    let n: i64 = conn.query_row(
        "SELECT COALESCE(MAX(n), 0) + 1 FROM pipeline_steps WHERE pipeline_id = ?1",
        params![pipeline],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO pipeline_steps (pipeline_id, n, author, kind, text, why, state, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![pipeline, n, author, kind, text, why, state, now],
    )?;
    Ok(conn.last_insert_rowid())
}

/// A step the runner adds: a loop's next cycle (the reader's words again),
/// or the planner's, queued or proposed. Cut to `STEP_BYTES` on a character.
#[allow(clippy::too_many_arguments)]
pub fn add_step(
    conn: &Connection,
    pipeline: i64,
    author: &str,
    kind: &str,
    text: &str,
    why: &str,
    state: &str,
    now: i64,
) -> Result<i64> {
    let text = cut(text.trim(), STEP_BYTES);
    let why = cut(why.trim(), 600);
    insert_step(
        conn,
        pipeline,
        author,
        step_kind(kind),
        &text,
        &why,
        state,
        now,
    )
}

/// At most `bytes` of `s`, on a character's edge.
pub fn cut(s: &str, bytes: usize) -> String {
    if s.len() <= bytes {
        return s.to_string();
    }
    let mut end = bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

/// The last `chars` characters of `s`: a reply's end is where its summary is.
pub fn tail(s: &str, chars: usize) -> String {
    let n = s.chars().count();
    if n <= chars {
        return s.to_string();
    }
    let skip = n - chars;
    format!("…{}", s.chars().skip(skip).collect::<String>())
}

pub fn set_state(conn: &Connection, id: i64, state: &str, why: &str, now: i64) -> Result<()> {
    let ended = matches!(state, "done" | "stopped");
    conn.execute(
        "UPDATE pipelines SET state = ?2, why = ?3, resume_at = 0,
                ended_at = CASE WHEN ?4 THEN ?5 ELSE ended_at END
         WHERE id = ?1",
        params![id, state, cut(why, 300), ended, now],
    )?;
    Ok(())
}

/// Paused until a limit's window resets: the runner goes on by itself then.
pub fn pause_until(conn: &Connection, id: i64, why: &str, at: i64) -> Result<()> {
    conn.execute(
        "UPDATE pipelines SET state = 'paused', why = ?2, resume_at = ?3 WHERE id = ?1",
        params![id, cut(why, 300), at],
    )?;
    Ok(())
}

/// Start: the worktree it runs in, and when it began.
pub fn started(
    conn: &Connection,
    id: i64,
    folder: &str,
    branch: &str,
    base: &str,
    now: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE pipelines SET state = 'running', why = '', resume_at = 0, folder = ?2, branch = ?3,
                base = CASE WHEN base = '' THEN ?4 ELSE base END,
                started_at = CASE WHEN started_at = 0 THEN ?5 ELSE started_at END
         WHERE id = ?1",
        params![id, folder, branch, base, now],
    )?;
    Ok(())
}

pub fn set_report(conn: &Connection, id: i64, doc: &str) -> Result<()> {
    conn.execute(
        "UPDATE pipelines SET report = ?2 WHERE id = ?1",
        params![id, doc],
    )?;
    Ok(())
}

/// The reader's ✕, and its Undo.
pub fn hide(conn: &Connection, desk_id: i64, id: i64, now: i64) -> Result<bool> {
    Ok(conn.execute(
        "UPDATE pipelines SET hidden_at = ?3 WHERE id = ?1 AND desk_id = ?2 AND state NOT IN ('running', 'waiting')",
        params![id, desk_id, now],
    )? > 0)
}

pub fn unhide(conn: &Connection, desk_id: i64, id: i64) -> Result<bool> {
    Ok(conn.execute(
        "UPDATE pipelines SET hidden_at = 0 WHERE id = ?1 AND desk_id = ?2",
        params![id, desk_id],
    )? > 0)
}

/// A stage starts: its session is not known until its first hook.
pub fn step_began(conn: &Connection, step: i64, base: &str, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE pipeline_steps SET state = 'running', base = CASE WHEN base = '' THEN ?2 ELSE base END,
                started_at = CASE WHEN started_at = 0 THEN ?3 ELSE started_at END,
                session = '', transcript = '', stopped_at = 0
         WHERE id = ?1",
        params![step, base, now],
    )?;
    Ok(())
}

/// A turn of the stage ended: its session, its transcript, what it said.
pub fn step_stopped(
    conn: &Connection,
    step: i64,
    session: &str,
    transcript: &str,
    reply: &str,
    now: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE pipeline_steps SET session = ?2, transcript = ?3, last_reply = ?4, stopped_at = ?5 WHERE id = ?1",
        params![step, session, transcript, tail(reply, RESULT_CHARS), now],
    )?;
    Ok(())
}

/// The fullest the stage's context got, as its status line said.
pub fn step_peak(conn: &Connection, step: i64, pct: i64) -> Result<()> {
    conn.execute(
        "UPDATE pipeline_steps SET ctx_peak = MAX(ctx_peak, ?2) WHERE id = ?1",
        params![step, pct],
    )?;
    Ok(())
}

/// A handoff asked for (`asked`): the stage is writing where it is, and
/// `handoff` goes from `k` fresh sessions so far to `-(k + 1)`, its last
/// Stop forgotten so the next one is the handoff's own. Then taken:
/// the stage goes on in a fresh session, and it is `k + 1`.
pub fn step_handoff(conn: &Connection, step: i64, asked: bool) -> Result<()> {
    conn.execute(
        if asked {
            "UPDATE pipeline_steps SET handoff = -(handoff + 1), stopped_at = 0 WHERE id = ?1 AND handoff >= 0"
        } else {
            "UPDATE pipeline_steps SET handoff = -handoff, stopped_at = 0 WHERE id = ?1 AND handoff < 0"
        },
        params![step],
    )?;
    Ok(())
}

/// A stage whose turn failed -- an error, a goal Claude Code cleared --
/// goes back in line: Resume starts it again in a fresh session, keeping
/// the commit it started from, so its commits are still all its own.
pub fn requeue_step(conn: &Connection, step: i64) -> Result<()> {
    conn.execute(
        "UPDATE pipeline_steps SET state = 'queued', stopped_at = 0,
                handoff = CASE WHEN handoff < 0 THEN -handoff - 1 ELSE handoff END
         WHERE id = ?1 AND state = 'running'",
        params![step],
    )?;
    Ok(())
}

/// How a stage ended.
pub struct End<'a> {
    pub state: &'a str,
    pub result: &'a str,
    pub commits: &'a str,
    pub diff_lines: i64,
}

pub fn step_ended(conn: &Connection, step: i64, e: &End, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE pipeline_steps SET state = ?2, result = ?3, commits = ?4, diff_lines = ?5, ended_at = ?6,
                handoff = CASE WHEN handoff < 0 THEN -handoff - 1 ELSE handoff END
         WHERE id = ?1",
        params![step, e.state, tail(e.result, RESULT_CHARS), e.commits, e.diff_lines, now],
    )?;
    Ok(())
}

/// The reader's Go on a proposed step, or their Skip. A skipped step stays
/// in the list as skipped; the planner sees that it was.
pub fn settle_step(
    conn: &Connection,
    desk_id: i64,
    id: i64,
    step: i64,
    go: bool,
    now: i64,
) -> Result<bool> {
    Ok(conn.execute(
        "UPDATE pipeline_steps SET state = ?3, ended_at = CASE WHEN ?3 = 'skipped' THEN ?4 ELSE 0 END
         WHERE id = ?2 AND pipeline_id = ?1 AND state IN ('proposed', 'queued')
           AND EXISTS (SELECT 1 FROM pipelines WHERE id = ?1 AND desk_id = ?5)",
        params![id, step, if go { "queued" } else { "skipped" }, now, desk_id],
    )? > 0)
}

/// What a stage is started with: the step, and what the pipeline's rules add
/// to every one. A goal step is Claude Code's `/goal`, which sets the
/// condition and starts the turn at once.
pub fn stage_prompt(p: &Pipeline, s: &Step, left_off: Option<&str>) -> String {
    let mut out = String::new();
    if s.kind == "goal" {
        out.push_str("/goal ");
        out.push_str(s.text.trim());
        return out;
    }
    out.push_str(s.text.trim());
    out.push_str("\n\n---\n");
    out.push_str(&format!(
        "This is stage {} of the snyvi pipeline \"{}\", in the worktree on branch {}. ",
        s.n, p.name, p.branch
    ));
    out.push_str(&format!(
        "Commit your own work on this branch when the step is done, with a message starting \"pipeline {} #{}: \". Never push, merge or release.",
        slug(&p.name),
        s.n
    ));
    if p.kind == "loop" {
        out.push_str(&format!(
            " If there is nothing left worth doing, change nothing and end your reply with the line {NOTHING_LEFT}."
        ));
    }
    if !p.rules.trim().is_empty() {
        out.push_str("\nRules: ");
        out.push_str(p.rules.trim());
    }
    if let Some(l) = left_off.filter(|l| !l.trim().is_empty()) {
        out.push_str("\nAn earlier session on this step ran out of room. Where it left off: ");
        out.push_str(l.trim());
    }
    out
}

/// The report: what each stage was asked, who asked it, what came of it.
/// A document, sent to the desk when the pipeline ends.
pub fn report(p: &Pipeline) -> String {
    let mut md = format!("# Pipeline: {}\n\n", p.name);
    md.push_str(&format!(
        "**{}** · {} · {} · branch `{}` in `{}`\n\n",
        p.state,
        p.kind,
        if p.autopilot {
            "autopilot"
        } else {
            "Go on each planner step"
        },
        p.branch,
        p.folder
    ));
    if !p.why.is_empty() {
        md.push_str(&format!("**Why it ended:** {}\n\n", p.why));
    }
    if !p.goal.is_empty() {
        md.push_str(&format!(
            "**{}:** {}\n\n",
            if p.kind == "loop" { "Step" } else { "Goal" },
            p.goal
        ));
    }
    md.push_str(
        "| # | by | step | state | context | commits | lines |\n|---|---|---|---|---|---|---|\n",
    );
    for s in &p.steps {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {}% | {} | {} |\n",
            s.n,
            s.author,
            cell(&s.text, 80),
            s.state,
            s.ctx_peak,
            s.commits.lines().count(),
            s.diff_lines
        ));
    }
    for s in p.steps.iter().filter(|s| s.state != "queued") {
        md.push_str(&format!("\n## {}. {}\n\n", s.n, cell(&s.text, 100)));
        if !s.why.is_empty() {
            md.push_str(&format!("*The planner:* {}\n\n", s.why));
        }
        if !s.session.is_empty() {
            md.push_str(&format!(
                "Session `{}` (`claude --resume {}`)\n\n",
                s.session, s.session
            ));
        }
        if !s.commits.is_empty() {
            md.push_str("```\n");
            md.push_str(&s.commits);
            md.push_str("\n```\n\n");
        }
        if !s.result.is_empty() {
            for line in s.result.lines() {
                md.push_str("> ");
                md.push_str(line);
                md.push('\n');
            }
        }
    }
    md
}

/// One line of a table cell: no pipes, no newlines, cut short.
fn cell(s: &str, chars: usize) -> String {
    let one: String = s
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "/");
    if one.chars().count() <= chars {
        one
    } else {
        format!("{}…", one.chars().take(chars).collect::<String>())
    }
}
