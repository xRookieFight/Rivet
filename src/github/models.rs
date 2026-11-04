//! Serde models for the REST v3 payloads we use. Everything optional is
//! `#[serde(default)]` because GitHub omits fields depending on token scopes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct User {
    pub login: String,
    pub id: u64,
    pub avatar_url: String,
    pub html_url: String,
    pub name: Option<String>,
    pub company: Option<String>,
    pub blog: Option<String>,
    pub location: Option<String>,
    pub email: Option<String>,
    pub bio: Option<String>,
    pub twitter_username: Option<String>,
    pub public_repos: u64,
    pub followers: u64,
    pub following: u64,
    #[serde(rename = "type")]
    pub kind: String,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Org {
    pub login: String,
    pub id: u64,
    pub avatar_url: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct License {
    pub key: String,
    pub name: String,
    pub spdx_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Permissions {
    pub admin: bool,
    pub maintain: bool,
    pub push: bool,
    pub triage: bool,
    pub pull: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Repo {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub owner: User,
    pub private: bool,
    pub visibility: Option<String>,
    pub html_url: String,
    pub clone_url: String,
    pub ssh_url: String,
    pub description: Option<String>,
    pub fork: bool,
    pub homepage: Option<String>,
    pub language: Option<String>,
    pub stargazers_count: u64,
    pub watchers_count: u64,
    pub subscribers_count: u64,
    pub forks_count: u64,
    pub open_issues_count: u64,
    pub default_branch: String,
    pub topics: Vec<String>,
    pub archived: bool,
    pub disabled: bool,
    pub is_template: bool,
    pub has_issues: bool,
    pub has_projects: bool,
    pub has_wiki: bool,
    pub has_discussions: bool,
    pub allow_merge_commit: Option<bool>,
    pub allow_squash_merge: Option<bool>,
    pub allow_rebase_merge: Option<bool>,
    pub delete_branch_on_merge: Option<bool>,
    pub license: Option<License>,
    pub permissions: Option<Permissions>,
    pub parent: Option<Box<Repo>>,
    pub pushed_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub size: u64,
}

impl Repo {
    pub fn can_push(&self) -> bool {
        self.permissions.as_ref().is_some_and(|p| p.push || p.admin)
    }
    pub fn is_admin(&self) -> bool {
        self.permissions.as_ref().is_some_and(|p| p.admin)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Label {
    pub id: u64,
    pub name: String,
    pub color: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Milestone {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub description: Option<String>,
    pub open_issues: u64,
    pub closed_issues: u64,
    pub due_on: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Reactions {
    pub total_count: u64,
    #[serde(rename = "+1")]
    pub plus1: u64,
    #[serde(rename = "-1")]
    pub minus1: u64,
    pub laugh: u64,
    pub hooray: u64,
    pub confused: u64,
    pub heart: u64,
    pub rocket: u64,
    pub eyes: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct PullRef {
    pub url: String,
    pub merged_at: Option<DateTime<Utc>>,
}

/// Issues and PRs share this shape in the issues/search endpoints.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Issue {
    pub id: u64,
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub user: User,
    pub state: String,
    pub state_reason: Option<String>,
    pub locked: bool,
    pub labels: Vec<Label>,
    pub assignees: Vec<User>,
    pub milestone: Option<Milestone>,
    pub comments: u64,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub html_url: String,
    pub repository_url: String,
    pub pull_request: Option<PullRef>,
    pub draft: Option<bool>,
    pub reactions: Option<Reactions>,
    pub author_association: String,
}

impl Issue {
    pub fn is_pr(&self) -> bool {
        self.pull_request.is_some()
    }

    /// "owner/name" pulled from repository_url, since search results don't embed the repo.
    pub fn repo_full_name(&self) -> String {
        self.repository_url.trim_start_matches("https://api.github.com/repos/").to_string()
    }

    pub fn item_state(&self) -> crate::widgets::ItemState {
        use crate::widgets::ItemState;
        if let Some(pr) = &self.pull_request {
            return if pr.merged_at.is_some() {
                ItemState::Merged
            } else if self.state == "closed" {
                ItemState::PrClosed
            } else if self.draft == Some(true) {
                ItemState::Draft
            } else {
                ItemState::Open
            };
        }
        match (self.state.as_str(), self.state_reason.as_deref()) {
            ("open", _) => ItemState::Open,
            (_, Some("not_planned")) => ItemState::ClosedNotPlanned,
            _ => ItemState::Closed,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Comment {
    pub id: u64,
    pub body: Option<String>,
    pub user: User,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub html_url: String,
    pub author_association: String,
    pub reactions: Option<Reactions>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct BranchRef {
    #[serde(rename = "ref")]
    pub ref_: String,
    pub sha: String,
    pub label: String,
    pub repo: Option<Repo>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Pull {
    pub id: u64,
    pub node_id: String,
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub user: User,
    pub state: String,
    pub draft: bool,
    pub merged: bool,
    pub mergeable: Option<bool>,
    pub mergeable_state: Option<String>,
    pub merged_by: Option<User>,
    pub merged_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub head: BranchRef,
    pub base: BranchRef,
    pub labels: Vec<Label>,
    pub assignees: Vec<User>,
    pub requested_reviewers: Vec<User>,
    pub milestone: Option<Milestone>,
    pub comments: u64,
    pub review_comments: u64,
    pub commits: u64,
    pub additions: u64,
    pub deletions: u64,
    pub changed_files: u64,
    pub html_url: String,
}

impl Pull {
    pub fn item_state(&self) -> crate::widgets::ItemState {
        use crate::widgets::ItemState;
        if self.merged || self.merged_at.is_some() {
            ItemState::Merged
        } else if self.state == "closed" {
            ItemState::PrClosed
        } else if self.draft {
            ItemState::Draft
        } else {
            ItemState::Open
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Review {
    pub id: u64,
    pub user: User,
    pub body: Option<String>,
    pub state: String,
    pub submitted_at: Option<DateTime<Utc>>,
    pub html_url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct ReviewComment {
    pub id: u64,
    pub body: String,
    pub path: String,
    pub line: Option<u64>,
    pub original_line: Option<u64>,
    pub side: Option<String>,
    pub diff_hunk: String,
    pub user: User,
    pub created_at: Option<DateTime<Utc>>,
    pub in_reply_to_id: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct ChangedFile {
    pub sha: Option<String>,
    pub filename: String,
    pub status: String,
    pub additions: u64,
    pub deletions: u64,
    pub changes: u64,
    pub patch: Option<String>,
    pub previous_filename: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct GitActor {
    pub name: String,
    pub email: String,
    pub date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct CommitInfo {
    pub message: String,
    pub author: GitActor,
    pub committer: GitActor,
    pub comment_count: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct CommitStats {
    pub additions: u64,
    pub deletions: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct ShaRef {
    pub sha: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Commit {
    pub sha: String,
    pub commit: CommitInfo,
    pub author: Option<User>,
    pub committer: Option<User>,
    pub html_url: String,
    pub parents: Vec<ShaRef>,
    pub stats: Option<CommitStats>,
    pub files: Vec<ChangedFile>,
}

impl Commit {
    pub fn title(&self) -> &str {
        self.commit.message.lines().next().unwrap_or("")
    }
    pub fn short_sha(&self) -> &str {
        &self.sha[..self.sha.len().min(7)]
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Compare {
    pub status: String,
    pub ahead_by: u64,
    pub behind_by: u64,
    pub total_commits: u64,
    pub commits: Vec<Commit>,
    pub files: Vec<ChangedFile>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Branch {
    pub name: String,
    pub commit: ShaRef,
    pub protected: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Tag {
    pub name: String,
    pub commit: ShaRef,
    pub zipball_url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Release {
    pub id: u64,
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub draft: bool,
    pub prerelease: bool,
    pub published_at: Option<DateTime<Utc>>,
    pub author: User,
    pub html_url: String,
}

/// Entry from the contents API, either a dir listing item or a single file.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Content {
    pub name: String,
    pub path: String,
    pub sha: String,
    pub size: u64,
    #[serde(rename = "type")]
    pub kind: String,
    pub content: Option<String>,
    pub encoding: Option<String>,
    pub html_url: Option<String>,
    pub download_url: Option<String>,
}

impl Content {
    pub fn is_dir(&self) -> bool {
        self.kind == "dir"
    }

    /// Decodes the base64 payload the contents API ships files with.
    pub fn decoded(&self) -> Option<Vec<u8>> {
        use base64::Engine;
        let raw: String = self.content.as_ref()?.chars().filter(|c| !c.is_whitespace()).collect();
        base64::engine::general_purpose::STANDARD.decode(raw).ok()
    }

    pub fn text(&self) -> Option<String> {
        self.decoded().and_then(|b| String::from_utf8(b).ok())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct NotificationSubject {
    pub title: String,
    pub url: Option<String>,
    pub latest_comment_url: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Notification {
    pub id: String,
    pub unread: bool,
    pub reason: String,
    pub updated_at: Option<DateTime<Utc>>,
    pub subject: NotificationSubject,
    pub repository: Repo,
}

impl Notification {
    /// Issue / PR number at the end of the subject url, if any.
    pub fn number(&self) -> Option<u64> {
        self.subject.url.as_ref()?.rsplit('/').next()?.parse().ok()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Workflow {
    pub id: u64,
    pub name: String,
    pub path: String,
    pub state: String,
    pub html_url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct WorkflowRun {
    pub id: u64,
    pub name: Option<String>,
    pub display_title: String,
    pub head_branch: Option<String>,
    pub head_sha: String,
    pub run_number: u64,
    pub event: String,
    pub status: Option<String>,
    pub conclusion: Option<String>,
    pub workflow_id: u64,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub run_started_at: Option<DateTime<Utc>>,
    pub actor: Option<User>,
    pub html_url: String,
    pub run_attempt: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Step {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub number: u64,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Job {
    pub id: u64,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub steps: Vec<Step>,
    pub html_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct CheckApp {
    pub id: u64,
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct CheckRun {
    pub app: Option<CheckApp>,
    pub id: u64,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub html_url: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Event {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub actor: User,
    pub repo: EventRepo,
    pub payload: serde_json::Value,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct EventRepo {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct SearchResult<T> {
    pub total_count: u64,
    pub incomplete_results: bool,
    pub items: Vec<T>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct CodeHit {
    pub name: String,
    pub path: String,
    pub sha: String,
    pub html_url: String,
    pub repository: Repo,
    pub text_matches: Vec<TextMatch>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct TextMatch {
    pub fragment: String,
    pub property: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct CommitSearchHit {
    pub sha: String,
    pub commit: CommitInfo,
    pub author: Option<User>,
    pub repository: Repo,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct LicenseTemplate {
    pub key: String,
    pub name: String,
    pub spdx_id: Option<String>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct ContributionDay {
    pub date: String,
    #[serde(rename = "contributionCount")]
    pub count: u64,
    pub color: String,
}

#[derive(Debug, Clone, Default)]
pub struct ContributionCalendar {
    pub total: u64,
    pub weeks: Vec<Vec<ContributionDay>>,
    pub commits: u64,
    pub issues: u64,
    pub pulls: u64,
    pub reviews: u64,
    /// Per kind, the repos the user contributed to and how much, largest first.
    pub commit_repos: Vec<(String, u64)>,
    pub pull_repos: Vec<(String, u64)>,
    pub issue_repos: Vec<(String, u64)>,
    pub review_repos: Vec<(String, u64)>,
    pub created_repos: Vec<String>,
    /// Years with contributions, newest first, for the year picker.
    pub years: Vec<i32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct BranchProtection {
    pub required_status_checks: Option<serde_json::Value>,
    pub enforce_admins: Option<serde_json::Value>,
    pub required_pull_request_reviews: Option<serde_json::Value>,
    pub required_linear_history: Option<serde_json::Value>,
    pub allow_force_pushes: Option<serde_json::Value>,
    pub allow_deletions: Option<serde_json::Value>,
    pub required_conversation_resolution: Option<serde_json::Value>,
}

/// Pinned repo pulled from GraphQL, flattened to what the dashboard shows.
#[derive(Debug, Clone, Default)]
pub struct PinnedRepo {
    pub owner: String,
    pub name: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub stars: u64,
    pub forks: u64,
    pub private: bool,
    pub archived: bool,
    pub parent: Option<String>,
}

/// Last commit for one path, as shown next to entries in the file browser.
#[derive(Debug, Clone, Default)]
pub struct PathCommit {
    pub sha: String,
    pub message: String,
    pub date: Option<DateTime<Utc>>,
}

/// What a repository card in the feed needs.
#[derive(Debug, Clone, Default)]
pub struct RepoSummary {
    pub full_name: String,
    pub description: Option<String>,
    pub stars: u64,
    pub language: Option<String>,
    pub private: bool,
    pub starred: bool,
    pub owner_avatar: String,
    pub pushed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default)]
pub struct ChangelogEntry {
    pub title: String,
    pub link: String,
    pub date: Option<DateTime<Utc>>,
}

/// Profile bits GitHub only shows on the web page (no API for them).
#[derive(Debug, Clone, Default)]
pub struct ProfileExtras {
    pub utc_offset_hours: Option<f32>,
    pub achievements: Vec<Achievement>,
    pub highlights: Vec<String>,
    pub stars: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct Achievement {
    pub name: String,
    pub image: String,
    pub tier: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct SocialAccount {
    pub provider: String,
    pub url: String,
}
