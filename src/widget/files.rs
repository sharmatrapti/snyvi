//! Widget files: a folder in `<config>/widgets/<name>/` holding a
//! `widget.json` and the command it names, which snyvi runs on a timer
//! while the widget is in view and draws what it prints (docs/WIDGETS.md).
//!
//! A desk's own widget is one level down, in `<config>/widgets/@<desk
//! id>/<name>/` (1.32): `@` is in no widget's name, so the two never meet,
//! and a desk renamed keeps its widgets. A name is shared or owned, never
//! both (`taken`).
//!
//! Nothing in here runs anything: this reads the folders, says what each
//! one asks for, hashes what the reader allowed, and writes a starter. The
//! running is the daemon's (`crate::server::widget_run`).

use super::{name_ok, LINES_DEFAULT, LINES_MAX, LINES_MIN};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The most a widget's folder may hold, all its files together: what is
/// hashed is what was allowed, and a folder this size is read in a moment.
pub const FOLDER_MAX: u64 = 64 * 1024;
/// How often a command may run, and how long it may take.
pub const EVERY_MIN: u64 = 5;
pub const EVERY_DEFAULT: u64 = 60;
pub const TIMEOUT_MAX: u64 = 30;
pub const TIMEOUT_DEFAULT: u64 = 10;

/// Where the reader's widget folders are.
pub fn dir(config_dir: &Path) -> PathBuf {
    config_dir.join("widgets")
}

/// Where desk `desk`'s own widgets are.
pub fn owned_dir(config_dir: &Path, desk: i64) -> PathBuf {
    dir(config_dir).join(format!("@{desk}"))
}

/// The folder of `owner`'s widget `name`: in its desk's folder, or for 0 --
/// shared or global -- in the widgets folder itself.
pub fn folder_of(config_dir: &Path, owner: i64, name: &str) -> PathBuf {
    if owner == 0 {
        dir(config_dir).join(name)
    } else {
        owned_dir(config_dir, owner).join(name)
    }
}

/// Where a pruned desk's widgets are kept: out of the runner's sight, and
/// still on disk (snyvi never deletes).
pub fn pruned_dir(config_dir: &Path) -> PathBuf {
    dir(config_dir).join(".pruned")
}

/// Where a removed agent's widget is kept: out of the runner's sight, as a
/// pruned desk's are, until Undo or a move by hand puts it back.
pub fn removed_dir(config_dir: &Path) -> PathBuf {
    dir(config_dir).join(".removed")
}

/// Desk `owner`'s widget `name` out of sight, to `.removed/<owner>-<name>-
/// <now>` (a `-2` after it when that is there already): moved, never
/// deleted. The id it is kept under, which Undo hands back.
pub fn remove(config_dir: &Path, owner: i64, name: &str, now: i64) -> Result<String, String> {
    let to = removed_dir(config_dir);
    std::fs::create_dir_all(&to).map_err(|e| format!("could not keep it: {e}"))?;
    let base = format!("{owner}-{name}-{now}");
    let id = (1..)
        .map(|n| {
            if n == 1 {
                base.clone()
            } else {
                format!("{base}-{n}")
            }
        })
        .find(|id| !to.join(id).exists())
        .unwrap_or(base);
    std::fs::rename(folder_of(config_dir, owner, name), to.join(&id))
        .map_err(|e| format!("could not move it: {e}"))?;
    Ok(id)
}

/// A removed widget back where it was: Undo. Refused for an id that is not
/// `owner`'s `name`, and when the name was taken meanwhile.
pub fn restore(config_dir: &Path, id: &str, owner: i64, name: &str) -> Result<(), String> {
    let ours = id
        .strip_prefix(&format!("{owner}-{name}-"))
        .is_some_and(|t| {
            t.starts_with(|c: char| c.is_ascii_digit())
                && t.bytes().all(|b| b.is_ascii_digit() || b == b'-')
        });
    let from = removed_dir(config_dir).join(id);
    if !ours || !from.is_dir() {
        return Err(format!("{name} was not removed from here"));
    }
    if let Some(why) = taken(config_dir, owner, name) {
        return Err(why);
    }
    std::fs::rename(&from, folder_of(config_dir, owner, name))
        .map_err(|e| format!("could not move it back: {e}"))
}

/// The desk an `@<id>` folder is for.
fn owner_of(folder_name: &str) -> Option<i64> {
    folder_name
        .strip_prefix('@')
        .and_then(|n| n.parse::<i64>().ok())
        .filter(|n| *n > 0)
}

/// Why `name` cannot be `owner`'s (0: shared), or None when it can. A
/// desk's own name may be another desk's own too, but not a shared one's;
/// a shared name may be no desk's own.
pub fn taken(config_dir: &Path, owner: i64, name: &str) -> Option<String> {
    clash(config_dir, None, owner, name)
}

/// Why widget `name` cannot move from `from` to `to` (0: shared), or None.
/// Its own folder is not in its way.
pub fn taken_moving(config_dir: &Path, from: i64, to: i64, name: &str) -> Option<String> {
    clash(config_dir, Some(from), to, name)
}

fn clash(config_dir: &Path, from: Option<i64>, to: i64, name: &str) -> Option<String> {
    if folder_of(config_dir, to, name).exists() {
        return Some(format!("a widget called {name} is already there"));
    }
    if to != 0 {
        let shared = from != Some(0) && dir(config_dir).join(name).exists();
        return shared.then(|| format!("a shared widget is called {name}: pick another name"));
    }
    std::fs::read_dir(dir(config_dir))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| owner_of(&e.file_name().to_string_lossy()))
        .filter(|d| Some(*d) != from)
        .find(|d| owned_dir(config_dir, *d).join(name).exists())
        .map(|d| format!("desk {d} has a widget of its own called {name}: rename one first"))
}

/// Where a widget runs: on the desk on the page, in the desk's folder, or
/// everywhere, in its own. Either way its own folder is first on PATH and
/// in `SNYVI_WIDGET_DIR`, so a desk widget reaches its script by name
/// (`lint`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    #[default]
    Desk,
    Global,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub command: String,
    #[serde(default = "every_default")]
    pub every: u64,
    #[serde(default = "timeout_default")]
    pub timeout: u64,
}

fn every_default() -> u64 {
    EVERY_DEFAULT
}
fn timeout_default() -> u64 {
    TIMEOUT_DEFAULT
}

/// One setting, drawn by snyvi on /sidebars as a row: a string, a number,
/// a choice or on/off.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub default: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<String>,
}

/// What a `widget.json` says.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub name: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub scope: Scope,
    pub run: Run,
    #[serde(default = "lines_default")]
    pub lines: u8,
    #[serde(default)]
    pub settings: BTreeMap<String, Field>,
}

fn lines_default() -> u8 {
    LINES_DEFAULT
}

impl Spec {
    /// The spec as it is run: every bound applied, the title filled in.
    fn bounded(mut self) -> Spec {
        self.run.every = self.run.every.max(EVERY_MIN);
        self.run.timeout = self.run.timeout.clamp(1, TIMEOUT_MAX);
        self.lines = self.lines.clamp(LINES_MIN, LINES_MAX);
        if self.title.trim().is_empty() {
            self.title = super::title_of(&self.name);
        }
        self.title = self.title.trim().chars().take(40).collect();
        self
    }

    /// The settings a run is given: each field's default, then what the
    /// reader set, for the fields the widget has.
    pub fn settings_with(&self, reader: &str) -> Value {
        let set: serde_json::Map<String, Value> = serde_json::from_str(reader).unwrap_or_default();
        let mut out = serde_json::Map::new();
        for (k, f) in &self.settings {
            out.insert(
                k.clone(),
                set.get(k).cloned().unwrap_or_else(|| f.default.clone()),
            );
        }
        Value::Object(out)
    }
}

/// A folder found under the widgets folder: its spec, or why it has none.
#[derive(Debug, Clone)]
pub struct Found {
    pub name: String,
    pub folder: PathBuf,
    pub spec: Result<Spec, String>,
    /// The desk it belongs to; 0 for a shared or global one.
    pub owner: i64,
}

impl Found {
    /// The widget's key among all of them: its name, or `@<desk>/<name>`
    /// for a desk's own, since two desks may each own a `status`.
    pub fn key(&self) -> String {
        if self.owner == 0 {
            self.name.clone()
        } else {
            format!("@{}/{}", self.owner, self.name)
        }
    }
}

/// The widget folders directly in `parent`, each for `owner`.
fn scan_in(parent: &Path, owner: i64) -> Vec<Found> {
    let Ok(rd) = std::fs::read_dir(parent) else {
        return Vec::new();
    };
    rd.flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name_ok(&name).then(|| Found {
                spec: read(&e.path(), &name).and_then(|s| {
                    if owner != 0 && s.scope == Scope::Global {
                        Err(
                            "a desk's own widget is on its desk: its widget.json cannot say global"
                                .into(),
                        )
                    } else {
                        Ok(s)
                    }
                }),
                name,
                folder: e.path(),
                owner,
            })
        })
        .collect()
}

/// Every widget folder: the shared and global ones by name, then each
/// desk's own. A folder whose `widget.json` does not read, or names another
/// widget, is listed with the reason, so /sidebars can say it rather than
/// leave the reader guessing; so is a shared one whose name a desk owns.
pub fn scan(config_dir: &Path) -> Vec<Found> {
    let base = dir(config_dir);
    let mut out = scan_in(&base, 0);
    out.sort_by(|a, b| a.name.cmp(&b.name));
    let mut desks: Vec<i64> = std::fs::read_dir(&base)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| owner_of(&e.file_name().to_string_lossy()))
        .collect();
    desks.sort_unstable();
    let mut owned = Vec::new();
    for d in desks {
        let mut mine = scan_in(&owned_dir(config_dir, d), d);
        mine.sort_by(|a, b| a.name.cmp(&b.name));
        owned.extend(mine);
    }
    for f in out.iter_mut() {
        if let Some(o) = owned.iter().find(|o| o.name == f.name) {
            f.spec = Err(format!(
                "desk {} has a widget of its own called {}: rename one of them",
                o.owner, f.name
            ));
        }
    }
    out.extend(owned);
    out
}

/// One folder's `widget.json`, read and bounded.
pub fn read(folder: &Path, name: &str) -> Result<Spec, String> {
    let text = std::fs::read_to_string(folder.join("widget.json"))
        .map_err(|_| "it has no widget.json".to_string())?;
    let spec: Spec =
        serde_json::from_str(&text).map_err(|e| format!("widget.json does not read: {e}"))?;
    if spec.name != name {
        return Err(format!(
            "widget.json says \"{}\", in a folder called {name}",
            spec.name
        ));
    }
    if spec.run.command.trim().is_empty() {
        return Err("widget.json has no run.command".into());
    }
    Ok(spec.bounded())
}

/// A stamp of everything in a folder, cheap to take every round: what
/// changed in it moves it, and only then is the folder hashed again.
pub fn stamp(folder: &Path) -> Option<(u128, u64, usize)> {
    let mut newest = 0u128;
    let (mut bytes, mut n) = (0u64, 0usize);
    for f in files(folder) {
        let m = std::fs::metadata(&f).ok()?;
        let t = m
            .modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_nanos();
        newest = newest.max(t);
        bytes += m.len();
        n += 1;
    }
    Some((newest, bytes, n))
}

/// Every regular file under a folder, in a fixed order; links are not
/// followed, so the hash is of what is in the folder and nothing it points
/// to.
fn files(folder: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut todo = vec![folder.to_path_buf()];
    while let Some(d) = todo.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let Ok(t) = e.file_type() else { continue };
            if t.is_dir() {
                todo.push(e.path());
            } else if t.is_file() {
                out.push(e.path());
            }
        }
    }
    out.sort();
    out
}

/// The hash of a whole folder -- every file's path in it and its bytes --
/// which is what the reader allows: a change to anything in it, the script
/// or a file it reads, asks again. Refused past `FOLDER_MAX`.
pub fn hash(folder: &Path) -> Result<String, String> {
    let mut h = blake3::Hasher::new();
    let mut total = 0u64;
    for f in files(folder) {
        let rel = f
            .strip_prefix(folder)
            .unwrap_or(&f)
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = std::fs::read(&f).map_err(|e| format!("could not read {rel}: {e}"))?;
        total += bytes.len() as u64;
        if total > FOLDER_MAX {
            return Err(format!(
                "the folder holds more than {} KB",
                FOLDER_MAX / 1024
            ));
        }
        h.update(rel.as_bytes());
        h.update(&[0]);
        h.update(&(bytes.len() as u64).to_le_bytes());
        h.update(&bytes);
    }
    Ok(h.finalize().to_hex().to_string())
}

/// The variable a run finds its own folder in, as the shell it runs in
/// writes it.
const DIR_VAR: &str = if cfg!(windows) {
    "%SNYVI_WIDGET_DIR%"
} else {
    "$SNYVI_WIDGET_DIR"
};

/// Whether a desk widget's command reaches the files in its own folder
/// (#111). A desk widget runs in the desk's folder, with its own first on
/// PATH and in `SNYVI_WIDGET_DIR`: a script of its own is named bare as the
/// command, or through the variable. A relative path to one of its files
/// (`./run.sh`), or its bare name as an argument (`sh run.sh`, `node x.js`),
/// is looked for in the desk's folder, where it is not -- so the run fails
/// on every desk, every time. Refused with how to write it. A global widget
/// runs in its own folder, where everything reaches.
pub fn lint(spec: &Spec, folder: &Path) -> Result<(), String> {
    if spec.scope == Scope::Global {
        return Ok(());
    }
    let ends = |w: &str| matches!(w, ";" | "&&" | "||" | "|" | "&" | "(" | "{");
    let mut first = true;
    for raw in spec.run.command.split_whitespace() {
        if ends(raw) {
            first = true;
            continue;
        }
        let then_first = raw.ends_with(';') || raw.ends_with('&') || raw.ends_with('|');
        let w = raw
            .trim_end_matches([';', '&', '|'])
            .trim_matches(|c| c == '"' || c == '\'');
        let skip = w.is_empty()
            || w.starts_with(['-', '$', '%', '~', '/', '\\'])
            || (first && w.contains('='))
            || w.contains("://");
        if !skip {
            let rel = w
                .strip_prefix("./")
                .or_else(|| w.strip_prefix(".\\"))
                .unwrap_or(w);
            let here = !rel.is_empty() && !rel.starts_with("..") && folder.join(rel).is_file();
            let pathed = w.contains(['/', '\\']);
            if here && (pathed || !first) {
                let sep = if cfg!(windows) { "\\" } else { "/" };
                return Err(format!(
                    "{rel} is in the widget's folder, but a desk widget runs in the desk's folder, where it is not: \
                     write \"{DIR_VAR}{sep}{rel}\" instead, or {rel} alone as the command for a script that runs by itself"
                ));
            }
            if here && first && !runnable(&folder.join(rel)) {
                return Err(format!(
                    "{rel} is named as the command but cannot run by itself: make it executable, \
                     or write sh \"{DIR_VAR}/{rel}\""
                ));
            }
        }
        // `LANG=C run.sh`: an assignment leaves the command still to come.
        if !(first && w.contains('=')) {
            first = false;
        }
        if then_first {
            first = true;
        }
    }
    Ok(())
}

/// Whether a file runs by itself: executable here, and on Windows by its
/// extension, which the shell goes by.
fn runnable(f: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(f).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        f.extension().is_some()
    }
}

/// A starter widget, for `snyvi widget new`: a `widget.json` and a script
/// that prints a body, on this system's shell.
pub fn starter(name: &str, scope: Scope) -> (String, &'static str, String) {
    let script = if cfg!(windows) { "run.ps1" } else { "run.sh" };
    // By name, as a desk widget must (`lint`): its folder is first on PATH.
    let command = if cfg!(windows) {
        format!("powershell -NoProfile -ExecutionPolicy Bypass -File \"{DIR_VAR}\\run.ps1\"")
    } else {
        "run.sh".to_string()
    };
    let spec = Spec {
        name: name.to_string(),
        title: super::title_of(name),
        scope,
        run: Run {
            command,
            every: 30,
            timeout: 5,
        },
        lines: 2,
        settings: BTreeMap::new(),
    };
    let json = serde_json::to_string_pretty(&spec).unwrap_or_default() + "\n";
    let body = if cfg!(windows) {
        "# stdin: {\"desk\":{...},\"settings\":{...},\"snyvi\":\"<version>\"}\n\
         # Print Markdown, or one JSON object: {\"body\":\"...\",\"tone\":\"ok\",\"count\":\"3\"}\n\
         $now = Get-Date -Format HH:mm\n\
         Write-Output \"**$(Split-Path -Leaf (Get-Location))** · $now\"\n"
            .to_string()
    } else {
        "#!/bin/sh\n\
         # stdin: {\"desk\":{...},\"settings\":{...},\"snyvi\":\"<version>\"}\n\
         # cwd: the desk's folder (a desk widget) or this folder (a global one);\n\
         # this folder is $SNYVI_WIDGET_DIR, first on PATH.\n\
         # Print Markdown, or one JSON object: {\"body\":\"...\",\"tone\":\"ok\",\"count\":\"3\"}\n\
         printf '**%s** · %s\\n' \"$(basename \"$PWD\")\" \"$(date +%H:%M)\"\n"
            .to_string()
    };
    (json, script, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("snyvi-wfiles-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(dir(&d).join("git")).unwrap();
        d
    }

    #[test]
    fn a_spec_is_read_and_bounded() {
        let cfg = folder("read");
        let w = dir(&cfg).join("git");
        std::fs::write(
            w.join("widget.json"),
            r#"{"name":"git","run":{"command":"./run.sh","every":1,"timeout":99},"lines":9,
               "settings":{"base":{"type":"string","default":"main"}}}"#,
        )
        .unwrap();
        let found = scan(&cfg);
        assert_eq!(found.len(), 1);
        let s = found[0].spec.clone().unwrap();
        assert_eq!(
            (s.run.every, s.run.timeout, s.lines),
            (EVERY_MIN, TIMEOUT_MAX, LINES_MAX)
        );
        assert_eq!(s.title, "Git");
        assert_eq!(
            s.settings_with(r#"{"base":"dev","other":1}"#),
            serde_json::json!({ "base": "dev" })
        );
        assert_eq!(s.settings_with("{}"), serde_json::json!({ "base": "main" }));
        let _ = std::fs::remove_dir_all(&cfg);
    }

    #[test]
    fn a_folder_that_says_another_name_is_listed_with_why() {
        let cfg = folder("name");
        std::fs::write(
            dir(&cfg).join("git").join("widget.json"),
            r#"{"name":"ci","run":{"command":"x"}}"#,
        )
        .unwrap();
        let found = scan(&cfg);
        assert!(found[0]
            .spec
            .as_ref()
            .unwrap_err()
            .contains("in a folder called git"));
        let _ = std::fs::remove_dir_all(&cfg);
    }

    #[test]
    fn the_hash_covers_every_file_and_has_a_cap() {
        let cfg = folder("hash");
        let w = dir(&cfg).join("git");
        std::fs::write(w.join("run.sh"), "echo a").unwrap();
        let a = hash(&w).unwrap();
        std::fs::create_dir_all(w.join("lib")).unwrap();
        std::fs::write(w.join("lib").join("x.sh"), "echo b").unwrap();
        let b = hash(&w).unwrap();
        assert_ne!(a, b, "a file the script could source is in the hash");
        assert_eq!(b, hash(&w).unwrap());
        std::fs::write(w.join("big"), vec![b'x'; FOLDER_MAX as usize]).unwrap();
        assert!(hash(&w).unwrap_err().contains("more than 64 KB"));
        let _ = std::fs::remove_dir_all(&cfg);
    }

    #[cfg(unix)]
    fn desk_spec(command: &str) -> Spec {
        Spec {
            name: "git".into(),
            title: "Git".into(),
            scope: Scope::Desk,
            run: Run {
                command: command.into(),
                every: 30,
                timeout: 5,
            },
            lines: 2,
            settings: BTreeMap::new(),
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_desk_widget_must_reach_its_own_files() {
        use std::os::unix::fs::PermissionsExt;
        let cfg = folder("lint");
        let w = dir(&cfg).join("git");
        std::fs::write(w.join("run.sh"), "#!/bin/sh\necho hi\n").unwrap();
        std::fs::set_permissions(w.join("run.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(w.join("x.js"), "1").unwrap();
        for bad in [
            "./run.sh",
            "sh run.sh",
            "node x.js",
            "cd . && sh ./run.sh",
            "true; node x.js",
        ] {
            let why = lint(&desk_spec(bad), &w).unwrap_err();
            assert!(why.contains("$SNYVI_WIDGET_DIR/"), "{bad}: {why}");
        }
        for good in [
            "run.sh",
            "sh \"$SNYVI_WIDGET_DIR/run.sh\"",
            "node \"$SNYVI_WIDGET_DIR/x.js\"",
            "git status --short",
            "LANG=C run.sh",
            "true && run.sh",
        ] {
            assert_eq!(lint(&desk_spec(good), &w), Ok(()), "{good}");
        }
        // A script named as the command has to be able to run.
        std::fs::set_permissions(w.join("run.sh"), std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(lint(&desk_spec("run.sh"), &w)
            .unwrap_err()
            .contains("make it executable"));
        // A global widget runs in its own folder: anything reaches.
        let mut g = desk_spec("sh ./run.sh");
        g.scope = Scope::Global;
        assert_eq!(lint(&g, &w), Ok(()));
        let _ = std::fs::remove_dir_all(&cfg);
    }

    #[cfg(unix)]
    #[test]
    fn a_starter_passes_its_own_check() {
        use std::os::unix::fs::PermissionsExt;
        let cfg = folder("starter");
        let w = dir(&cfg).join("git");
        let (json, script, body) = starter("git", Scope::Desk);
        std::fs::write(w.join("widget.json"), json).unwrap();
        std::fs::write(w.join(script), body).unwrap();
        std::fs::set_permissions(w.join(script), std::fs::Permissions::from_mode(0o755)).unwrap();
        let spec = read(&w, "git").unwrap();
        assert_eq!(lint(&spec, &w), Ok(()));
        let _ = std::fs::remove_dir_all(&cfg);
    }

    fn write_spec(folder: &Path, name: &str, scope: &str) {
        std::fs::create_dir_all(folder).unwrap();
        std::fs::write(
            folder.join("widget.json"),
            format!(r#"{{"name":"{name}","scope":"{scope}","run":{{"command":"run.sh"}}}}"#),
        )
        .unwrap();
    }

    #[test]
    fn a_desks_own_widgets_are_found_under_its_id() {
        let cfg = folder("owned");
        write_spec(&dir(&cfg).join("git"), "git", "desk");
        write_spec(&folder_of(&cfg, 7, "arena"), "arena", "desk");
        write_spec(&folder_of(&cfg, 9, "arena"), "arena", "desk");
        write_spec(&folder_of(&cfg, 7, "left"), "left", "global");
        // Not a desk's folder: no id, or not one.
        write_spec(&dir(&cfg).join("@x").join("odd"), "odd", "desk");
        write_spec(&pruned_dir(&cfg).join("3-1").join("old"), "old", "desk");
        let found = scan(&cfg);
        let keys: Vec<String> = found.iter().map(Found::key).collect();
        assert_eq!(keys, ["git", "@7/arena", "@7/left", "@9/arena"]);
        assert!(found[0].spec.is_ok() && found[1].spec.is_ok() && found[3].spec.is_ok());
        assert!(found[2]
            .spec
            .as_ref()
            .unwrap_err()
            .contains("cannot say global"));
        assert_eq!((found[1].owner, found[3].owner), (7, 9));
        // A shared folder a desk owns the name of is not run.
        write_spec(&dir(&cfg).join("arena"), "arena", "desk");
        let found = scan(&cfg);
        let shared = found.iter().find(|f| f.key() == "arena").unwrap();
        assert!(shared
            .spec
            .as_ref()
            .unwrap_err()
            .contains("desk 7 has a widget of its own"));
        let _ = std::fs::remove_dir_all(&cfg);
    }

    #[test]
    fn a_name_is_shared_or_owned_never_both() {
        let cfg = folder("taken");
        write_spec(&dir(&cfg).join("git"), "git", "desk");
        write_spec(&folder_of(&cfg, 7, "arena"), "arena", "desk");
        // Another desk may own one of the same name; the same desk may not.
        assert_eq!(taken(&cfg, 9, "arena"), None);
        assert!(taken(&cfg, 7, "arena").unwrap().contains("already there"));
        // A desk may not own a shared name, nor share an owned one.
        assert!(taken(&cfg, 7, "git").unwrap().contains("shared widget"));
        assert!(taken(&cfg, 0, "arena").unwrap().contains("desk 7"));
        assert_eq!(taken(&cfg, 0, "new"), None);
        // Moving, its own folder is not in its way; another desk's is.
        assert_eq!(taken_moving(&cfg, 7, 0, "arena"), None);
        assert_eq!(taken_moving(&cfg, 0, 7, "git"), None);
        write_spec(&folder_of(&cfg, 9, "arena"), "arena", "desk");
        assert!(taken_moving(&cfg, 7, 0, "arena")
            .unwrap()
            .contains("desk 9"));
        let _ = std::fs::remove_dir_all(&cfg);
    }

    #[test]
    fn a_folder_moved_keeps_its_hash() {
        let cfg = folder("moved");
        let from = dir(&cfg).join("git");
        std::fs::write(from.join("run.sh"), "echo a").unwrap();
        let before = hash(&from).unwrap();
        let to = folder_of(&cfg, 7, "git");
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::rename(&from, &to).unwrap();
        assert_eq!(
            hash(&to).unwrap(),
            before,
            "what was allowed is still allowed"
        );
        let _ = std::fs::remove_dir_all(&cfg);
    }

    #[test]
    fn a_removed_widget_is_kept_out_of_sight_and_comes_back() {
        let cfg = folder("removed");
        write_spec(&folder_of(&cfg, 7, "arena"), "arena", "desk");
        std::fs::write(folder_of(&cfg, 7, "arena").join("run.sh"), "echo a").unwrap();
        let before = hash(&folder_of(&cfg, 7, "arena")).unwrap();
        let id = remove(&cfg, 7, "arena", 100).unwrap();
        assert_eq!(id, "7-arena-100");
        assert!(
            scan(&cfg).iter().all(|f| f.name != "arena"),
            "nothing runs from .removed"
        );
        assert!(
            removed_dir(&cfg).join(&id).join("run.sh").is_file(),
            "moved, not deleted"
        );
        // Removed again in the same second: kept beside it, not over it.
        write_spec(&folder_of(&cfg, 7, "arena"), "arena", "desk");
        assert_eq!(remove(&cfg, 7, "arena", 100).unwrap(), "7-arena-100-2");
        // Undo only puts back what was this desk's and this name's.
        assert!(restore(&cfg, &id, 9, "arena").is_err());
        assert!(restore(&cfg, "../../etc", 7, "arena").is_err());
        assert!(restore(&cfg, "7-arena-", 7, "arena").is_err());
        restore(&cfg, &id, 7, "arena").unwrap();
        assert_eq!(
            hash(&folder_of(&cfg, 7, "arena")).unwrap(),
            before,
            "allowed as it was"
        );
        // The name taken meanwhile: the other stays where it is kept.
        assert!(restore(&cfg, "7-arena-100-2", 7, "arena")
            .unwrap_err()
            .contains("already there"));
        let _ = std::fs::remove_dir_all(&cfg);
    }

    #[test]
    fn a_starter_reads_back() {
        let (json, _, _) = starter("disk-free", Scope::Global);
        let s: Spec = serde_json::from_str(&json).unwrap();
        assert_eq!(
            (s.name.as_str(), s.title.as_str(), s.scope),
            ("disk-free", "Disk free", Scope::Global)
        );
    }
}
