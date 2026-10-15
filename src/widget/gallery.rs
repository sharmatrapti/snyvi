//! A gallery: the box an agent fills with what an image or a video model
//! made for the reader (docs/WIDGETS.md, "A gallery").
//!
//! It is a pushed box like any other -- the card that asks first, Not now,
//! Remove and its Undo, the six a desk has room for -- whose body snyvi
//! draws itself: the newest outputs as tiles in one row, in the room the
//! box keeps (`lines` × 20 px), so a new one never moves what is below.
//! What tells `push` a body is a gallery's is `MARK` at the top of its
//! Markdown, which the strict Markdown drops: the tiles are drawn again from
//! `gallery_items` whenever it is pushed, allowed, or brought back.
//!
//! **An output is a file, copied in.** The agent downloads what a model
//! gave it -- a provider's link expires within the hour, and a picture from
//! another host in the rail would be a request leaving the machine on every
//! draw -- and names the file; snyvi copies it to `gallery/<hash>.<ext>`
//! in its data folder, so the same output twice is one file, and the
//! gallery shows what was made even after the agent's folder is cleaned.
//!
//! **Nothing is deleted.** Every output stays a row and a file. A tile
//! still being made is a row with a `job` and no file, filled when the
//! same job comes with one.
//!
//! **Pick** is the reader's: an answered turn for the panel that added the
//! output, carried to its next prompt by `thread::take_untold`, as an
//! answer on On you is.

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;

use super::{Body, Tone};

/// The first line of a gallery's Markdown: an HTML comment, which the
/// strict Markdown leaves out, and which says to `push` that the tiles
/// are to be drawn from the outputs rather than the text.
pub const MARK: &str = "<!-- gallery -->";

/// The folder in snyvi's data folder the outputs are copied into.
pub const DIR: &str = "gallery";

/// A gallery's room when it does not say: four lines, 76 px tiles.
pub const LINES_DEFAULT: u8 = 4;

/// The tiles a box draws. The row is cut by the sidebar's width; these are
/// enough for the widest one.
const TILES: usize = 8;

/// The most outputs the viewer lists at once.
pub const LIST_MAX: usize = 400;

/// What each field keeps.
pub const CAPTION_CHARS: usize = 200;
pub const MODEL_CHARS: usize = 80;
pub const PROMPT_CHARS: usize = 4000;
pub const JOB_CHARS: usize = 80;
pub const FAILED_CHARS: usize = 300;
pub const LINE_CHARS: usize = 300;

/// A picture: what a browser draws in an `<img>`, raster only. An SVG is
/// left out: it is a document, not an output, and has no size to read.
pub const PICTURE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "avif"];
/// A clip: what a `<video>` plays.
pub const CLIP_EXTS: &[&str] = &["mp4", "webm", "mov", "m4v"];

/// The most a picture may be, as a document's; a clip, as a sent video's.
pub const PICTURE_MAX: u64 = crate::receive::MAX_BYTES as u64;
pub const CLIP_MAX: u64 = crate::receive::MAX_MEDIA_BYTES;

/// "image" or "video" for an extension a gallery takes.
pub fn kind_of(ext: &str) -> Option<&'static str> {
    if PICTURE_EXTS.contains(&ext) {
        Some("image")
    } else if CLIP_EXTS.contains(&ext) {
        Some("video")
    } else {
        None
    }
}

/// A file's name in the gallery's folder, `<16 hex>.<ext>`: the only names
/// `/gallery/{file}` serves, so nothing else in the data folder is reached.
pub fn file_name_ok(name: &str) -> bool {
    let Some((hash, ext)) = name.split_once('.') else {
        return false;
    };
    hash.len() == 16
        && hash
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        && kind_of(ext).is_some()
}

/// A file copied in: its name in the folder, its kind, and a picture's size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    pub file: String,
    pub kind: &'static str,
    pub width: u32,
    pub height: u32,
}

/// Copy `from` into `dir`, named by its content, unless it is there
/// already. Refusals are sentences for the agent: what it may send, and
/// why this one is not that.
pub fn copy_in(dir: &Path, from: &Path) -> Result<Stored> {
    use std::io::{Read, Write};
    if !from.is_absolute() {
        bail!("{} is not an absolute path", from.display());
    }
    let ext = crate::render::ext_of(&from.to_string_lossy());
    let ext = if ext == "jpeg" {
        "jpg".to_string()
    } else {
        ext
    };
    let Some(kind) = kind_of(&ext) else {
        bail!(
            "a gallery takes a picture ({}) or a clip ({}), not .{ext}",
            PICTURE_EXTS.join(", "),
            CLIP_EXTS.join(", ")
        );
    };
    let mut src =
        std::fs::File::open(from).with_context(|| format!("reading {}", from.display()))?;
    let len = src.metadata()?.len();
    let cap = if kind == "image" {
        PICTURE_MAX
    } else {
        CLIP_MAX
    };
    if len == 0 {
        bail!("{} is empty", from.display());
    }
    if len > cap {
        bail!(
            "{} is {} MB; a gallery keeps a picture up to {} MB and a clip up to {} MB",
            from.display(),
            len >> 20,
            PICTURE_MAX >> 20,
            CLIP_MAX >> 20
        );
    }
    std::fs::create_dir_all(dir)?;
    // Two copies at once each have their own: the counter, in this process.
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = dir.join(format!(".copy-{}-{n}", std::process::id()));
    let mut out = std::fs::File::create(&tmp)?;
    let mut hasher = blake3::Hasher::new();
    let mut head = Vec::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut read = 0u64;
    let copied = (|| -> Result<()> {
        loop {
            let n = src.read(&mut buf)?;
            if n == 0 {
                return Ok(());
            }
            read += n as u64;
            if read > cap {
                bail!(
                    "{} grew past what a gallery keeps while it was copied",
                    from.display()
                );
            }
            if head.len() < 64 * 1024 {
                head.extend_from_slice(&buf[..n.min(64 * 1024 - head.len())]);
            }
            hasher.update(&buf[..n]);
            out.write_all(&buf[..n])?;
        }
    })();
    drop(out);
    if let Err(e) = copied {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    // A picture is checked to be one: its header gives a size. AVIF's is
    // not read, and a clip is the player's to judge.
    let (width, height) = match crate::render::picture_size(&head) {
        Some(s) => s,
        None if kind == "image" && ext != "avif" => {
            let _ = std::fs::remove_file(&tmp);
            bail!("{} does not read as a .{ext} picture", from.display());
        }
        None => (0, 0),
    };
    let file = format!("{}.{ext}", &hasher.finalize().to_hex()[..16]);
    let at = dir.join(&file);
    if at.exists() {
        let _ = std::fs::remove_file(&tmp);
    } else {
        std::fs::rename(&tmp, &at)?;
    }
    Ok(Stored {
        file,
        kind,
        width,
        height,
    })
}

/// One output, as the viewer reads it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Item {
    pub id: i64,
    pub desk_id: i64,
    pub name: String,
    pub job: String,
    /// Its file in the gallery's folder; empty while it is being made.
    pub file: String,
    pub kind: String,
    /// The path it was copied from, for the agent to find it again.
    pub source: String,
    pub caption: String,
    pub model: String,
    pub prompt: String,
    pub cost_micros: i64,
    pub poster: String,
    pub failed: String,
    pub width: u32,
    pub height: u32,
    pub pane: String,
    pub writer: String,
    pub made_at: i64,
}

const COLS: &str =
    "id, desk_id, name, job, file, kind, source, caption, model, prompt, cost_micros,
     poster, failed, width, height, pane, writer, made_at";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Item> {
    Ok(Item {
        id: r.get(0)?,
        desk_id: r.get(1)?,
        name: r.get(2)?,
        job: r.get(3)?,
        file: r.get(4)?,
        kind: r.get(5)?,
        source: r.get(6)?,
        caption: r.get(7)?,
        model: r.get(8)?,
        prompt: r.get(9)?,
        cost_micros: r.get(10)?,
        poster: r.get(11)?,
        failed: r.get(12)?,
        width: r.get::<_, i64>(13)?.max(0) as u32,
        height: r.get::<_, i64>(14)?.max(0) as u32,
        pane: r.get(15)?,
        writer: r.get(16)?,
        made_at: r.get(17)?,
    })
}

/// What a writer adds: an output (`stored`), a tile still being made (a
/// `job` and nothing stored), or one that failed (`failed`).
#[derive(Debug, Default, Clone)]
pub struct Add {
    pub job: String,
    pub stored: Option<Stored>,
    pub source: String,
    pub caption: String,
    pub model: String,
    pub prompt: String,
    pub cost_micros: Option<i64>,
    pub poster: Option<Stored>,
    pub failed: String,
    pub pane: String,
    pub writer: String,
}

/// Text cut to `n` characters, on one line unless it is a prompt.
pub fn clip(s: &str, n: usize) -> String {
    s.trim().chars().take(n).collect()
}

/// A cost in dollars as millionths: what a model's price is said in, kept
/// whole so the box's sum is exact. Negative and absurd costs are refused.
pub fn micros(dollars: f64) -> Option<i64> {
    (dollars.is_finite() && (0.0..=100_000.0).contains(&dollars))
        .then(|| (dollars * 1_000_000.0).round() as i64)
}

/// Keep `a` on desk `desk_id`'s gallery `name`. A `job` already there and
/// not yet filled is filled (or marked failed) in its place, so its tile
/// becomes the output where it stood; a job that has its output already,
/// sent again with a file, is a new output beside it -- the first is kept.
/// The output's id.
pub fn add(conn: &Connection, desk_id: i64, name: &str, a: &Add, now: i64) -> Result<i64> {
    let open: Option<i64> = if a.job.is_empty() {
        None
    } else {
        conn.query_row(
            "SELECT id FROM gallery_items WHERE desk_id = ?1 AND name = ?2 AND job = ?3 AND file = ''
             ORDER BY id DESC LIMIT 1",
            params![desk_id, name, a.job],
            |r| r.get(0),
        )
        .optional()?
    };
    let (file, kind, w, h) = match &a.stored {
        Some(s) => (s.file.as_str(), s.kind, s.width as i64, s.height as i64),
        None => ("", "", 0, 0),
    };
    let poster = a.poster.as_ref().map(|p| p.file.as_str()).unwrap_or("");
    if let Some(id) = open {
        conn.execute(
            "UPDATE gallery_items SET file = ?2, kind = CASE WHEN ?3 = '' THEN kind ELSE ?3 END,
               source = CASE WHEN ?4 = '' THEN source ELSE ?4 END,
               caption = CASE WHEN ?5 = '' THEN caption ELSE ?5 END,
               model = CASE WHEN ?6 = '' THEN model ELSE ?6 END,
               prompt = CASE WHEN ?7 = '' THEN prompt ELSE ?7 END,
               cost_micros = COALESCE(?8, cost_micros),
               poster = CASE WHEN ?9 = '' THEN poster ELSE ?9 END,
               failed = ?10, width = ?11, height = ?12, pane = ?13, writer = ?14, updated_at = ?15
             WHERE id = ?1",
            params![
                id,
                file,
                kind,
                a.source,
                a.caption,
                a.model,
                a.prompt,
                a.cost_micros,
                poster,
                a.failed,
                w,
                h,
                a.pane,
                a.writer,
                now
            ],
        )?;
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO gallery_items (desk_id, name, job, file, kind, source, caption, model, prompt,
           cost_micros, poster, failed, width, height, pane, writer, made_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?17)",
        params![
            desk_id,
            name,
            a.job,
            file,
            kind,
            a.source,
            a.caption,
            a.model,
            a.prompt,
            a.cost_micros.unwrap_or(0),
            poster,
            a.failed,
            w,
            h,
            a.pane,
            a.writer,
            now
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// A gallery's outputs, newest first: one desk's box, or -- `all` -- every
/// desk's galleries, for the viewer's All desks.
pub fn items(
    conn: &Connection,
    desk_id: i64,
    name: &str,
    all: bool,
    limit: usize,
) -> Result<Vec<Item>> {
    let mut st = conn.prepare(&format!(
        "SELECT {COLS} FROM gallery_items WHERE ?3 OR (desk_id = ?1 AND name = ?2)
         ORDER BY made_at DESC, id DESC LIMIT ?4"
    ))?;
    let rows = st.query_map(params![desk_id, name, all, limit as i64], row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn item(conn: &Connection, id: i64) -> Result<Option<Item>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLS} FROM gallery_items WHERE id = ?1"),
            [id],
            row,
        )
        .optional()?)
}

/// How many outputs a box has, and what they cost together.
pub fn totals(conn: &Connection, desk_id: i64, name: &str) -> Result<(i64, i64)> {
    Ok(conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(cost_micros), 0) FROM gallery_items WHERE desk_id = ?1 AND name = ?2",
        params![desk_id, name],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?)
}

/// The box's count, eight characters at most: what it cost when anything
/// said, else how many outputs.
pub fn count_label(n: i64, cost_micros: i64) -> String {
    if cost_micros <= 0 {
        return if n > 99_999 {
            "99999+".into()
        } else {
            n.to_string()
        };
    }
    let d = cost_micros as f64 / 1_000_000.0;
    match d {
        d if d < 0.01 => "<$0.01".into(),
        d if d < 100.0 => format!("${d:.2}"),
        d if d < 100_000.0 => format!("${d:.0}"),
        _ => "$99999+".into(),
    }
}

fn attr(s: &str) -> String {
    html_escape::encode_double_quoted_attribute(s).into_owned()
}

/// A tile's tip: its caption, its model and its cost, or why it failed.
fn tip(i: &Item) -> String {
    let mut t = if i.caption.is_empty() {
        i.job.clone()
    } else {
        i.caption.clone()
    };
    if !i.model.is_empty() {
        t = if t.is_empty() {
            i.model.clone()
        } else {
            format!("{t} · {}", i.model)
        };
    }
    if i.cost_micros > 0 {
        t.push_str(&format!(" · {}", count_label(1, i.cost_micros)));
    }
    if !i.failed.is_empty() {
        t = format!("Failed: {}", i.failed);
    }
    t
}

/// The box's body: the newest outputs as tiles, each a button the viewer
/// opens at. A picture is drawn from its file, a clip from its poster or as
/// a play mark, one being made says so, one that failed is marked. Every
/// cell has its size before anything loads.
pub fn strip_html(items: &[Item], lines: u8) -> String {
    let mut h = format!(
        "<div class=\"gal\" style=\"--gal:{}px\">",
        (lines as u32 * 20).saturating_sub(4)
    );
    for i in items.iter().take(TILES) {
        let t = attr(&tip(i));
        let inner = if !i.failed.is_empty() {
            "<span class=\"gal-x\">failed</span>".to_string()
        } else if i.file.is_empty() {
            format!(
                "<span class=\"gal-p\">making{}</span>",
                if i.model.is_empty() {
                    String::new()
                } else {
                    format!("<br>{}", attr(&clip(&i.model, 14)))
                }
            )
        } else if i.kind == "video" && i.poster.is_empty() {
            "<span class=\"gal-v\">▶</span>".to_string()
        } else {
            let src = if i.kind == "video" {
                &i.poster
            } else {
                &i.file
            };
            format!(
                "<img src=\"/gallery/{}\" alt=\"{t}\" loading=\"lazy\" decoding=\"async\">{}",
                attr(src),
                if i.kind == "video" {
                    "<span class=\"gal-v on\">▶</span>"
                } else {
                    ""
                }
            )
        };
        h.push_str(&format!(
            "<button type=\"button\" class=\"gal-t\" data-gal=\"{}\" data-tip=\"{t}\" aria-label=\"{t}\">{inner}</button>",
            i.id
        ));
    }
    h.push_str("</div>");
    h
}

/// The Markdown a gallery keeps as its body, for `read_widgets`: `MARK`,
/// then a line for each of its newest outputs.
pub fn summary_md(items: &[Item], n: i64) -> String {
    let mut md = format!(
        "{MARK}\nA gallery, {n} output{}.",
        if n == 1 { "" } else { "s" }
    );
    for i in items.iter().take(TILES) {
        let what = if !i.failed.is_empty() {
            format!("failed: {}", clip(&i.failed, 80))
        } else if i.file.is_empty() {
            "being made".to_string()
        } else {
            clip(&i.source, 160)
        };
        let line = format!(
            "\n- #{} {} ({what})",
            i.id,
            clip(&tip(i), 100).replace('\n', " ")
        );
        if md.chars().count() + line.chars().count() > super::BODY_MAX {
            break;
        }
        md.push_str(&line);
    }
    md
}

/// The body a gallery is pushed with, before its tiles are drawn: what
/// `push` reads `MARK` in, the room it keeps, and `said`, the newest
/// output in words, which the card that asks first shows.
pub fn raw(lines: u8, said: &str) -> String {
    let said = clip(said, 200).replace('\n', " ");
    serde_json::json!({ "body": format!("{MARK}\nA gallery: {said}"), "lines": lines, "stale_after": 0 })
        .to_string()
}

/// Whether a body is a gallery's.
pub fn is_gallery(md: &str) -> bool {
    md.trim_start().starts_with(MARK)
}

/// A gallery's body as it is now, and its tiles: what `push` keeps in the
/// place of the Markdown it was sent. The room is the one it was sent with.
pub fn body_now(
    conn: &Connection,
    desk_id: i64,
    name: &str,
    sent: &Body,
) -> Result<(Body, String)> {
    let shown = items(conn, desk_id, name, false, TILES)?;
    let (n, cost) = totals(conn, desk_id, name)?;
    // The count is money or a number, never an alarm: a failed output is
    // marked on its own tile.
    let b = Body {
        md: summary_md(&shown, n),
        tone: Tone::None,
        count: count_label(n, cost),
        lines: sent.lines,
        stale_after: 0,
    };
    let html = strip_html(&shown, sent.lines);
    Ok((b, html))
}

/// The reader picked output `id`, with a line: an answered turn for the
/// panel that added it, which its next prompt carries. The desk it was on.
pub fn pick(conn: &Connection, id: i64, line: &str, now: i64) -> Result<Option<i64>> {
    let Some(i) = item(conn, id)? else {
        return Ok(None);
    };
    let what = if i.caption.is_empty() {
        i.job.clone()
    } else {
        i.caption.clone()
    };
    // The reader's words before the path: the brief cuts a long answer.
    let mut answer = format!("#{} \"{}\"", i.id, clip(&what, 60));
    let line = clip(line, LINE_CHARS);
    if !line.is_empty() {
        answer.push_str(&format!(": {line}"));
    }
    if !i.source.is_empty() {
        answer.push_str(&format!(" ({})", i.source));
    }
    conn.execute(
        "INSERT INTO turns (desk_id, pane, by, kind, via, text, answer, answered_in, answered_at, created_at)
         VALUES (?1, ?2, 'you', 'decide', 'gallery', ?3, ?4, 'gallery', ?5, ?5)",
        params![
            i.desk_id,
            i.pane,
            if i.name == "gallery" {
                "Which one in the gallery?".to_string()
            } else {
                format!("Which one in the gallery {}?", i.name)
            },
            answer,
            now
        ],
    )?;
    Ok(Some(i.desk_id))
}

#[cfg(test)]
mod tests;
