//! A desk's links: the places its work lives on the web -- the live site, a
//! preview, a dashboard -- one click away in the rail's foot, beside the
//! repository (#162, plan 720260bb53). An agent pins one with MCP `pin_link`.
//!
//! A card (`crate::card`) is a panel's passing word and goes with its
//! session; a link is the desk's and stays until the reader takes it out of
//! view. Its ✕ only sets `hidden`, and Undo clears it: nothing is deleted,
//! and the same url pinned again does not bring back one the reader hid.
//!
//! A link only shows. It opens on the reader's click, in their browser; the
//! host beside its url is snyvi's reading of it, never the agent's word.
//!
//! Kept as JSON in one column of the desk's row: a handful per desk, read
//! with the desk and sent with it, so the page has them with everything
//! else it draws a desk from.

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 1.43: the column (`store::MIGRATIONS`).
pub const COLUMN_1_43: &str = "ALTER TABLE desks ADD COLUMN links TEXT NOT NULL DEFAULT '[]'";

/// How many a desk shows; a seventh is refused until one is taken out.
pub const SHOWN: usize = 6;
/// How many it keeps, the hidden ones with them: past this the oldest
/// hidden one makes room, since a ✕ that long ago has had its Undo.
const KEPT: usize = 24;
const LABEL_MAX: usize = 32;
const BY_MAX: usize = 60;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub id: i64,
    pub label: String,
    pub url: String,
    pub host: String,
    /// Who pinned it: the agent's name, as its client gave it.
    #[serde(default)]
    pub by: String,
    pub at: i64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// What a pin came to.
#[derive(Debug, PartialEq)]
pub enum Pinned {
    /// A new link, by its id.
    New(i64),
    /// The url was there: its label is the new one, where it stands.
    Updated(i64),
    /// The reader hid this url, and it stays hidden.
    Hidden(i64),
    /// The desk shows `SHOWN` already.
    Full,
}

/// The body of `POST /api/panes/{id}/links`, checked: `{label, url}`. The
/// label and url as kept, with the host, or why not in words the agent reads.
pub fn parse(b: &Value) -> Result<(String, String, String), String> {
    let s = |k: &str| b.get(k).and_then(Value::as_str).unwrap_or("").trim();
    let label = crate::card::one_line(s("label"));
    if label.is_empty() {
        return Err("a link needs a label".into());
    }
    if label.chars().count() > LABEL_MAX {
        return Err(format!("a link's label is at most {LABEL_MAX} characters"));
    }
    let (url, host) = crate::card::link(s("url")).map_err(|e| {
        e.replace("a link card needs", "a link needs")
            .replace("a card links to", "a link goes to")
    })?;
    Ok((label, url, host))
}

/// Pin `label` at `url` on `links`. The same url is the same link: its label
/// changes where it stands, unless the reader hid it.
pub fn pin(
    links: &mut Vec<Link>,
    label: &str,
    url: &str,
    host: &str,
    by: &str,
    now: i64,
) -> Pinned {
    if let Some(l) = links.iter_mut().find(|l| l.url == url) {
        if l.hidden {
            return Pinned::Hidden(l.id);
        }
        l.label = label.into();
        return Pinned::Updated(l.id);
    }
    if links.iter().filter(|l| !l.hidden).count() >= SHOWN {
        return Pinned::Full;
    }
    if links.len() >= KEPT {
        if let Some(i) = links
            .iter()
            .enumerate()
            .filter(|(_, l)| l.hidden)
            .min_by_key(|(_, l)| l.at)
            .map(|(i, _)| i)
        {
            links.remove(i);
        }
    }
    let id = links.iter().map(|l| l.id).max().unwrap_or(0) + 1;
    links.push(Link {
        id,
        label: label.into(),
        url: url.into(),
        host: host.into(),
        by: by.trim().chars().take(BY_MAX).collect(),
        at: now,
        hidden: false,
    });
    Pinned::New(id)
}

/// Hide or show link `id` again. Whether it was there to change.
pub fn set_hidden(links: &mut [Link], id: i64, hidden: bool) -> bool {
    match links.iter_mut().find(|l| l.id == id) {
        Some(l) if l.hidden != hidden => {
            l.hidden = hidden;
            true
        }
        Some(_) => true,
        None => false,
    }
}

/// The column as the desk's row holds it; a value that does not read is no
/// links rather than no desk.
pub fn from_column(text: &str) -> Vec<Link> {
    serde_json::from_str(text).unwrap_or_default()
}

fn read(conn: &Connection, desk_id: i64) -> Result<Option<Vec<Link>>> {
    let text: Option<String> = conn
        .query_row(
            "SELECT links FROM desks WHERE id = ?1 AND closed_at = 0",
            [desk_id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(text.map(|t| from_column(&t)))
}

fn write(conn: &Connection, desk_id: i64, links: &[Link]) -> Result<()> {
    conn.execute(
        "UPDATE desks SET links = ?2 WHERE id = ?1",
        params![desk_id, serde_json::to_string(links)?],
    )?;
    Ok(())
}

/// `pin` on desk `desk_id`'s links; `None` for a desk that is not there.
pub fn pin_on(
    conn: &Connection,
    desk_id: i64,
    (label, url, host): (&str, &str, &str),
    by: &str,
    now: i64,
) -> Result<Option<Pinned>> {
    let Some(mut links) = read(conn, desk_id)? else {
        return Ok(None);
    };
    let did = pin(&mut links, label, url, host, by, now);
    if matches!(did, Pinned::New(_) | Pinned::Updated(_)) {
        write(conn, desk_id, &links)?;
    }
    Ok(Some(did))
}

/// `set_hidden` on desk `desk_id`'s links. Whether the link was there.
pub fn hide_on(conn: &Connection, desk_id: i64, id: i64, hidden: bool) -> Result<bool> {
    let Some(mut links) = read(conn, desk_id)? else {
        return Ok(false);
    };
    if !set_hidden(&mut links, id, hidden) {
        return Ok(false);
    }
    write(conn, desk_id, &links)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pinned(links: &mut Vec<Link>, label: &str, url: &str, at: i64) -> Pinned {
        pin(links, label, url, "h", "claude-code", at)
    }

    #[test]
    fn a_link_is_the_web_with_a_short_label() {
        assert_eq!(
            parse(&json!({ "label": " Live\nsite ", "url": "https://snyvi.com/" })),
            Ok((
                "Live site".into(),
                "https://snyvi.com/".into(),
                "snyvi.com".into()
            ))
        );
        assert!(
            parse(&json!({ "label": "x", "url": "javascript:alert(1)" }))
                .unwrap_err()
                .contains("a link goes to http")
        );
        assert!(parse(&json!({ "label": "", "url": "https://a.b" })).is_err());
        assert!(parse(&json!({ "label": "x".repeat(33), "url": "https://a.b" })).is_err());
        assert!(parse(&json!({ "label": "x", "url": "https://me:pw@a.b" })).is_err());
    }

    #[test]
    fn the_same_url_is_the_same_link_and_a_hidden_one_stays_hidden() {
        let mut l = Vec::new();
        assert_eq!(pinned(&mut l, "Site", "https://a.b", 1), Pinned::New(1));
        assert_eq!(
            pinned(&mut l, "Live site", "https://a.b", 2),
            Pinned::Updated(1)
        );
        assert_eq!((l.len(), l[0].label.as_str(), l[0].at), (1, "Live site", 1));
        assert!(set_hidden(&mut l, 1, true));
        assert_eq!(pinned(&mut l, "Site", "https://a.b", 3), Pinned::Hidden(1));
        assert!(l[0].hidden, "an agent's pin never undoes the reader's ✕");
        assert!(set_hidden(&mut l, 1, false), "Undo");
        assert!(!set_hidden(&mut l, 9, true));
    }

    #[test]
    fn six_show_and_the_oldest_hidden_makes_room_past_the_keep() {
        let mut l = Vec::new();
        for i in 0..SHOWN as i64 {
            assert_eq!(
                pinned(&mut l, "x", &format!("https://a.b/{i}"), i),
                Pinned::New(i + 1)
            );
        }
        assert_eq!(pinned(&mut l, "x", "https://a.b/7", 9), Pinned::Full);
        for id in 1..=SHOWN as i64 {
            set_hidden(&mut l, id, true);
        }
        for i in SHOWN..KEPT + 3 {
            let did = pinned(&mut l, "x", &format!("https://a.b/{i}"), i as i64);
            if matches!(did, Pinned::Full) {
                let last = l.iter().filter(|x| !x.hidden).map(|x| x.id).min().unwrap();
                set_hidden(&mut l, last, true);
            }
        }
        assert!(l.len() <= KEPT);
        assert!(l.iter().filter(|x| !x.hidden).count() <= SHOWN);
        // Ids are never reused, so an Undo on the page finds its own link.
        let mut ids: Vec<_> = l.iter().map(|x| x.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), l.len());
    }

    #[test]
    fn a_column_that_does_not_read_is_no_links() {
        assert_eq!(from_column("not json"), Vec::<Link>::new());
        assert_eq!(from_column("[]"), Vec::<Link>::new());
    }
}
