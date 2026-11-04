//! Local git operations on top of libgit2.
//!
//! Network operations authenticate over HTTPS with the signed-in account's
//! token, the same way `gh` sets up git credentials.

use anyhow::{Context, Result, anyhow, bail};
use git2::{
    AnnotatedCommit, BranchType, Cred, DiffOptions, FetchOptions, IndexAddOption, PushOptions, RemoteCallbacks, Repository, Signature,
    Status, StatusOptions, build::CheckoutBuilder,
};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Added,
    Modified,
    Deleted,
    Renamed,
    Untracked,
    Conflicted,
    TypeChange,
}

#[derive(Debug, Clone)]
pub struct FileStatus {
    pub path: String,
    pub change: Change,
}

#[derive(Debug, Clone, Default)]
pub struct WorkingState {
    pub branch: String,
    pub head_short: String,
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub staged: Vec<FileStatus>,
    pub unstaged: Vec<FileStatus>,
    pub conflicts: Vec<String>,
    pub merging: bool,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub sha: String,
    pub summary: String,
    pub author: String,
    pub time: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct LocalBranch {
    pub name: String,
    pub is_head: bool,
    pub remote: bool,
}

fn callbacks(token: Option<String>) -> RemoteCallbacks<'static> {
    let mut cb = RemoteCallbacks::new();
    let mut tries = 0;
    cb.credentials(move |_url, username, allowed| {
        tries += 1;
        // libgit2 keeps asking forever if creds are wrong, so bail after a couple tries
        if tries > 3 {
            return Err(git2::Error::from_str("authentication failed"));
        }
        if allowed.contains(git2::CredentialType::USER_PASS_PLAINTEXT)
            && let Some(t) = &token
        {
            return Cred::userpass_plaintext("x-access-token", t);
        }
        if allowed.contains(git2::CredentialType::SSH_KEY) {
            return Cred::ssh_key_from_agent(username.unwrap_or("git"));
        }
        Cred::default()
    });
    cb
}

pub fn clone(url: &str, dest: &Path, token: Option<String>) -> Result<PathBuf> {
    let mut fo = FetchOptions::new();
    fo.remote_callbacks(callbacks(token));
    git2::build::RepoBuilder::new().fetch_options(fo).clone(url, dest).with_context(|| format!("cloning {url}"))?;
    Ok(dest.to_path_buf())
}

pub fn open(path: &Path) -> Result<Repository> {
    Repository::discover(path).with_context(|| format!("{} is not a git repository", path.display()))
}

fn change_of(s: Status, index: bool) -> Option<Change> {
    if s.is_conflicted() {
        return Some(Change::Conflicted);
    }
    if index {
        if s.is_index_new() {
            Some(Change::Added)
        } else if s.is_index_modified() {
            Some(Change::Modified)
        } else if s.is_index_deleted() {
            Some(Change::Deleted)
        } else if s.is_index_renamed() {
            Some(Change::Renamed)
        } else if s.is_index_typechange() {
            Some(Change::TypeChange)
        } else {
            None
        }
    } else if s.is_wt_new() {
        Some(Change::Untracked)
    } else if s.is_wt_modified() {
        Some(Change::Modified)
    } else if s.is_wt_deleted() {
        Some(Change::Deleted)
    } else if s.is_wt_renamed() {
        Some(Change::Renamed)
    } else if s.is_wt_typechange() {
        Some(Change::TypeChange)
    } else {
        None
    }
}

pub fn working_state(path: &Path) -> Result<WorkingState> {
    let repo = open(path)?;
    let mut st = WorkingState::default();
    match repo.head() {
        Ok(head) => {
            st.branch = head.shorthand().unwrap_or("HEAD").to_string();
            if let Some(oid) = head.target() {
                st.head_short = oid.to_string()[..7].to_string();
            }
            if head.is_branch()
                && let Ok(b) = repo.find_branch(&st.branch, BranchType::Local)
                && let Ok(up) = b.upstream()
            {
                st.upstream = up.name().ok().flatten().map(str::to_string);
                if let (Some(l), Some(r)) = (b.get().target(), up.get().target()) {
                    let (a, bh) = repo.graph_ahead_behind(l, r)?;
                    st.ahead = a;
                    st.behind = bh;
                }
            }
        }
        // fresh repo with no commits yet
        Err(_) => st.branch = "main".into(),
    }
    st.merging = repo.state() == git2::RepositoryState::Merge;

    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true).renames_head_to_index(true);
    for e in repo.statuses(Some(&mut opts))?.iter() {
        let path = e.path().unwrap_or_default().to_string();
        let s = e.status();
        if s.is_conflicted() {
            st.conflicts.push(path);
            continue;
        }
        if let Some(c) = change_of(s, true) {
            st.staged.push(FileStatus { path: path.clone(), change: c });
        }
        if let Some(c) = change_of(s, false) {
            st.unstaged.push(FileStatus { path, change: c });
        }
    }
    Ok(st)
}

pub fn stage(path: &Path, files: &[String]) -> Result<()> {
    let repo = open(path)?;
    let mut idx = repo.index()?;
    let root = repo.workdir().ok_or_else(|| anyhow!("bare repository"))?.to_path_buf();
    for f in files {
        if root.join(f).exists() {
            idx.add_path(Path::new(f))?;
        } else {
            idx.remove_path(Path::new(f))?;
        }
    }
    idx.write()?;
    Ok(())
}

pub fn stage_all(path: &Path) -> Result<()> {
    let repo = open(path)?;
    let mut idx = repo.index()?;
    idx.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;
    idx.update_all(["*"].iter(), None)?;
    idx.write()?;
    Ok(())
}

pub fn unstage(path: &Path, files: &[String]) -> Result<()> {
    let repo = open(path)?;
    match repo.head().ok().and_then(|h| h.peel_to_commit().ok()) {
        Some(c) => repo.reset_default(Some(c.as_object()), files.iter())?,
        None => {
            // no HEAD yet, so unstaging means dropping it from the index
            let mut idx = repo.index()?;
            for f in files {
                idx.remove_path(Path::new(f))?;
            }
            idx.write()?;
        }
    }
    Ok(())
}

pub fn discard(path: &Path, files: &[String]) -> Result<()> {
    let repo = open(path)?;
    let mut cb = CheckoutBuilder::new();
    cb.force();
    for f in files {
        cb.path(f);
    }
    repo.checkout_head(Some(&mut cb))?;
    Ok(())
}

pub fn commit(path: &Path, message: &str) -> Result<String> {
    let mut repo = open(path)?;
    // finishing a merge: MERGE_HEAD becomes the second parent
    let mut merge_heads = Vec::new();
    if repo.state() == git2::RepositoryState::Merge {
        repo.mergehead_foreach(|oid| {
            merge_heads.push(*oid);
            true
        })?;
    }
    let sig = repo.signature().context("git user.name / user.email are not configured")?;
    let mut idx = repo.index()?;
    if idx.has_conflicts() {
        bail!("resolve all conflicts before committing");
    }
    let tree = repo.find_tree(idx.write_tree()?)?;
    let mut parents = Vec::new();
    if let Ok(h) = repo.head()
        && let Ok(c) = h.peel_to_commit()
    {
        parents.push(c);
    }
    for oid in merge_heads {
        parents.push(repo.find_commit(oid)?);
    }
    let refs: Vec<&git2::Commit> = parents.iter().collect();
    let oid = repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &refs)?;
    repo.cleanup_state()?;
    Ok(oid.to_string())
}

pub fn file_diff(path: &Path, file: &str, staged: bool) -> Result<String> {
    let repo = open(path)?;
    let mut opts = DiffOptions::new();
    opts.pathspec(file).include_untracked(true).show_untracked_content(true).recurse_untracked_dirs(true);
    let diff = if staged {
        let head = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
        repo.diff_tree_to_index(head.as_ref(), None, Some(&mut opts))?
    } else {
        repo.diff_index_to_workdir(None, Some(&mut opts))?
    };
    let mut out = String::new();
    diff.print(git2::DiffFormat::Patch, |_d, _h, line| {
        let prefix = match line.origin() {
            '+' | '-' | ' ' => line.origin().to_string(),
            _ => String::new(),
        };
        out.push_str(&prefix);
        out.push_str(&String::from_utf8_lossy(line.content()));
        true
    })?;
    Ok(out)
}

pub fn log(path: &Path, limit: usize) -> Result<Vec<LogEntry>> {
    let repo = open(path)?;
    let mut walk = repo.revwalk()?;
    if walk.push_head().is_err() {
        return Ok(vec![]);
    }
    walk.set_sorting(git2::Sort::TIME)?;
    let mut out = Vec::new();
    for oid in walk.take(limit) {
        let c = repo.find_commit(oid?)?;
        out.push(LogEntry {
            sha: c.id().to_string(),
            summary: c.summary().ok().flatten().unwrap_or("").to_string(),
            author: c.author().name().unwrap_or("").to_string(),
            time: chrono::DateTime::from_timestamp(c.time().seconds(), 0).unwrap_or_default(),
        });
    }
    Ok(out)
}

pub fn branches(path: &Path) -> Result<Vec<LocalBranch>> {
    let repo = open(path)?;
    let mut out = Vec::new();
    for b in repo.branches(None)? {
        let (b, kind) = b?;
        let name = b.name()?.unwrap_or_default().to_string();
        if name.ends_with("/HEAD") {
            continue;
        }
        out.push(LocalBranch { is_head: b.is_head(), remote: kind == BranchType::Remote, name });
    }
    out.sort_by(|a, b| (a.remote, &a.name).cmp(&(b.remote, &b.name)));
    Ok(out)
}

pub fn create_branch(path: &Path, name: &str, checkout: bool) -> Result<()> {
    let repo = open(path)?;
    let head = repo.head()?.peel_to_commit()?;
    repo.branch(name, &head, false)?;
    if checkout {
        switch_branch(path, name)?;
    }
    Ok(())
}

pub fn switch_branch(path: &Path, name: &str) -> Result<()> {
    let repo = open(path)?;
    // remote branch picked: make a local tracking branch first, like `git switch` does
    let local_name = if let Ok(rb) = repo.find_branch(name, BranchType::Remote) {
        let short = name.split_once('/').map(|(_, b)| b).unwrap_or(name).to_string();
        if repo.find_branch(&short, BranchType::Local).is_err() {
            let c = rb.get().peel_to_commit()?;
            let mut lb = repo.branch(&short, &c, false)?;
            lb.set_upstream(Some(name))?;
        }
        short
    } else {
        name.to_string()
    };
    let refname = format!("refs/heads/{local_name}");
    let obj = repo.revparse_single(&refname)?;
    repo.checkout_tree(&obj, Some(CheckoutBuilder::new().safe()))?;
    repo.set_head(&refname)?;
    Ok(())
}

pub fn delete_branch(path: &Path, name: &str) -> Result<()> {
    let repo = open(path)?;
    repo.find_branch(name, BranchType::Local)?.delete()?;
    Ok(())
}

pub fn remotes(path: &Path) -> Result<Vec<(String, String)>> {
    let repo = open(path)?;
    let names = repo.remotes()?;
    Ok(names
        .iter()
        .filter_map(|r| r.ok().flatten())
        .filter_map(|n| repo.find_remote(n).ok().map(|r| (n.to_string(), r.url().unwrap_or_default().to_string())))
        .collect())
}

pub fn add_remote(path: &Path, name: &str, url: &str) -> Result<()> {
    open(path)?.remote(name, url)?;
    Ok(())
}

pub fn remove_remote(path: &Path, name: &str) -> Result<()> {
    open(path)?.remote_delete(name)?;
    Ok(())
}

pub fn set_remote_url(path: &Path, name: &str, url: &str) -> Result<()> {
    open(path)?.remote_set_url(name, url)?;
    Ok(())
}

fn upstream_remote(repo: &Repository) -> String {
    repo.head()
        .ok()
        .and_then(|h| h.shorthand().ok().map(str::to_string))
        .and_then(|b| repo.branch_upstream_remote(&format!("refs/heads/{b}")).ok())
        .and_then(|buf| buf.as_str().ok().map(str::to_string))
        .unwrap_or_else(|| "origin".into())
}

pub fn fetch(path: &Path, token: Option<String>) -> Result<()> {
    let repo = open(path)?;
    let remote_name = upstream_remote(&repo);
    let mut remote = repo.find_remote(&remote_name)?;
    let mut fo = FetchOptions::new();
    fo.remote_callbacks(callbacks(token));
    fo.prune(git2::FetchPrune::On);
    remote.fetch::<&str>(&[], Some(&mut fo), None)?;
    Ok(())
}

pub fn push(path: &Path, token: Option<String>) -> Result<()> {
    let repo = open(path)?;
    let head = repo.head()?;
    let branch = head.shorthand().map_err(|_| anyhow!("detached HEAD"))?.to_string();
    let remote_name = upstream_remote(&repo);
    let mut remote = repo.find_remote(&remote_name)?;
    let mut po = PushOptions::new();
    let mut cb = callbacks(token);
    let rejected = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
    let rej = rejected.clone();
    cb.push_update_reference(move |_r, status| {
        if let Some(s) = status {
            *rej.lock().unwrap() = Some(s.to_string());
        }
        Ok(())
    });
    po.remote_callbacks(cb);
    let spec = format!("refs/heads/{branch}:refs/heads/{branch}");
    remote.push(&[spec.as_str()], Some(&mut po))?;
    if let Some(r) = rejected.lock().unwrap().take() {
        bail!("push rejected: {r}");
    }
    // first push of a new branch: track it so ahead/behind works afterwards
    if let Ok(mut b) = repo.find_branch(&branch, BranchType::Local)
        && b.upstream().is_err()
    {
        let _ = b.set_upstream(Some(&format!("{remote_name}/{branch}")));
    }
    Ok(())
}

/// Outcome of a merge or pull so the UI can tell the user what happened.
#[derive(Debug, Clone, PartialEq)]
pub enum MergeOutcome {
    UpToDate,
    FastForward,
    Merged,
    Conflicts(usize),
}

fn merge_annotated(repo: &Repository, theirs: &AnnotatedCommit, label: &str) -> Result<MergeOutcome> {
    let (analysis, _) = repo.merge_analysis(&[theirs])?;
    if analysis.is_up_to_date() {
        return Ok(MergeOutcome::UpToDate);
    }
    if analysis.is_fast_forward() {
        let mut head = repo.head()?;
        head.set_target(theirs.id(), &format!("fast-forward to {label}"))?;
        repo.checkout_head(Some(CheckoutBuilder::new().safe()))?;
        return Ok(MergeOutcome::FastForward);
    }
    repo.merge(&[theirs], None, Some(CheckoutBuilder::new().safe().allow_conflicts(true)))?;
    let idx = repo.index()?;
    if idx.has_conflicts() {
        let n = idx.conflicts()?.count();
        return Ok(MergeOutcome::Conflicts(n));
    }
    let sig: Signature = repo.signature()?;
    let mut idx = repo.index()?;
    let tree = repo.find_tree(idx.write_tree()?)?;
    let ours = repo.head()?.peel_to_commit()?;
    let their_commit = repo.find_commit(theirs.id())?;
    let head_name = repo.head()?.shorthand().unwrap_or("HEAD").to_string();
    repo.commit(Some("HEAD"), &sig, &sig, &format!("Merge {label} into {head_name}"), &tree, &[&ours, &their_commit])?;
    repo.cleanup_state()?;
    Ok(MergeOutcome::Merged)
}

pub fn merge_branch(path: &Path, branch: &str) -> Result<MergeOutcome> {
    let repo = open(path)?;
    let r = repo.find_branch(branch, BranchType::Local).or_else(|_| repo.find_branch(branch, BranchType::Remote))?;
    let ac = repo.reference_to_annotated_commit(r.get())?;
    merge_annotated(&repo, &ac, &format!("branch '{branch}'"))
}

pub fn pull(path: &Path, token: Option<String>) -> Result<MergeOutcome> {
    fetch(path, token)?;
    let repo = open(path)?;
    let head = repo.head()?;
    let branch = head.shorthand().map_err(|_| anyhow!("detached HEAD"))?.to_string();
    let local = repo.find_branch(&branch, BranchType::Local)?;
    let up = local.upstream().context("this branch has no upstream to pull from")?;
    let ac = repo.reference_to_annotated_commit(up.get())?;
    let label = up.name()?.unwrap_or("upstream").to_string();
    merge_annotated(&repo, &ac, &label)
}

/// Working copy of a conflicted file, markers included, for the merge resolver.
pub fn conflict_text(path: &Path, file: &str) -> Result<String> {
    let repo = open(path)?;
    let idx = repo.index()?;
    for c in idx.conflicts()? {
        let c = c?;
        let p = c.our.as_ref().or(c.their.as_ref()).map(|e| String::from_utf8_lossy(&e.path).into_owned());
        if p.as_deref() == Some(file) {
            let root = repo.workdir().ok_or_else(|| anyhow!("bare repository"))?;
            return Ok(std::fs::read_to_string(root.join(file))?);
        }
    }
    bail!("{file} is not conflicted")
}

/// Writes the resolved content and stages it, which clears the conflict entry.
pub fn resolve_conflict(path: &Path, file: &str, content: &str) -> Result<()> {
    let repo = open(path)?;
    let root = repo.workdir().ok_or_else(|| anyhow!("bare repository"))?;
    std::fs::write(root.join(file), content)?;
    let mut idx = repo.index()?;
    idx.add_path(Path::new(file))?;
    idx.write()?;
    Ok(())
}

pub fn abort_merge(path: &Path) -> Result<()> {
    let repo = open(path)?;
    let head = repo.head()?.peel_to_commit()?;
    repo.reset(head.as_object(), git2::ResetType::Hard, None)?;
    repo.cleanup_state()?;
    Ok(())
}

/// Best guess at "owner/name" on github.com for the origin remote.
pub fn github_slug(path: &Path) -> Option<(String, String)> {
    let repo = open(path).ok()?;
    let r = repo.find_remote("origin").ok()?;
    parse_github_url(r.url().ok()?)
}

pub fn parse_github_url(url: &str) -> Option<(String, String)> {
    let rest = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("git@github.com:"))
        .or_else(|| url.strip_prefix("ssh://git@github.com/"))?;
    let rest = rest.trim_end_matches('/').trim_end_matches(".git");
    let (o, n) = rest.split_once('/')?;
    Some((o.to_string(), n.to_string()))
}

/// Splits a working-tree file containing conflict markers into hunks the
/// resolver can show side by side.
#[derive(Debug, Clone, PartialEq)]
pub enum ConflictHunk {
    Common(String),
    Conflict { ours: String, theirs: String },
}

pub fn parse_conflict_markers(text: &str) -> Vec<ConflictHunk> {
    let mut out = Vec::new();
    let mut common = String::new();
    let mut ours = String::new();
    let mut theirs = String::new();
    // 0 = outside, 1 = ours, 2 = base (diff3, skipped), 3 = theirs
    let mut mode = 0;
    for line in text.split_inclusive('\n') {
        let t = line.trim_end_matches(['\n', '\r']);
        if mode == 0 && t.starts_with("<<<<<<<") {
            if !common.is_empty() {
                out.push(ConflictHunk::Common(std::mem::take(&mut common)));
            }
            mode = 1;
        } else if mode == 1 && t.starts_with("|||||||") {
            mode = 2;
        } else if (mode == 1 || mode == 2) && t.starts_with("=======") {
            mode = 3;
        } else if mode == 3 && t.starts_with(">>>>>>>") {
            out.push(ConflictHunk::Conflict { ours: std::mem::take(&mut ours), theirs: std::mem::take(&mut theirs) });
            mode = 0;
        } else {
            match mode {
                0 => common.push_str(line),
                1 => ours.push_str(line),
                3 => theirs.push_str(line),
                _ => {}
            }
        }
    }
    if !common.is_empty() {
        out.push(ConflictHunk::Common(common));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_urls() {
        assert_eq!(parse_github_url("https://github.com/a/b.git"), Some(("a".into(), "b".into())));
        assert_eq!(parse_github_url("git@github.com:a/b.git"), Some(("a".into(), "b".into())));
        assert_eq!(parse_github_url("https://gitlab.com/a/b"), None);
    }

    #[test]
    fn splits_conflict_markers() {
        let text = "a\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\nb\n";
        let hunks = parse_conflict_markers(text);
        assert_eq!(
            hunks,
            vec![
                ConflictHunk::Common("a\n".into()),
                ConflictHunk::Conflict { ours: "ours\n".into(), theirs: "theirs\n".into() },
                ConflictHunk::Common("b\n".into()),
            ]
        );
    }

    #[test]
    fn diff3_base_section_is_dropped() {
        let text = "<<<<<<< HEAD\nx\n||||||| base\ny\n=======\nz\n>>>>>>> other\n";
        let hunks = parse_conflict_markers(text);
        assert_eq!(hunks, vec![ConflictHunk::Conflict { ours: "x\n".into(), theirs: "z\n".into() }]);
    }
}
