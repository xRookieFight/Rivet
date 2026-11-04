//! Single pull request: conversation, commits, checks and files changed, with
//! reviews (including line comments), merging and the metadata sidebar.

use super::{RepoSub, on};
use crate::app::Cx;
use crate::github::models::{ChangedFile, CheckRun, Comment, Commit, Label, Pull, Repo, Review, ReviewComment, User};
use crate::icons::{Icon, icon};
use crate::route::{RepoView, Route};
use crate::task::Load;
use crate::theme::semibold;
use crate::views::common::{self, Composer};
use crate::widgets::{self as w, FlashKind, Variant};
use egui::{CornerRadius, Frame, Margin, RichText, Stroke, Ui};
use egui_commonmark::CommonMarkCache;
use serde_json::{Value, json};
use std::collections::HashSet;

/// A line comment queued for the next review submission.
#[derive(Clone)]
struct PendingComment {
    path: String,
    line: u64,
    side: &'static str,
    body: String,
}

pub struct PullView {
    number: u64,
    tab: usize,
    pull: Load<Pull>,
    comments: Load<Vec<Comment>>,
    reviews: Load<Vec<Review>>,
    review_comments: Load<Vec<ReviewComment>>,
    files: Load<Vec<ChangedFile>>,
    commits: Load<Vec<Commit>>,
    checks: Load<Vec<CheckRun>>,
    labels_all: Load<Vec<Label>>,
    users_all: Load<Vec<User>>,
    composer: Composer,
    md: CommonMarkCache,
    action: Load<()>,
    merge_method: &'static str,
    merge_confirm: bool,
    merge_title: String,
    merge_msg: String,
    review_body: String,
    review_event: &'static str,
    pending: Vec<PendingComment>,
    line_form: Option<(String, String, bool, String)>,
    collapsed: HashSet<String>,
    lf: String,
    af: String,
    rf: String,
    editing_title: Option<String>,
}

impl PullView {
    pub fn new(number: u64) -> Self {
        Self {
            number,
            tab: 0,
            pull: Load::Idle,
            comments: Load::Idle,
            reviews: Load::Idle,
            review_comments: Load::Idle,
            files: Load::Idle,
            commits: Load::Idle,
            checks: Load::Idle,
            labels_all: Load::Idle,
            users_all: Load::Idle,
            composer: Composer::default(),
            md: CommonMarkCache::default(),
            action: Load::Idle,
            merge_method: "merge",
            merge_confirm: false,
            merge_title: String::new(),
            merge_msg: String::new(),
            review_body: String::new(),
            review_event: "COMMENT",
            pending: vec![],
            line_form: None,
            collapsed: HashSet::new(),
            lf: String::new(),
            af: String::new(),
            rf: String::new(),
            editing_title: None,
        }
    }

    fn load(&mut self, ctx: &egui::Context, cx: &Cx, repo: &Repo) {
        let (o, n) = on(repo);
        let num = self.number;
        macro_rules! spawn {
            ($field:ident, $call:ident) => {{
                let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
                self.$field = Load::spawn(ctx, move || g.$call(&o2, &n2, num));
            }};
        }
        spawn!(pull, pull);
        spawn!(comments, issue_comments);
        spawn!(reviews, reviews);
        spawn!(review_comments, review_comments);
        spawn!(files, pull_files);
        spawn!(commits, pull_commits);
        self.checks = Load::Idle;
    }

    /// Runs an action and reloads the whole PR afterwards, simplest way to stay consistent.
    fn act(&mut self, ctx: &egui::Context, job: impl FnOnce() -> anyhow::Result<()> + Send + 'static) {
        self.action = Load::spawn(ctx, job);
    }
}

impl RepoSub for PullView {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let ctx = ui.ctx().clone();
        let (o, n) = on(repo);
        if self.pull.is_idle() {
            self.load(&ctx, cx, repo);
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.labels_all = Load::spawn(&ctx, move || g.labels(&o2, &n2));
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.users_all = Load::spawn(&ctx, move || g.assignees(&o2, &n2));
        }
        match self.action.take() {
            Some(Ok(())) => self.load(&ctx, cx, repo),
            Some(Err(e)) => cx.err(e),
            None => {}
        }
        let pull = match self.pull.poll() {
            Load::Ready(p) => p.clone(),
            Load::Failed(e) => {
                let e = e.clone();
                super::section(ui, |ui| w::flash(ui, FlashKind::Danger, &e));
                return;
            }
            _ => {
                w::spinner(ui);
                return;
            }
        };
        if self.checks.is_idle() {
            let (g, o2, n2, sha) = (cx.gh.clone(), o.clone(), n.clone(), pull.head.sha.clone());
            self.checks = Load::spawn(&ctx, move || g.check_runs(&o2, &n2, &sha));
        }
        let pal = w::p(ui);
        let can_edit = repo.can_push() || pull.user.login == cx.me.login;

        super::section(ui, |ui| {
            match &mut self.editing_title {
                Some(t) => {
                    let mut done = None;
                    ui.horizontal(|ui| {
                        w::text_input(ui, t, "Title", (ui.available_width() - 180.0).max(200.0));
                        if w::button(ui, Variant::Default, None, "Save").clicked() {
                            done = Some(true);
                        }
                        if w::button(ui, Variant::Invisible, None, "Cancel").clicked() {
                            done = Some(false);
                        }
                    });
                    if let Some(save) = done {
                        let title = t.trim().to_string();
                        self.editing_title = None;
                        if save {
                            let (g, o2, n2, num) = (cx.gh.clone(), o.clone(), n.clone(), self.number);
                            self.act(&ctx, move || g.update_pull(&o2, &n2, num, &json!({ "title": title })).map(|_| ()));
                        }
                    }
                }
                None => {
                    ui.horizontal_top(|ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2((ui.available_width() - 120.0).max(200.0), 0.0),
                            egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true),
                            |ui| {
                                ui.add(egui::Label::new(RichText::new(&pull.title).size(32.0)).wrap());
                                ui.label(RichText::new(format!("#{}", pull.number)).size(32.0).color(pal.fg_muted));
                            },
                        );
                        w::right(ui, |ui| {
                            if can_edit && w::button(ui, Variant::Default, None, "Edit").clicked() {
                                self.editing_title = Some(pull.title.clone());
                            }
                        });
                    });
                }
            }
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                w::state_label(ui, pull.item_state(), true);
                ui.spacing_mut().item_spacing.x = 4.0;
                if w::title_link(ui, &pull.user.login, 14.0).clicked() {
                    cx.go(Route::User(pull.user.login.clone()));
                }
                let verb = if pull.merged { "merged" } else { "wants to merge" };
                w::muted(ui, format!("{verb} {} commit{} into", pull.commits, if pull.commits == 1 { "" } else { "s" }));
                w::branch_pill(ui, &pull.base.ref_);
                w::muted(ui, "from");
                w::branch_pill(ui, &pull.head.label);
                if w::icon_button_invisible(ui, Icon::Copy, "Copy head branch name").clicked() {
                    ui.ctx().copy_text(pull.head.ref_.clone());
                }
            });
            ui.add_space(8.0);
            let checks_n = self.checks.ready().map(|c| c.len() as u64);
            let tabs = [
                (Icon::CommentDiscussion, "Conversation", Some(pull.comments + pull.review_comments)),
                (Icon::GitCommit, "Commits", Some(pull.commits)),
                (Icon::Checklist, "Checks", checks_n),
                (Icon::FileDiff, "Files changed", Some(pull.changed_files)),
            ];
            ui.horizontal(|ui| {
                if let Some(t) = w::underline_nav(ui, &tabs, self.tab) {
                    self.tab = t;
                }
                w::right(ui, |ui| {
                    ui.add_space(4.0);
                    ui.scope(|ui| common::diffstat_blocks(ui, pull.additions, pull.deletions));
                });
            });
            w::divider(ui);
            ui.add_space(16.0);
            match self.tab {
                0 => {
                    ui.horizontal_top(|ui| {
                        let main_w = (ui.available_width() - 280.0).max(400.0);
                        ui.allocate_ui_with_layout(egui::vec2(main_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                            ui.set_width(main_w);
                            self.conversation(ui, cx, repo, &pull, can_edit);
                        });
                        ui.add_space(24.0);
                        ui.vertical(|ui| {
                            ui.set_width(256.0);
                            self.sidebar(ui, cx, repo, &pull);
                        });
                    });
                }
                1 => self.commits_tab(ui, cx, repo),
                2 => self.checks_tab(ui),
                _ => self.files_tab(ui, cx, repo, &pull),
            }
        });
    }
}

impl PullView {
    fn conversation(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo, pull: &Pull, can_edit: bool) {
        let ctx = ui.ctx().clone();
        let pal = w::p(ui);
        let (o, n) = on(repo);
        let num = self.number;
        let act = common::comment_card(
            ui,
            &mut self.md,
            &pull.user,
            pull.created_at.as_ref(),
            pull.body.as_deref().unwrap_or(""),
            "",
            None,
            false,
            false,
            cx,
        );
        if let Some(r) = act.react {
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.act(&ctx, move || g.react(&o2, &n2, &format!("issues/{num}"), r));
        }
        ui.add_space(16.0);

        // interleave comments and reviews by time, like the real timeline
        enum Item {
            C(Comment),
            R(Review),
        }
        let mut items: Vec<(chrono::DateTime<chrono::Utc>, Item)> = vec![];
        if let Some(cs) = self.comments.ready() {
            items.extend(cs.iter().cloned().map(|c| (c.created_at.unwrap_or_default(), Item::C(c))));
        }
        if let Some(rs) = self.reviews.ready() {
            items.extend(rs.iter().filter(|&r| r.state != "PENDING").cloned().map(|r| (r.submitted_at.unwrap_or_default(), Item::R(r))));
        }
        items.sort_by_key(|(t, _)| *t);
        for (_, it) in items {
            match it {
                Item::C(c) => {
                    let act = common::comment_card(
                        ui,
                        &mut self.md,
                        &c.user,
                        c.created_at.as_ref(),
                        c.body.as_deref().unwrap_or(""),
                        &c.author_association,
                        c.reactions.as_ref(),
                        c.user.login == pull.user.login,
                        c.user.login == cx.me.login,
                        cx,
                    );
                    if act.delete {
                        let (g, o2, n2, id) = (cx.gh.clone(), o.clone(), n.clone(), c.id);
                        self.act(&ctx, move || g.delete_comment(&o2, &n2, id));
                    }
                    if let Some(r) = act.react {
                        let (g, o2, n2, id) = (cx.gh.clone(), o.clone(), n.clone(), c.id);
                        self.act(&ctx, move || g.react(&o2, &n2, &format!("issues/comments/{id}"), r));
                    }
                    if act.quote {
                        self.composer.text.push_str(&c.body.unwrap_or_default().lines().map(|l| format!("> {l}\n")).collect::<String>());
                    }
                }
                Item::R(r) => {
                    let (ic, color, verb) = match r.state.as_str() {
                        "APPROVED" => (Icon::CheckCircleFill, pal.success, "approved these changes"),
                        "CHANGES_REQUESTED" => (Icon::FileDiff, pal.danger, "requested changes"),
                        "DISMISSED" => (Icon::X, pal.fg_muted, "had a review dismissed"),
                        _ => (Icon::Eye, pal.fg_muted, "reviewed"),
                    };
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        icon(ui, ic, color);
                        w::avatar(ui, &r.user.avatar_url, 20.0);
                        ui.label(RichText::new(&r.user.login).font(semibold(14.0)));
                        w::muted(ui, verb);
                        if let Some(t) = &r.submitted_at {
                            w::muted_small(ui, w::rel_time(t));
                        }
                    });
                    if let Some(b) = r.body.as_ref().filter(|b| !b.trim().is_empty()) {
                        ui.add_space(8.0);
                        common::comment_card(ui, &mut self.md, &r.user, r.submitted_at.as_ref(), b, "", None, false, false, cx);
                    }
                }
            }
            ui.add_space(16.0);
        }

        self.merge_box(ui, cx, repo, pull);
        ui.add_space(16.0);
        w::divider(ui);
        ui.add_space(16.0);

        ui.horizontal_top(|ui| {
            w::avatar(ui, &cx.me.avatar_url, 40.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("Add a comment").font(semibold(16.0)));
                let mut submit = false;
                let mut state: Option<&str> = None;
                let open = pull.state == "open";
                let busy = self.action.is_loading();
                self.composer.ui(ui, "Use Markdown to format your comment", 5, |ui, text| {
                    if w::button_ex(ui, Variant::Primary, None, "Comment", None, !text.trim().is_empty() && !busy).clicked() {
                        submit = true;
                    }
                    if can_edit && !pull.merged {
                        if open {
                            if w::button_ex(ui, Variant::Default, Some(Icon::GitPullRequestClosed), "Close pull request", None, !busy)
                                .clicked()
                            {
                                state = Some("closed");
                            }
                        } else if w::button_ex(ui, Variant::Default, None, "Reopen pull request", None, !busy).clicked() {
                            state = Some("open");
                        }
                    }
                });
                if submit || state.is_some() {
                    let text = std::mem::take(&mut self.composer.text);
                    let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
                    let state = state.map(str::to_string);
                    self.act(&ctx, move || {
                        if !text.trim().is_empty() {
                            g.add_comment(&o2, &n2, num, &text)?;
                        }
                        if let Some(s) = state {
                            g.update_pull(&o2, &n2, num, &json!({ "state": s }))?;
                        }
                        Ok(())
                    });
                }
            });
        });
    }

    fn merge_box(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo, pull: &Pull) {
        let ctx = ui.ctx().clone();
        let pal = w::p(ui);
        let (o, n) = on(repo);
        let num = self.number;
        if pull.merged {
            w::flash(
                ui,
                FlashKind::Info,
                &format!(
                    "Pull request successfully merged{}.",
                    pull.merged_by.as_ref().map(|u| format!(" by {}", u.login)).unwrap_or_default()
                ),
            );
            if repo.can_push() && w::button(ui, Variant::Default, Some(Icon::Trash), "Delete branch").clicked() {
                let (g, o2, n2, b) = (cx.gh.clone(), o.clone(), n.clone(), pull.head.ref_.clone());
                self.act(&ctx, move || g.delete_branch(&o2, &n2, &b));
                cx.ok(format!("Deleted {}", pull.head.ref_));
            }
            return;
        }
        if pull.state != "open" {
            return;
        }
        let checks = self.checks.ready().cloned().unwrap_or_default();
        let failing =
            checks.iter().filter(|c| matches!(c.conclusion.as_deref(), Some("failure") | Some("timed_out") | Some("cancelled"))).count();
        let pending = checks.iter().filter(|c| c.status != "completed").count();
        Frame::new().stroke(Stroke::new(1.0, pal.border)).corner_radius(CornerRadius::same(6)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let row = |ui: &mut Ui, ic: Icon, color: egui::Color32, title: &str, sub: &str| {
                Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        icon(ui, ic, color);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(title).font(semibold(14.0)));
                            if !sub.is_empty() {
                                w::muted_small(ui, sub);
                            }
                        });
                    });
                });
                w::divider(ui);
            };
            if !checks.is_empty() {
                if failing > 0 {
                    row(
                        ui,
                        Icon::XCircleFill,
                        pal.danger,
                        "Some checks were not successful",
                        &format!("{failing} failing, {} total", checks.len()),
                    );
                } else if pending > 0 {
                    row(
                        ui,
                        Icon::DotFill,
                        pal.attention,
                        "Some checks haven't completed yet",
                        &format!("{pending} pending, {} total", checks.len()),
                    );
                } else {
                    row(ui, Icon::CheckCircleFill, pal.success, "All checks have passed", &format!("{} successful checks", checks.len()));
                }
            }
            if pull.draft {
                row(
                    ui,
                    Icon::GitPullRequestDraft,
                    pal.fg_muted,
                    "This pull request is still a work in progress",
                    "Draft pull requests cannot be merged.",
                );
                if repo.can_push() || pull.user.login == cx.me.login {
                    Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                        if w::button(ui, Variant::Default, None, "Ready for review").clicked() {
                            let (g, id) = (cx.gh.clone(), pull.node_id.clone());
                            self.act(&ctx, move || g.mark_ready_for_review(&id));
                        }
                    });
                }
                return;
            }
            match (pull.mergeable, pull.mergeable_state.as_deref()) {
                (Some(false), _) | (_, Some("dirty")) => row(
                    ui,
                    Icon::Alert,
                    pal.danger,
                    "This branch has conflicts that must be resolved",
                    "Resolve conflicts locally, then push.",
                ),
                (_, Some("blocked")) => {
                    row(ui, Icon::Alert, pal.attention, "Merging is blocked", "Required reviews or checks are not satisfied.")
                }
                (_, Some("behind")) => row(ui, Icon::Alert, pal.attention, "This branch is out-of-date with the base branch", ""),
                (None, _) => row(ui, Icon::Clock, pal.fg_muted, "Checking for the ability to merge automatically…", "Reload in a moment."),
                _ => row(ui, Icon::Check, pal.success, "No conflicts with base branch", "Merging can be performed automatically."),
            }
            if !repo.can_push() {
                Frame::new()
                    .inner_margin(Margin::same(16))
                    .show(ui, |ui| w::muted(ui, "Only those with write access to this repository can merge pull requests."));
                return;
            }
            Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let methods: Vec<(&'static str, &str)> = [
                    ("merge", "Create a merge commit", repo.allow_merge_commit.unwrap_or(true)),
                    ("squash", "Squash and merge", repo.allow_squash_merge.unwrap_or(true)),
                    ("rebase", "Rebase and merge", repo.allow_rebase_merge.unwrap_or(true)),
                ]
                .into_iter()
                .filter(|m| m.2)
                .map(|m| (m.0, m.1))
                .collect();
                if !methods.iter().any(|m| m.0 == self.merge_method)
                    && let Some(m) = methods.first()
                {
                    self.merge_method = m.0;
                }
                let label = methods.iter().find(|m| m.0 == self.merge_method).map(|m| m.1).unwrap_or("Merge pull request");
                if self.merge_confirm {
                    w::form_label(ui, "Commit message", false);
                    w::text_input(ui, &mut self.merge_title, "", ui.available_width());
                    ui.add_space(6.0);
                    w::text_area(ui, &mut self.merge_msg, "Extended description", 3, false);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if w::button_ex(ui, Variant::Primary, None, "Confirm merge", None, !self.action.is_loading()).clicked() {
                            let (g, o2, n2, m, t, msg) =
                                (cx.gh.clone(), o.clone(), n.clone(), self.merge_method, self.merge_title.clone(), self.merge_msg.clone());
                            self.act(&ctx, move || g.merge_pull(&o2, &n2, num, m, &t, &msg));
                            self.merge_confirm = false;
                        }
                        if w::button(ui, Variant::Default, None, "Cancel").clicked() {
                            self.merge_confirm = false;
                        }
                    });
                } else {
                    ui.horizontal(|ui| {
                        let ok = pull.mergeable != Some(false);
                        let v = if failing == 0 { Variant::Primary } else { Variant::Danger };
                        let txt = if self.merge_method == "merge" { "Merge pull request".to_string() } else { label.to_string() };
                        if w::button_ex(ui, v, None, &txt, None, ok).clicked() {
                            self.merge_confirm = true;
                            self.merge_title = format!("{} (#{})", pull.title, pull.number);
                            self.merge_msg =
                                if self.merge_method == "squash" { String::new() } else { pull.body.clone().unwrap_or_default() };
                        }
                        let more = w::icon_button(ui, Icon::TriangleDown, "Select merge method");
                        egui::Popup::menu(&more).width(280.0).show(|ui| {
                            for (k, l) in &methods {
                                if w::menu_item_ex(ui, None, l, self.merge_method == *k, false).clicked() {
                                    self.merge_method = k;
                                }
                            }
                        });
                    });
                }
            });
        });
    }

    fn sidebar(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo, pull: &Pull) {
        let ctx = ui.ctx().clone();
        let (o, n) = on(repo);
        let num = self.number;
        let can = repo.permissions.as_ref().is_some_and(|p| p.triage || p.push || p.admin);
        let users = self.users_all.ready().cloned().unwrap_or_default();

        let reviewed: Vec<(User, String)> =
            self.reviews.ready().map(|rs| rs.iter().map(|r| (r.user.clone(), r.state.clone())).collect()).unwrap_or_default();
        let requested: Vec<User> = pull.requested_reviewers.clone();
        let mut req: Vec<String> = requested.iter().map(|u| u.login.clone()).collect();
        let before = req.clone();
        let mut changed = false;
        common::sidebar_section(
            ui,
            "Reviewers",
            can,
            |ui| {
                if requested.is_empty() && reviewed.is_empty() {
                    w::muted_small(ui, "No reviews");
                }
                let pal = w::p(ui);
                for u in &requested {
                    ui.horizontal(|ui| {
                        w::avatar(ui, &u.avatar_url, 20.0);
                        ui.label(&u.login);
                        w::right(ui, |ui| icon(ui, Icon::DotFill, pal.attention).on_hover_text("Awaiting review"));
                    });
                }
                let mut seen = HashSet::new();
                for (u, state) in reviewed.iter().rev() {
                    if seen.insert(u.login.clone()) {
                        let (ic, color, tip) = match state.as_str() {
                            "APPROVED" => (Icon::Check, pal.success, "Approved"),
                            "CHANGES_REQUESTED" => (Icon::FileDiff, pal.danger, "Requested changes"),
                            _ => (Icon::Comment, pal.fg_muted, "Left review comments"),
                        };
                        ui.horizontal(|ui| {
                            w::avatar(ui, &u.avatar_url, 20.0);
                            ui.label(&u.login);
                            w::right(ui, |ui| icon(ui, ic, color).on_hover_text(tip));
                        });
                    }
                }
            },
            |ui| {
                let candidates: Vec<User> = users.iter().filter(|u| u.login != pull.user.login).cloned().collect();
                changed = common::user_picker(ui, "Request up to 15 reviewers", &candidates, &mut req, &mut self.rf, 15);
            },
        );
        if changed {
            let add: Vec<String> = req.iter().filter(|x| !before.contains(x)).cloned().collect();
            let remove: Vec<String> = before.iter().filter(|x| !req.contains(x)).cloned().collect();
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.act(&ctx, move || {
                if !add.is_empty() {
                    g.request_reviewers(&o2, &n2, num, &add)?;
                }
                if !remove.is_empty() {
                    g.remove_reviewers(&o2, &n2, num, &remove)?;
                }
                Ok(())
            });
        }

        let mut assignees: Vec<String> = pull.assignees.iter().map(|a| a.login.clone()).collect();
        let cur_a = pull.assignees.clone();
        let mut changed_a = false;
        common::sidebar_section(
            ui,
            "Assignees",
            can,
            |ui| {
                if cur_a.is_empty() {
                    w::muted_small(ui, "No one assigned");
                }
                for a in &cur_a {
                    ui.horizontal(|ui| {
                        w::avatar(ui, &a.avatar_url, 20.0);
                        ui.label(&a.login);
                    });
                }
            },
            |ui| changed_a = common::user_picker(ui, "Assign up to 10 people", &users, &mut assignees, &mut self.af, 10),
        );
        if changed_a {
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.act(&ctx, move || g.update_issue(&o2, &n2, num, &json!({ "assignees": assignees })).map(|_| ()));
        }

        let all = self.labels_all.ready().cloned().unwrap_or_default();
        let mut labels: Vec<String> = pull.labels.iter().map(|l| l.name.clone()).collect();
        let cur_l = pull.labels.clone();
        let mut changed_l = false;
        common::sidebar_section(
            ui,
            "Labels",
            can,
            |ui| {
                if cur_l.is_empty() {
                    w::muted_small(ui, "None yet");
                }
                ui.horizontal_wrapped(|ui| {
                    for l in &cur_l {
                        w::issue_label(ui, &l.name, &l.color);
                    }
                });
            },
            |ui| changed_l = common::label_picker(ui, &all, &mut labels, &mut self.lf),
        );
        if changed_l {
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.act(&ctx, move || g.update_issue(&o2, &n2, num, &json!({ "labels": labels })).map(|_| ()));
        }
        if let Some(m) = &pull.milestone {
            common::sidebar_section(
                ui,
                "Milestone",
                false,
                |ui| {
                    w::meta(ui, Icon::Milestone, m.title.as_str());
                },
                |_| {},
            );
        }
        if w::button(ui, Variant::Invisible, Some(Icon::LinkExternal), "Open on github.com").clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(&pull.html_url));
        }
    }

    fn commits_tab(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let pal = w::p(ui);
        let (o, n) = on(repo);
        w::show_load(ui, &mut self.commits, |ui, commits| {
            w::bordered(ui, |ui| {
                for c in commits.iter() {
                    let resp = w::list_row(ui, |ui| {
                        ui.horizontal(|ui| {
                            icon(ui, Icon::GitCommit, pal.fg_muted);
                            ui.vertical(|ui| {
                                ui.label(RichText::new(c.title()).font(semibold(14.0)));
                                let who = c.author.as_ref().map(|a| a.login.clone()).unwrap_or_else(|| c.commit.author.name.clone());
                                let when = c.commit.author.date.as_ref().map(w::rel_time).unwrap_or_default();
                                w::muted_small(ui, format!("{who} committed {when}"));
                            });
                            w::right(ui, |ui| ui.label(RichText::new(c.short_sha()).monospace().color(pal.fg_muted)));
                        });
                    });
                    if resp.clicked() {
                        cx.go(Route::repo(&o, &n, RepoView::Commit(c.sha.clone())));
                    }
                    w::divider(ui);
                }
            });
        });
    }

    fn checks_tab(&mut self, ui: &mut Ui) {
        let pal = w::p(ui);
        w::show_load(ui, &mut self.checks, |ui, checks| {
            if checks.is_empty() {
                w::blankslate(ui, Icon::Checklist, "No checks", "No check runs were reported for the latest commit.");
                return;
            }
            w::bordered(ui, |ui| {
                for c in checks.iter() {
                    w::list_row(ui, |ui| {
                        ui.horizontal(|ui| {
                            common::run_status_icon(ui, &c.status, c.conclusion.as_deref());
                            if let Some(app) = &c.app {
                                // app logos live under /in/<app id>, same as on the checks page
                                w::org_avatar(ui, &format!("https://avatars.githubusercontent.com/in/{}?v=4", app.id), 20.0)
                                    .on_hover_text(&app.name);
                            }
                            ui.label(RichText::new(&c.name).font(semibold(14.0)));
                            let state = c.conclusion.clone().unwrap_or_else(|| c.status.clone());
                            w::muted_small(ui, state.replace('_', " "));
                            w::right(ui, |ui| {
                                if let Some(u) = &c.html_url
                                    && w::link(ui, "Details").clicked()
                                {
                                    ui.ctx().open_url(egui::OpenUrl::new_tab(u));
                                }
                                if let (Some(s), Some(e)) = (c.started_at, c.completed_at) {
                                    w::muted_small(ui, format!("{}s", (e - s).num_seconds()));
                                }
                            });
                        });
                    });
                    w::divider(ui);
                }
            });
            let _ = pal;
        });
    }

    fn files_tab(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo, pull: &Pull) {
        let ctx = ui.ctx().clone();
        let pal = w::p(ui);
        let (o, n) = on(repo);
        let num = self.number;
        let files = self.files.ready().cloned();
        let rcs = self.review_comments.ready().cloned().unwrap_or_default();
        ui.horizontal(|ui| {
            w::muted(ui, format!("{} files · +{} −{}", pull.changed_files, pull.additions, pull.deletions));
            w::right(ui, |ui| {
                let label = if self.pending.is_empty() {
                    "Review changes".to_string()
                } else {
                    format!("Finish your review ({})", self.pending.len())
                };
                let b = w::button_ex(ui, Variant::Primary, None, &label, Some(Icon::TriangleDown), pull.state == "open");
                egui::Popup::menu(&b)
                    .width(420.0)
                    .align(egui::RectAlign::BOTTOM_END)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        ui.label(RichText::new("Finish your review").font(semibold(14.0)));
                        w::text_area(ui, &mut self.review_body, "Leave a comment", 4, false);
                        let own = pull.user.login == cx.me.login;
                        ui.radio_value(&mut self.review_event, "COMMENT", "Comment — general feedback without explicit approval.");
                        ui.add_enabled_ui(!own, |ui| {
                            ui.radio_value(&mut self.review_event, "APPROVE", "Approve — give your approval to merge these changes.");
                            ui.radio_value(&mut self.review_event, "REQUEST_CHANGES", "Request changes — feedback that must be addressed.");
                        });
                        if own {
                            w::muted_small(ui, "You can't approve or request changes on your own pull request.");
                        }
                        ui.add_space(8.0);
                        let needs_body = self.review_event != "APPROVE" && self.review_body.trim().is_empty() && self.pending.is_empty();
                        if w::button_ex(ui, Variant::Primary, None, "Submit review", None, !needs_body).clicked() {
                            let comments: Vec<Value> = self
                                .pending
                                .iter()
                                .map(|c| json!({ "path": c.path, "line": c.line, "side": c.side, "body": c.body }))
                                .collect();
                            let (g, o2, n2, ev, body) = (cx.gh.clone(), o.clone(), n.clone(), self.review_event, self.review_body.clone());
                            self.act(&ctx, move || g.submit_review(&o2, &n2, num, ev, &body, comments));
                            self.pending.clear();
                            self.review_body.clear();
                            cx.ok("Review submitted");
                            ui.close();
                        }
                    });
            });
        });
        ui.add_space(12.0);
        let Some(files) = files else {
            w::spinner(ui);
            return;
        };
        for f in &files {
            let mut open = !self.collapsed.contains(&f.filename);
            let mut start_comment = false;
            common::file_diff_box(ui, f, &mut open, |ui| {
                if pull.state == "open" && w::button(ui, Variant::Invisible, Some(Icon::Comment), "Comment on line").clicked() {
                    start_comment = true;
                }
            });
            if open {
                self.collapsed.remove(&f.filename);
            } else {
                self.collapsed.insert(f.filename.clone());
            }
            if start_comment {
                self.line_form = Some((f.filename.clone(), String::new(), false, String::new()));
            }
            // line comment form for this file
            let mut close_form = false;
            if let Some((path, line, left, body)) = &mut self.line_form
                && path == &f.filename
            {
                Frame::new().stroke(Stroke::new(1.0, pal.border)).corner_radius(CornerRadius::same(6)).inner_margin(Margin::same(12)).show(
                    ui,
                    |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label("Line");
                            w::text_input(ui, line, "e.g. 42", 80.0);
                            ui.checkbox(left, "On the old (left) side");
                        });
                        w::text_area(ui, body, "Leave a comment on this line", 3, false);
                        ui.horizontal(|ui| {
                            let ln: Option<u64> = line.trim().parse().ok();
                            if w::button_ex(ui, Variant::Primary, None, "Add review comment", None, ln.is_some() && !body.trim().is_empty())
                                .clicked()
                            {
                                self.pending.push(PendingComment {
                                    path: path.clone(),
                                    line: ln.unwrap_or(1),
                                    side: if *left { "LEFT" } else { "RIGHT" },
                                    body: body.clone(),
                                });
                                close_form = true;
                            }
                            if w::button(ui, Variant::Default, None, "Cancel").clicked() {
                                close_form = true;
                            }
                        });
                    },
                );
                ui.add_space(12.0);
            }
            if close_form {
                self.line_form = None;
            }
            for c in self.pending.iter().filter(|c| c.path == f.filename) {
                w::flash(ui, FlashKind::Warning, &format!("Pending · line {} ({}): {}", c.line, c.side.to_lowercase(), c.body));
                ui.add_space(6.0);
            }
            let existing: Vec<&ReviewComment> = rcs.iter().filter(|c| c.path == f.filename).collect();
            if !existing.is_empty() {
                for c in existing {
                    Frame::new()
                        .fill(pal.canvas_subtle)
                        .stroke(Stroke::new(1.0, pal.border))
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(Margin::same(10))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                w::avatar(ui, &c.user.avatar_url, 20.0);
                                ui.label(RichText::new(&c.user.login).font(semibold(14.0)));
                                if let Some(l) = c.line.or(c.original_line) {
                                    w::muted_small(ui, format!("line {l}"));
                                }
                                if let Some(t) = &c.created_at {
                                    w::muted_small(ui, w::rel_time(t));
                                }
                            });
                            common::markdown(ui, &mut self.md, &c.body, None);
                        });
                    ui.add_space(6.0);
                }
                ui.add_space(10.0);
            }
        }
    }
}
