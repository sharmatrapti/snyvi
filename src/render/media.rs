//! A song or a video inside a document: which files a player takes, and the
//! player itself, a button until it is pressed (#117, #120), as the
//! renderer writes it and as an old document's is served.

use super::*;

/// What a browser can play: video, then audio. `.ogg` is usually audio, and an
/// `<audio>` element plays it; `.ogv` is the video one.
pub const MEDIA_EXTS: &[(&str, &str)] = &[
    ("mp4", "video"),
    ("webm", "video"),
    ("mov", "video"),
    ("m4v", "video"),
    ("ogv", "video"),
    ("mp3", "audio"),
    ("wav", "audio"),
    ("m4a", "audio"),
    ("ogg", "audio"),
    ("oga", "audio"),
    ("flac", "audio"),
    ("opus", "audio"),
    ("aac", "audio"),
];

/// "video" or "audio" for an extension a player can take, else None.
pub fn media_kind(ext: &str) -> Option<&'static str> {
    MEDIA_EXTS.iter().find(|(e, _)| *e == ext).map(|(_, k)| *k)
}

/// The player for a media document or file, pointed at wherever its bytes are
/// served.
pub fn media_body(src_url: &str, ext: &str) -> String {
    let src = html_escape::encode_double_quoted_attribute(src_url);
    let kind = media_kind(ext).unwrap_or("audio");
    format!(
        "<p class=\"doc-image doc-media\">{}</p>",
        player(kind, &src, "")
    )
}

/// A player as a button until it is pressed (#117, #120). In the Linux
/// window's engine (WebKitGTK) an `<audio controls>` is a GStreamer pipeline
/// and a scripted set of controls the moment it is in the page: 83 of them
/// held the page 1.9 s before a byte was fetched, and `preload="metadata"`
/// then fetched all 83 and stuttered on for 8 s more. Worse, a player that
/// had loaded ahead and sat waiting took 8.6 s from Play to sound, where one
/// made on the press sounds in half a second. Chromium does neither, which is
/// why the web was quick and the window was not.
///
/// So the real element waits in a `<template>`, inert -- 83 of these cost
/// 13 ms -- and a press puts it in the button's place and plays it
/// (`ui/app/04-doc.js`, `data-w="play"`). The button is the player's own box,
/// a song's 54 px or a video's 16:9, so the swap moves nothing. What it costs
/// is the length and a video's first frame, unseen until Play.
///
/// `src` and `title` come already escaped for a double-quoted attribute
/// (the Markdown path copies them across from comrak and ammonia), and an
/// attribute-escaped string reads the same as text. Styled inline: app.css
/// is first paint, at its budget, and a player is only ever in a document.
pub fn player(kind: &str, src: &str, title: &str) -> String {
    let label = if title.is_empty() {
        file_label(src)
    } else {
        title.to_string()
    };
    let titled = if title.is_empty() {
        String::new()
    } else {
        format!(" title=\"{title}\"")
    };
    let (el, look, face) = if kind == "video" {
        (
            "video",
            "display:block;width:100%;aspect-ratio:16/9;max-height:70vh;object-fit:contain;\
             background:#000;border-radius:var(--radius)",
            "display:flex;align-items:center;justify-content:center;gap:10px;width:100%;\
             aspect-ratio:16/9;max-height:70vh;background:#000;color:#fff;border:0;\
             border-radius:var(--radius)",
        )
    } else {
        (
            "audio",
            "display:block;width:100%;height:54px",
            "display:flex;align-items:center;gap:12px;width:100%;height:54px;padding:0 18px;\
             box-sizing:border-box;border:1px solid var(--rule-2);border-radius:27px;\
             background:var(--bg-raise);color:var(--fg)",
        )
    };
    format!(
        "<button type=\"button\" class=\"doc-play\" data-w=\"play\" aria-label=\"Play {label}\" \
         style=\"{face};font:inherit;text-align:left;cursor:pointer\">\
         <span aria-hidden=\"true\">\u{25b6}</span>\
         <span style=\"min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap\">{label}</span>\
         </button><template><{el} controls preload=\"auto\" src=\"{src}\"{titled} style=\"{look}\"></{el}></template>"
    )
}

/// The file a player plays, by name, for a player with no title: the last
/// part of `src` (attribute-escaped), its `%20`s read, escaped again. Only
/// what an attribute's escaping writes is read back: html_escape's decoder
/// knows all 2,125 named entities, and its table put the binary over its
/// budget for a file name.
fn file_label(src: &str) -> String {
    let src = src
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    let path = src.split(['?', '#']).next().unwrap_or("");
    let name = path.rsplit('/').next().unwrap_or(path);
    let bytes = name.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    html_escape::encode_double_quoted_attribute(&String::from_utf8_lossy(&out)).into_owned()
}

/// A document stored before 1.34 holds the old players, made in the page the
/// moment it opens (`player`). Turned into buttons as it is served, so a
/// document already in the library opens as quickly as a new one, with no
/// resend and nothing rewritten in the store.
pub fn lazy_players(html: &str) -> std::borrow::Cow<'_, str> {
    const OLD: [(&str, &str); 2] = [
        ("audio", "<audio controls preload=\"metadata\" src=\""),
        ("video", "<video controls preload=\"metadata\" src=\""),
    ];
    if !OLD.iter().any(|(_, o)| html.contains(o)) {
        return std::borrow::Cow::Borrowed(html);
    }
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    loop {
        let next = OLD
            .iter()
            .filter_map(|(k, o)| rest.find(o).map(|i| (i, *k, *o)))
            .min_by_key(|(i, ..)| *i);
        let Some((i, kind, open)) = next else { break };
        let tail = &rest[i..];
        let close = format!("</{kind}>");
        let (Some(end), Some(gt)) = (tail.find(&close), tail.find('>')) else {
            break;
        };
        let tag = &tail[..gt];
        let src = &tag[open.len()..];
        let src = &src[..src.find('"').unwrap_or(src.len())];
        let title = tag
            .find(" title=\"")
            .map(|t| &tag[t + 8..])
            .map(|t| &t[..t.find('"').unwrap_or(t.len())])
            .unwrap_or("");
        out.push_str(&rest[..i]);
        out.push_str(&player(kind, src, title));
        rest = &tail[end + close.len()..];
    }
    out.push_str(rest);
    std::borrow::Cow::Owned(out)
}

/// A Markdown image whose path is a video or a song, `![take 2](take2.mp4)`,
/// becomes a player for it, the alt text its title. Done on the finished
/// HTML, where comrak (`<img src=".." alt=".." />`) and ammonia
/// (`<img src=".." alt="..">`) both write the attributes escaped and
/// double-quoted, so they are copied across as they are.
///
/// It is a button until pressed (`player`). A video sits in a 16:9 box from
/// the first paint, letterboxed, so the page does not move when the take's
/// own size arrives; a song's is one fixed height.
///
/// A picture under the document's own folder (`files`: the `/files/<id>/`
/// prefix its paths carry, and the folder on disk) is given its size from
/// the file's header and `loading="lazy"`: a page of screenshots fetches
/// only the ones in view, and the box of each is there before it is, so
/// nothing moves when one arrives. A picture whose size cannot be read --
/// a remote one, an SVG, a file that is not what its name says -- stays as
/// it was: eager, and never a shift.
pub(super) fn players(html: String, files: Option<(&str, &Path)>) -> String {
    const IMG: &str = "<img src=\"";
    if !html.contains(IMG) {
        return html;
    }
    let attr = |tag: &str, name: &str| -> Option<String> {
        let at = tag.find(&format!(" {name}=\""))? + name.len() + 3;
        let end = tag[at..].find('"')?;
        Some(tag[at..at + end].to_string())
    };
    let mut out = String::with_capacity(html.len());
    let mut rest = html.as_str();
    while let Some(i) = rest.find(IMG) {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let Some(close) = tail.find('>') else {
            break;
        };
        let tag = &tail[..=close];
        let src = attr(tag, "src").unwrap_or_default();
        let path = src.split(['?', '#']).next().unwrap_or("");
        match media_kind(&ext_of(path)) {
            None => match files.and_then(|(base, dir)| sized(tag, path, base, dir)) {
                Some(t) => out.push_str(&t),
                None => out.push_str(tag),
            },
            Some(kind) => out.push_str(&player(kind, &src, &attr(tag, "alt").unwrap_or_default())),
        }
        rest = &tail[close + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_video_or_a_song_in_markdown_plays_in_place() {
        let r = Renderer::new();
        let md = |src: &str| r.render_with_base(Kind::Markdown, None, src, Some("/files/abc/"));
        for (file, tag) in [
            (
                "take2.mp4",
                "<template><video controls preload=\"auto\" src=\"/files/abc/take2.mp4\"",
            ),
            (
                "cut.webm",
                "<template><video controls preload=\"auto\" src=\"/files/abc/cut.webm\"",
            ),
            (
                "song.mp3",
                "<template><audio controls preload=\"auto\" src=\"/files/abc/song.mp3\"",
            ),
            (
                "bed.ogg",
                "<template><audio controls preload=\"auto\" src=\"/files/abc/bed.ogg\"",
            ),
        ] {
            let html = md(&format!("Here:\n\n![take \"2\"]({file})\n"));
            assert!(html.contains(tag), "{file}: {html}");
            assert!(
                html.contains("title=\"take &quot;2&quot;\""),
                "{file}: {html}"
            );
            assert!(!html.contains("<img"), "{file}: {html}");
            // Until Play, a button the page swaps for the template's player
            // (#117, #120); nothing outside a template is a player.
            assert!(
                html.contains("data-w=\"play\" aria-label=\"Play take &quot;2&quot;\""),
                "{file}: {html}"
            );
            let outside = html.split("<template>").next().unwrap_or("");
            assert!(
                !outside.contains("<audio") && !outside.contains("<video"),
                "{file}: {html}"
            );
        }
        // A video's box is there before its size is known, so nothing moves.
        assert!(md("![a](a.mp4)").contains("aspect-ratio:16/9"));
        // A picture stays a picture, and a query is not an extension.
        assert!(md("![a](a.png)").contains("<img src=\"/files/abc/a.png\""));
        assert!(md("![a](a.mp4?t=3)").contains("<video"));
        // Raw HTML takes the sanitizer's road and still gets its player, but
        // a player written by hand does not get through.
        let raw =
            md("<b>hi</b>\n\n![a](a.mp4)\n\n<video src=\"x.mp4\" onplay=\"alert(1)\"></video>");
        assert_eq!(raw.matches("<video").count(), 1, "{raw}");
        assert!(!raw.contains("onplay"), "{raw}");
    }

    /// A document stored with the old players opens with buttons all the
    /// same, its titles and sources kept; one with none is not copied.
    #[test]
    fn an_old_player_is_a_button_as_it_is_served() {
        let old = "<p>a</p><audio controls preload=\"metadata\" src=\"/files/x/my%20take.mp3\" \
                   style=\"display:block;width:100%;height:54px\"></audio>\
                   <p class=\"doc-image doc-media\"><video controls preload=\"metadata\" src=\"/files/x/cut.mp4\" title=\"Cut &amp; run\" \
                   style=\"max-width:100%\"></video></p><p>b</p>";
        let new = lazy_players(old);
        assert!(new.starts_with("<p>a</p><button"), "{new}");
        assert!(new.ends_with("</template></p><p>b</p>"), "{new}");
        assert!(new.contains("aria-label=\"Play my take.mp3\""), "{new}");
        assert!(new.contains("aria-label=\"Play Cut &amp; run\""), "{new}");
        assert!(new.contains("<template><video controls preload=\"auto\" src=\"/files/x/cut.mp4\" title=\"Cut &amp; run\""), "{new}");
        assert!(!new.contains("preload=\"metadata\""), "{new}");
        assert!(matches!(
            lazy_players("<p>none</p>"),
            std::borrow::Cow::Borrowed(_)
        ));
        // A song sent on its own is the same button.
        let one = media_body("/files/x/a%3Cb.ogg", "ogg");
        assert!(one.contains("aria-label=\"Play a&lt;b.ogg\""), "{one}");
        assert!(one.contains("<template><audio controls"), "{one}");
    }
}
