//! A panel's cards: the small things an agent puts above the rail's foot
//! while it works -- a place to go (a preview, a PR, a deploy) or a word on
//! where the work is ("building 3/7") -- each for as long as it says
//! (#162, MCP `show_card`). They are the panel's passing word, where a
//! widget is the desk's lasting figure: kept in the pane's status, in
//! memory, and gone with the session that put them there.
//!
//! A card only shows. A link opens on the reader's click, in their browser,
//! and nothing a card carries is ever typed anywhere.

use serde::Serialize;
use serde_json::Value;

/// How many a panel keeps; the rail shows the newest three and counts the rest.
pub const KEEP: usize = 6;
const ID_MAX: usize = 32;
const LABEL_MAX: usize = 32;
const TEXT_MAX: usize = 80;
const URL_MAX: usize = 2048;
/// A lifetime, in seconds, when one is given: long enough to be read, short
/// enough that "for" never means "forever" (that is the panel's, `for: 0`).
const LIFE_MIN: i64 = 10;
const LIFE_MAX: i64 = 3600;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Card {
    pub id: String,
    /// `link` or `text`.
    pub kind: &'static str,
    pub label: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub url: String,
    /// The host the link goes to, written by snyvi and not the agent: what
    /// the card shows beside the label, so a label cannot pass for a place.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub host: String,
    #[serde(skip_serializing_if = "str::is_empty")]
    pub tone: &'static str,
    /// When it was put, and how many seconds it lasts from then; 0 is while
    /// the session runs. An update in place keeps `at` and restarts nothing
    /// unless it brings a lifetime of its own.
    pub at: i64,
    #[serde(rename = "for", skip_serializing_if = "is_zero")]
    pub life: i64,
}

fn is_zero(n: &i64) -> bool {
    *n == 0
}

impl Card {
    pub fn live(&self, now: i64) -> bool {
        self.life == 0 || now < self.at + self.life
    }
}

/// What an agent asked: a card to put, or one to take away.
#[derive(Debug, PartialEq)]
pub enum Op {
    Put(Card),
    Clear(String),
}

/// The body of `POST /api/panes/{id}/card`, checked: `{id?, kind, label,
/// url?, tone?, for?}` puts one, `{id, clear: true}` takes it away. The
/// reason it was refused otherwise, in words the agent reads.
pub fn parse(b: &Value, now: i64) -> Result<Op, String> {
    let s = |k: &str| b.get(k).and_then(Value::as_str).unwrap_or("").trim();
    let id = s("id");
    if id.len() > ID_MAX
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!("id is at most {ID_MAX} letters, digits, - and _"));
    }
    if b.get("clear").and_then(Value::as_bool) == Some(true) {
        return if id.is_empty() {
            Err("clear needs the card's id".into())
        } else {
            Ok(Op::Clear(id.into()))
        };
    }
    let kind = match s("kind") {
        "link" => "link",
        "text" | "" => "text",
        _ => return Err("kind is link or text".into()),
    };
    let label = one_line(s("label"));
    let max = if kind == "link" { LABEL_MAX } else { TEXT_MAX };
    if label.is_empty() {
        return Err("a card needs a label".into());
    }
    if label.chars().count() > max {
        return Err(format!("a {kind} card's label is at most {max} characters"));
    }
    let (url, host) = if kind == "link" {
        link(s("url"))?
    } else {
        (String::new(), String::new())
    };
    let tone = match s("tone") {
        "ok" => "ok",
        "warn" => "warn",
        "bad" => "bad",
        "run" => "run",
        "" | "none" => "",
        _ => return Err("tone is ok, warn, bad, run or none".into()),
    };
    let life = match b.get("for") {
        None | Some(Value::Null) => 0,
        Some(Value::String(p)) if p == "panel" => 0,
        Some(v) => match v.as_i64() {
            Some(n) if (LIFE_MIN..=LIFE_MAX).contains(&n) => n,
            _ => {
                return Err(format!(
                    "for is seconds, {LIFE_MIN} to {LIFE_MAX}, or \"panel\""
                ))
            }
        },
    };
    Ok(Op::Put(Card {
        id: id.into(),
        kind,
        label,
        url,
        host,
        tone,
        at: now,
        life,
    }))
}

/// A link a card may carry: the web, or this machine (a dev server, a test
/// window). Never `javascript:`, `data:`, `file:` or anything else a click
/// could run or read. The URL as given, and the host to show.
pub(crate) fn link(raw: &str) -> Result<(String, String), String> {
    if raw.is_empty() {
        return Err("a link card needs a url".into());
    }
    if raw.len() > URL_MAX || raw.chars().any(char::is_control) || raw.contains(char::is_whitespace)
    {
        return Err("the url is one line, without spaces".into());
    }
    let rest = raw
        .strip_prefix("https://")
        .or_else(|| raw.strip_prefix("http://"))
        .ok_or("a card links to http or https only")?;
    // The authority, without a user:password@ in front of it: what the card
    // shows is where the click goes.
    let auth = rest.split(['/', '?', '#']).next().unwrap_or("");
    if auth.contains('@') {
        return Err("the url may not carry a user or password".into());
    }
    if auth.is_empty() {
        return Err("the url has no host".into());
    }
    Ok((raw.into(), auth.to_ascii_lowercase()))
}

/// Tabs, newlines and the like as one space each: a card is one line.
pub(crate) fn one_line(s: &str) -> String {
    s.split(|c: char| c.is_control() || c == '\u{2028}' || c == '\u{2029}')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `op` applied to a panel's cards at `now`. A card with the id of one there
/// replaces it where it stands -- a count going 3/7, 4/7 does not rise again
/// -- keeping when it was first put unless it brings a lifetime of its own.
/// An id-less card gets one. The ones whose time is up go first, then the
/// oldest past `KEEP`. Whether anything changed.
pub fn apply(cards: &mut Vec<Card>, op: Op, now: i64) -> bool {
    let before = cards.clone();
    cards.retain(|c| c.live(now));
    match op {
        Op::Clear(id) => cards.retain(|c| c.id != id),
        Op::Put(mut c) => {
            if c.id.is_empty() {
                c.id = (1..)
                    .map(|n| format!("c{n}"))
                    .find(|id| !cards.iter().any(|o| &o.id == id))
                    .unwrap_or_default();
            }
            match cards.iter_mut().find(|o| o.id == c.id) {
                Some(o) => {
                    // An update with no lifetime of its own keeps the one
                    // running, and when the card was first put.
                    if c.life == 0 {
                        c.at = o.at;
                        c.life = o.life;
                    }
                    *o = c;
                }
                None => {
                    cards.push(c);
                    let over = cards.len().saturating_sub(KEEP);
                    cards.drain(..over);
                }
            }
        }
    }
    *cards != before
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn put(b: Value) -> Card {
        match parse(&b, 100).unwrap() {
            Op::Put(c) => c,
            o => panic!("{o:?}"),
        }
    }

    #[test]
    fn a_link_card_goes_to_the_web_or_this_machine_and_nowhere_else() {
        let c =
            put(json!({ "kind": "link", "label": "preview", "url": "http://127.0.0.1:7801/x" }));
        assert_eq!(c.host, "127.0.0.1:7801");
        assert_eq!(
            put(json!({ "kind": "link", "label": "PR", "url": "https://GitHub.com/o/r/pull/86" }))
                .host,
            "github.com"
        );
        for bad in [
            "javascript:alert(1)",
            "data:text/html,hi",
            "file:///etc/passwd",
            "https://user:pw@example.com/",
            "https://",
            "https://a.b/x y",
            "https://a.b/\nx",
            "ftp://a.b",
        ] {
            assert!(
                parse(&json!({ "kind": "link", "label": "x", "url": bad }), 0).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn a_card_is_one_short_line_with_a_kind_a_tone_and_a_lifetime() {
        assert_eq!(
            put(json!({ "label": "building\n3/7" })).label,
            "building 3/7"
        );
        assert!(parse(&json!({ "label": "" }), 0).is_err());
        assert!(parse(
            &json!({ "kind": "link", "label": "x".repeat(33), "url": "https://a.b" }),
            0
        )
        .is_err());
        assert!(parse(&json!({ "label": "x".repeat(81) }), 0).is_err());
        assert!(parse(&json!({ "kind": "box", "label": "x" }), 0).is_err());
        assert!(parse(&json!({ "label": "x", "tone": "loud" }), 0).is_err());
        assert!(parse(&json!({ "label": "x", "for": 5 }), 0).is_err());
        assert!(parse(&json!({ "label": "x", "for": 7200 }), 0).is_err());
        assert!(parse(&json!({ "id": "a b", "label": "x" }), 0).is_err());
        assert_eq!(put(json!({ "label": "x", "for": 30 })).life, 30);
        assert_eq!(put(json!({ "label": "x", "for": "panel" })).life, 0);
        assert_eq!(
            parse(&json!({ "id": "b", "clear": true }), 0).unwrap(),
            Op::Clear("b".into())
        );
        assert!(parse(&json!({ "clear": true }), 0).is_err());
    }

    #[test]
    fn the_same_id_updates_in_place_and_the_oldest_leave_past_six() {
        let mut cards = vec![];
        let at = |b: Value, now| parse(&b, now).unwrap();
        assert!(apply(
            &mut cards,
            at(json!({ "id": "build", "label": "1/7" }), 10),
            10
        ));
        assert!(apply(&mut cards, at(json!({ "label": "other" }), 11), 11));
        assert!(apply(
            &mut cards,
            at(json!({ "id": "build", "label": "2/7" }), 12),
            12
        ));
        assert_eq!(cards[0].label, "2/7");
        assert_eq!(cards[0].at, 10, "an update keeps when it was first put");
        assert_eq!(cards[1].id, "c1");
        assert!(
            !apply(
                &mut cards,
                at(json!({ "id": "build", "label": "2/7" }), 13),
                13
            ),
            "the same again changes nothing"
        );
        for n in 0..6 {
            apply(&mut cards, at(json!({ "label": format!("n{n}") }), 20), 20);
        }
        assert_eq!(cards.len(), KEEP);
        assert!(
            !cards.iter().any(|c| c.id == "build"),
            "the oldest went first"
        );
        assert!(apply(&mut cards, Op::Clear("c3".into()), 21));
        assert!(!cards.iter().any(|c| c.id == "c3"));
    }

    #[test]
    fn a_card_with_a_lifetime_is_gone_once_it_has_run() {
        let mut cards = vec![];
        apply(
            &mut cards,
            parse(
                &json!({ "id": "t", "label": "tests passed", "for": 30 }),
                100,
            )
            .unwrap(),
            100,
        );
        assert!(cards[0].live(129) && !cards[0].live(130));
        // A count updated without a lifetime keeps the one running.
        apply(
            &mut cards,
            parse(&json!({ "id": "t", "label": "still" }), 110).unwrap(),
            110,
        );
        assert_eq!((cards[0].at, cards[0].life), (100, 30));
        assert!(
            apply(&mut cards, Op::Clear("nothing".into()), 131),
            "the expired one is dropped on the next write"
        );
        assert!(cards.is_empty());
    }
}
