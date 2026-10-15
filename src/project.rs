//! Map a working directory to a project (git root or the directory itself).

use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct ProjectInfo {
    pub root: PathBuf,
    pub name: String,
}

pub fn resolve(start: &Path) -> ProjectInfo {
    let start = if start.is_file() {
        start
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| start.to_path_buf())
    } else {
        start.to_path_buf()
    };
    let start = start.canonicalize().unwrap_or(start);
    let mut cur: Option<&Path> = Some(&start);
    while let Some(dir) = cur {
        if dir.join(".git").exists() {
            return ProjectInfo {
                root: dir.to_path_buf(),
                name: dir_name(dir),
            };
        }
        cur = dir.parent();
    }
    ProjectInfo {
        name: dir_name(&start),
        root: start,
    }
}

/// `resolve`, with a desk's folder as the floor of the walk up. A repository
/// *above* the desk that holds other repositories -- one made a folder too
/// high, around every project the reader keeps beside it, or a home folder's
/// dotfiles -- is not the desk's project: the desk's folder is. What the
/// reader gave a desk is what they called the project, and the walk up from
/// it otherwise filed a desk with no `.git` of its own under its parent
/// folder's name. A monorepo holds no repository of its own below it, so a
/// desk on one of its folders still files under the monorepo, as before. A
/// repository inside the desk still wins: the floor only stops the walk
/// going past it.
pub fn resolve_on_desk(start: &Path, desk: &Path) -> ProjectInfo {
    let found = resolve(start);
    let desk = desk.canonicalize().unwrap_or_else(|_| desk.to_path_buf());
    let start = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
    if start.starts_with(&desk)
        && desk != found.root
        && desk.starts_with(&found.root)
        && holds_a_repository(&found.root)
    {
        return ProjectInfo {
            name: dir_name(&desk),
            root: desk,
        };
    }
    found
}

/// How far down, and how many folders, `holds_a_repository` looks: a folder
/// of projects has them near the top, and a send must not walk a disk.
const HOLD_DEPTH: usize = 3;
const HOLD_DIRS: usize = 2000;

/// Whether a repository has another one in it: a `.git` folder below its
/// root, near the top. A `.git` *file* is a submodule or a worktree, which a
/// monorepo may well have, and says nothing. Dependencies and build output
/// are not looked in.
fn holds_a_repository(root: &Path) -> bool {
    let mut seen = 0;
    let mut level = vec![root.to_path_buf()];
    for _ in 0..HOLD_DEPTH {
        let mut next = Vec::new();
        for dir in level {
            let Ok(rd) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in rd.flatten() {
                if !e.file_type().is_ok_and(|t| t.is_dir()) {
                    continue;
                }
                let name = e.file_name();
                let name = name.to_string_lossy();
                if name.starts_with('.') || name == "node_modules" || name == "target" {
                    continue;
                }
                if e.path().join(".git").is_dir() {
                    return true;
                }
                seen += 1;
                if seen >= HOLD_DIRS {
                    return false;
                }
                next.push(e.path());
            }
        }
        level = next;
    }
    false
}

fn dir_name(p: &Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "root".to_string())
}

/// The branch a folder is on, found by walking up to the repository it is in.
///
/// The same read a pane's prompt does in the shell's own builtins (see
/// `crate::prompt`), for the callers that are not a shell: the pane header,
/// which is how a pane running `cmd.exe`, or a shell snyvi has no dressing
/// for, still says which branch it is on. No process.
pub fn head_of(dir: &Path) -> Option<String> {
    let mut cur: Option<&Path> = Some(dir);
    let root = loop {
        let d = cur?;
        if d.join(".git").exists() {
            break d;
        }
        cur = d.parent();
    };
    let dot = root.join(".git");
    // A worktree or a submodule keeps .git as a file naming the real one.
    let git = if dot.is_file() {
        let line = std::fs::read_to_string(&dot).ok()?;
        let named = Path::new(line.trim().strip_prefix("gitdir: ")?).to_path_buf();
        if named.is_absolute() {
            named
        } else {
            root.join(named)
        }
    } else {
        dot
    };
    let head = std::fs::read_to_string(git.join("HEAD")).ok()?;
    let head = head.trim();
    Some(
        head.strip_prefix("ref: refs/heads/")
            .map(str::to_string)
            .unwrap_or_else(|| head.chars().take(8).collect()),
    )
}

/// Whether the tree has changes -- the one part of this that git alone can
/// answer, and the reason it is here rather than in a prompt: it runs a
/// process, so the daemon does it on its own tick and a prompt never waits
/// for it. Untracked files do not count; they are not changes to a branch.
pub fn modified(dir: &Path) -> Option<bool> {
    let mut cmd = std::process::Command::new("git");
    // No repository's config runs a command on the daemon's tick: an
    // fsmonitor hook is exactly that, and a panel can `cd` anywhere.
    cmd.args([
        "-c",
        "core.fsmonitor=false",
        "--no-optional-locks",
        "status",
        "--porcelain",
        "--untracked-files=no",
    ])
    .current_dir(dunce::simplified(dir))
    .stderr(std::process::Stdio::null());
    // The daemon has no console on Windows, so a console program it runs is
    // given one of its own: a window flashed up on every tick, for every
    // folder a panel works in.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(crate::platform::CREATE_NO_WINDOW);
    }
    let out = cmd.output().ok()?;
    out.status.success().then_some(!out.stdout.is_empty())
}

/// Current branch name, if the project is a git repo. Cheap: reads HEAD, in
/// `.git` or -- in a worktree, where `.git` is a file -- in the folder it
/// names (`head_of`). Only `root`'s own: a folder in no repository is on no
/// branch, whatever repository is above it.
pub fn branch(root: &Path) -> Option<String> {
    if !root.join(".git").exists() {
        return None;
    }
    head_of(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_branch_is_read_from_a_folder_inside_the_repository() {
        let dir = crate::store::tempdir::Dir::new("snyvi-head");
        let deep = dir.path.join("a/b/c");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::create_dir_all(dir.path.join(".git")).unwrap();
        std::fs::write(
            dir.path.join(".git/HEAD"),
            "ref: refs/heads/claude/snappy-reading\n",
        )
        .unwrap();
        assert_eq!(head_of(&deep).as_deref(), Some("claude/snappy-reading"));
    }

    #[test]
    fn a_detached_head_is_the_commit_it_is_on() {
        let dir = crate::store::tempdir::Dir::new("snyvi-head");
        std::fs::create_dir_all(dir.path.join(".git")).unwrap();
        std::fs::write(
            dir.path.join(".git/HEAD"),
            "9adceca1234567890abcdef1234567890abcdef1\n",
        )
        .unwrap();
        assert_eq!(head_of(&dir.path).as_deref(), Some("9adceca1"));
    }

    #[test]
    fn a_worktree_keeps_git_as_a_file_naming_the_real_one() {
        let dir = crate::store::tempdir::Dir::new("snyvi-head");
        let real = dir.path.join("store/worktrees/one");
        std::fs::create_dir_all(&real).unwrap();
        std::fs::write(real.join("HEAD"), "ref: refs/heads/side\n").unwrap();
        let tree = dir.path.join("tree");
        std::fs::create_dir_all(&tree).unwrap();
        std::fs::write(tree.join(".git"), format!("gitdir: {}\n", real.display())).unwrap();
        assert_eq!(head_of(&tree).as_deref(), Some("side"));
        // And a project there says its branch, as a document's head shows it.
        assert_eq!(branch(&tree).as_deref(), Some("side"));
        assert_eq!(branch(&tree.join("sub")), None, "only the root's own");
    }

    #[test]
    fn a_desk_under_a_repository_a_folder_too_high_is_its_own_project() {
        // D:\workspaces\.git, a desk on D:\workspaces\taskflow with none, and
        // taskflow\app with its own.
        let dir = crate::store::tempdir::Dir::new("snyvi-floor");
        let desk = dir.path.join("taskflow");
        let app = desk.join("app");
        std::fs::create_dir_all(dir.path.join(".git")).unwrap();
        std::fs::create_dir_all(app.join(".git")).unwrap();
        std::fs::create_dir_all(desk.join("supabase")).unwrap();
        let on = |p: &Path| resolve_on_desk(p, &desk);
        assert_eq!(on(&desk).name, "taskflow");
        assert_eq!(on(&desk.join("supabase")).name, "taskflow");
        assert_eq!(
            on(&app).name,
            "app",
            "a repository inside the desk still wins"
        );
        // Outside the desk the floor says nothing.
        assert_eq!(on(&dir.path).root, resolve(&dir.path).root);
        // And a desk on its repository's own folder is that repository.
        assert_eq!(resolve_on_desk(&app, &app).root, resolve(&app).root);
    }

    #[test]
    fn a_desk_on_a_monorepos_folder_is_the_monorepo() {
        // One repository, no other inside it: its folders are its own.
        let dir = crate::store::tempdir::Dir::new("snyvi-mono");
        let web = dir.path.join("packages/web");
        std::fs::create_dir_all(dir.path.join(".git")).unwrap();
        std::fs::create_dir_all(web.join("src")).unwrap();
        // A submodule keeps `.git` as a file, and says nothing.
        std::fs::create_dir_all(dir.path.join("vendor/lib")).unwrap();
        std::fs::write(
            dir.path.join("vendor/lib/.git"),
            "gitdir: ../../.git/modules/lib
",
        )
        .unwrap();
        // Nor does a dependency that came with its own.
        std::fs::create_dir_all(web.join("node_modules/dep/.git")).unwrap();
        let got = resolve_on_desk(&web.join("src"), &web);
        assert_eq!(got.root, resolve(&web).root);
        assert_ne!(got.name, "web");
    }

    #[test]
    fn a_folder_outside_any_repository_is_on_no_branch() {
        let dir = crate::store::tempdir::Dir::new("snyvi-head");
        // /tmp is not a repository; neither is a fresh folder under it.
        assert_eq!(head_of(&dir.path), None);
    }
}
