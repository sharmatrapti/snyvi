//! Where a stage's `/goal` stands, read from its transcript.
//!
//! Claude Code's Stop hook says nothing of a goal (spike S1): the judge's
//! verdicts are written to the session's transcript, as attachments --
//!
//! - `{"type":"goal_status","met":false,"sentinel":true,"condition":…}` when
//!   the goal is set;
//! - `{"type":"goal_status","met":true,"condition":…,"reason":…}` when the
//!   judge finds it met;
//! - `met:true` with `sentinel:true` when it is taken off, and after an error
//!   no turn can get past (auth, credits), a `system`/`informational` entry
//!   "Goal cleared after an unrecoverable error (…)".
//!
//! So the runner reads the transcript, from where it last stopped reading to
//! the end, and keeps the newest of those. It parses only the lines that
//! carry one -- a transcript is megabytes of tool output -- and only whole
//! lines: the one being written is read on the next look.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Where the goal of a session stands.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Goal {
    /// No goal was set in what has been read.
    #[default]
    None,
    /// Set, and not met yet: Claude Code keeps the turn going.
    Active,
    /// The judge found it met, and why.
    Met(String),
    /// Taken off without a verdict: `/goal clear`, or the reader's Esc.
    Ended,
    /// Cleared after an error no retry gets past; the entry's words.
    Cleared(String),
}

impl Goal {
    pub fn active(&self) -> bool {
        *self == Goal::Active
    }
}

/// The most read at one look. A stage writes far less than this between two
/// looks; one that wrote more is read on, from where this one stopped.
const LOOK_MAX: u64 = 4 << 20;

/// What `scan` takes back: how far the file was read, in bytes, and the goal
/// as of there.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Seen {
    pub offset: u64,
    pub goal: Goal,
}

/// Read on from `from.offset` to the last whole line, and say where the goal
/// stands after it. A file shorter than the offset is a new one: read from
/// its start.
pub fn scan(path: &Path, from: &Seen) -> std::io::Result<Seen> {
    let mut f = std::fs::File::open(path)?;
    let len = f.metadata()?.len();
    let start = if len < from.offset { 0 } else { from.offset };
    let goal = if start == 0 {
        Goal::None
    } else {
        from.goal.clone()
    };
    f.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::new();
    f.take(LOOK_MAX).read_to_end(&mut buf)?;
    let Some(last) = buf.iter().rposition(|b| *b == b'\n') else {
        return Ok(Seen {
            offset: start,
            goal,
        });
    };
    let whole = &buf[..=last];
    Ok(Seen {
        offset: start + whole.len() as u64,
        goal: lines(whole, goal),
    })
}

/// The goal after these lines, given where it stood before them.
pub fn lines(bytes: &[u8], mut goal: Goal) -> Goal {
    for line in bytes.split(|b| *b == b'\n') {
        if let Some(g) = one(line) {
            goal = g;
        }
    }
    goal
}

/// What one line says of the goal, if anything.
fn one(line: &[u8]) -> Option<Goal> {
    let text = std::str::from_utf8(line).ok()?;
    let status = text.contains("\"goal_status\"");
    let cleared = text.contains("Goal cleared after");
    if !status && !cleared {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    if cleared && v["type"] == "system" {
        let said = v["content"].as_str().unwrap_or_default();
        if said.starts_with("Goal cleared after") {
            return Some(Goal::Cleared(said.chars().take(300).collect()));
        }
    }
    let a = &v["attachment"];
    if a["type"] != "goal_status" {
        return None;
    }
    let met = a["met"].as_bool().unwrap_or(false);
    let sentinel = a["sentinel"].as_bool().unwrap_or(false);
    Some(match (met, sentinel) {
        (false, _) => Goal::Active,
        (true, false) => Goal::Met(
            a["reason"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .take(600)
                .collect(),
        ),
        (true, true) => Goal::Ended,
    })
}

/// Whether a path a Stop hook gave is a transcript of that session, under
/// Claude Code's projects folder: `…/projects/<folder>/<session>.jsonl`.
/// The runner reads only what this lets through, so a token holder cannot
/// point it at another file.
pub fn ours(path: &str, session: &str) -> bool {
    let p = Path::new(path);
    p.is_absolute()
        && crate::desk::valid_session(session)
        && p.file_name().and_then(|n| n.to_str()) == Some(format!("{session}.jsonl").as_str())
        && p.parent()
            .and_then(Path::parent)
            .and_then(|d| d.file_name())
            .and_then(|n| n.to_str())
            == Some("projects")
        && !path.contains("/../")
        && !path.contains("\\..\\")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lines from the spike's transcripts (S1, S3), as Claude Code
    /// 2.1.300 wrote them, cut to the fields that matter.
    const SET: &str = r#"{"parentUuid":null,"type":"attachment","attachment":{"type":"goal_status","met":false,"sentinel":true,"condition":"the file done.txt exists and contains ok. Stop after 4 turns."},"sessionId":"9743f452-1df5-43f1-9661-fcbfcf1a7638"}"#;
    const MET: &str = r#"{"parentUuid":"f0d9","isSidechain":false,"attachment":{"type":"goal_status","met":true,"condition":"the file done.txt exists and contains ok.","reason":"The transcript shows the Bash call printf ok > done.txt"},"type":"attachment"}"#;
    const OFF: &str = r#"{"type":"attachment","attachment":{"type":"goal_status","met":true,"sentinel":true,"condition":"x"}}"#;
    const CLEARED: &str = r#"{"type":"system","subtype":"informational","content":"Goal cleared after an unrecoverable error (authentication failed): \"the file done.txt…\". Run /goal again to continue.","level":"warning"}"#;
    const TOOL: &str = r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"a file that mentions \"goal_status\" in passing"}]}}"#;

    fn of(lines_: &[&str]) -> Goal {
        lines(lines_.join("\n").as_bytes(), Goal::None)
    }

    #[test]
    fn the_newest_word_on_the_goal_is_where_it_stands() {
        assert_eq!(of(&[]), Goal::None);
        assert_eq!(of(&[SET]), Goal::Active);
        assert_eq!(
            of(&[SET, MET]),
            Goal::Met("The transcript shows the Bash call printf ok > done.txt".into())
        );
        assert_eq!(of(&[SET, OFF]), Goal::Ended);
        assert!(
            matches!(of(&[SET, OFF, CLEARED]), Goal::Cleared(s) if s.contains("authentication failed"))
        );
        // A tool's output that only names the word is not a verdict.
        assert_eq!(of(&[SET, TOOL]), Goal::Active);
        // A goal set again after one was met is active again.
        assert_eq!(of(&[SET, MET, SET]), Goal::Active);
    }

    #[test]
    fn a_scan_reads_on_from_where_it_stopped_and_only_whole_lines() {
        let dir = crate::store::tempdir::Dir::new("snyvi-transcript");
        let f = dir.path.join("t.jsonl");
        std::fs::write(&f, format!("{SET}\n{TOOL}\n")).unwrap();
        let a = scan(&f, &Seen::default()).unwrap();
        assert_eq!(a.goal, Goal::Active);
        assert_eq!(a.offset, std::fs::metadata(&f).unwrap().len());
        // Half a line: not read yet.
        let mut all = std::fs::read_to_string(&f).unwrap();
        all.push_str(&MET[..20]);
        std::fs::write(&f, &all).unwrap();
        let b = scan(&f, &a).unwrap();
        assert_eq!(b, a);
        all.push_str(&MET[20..]);
        all.push('\n');
        std::fs::write(&f, &all).unwrap();
        let c = scan(&f, &b).unwrap();
        assert!(matches!(c.goal, Goal::Met(_)));
        // A new, shorter file is read from its start.
        std::fs::write(&f, format!("{TOOL}\n")).unwrap();
        assert_eq!(scan(&f, &c).unwrap().goal, Goal::None);
    }

    #[test]
    fn only_a_sessions_own_transcript_is_read() {
        let s = "9743f452-1df5-43f1-9661-fcbfcf1a7638";
        // Absolute the way this platform writes it: `/home/u` has no drive
        // on Windows, and is not absolute there.
        let home = std::env::temp_dir();
        let at = |rest: &[&str]| {
            rest.iter()
                .fold(home.clone(), |p, part| p.join(part))
                .to_string_lossy()
                .into_owned()
        };
        let mine = format!("{s}.jsonl");
        assert!(ours(&at(&[".claude", "projects", "-w", &mine]), s));
        assert!(!ours(
            &at(&[".claude", "projects", "-w", &format!("{s}.json")]),
            s
        ));
        assert!(!ours(&at(&[".claude", "projects", "-w", "other.jsonl"]), s));
        assert!(!ours("/etc/passwd", s));
        assert!(!ours(&format!("relative/projects/-w/{mine}"), s));
        assert!(!ours(&at(&[".claude", "projects", "..", "x", &mine]), s));
        assert!(!ours(&at(&[".ssh", "x", &mine]), s));
    }
}
