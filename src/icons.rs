//! Octicons, bundled as SVG (MIT, see assets/octicons/LICENSE).
//!
//! The SVGs are baked with a white fill so `tint` gives us any color we want.

use egui::{Color32, Image, ImageSource, Response, Sense, Ui, Vec2};

macro_rules! icons {
    ($($variant:ident => $file:literal),* $(,)?) => {
        // the whole bundled set is listed, not every icon has a screen using it yet
        #[allow(dead_code)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Icon { $($variant),* }

        impl Icon {
            pub fn source(self) -> ImageSource<'static> {
                match self {
                    $(Icon::$variant => ImageSource::Bytes {
                        uri: concat!("bytes://octicon/", $file, ".svg").into(),
                        bytes: egui::load::Bytes::Static(include_bytes!(concat!("../assets/octicons/", $file, ".svg"))),
                    }),*
                }
            }
        }
    };
}

icons! {
    Alert => "alert",
    Archive => "archive",
    ArrowDown => "arrow-down",
    ArrowLeft => "arrow-left",
    ArrowRight => "arrow-right",
    ArrowSwitch => "arrow-switch",
    ArrowUp => "arrow-up",
    Beaker => "beaker",
    Bell => "bell",
    BellFill => "bell-fill",
    Book => "book",
    Bookmark => "bookmark",
    Browser => "browser",
    Check => "check",
    CheckCircle => "check-circle",
    CheckCircleFill => "check-circle-fill",
    Checklist => "checklist",
    ChevronDown => "chevron-down",
    ChevronLeft => "chevron-left",
    ChevronRight => "chevron-right",
    Circle => "circle",
    Clock => "clock",
    Code => "code",
    CodeReview => "code-review",
    CommandPalette => "command-palette",
    Comment => "comment",
    CommentDiscussion => "comment-discussion",
    Container => "container",
    Copy => "copy",
    Cpu => "cpu",
    Dash => "dash",
    DesktopDownload => "desktop-download",
    DeviceDesktop => "device-desktop",
    Diff => "diff",
    DotFill => "dot-fill",
    Download => "download",
    Ellipsis => "ellipsis",
    Eye => "eye",
    EyeClosed => "eye-closed",
    FeedForked => "feed-forked",
    FeedIssueOpen => "feed-issue-open",
    FeedPublic => "feed-public",
    FeedPullRequestOpen => "feed-pull-request-open",
    FeedRepo => "feed-repo",
    FeedStar => "feed-star",
    File => "file",
    FileAdded => "file-added",
    FileBadge => "file-badge",
    FileCode => "file-code",
    FileDiff => "file-diff",
    FileDirectory => "file-directory",
    FileDirectoryFill => "file-directory-fill",
    FileMoved => "file-moved",
    FileRemoved => "file-removed",
    FileSymlinkFile => "file-symlink-file",
    FileZip => "file-zip",
    Filter => "filter",
    Fold => "fold",
    Gear => "gear",
    GitBranch => "git-branch",
    GitCommit => "git-commit",
    GitCompare => "git-compare",
    GitMerge => "git-merge",
    GitMergeQueue => "git-merge-queue",
    GitPullRequest => "git-pull-request",
    GitPullRequestClosed => "git-pull-request-closed",
    GitPullRequestDraft => "git-pull-request-draft",
    Globe => "globe",
    Graph => "graph",
    Heart => "heart",
    History => "history",
    Home => "home",
    Hubot => "hubot",
    Inbox => "inbox",
    Info => "info",
    IssueClosed => "issue-closed",
    IssueDraft => "issue-draft",
    IssueOpened => "issue-opened",
    IssueReopened => "issue-reopened",
    KebabHorizontal => "kebab-horizontal",
    Key => "key",
    Law => "law",
    Link => "link",
    LinkExternal => "link-external",
    ListUnordered => "list-unordered",
    Location => "location",
    Lock => "lock",
    Log => "log",
    Mail => "mail",
    MarkGithub => "mark-github",
    Megaphone => "megaphone",
    Milestone => "milestone",
    Moon => "moon",
    Note => "note",
    Organization => "organization",
    Package => "package",
    Paintbrush => "paintbrush",
    Pencil => "pencil",
    People => "people",
    Person => "person",
    Pin => "pin",
    Play => "play",
    Plus => "plus",
    PlusCircle => "plus-circle",
    Project => "project",
    Pulse => "pulse",
    Question => "question",
    Repo => "repo",
    RepoForked => "repo-forked",
    RepoLocked => "repo-locked",
    RepoPush => "repo-push",
    RepoTemplate => "repo-template",
    Rocket => "rocket",
    Rss => "rss",
    Search => "search",
    Shield => "shield",
    ShieldCheck => "shield-check",
    SidebarCollapse => "sidebar-collapse",
    SidebarExpand => "sidebar-expand",
    SignOut => "sign-out",
    Skip => "skip",
    Smiley => "smiley",
    SocialX => "social-x",
    SocialYoutube => "social-youtube",
    SquareFill => "square-fill",
    Star => "star",
    StarFill => "star-fill",
    Stop => "stop",
    Stopwatch => "stopwatch",
    Sun => "sun",
    Sync => "sync",
    Tag => "tag",
    Tasklist => "tasklist",
    Terminal => "terminal",
    ThreeBars => "three-bars",
    Tools => "tools",
    Trash => "trash",
    TriangleDown => "triangle-down",
    Typography => "typography",
    Unfold => "unfold",
    Unlock => "unlock",
    Unverified => "unverified",
    Upload => "upload",
    Verified => "verified",
    Versions => "versions",
    Workflow => "workflow",
    X => "x",
    XCircle => "x-circle",
    XCircleFill => "x-circle-fill",
}

impl Icon {
    pub fn image(self, size: f32, color: Color32) -> Image<'static> {
        Image::new(self.source()).fit_to_exact_size(Vec2::splat(size)).tint(color)
    }
}

/// Paints an icon inline, the way an octicon sits next to text on github.com.
pub fn icon(ui: &mut Ui, icon: Icon, color: Color32) -> Response {
    icon_sized(ui, icon, 16.0, color)
}

pub fn icon_sized(ui: &mut Ui, icon: Icon, size: f32, color: Color32) -> Response {
    ui.add(icon.image(size, color).sense(Sense::hover()))
}
