//! Every screen in the app is addressed by a `Route`, which keeps back/forward
//! history and the command palette simple.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Route {
    Dashboard,
    Feed,
    Notifications,
    Search { q: String, kind: SearchKind },
    MyItems { pulls: bool },
    User(String),
    NewRepo,
    Repo(RepoRoute),
    Local(Option<PathBuf>),
    Clone { url: String },
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SearchKind {
    #[default]
    Repositories,
    Code,
    Issues,
    PullRequests,
    Users,
    Commits,
}

impl SearchKind {
    pub const ALL: [SearchKind; 6] =
        [SearchKind::Code, SearchKind::Repositories, SearchKind::Issues, SearchKind::PullRequests, SearchKind::Users, SearchKind::Commits];

    pub fn label(self) -> &'static str {
        match self {
            SearchKind::Repositories => "Repositories",
            SearchKind::Code => "Code",
            SearchKind::Issues => "Issues",
            SearchKind::PullRequests => "Pull requests",
            SearchKind::Users => "Users",
            SearchKind::Commits => "Commits",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoRoute {
    pub owner: String,
    pub name: String,
    pub view: RepoView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepoView {
    Code { git_ref: Option<String>, path: String },
    Blob { git_ref: Option<String>, path: String },
    Issues { q: Option<String> },
    Issue(u64),
    NewIssue,
    Pulls { q: Option<String> },
    Pull(u64),
    NewPull { base: Option<String>, head: Option<String> },
    Actions { workflow: Option<u64> },
    Run(u64),
    Commits { git_ref: Option<String> },
    Commit(String),
    Branches,
    Tags,
    Compare { base: String, head: String },
    Settings,
    Readme,
    Templates,
    CommunityFiles,
    Labels,
}

impl RepoView {
    /// Which top tab lights up for this view.
    pub fn tab(&self) -> usize {
        match self {
            RepoView::Issues { .. } | RepoView::Issue(_) | RepoView::NewIssue | RepoView::Labels => 1,
            RepoView::Pulls { .. } | RepoView::Pull(_) | RepoView::NewPull { .. } | RepoView::Compare { .. } => 2,
            RepoView::Actions { .. } | RepoView::Run(_) => 3,
            RepoView::Readme | RepoView::Templates | RepoView::CommunityFiles => 4,
            RepoView::Settings => 5,
            _ => 0,
        }
    }
}

impl Route {
    pub fn repo(owner: &str, name: &str, view: RepoView) -> Route {
        Route::Repo(RepoRoute { owner: owner.into(), name: name.into(), view })
    }

    pub fn repo_home(owner: &str, name: &str) -> Route {
        Route::repo(owner, name, RepoView::Code { git_ref: None, path: String::new() })
    }

    /// Parses "owner/name" into a repo route.
    pub fn from_slug(slug: &str) -> Option<Route> {
        let (o, n) = slug.trim().trim_matches('/').split_once('/')?;
        if o.is_empty() || n.is_empty() || n.contains('/') {
            return None;
        }
        Some(Route::repo_home(o, n))
    }

    /// Maps a github.com URL to an in-app route so links stay inside Rivet.
    pub fn from_github_url(url: &str) -> Option<Route> {
        let rest = url.strip_prefix("https://github.com/")?;
        let rest = rest.split(['?', '#']).next()?.trim_end_matches('/');
        let parts: Vec<&str> = rest.split('/').collect();
        match parts.as_slice() {
            [user] if !user.is_empty() => Some(Route::User(user.to_string())),
            [o, n] => Some(Route::repo_home(o, n)),
            [o, n, "issues"] => Some(Route::repo(o, n, RepoView::Issues { q: None })),
            [o, n, "pulls"] => Some(Route::repo(o, n, RepoView::Pulls { q: None })),
            [o, n, "actions"] => Some(Route::repo(o, n, RepoView::Actions { workflow: None })),
            [o, n, "issues", num] => num.parse().ok().map(|x| Route::repo(o, n, RepoView::Issue(x))),
            [o, n, "pull", num, ..] => num.parse().ok().map(|x| Route::repo(o, n, RepoView::Pull(x))),
            [o, n, "commit", sha] => Some(Route::repo(o, n, RepoView::Commit(sha.to_string()))),
            [o, n, "actions", "runs", id, ..] => id.parse().ok().map(|x| Route::repo(o, n, RepoView::Run(x))),
            [o, n, "tree", r, path @ ..] => Some(Route::repo(o, n, RepoView::Code { git_ref: Some(r.to_string()), path: path.join("/") })),
            [o, n, "blob", r, path @ ..] => Some(Route::repo(o, n, RepoView::Blob { git_ref: Some(r.to_string()), path: path.join("/") })),
            _ => None,
        }
    }

    /// Screens that work on a local clone, which some platforms can't offer.
    pub fn needs_local_git(&self) -> bool {
        matches!(self, Route::Local(_) | Route::Clone { .. })
    }

    pub fn title(&self) -> String {
        match self {
            Route::Dashboard => "Dashboard".into(),
            Route::Feed => "Feed".into(),
            Route::Notifications => "Notifications".into(),
            Route::Search { q, .. } => format!("Search · {q}"),
            Route::MyItems { pulls: true } => "Pull requests".into(),
            Route::MyItems { pulls: false } => "Issues".into(),
            Route::User(u) => u.clone(),
            Route::NewRepo => "New repository".into(),
            Route::Repo(r) => format!("{}/{}", r.owner, r.name),
            Route::Local(_) => "Local repositories".into(),
            Route::Clone { .. } => "Clone a repository".into(),
            Route::Settings => "Settings".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_github_urls() {
        assert_eq!(
            Route::from_github_url("https://github.com/rust-lang/rust/pull/42"),
            Some(Route::repo("rust-lang", "rust", RepoView::Pull(42)))
        );
        assert_eq!(
            Route::from_github_url("https://github.com/a/b/issues/7#issuecomment-1"),
            Some(Route::repo("a", "b", RepoView::Issue(7)))
        );
        assert_eq!(
            Route::from_github_url("https://github.com/a/b/blob/main/src/lib.rs"),
            Some(Route::repo("a", "b", RepoView::Blob { git_ref: Some("main".into()), path: "src/lib.rs".into() }))
        );
        assert_eq!(Route::from_github_url("https://example.com/a"), None);
    }

    #[test]
    fn slug_parsing() {
        assert_eq!(Route::from_slug("a/b"), Some(Route::repo_home("a", "b")));
        assert_eq!(Route::from_slug("a"), None);
        assert_eq!(Route::from_slug("a/b/c"), None);
    }
}
