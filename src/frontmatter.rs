//! A Markdown file's frontmatter: the `---` block that skills, memory files
//! and most static-site posts open with. comrak is told to leave it out of
//! the page (`render.rs`); this reads the two lines snyvi wants from it, a
//! title and a name, as plain `key: value` lines. Not a YAML parser: a value
//! that is a block (`title: >`) is no title, and the file name stands.
//!
//! Before 1.33 the block was read as Markdown: the opening `---` a rule, the
//! YAML a paragraph, and the closing `---` made that paragraph a setext H2
//! of 250 characters, in the outline too (audit bf13a1859b, F3).

/// What a document's frontmatter gave, and where its body starts.
#[derive(Debug, Default, PartialEq)]
pub struct Front {
    /// `title:`, which outranks the body's H1.
    pub title: Option<String>,
    /// `name:`, which is often a slug, so it stands only where there is no H1.
    pub name: Option<String>,
    /// The byte the body starts at: 0 when there is no frontmatter.
    pub body: usize,
}

impl Front {
    /// The frontmatter at the very top of `content`, as comrak finds it: a
    /// first line of `---` and a later line of `---` closing it.
    pub fn of(content: &str) -> Front {
        let mut lines = content.split_inclusive('\n');
        let Some(first) = lines.next().filter(|l| l.trim_end() == "---") else {
            return Front::default();
        };
        let mut at = first.len();
        let mut front = Front::default();
        for line in lines {
            at += line.len();
            let l = line.trim_end();
            if l == "---" {
                front.body = at;
                return front;
            }
            let Some((key, value)) = l.split_once(':') else {
                continue;
            };
            let value = value.trim().trim_matches(|c| c == '"' || c == '\'').trim();
            let slot = match key {
                "title" => &mut front.title,
                "name" => &mut front.name,
                _ => continue,
            };
            if slot.is_none() && !value.is_empty() && !matches!(value, ">" | "|" | ">-" | "|-") {
                *slot = Some(value.to_string());
            }
        }
        // Never closed: not frontmatter, and comrak renders it as it is.
        Front::default()
    }
}

/// A Markdown document's own title: `title:`, else the first `# ` heading
/// in the 40 lines after the frontmatter -- so a YAML `# comment` is never
/// one -- else `name:`.
pub fn title(content: &str) -> Option<String> {
    let front = Front::of(content);
    let body = &content[front.body..];
    let h1 = || {
        body.lines().take(40).find_map(|line| {
            let h = line.trim().strip_prefix("# ")?;
            let h = h.trim().trim_end_matches('#').trim();
            (!h.is_empty()).then(|| h.to_string())
        })
    };
    front.title.or_else(h1).or(front.name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{strip_leading_h1, title_for, Kind, Renderer};

    const MEMORY: &str = "---\nname: account-ui-in-flight\ndescription: \"the audit\"\nmetadata:\n  type: project\n---\n\nAudit doc 2dbcf2e538.\n";

    #[test]
    fn the_block_is_read_and_the_body_found() {
        let f = Front::of(MEMORY);
        assert_eq!(f.name.as_deref(), Some("account-ui-in-flight"));
        assert_eq!(f.title, None);
        assert!(MEMORY[f.body..].starts_with("\nAudit doc"));
        let quoted = Front::of("---\r\ntitle: 'A plan'\r\n---\r\n# H1\r\n");
        assert_eq!(quoted.title.as_deref(), Some("A plan"));
        assert_eq!(
            &"---\r\ntitle: 'A plan'\r\n---\r\n# H1\r\n"[quoted.body..],
            "# H1\r\n"
        );
        // A block value, a rule further down, a block never closed: no title.
        assert_eq!(Front::of("---\ntitle: >\n  folded\n---\n").title, None);
        assert_eq!(Front::of("intro\n---\ntitle: x\n---\n"), Front::default());
        assert_eq!(Front::of("---\ntitle: x\nno end\n"), Front::default());
    }

    /// The title is the explicit one, then `title:`, then the H1 after the
    /// block -- never a YAML `# comment` inside it -- then `name:`, then the
    /// file name.
    #[test]
    fn the_title_comes_from_the_frontmatter_before_the_file_name() {
        let md = |c: &str| title_for(None, Kind::Markdown, Some("/p/memory/x.md"), c);
        assert_eq!(md(MEMORY), "account-ui-in-flight");
        assert_eq!(
            md("---\ntitle: Real title\nname: slug\n---\n# H1\n"),
            "Real title"
        );
        assert_eq!(md("---\nname: slug\n---\n# The heading\n"), "The heading");
        assert_eq!(md("---\n# a comment\nlayout: post\n---\nbody\n"), "x.md");
        assert_eq!(
            title_for(Some("Given"), Kind::Markdown, None, MEMORY),
            "Given"
        );
    }

    /// The page shows none of the block: no rule, no giant heading, no id
    /// for the outline. An H1 under it is the title, and goes as it would
    /// at the top.
    #[test]
    fn the_frontmatter_is_not_printed() {
        let r = Renderer::new();
        let html = r.render(Kind::Markdown, None, MEMORY);
        assert!(!html.contains("<hr"), "{html}");
        assert!(!html.contains("<h2"), "{html}");
        assert!(!html.contains("account-ui-in-flight"), "{html}");
        assert!(html.contains("Audit doc 2dbcf2e538."), "{html}");
        let src = "---\ntitle: T\n---\n\n# Heading\n\nbody\n";
        let body = strip_leading_h1(src, "T").unwrap();
        assert_eq!(body, "---\ntitle: T\n---\n\nbody\n");
        let html = r.render(Kind::Markdown, None, &body);
        assert!(
            !html.contains("<h1") && html.contains("<p>body</p>"),
            "{html}"
        );
    }
}
