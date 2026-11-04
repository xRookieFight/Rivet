//! Single issue: title editing, timeline of comments with reactions, the
//! comment box with close / reopen, and the metadata sidebar.

use super::{RepoSub, on};
use crate::app::Cx;
use crate::github::models::{Comment, Issue, Label, Milestone, Repo, User};
use crate::icons::Icon;
use crate::route::{RepoView, Route};
use crate::task::Load;
use crate::views::common::{self, Composer};
use crate::widgets::{self as w, Variant};
use egui::{RichText, Ui};
use egui_commonmark::CommonMarkCache;
use serde_json::{Value, json};

pub struct IssueView {
    number: u64,
    issue: Load<Issue>,
    comments: Load<Vec<Comment>>,
    labels_all: Load<Vec<Label>>,
    users_all: Load<Vec<User>>,
    milestones: Load<Vec<Milestone>>,
    composer: Composer,
    md: CommonMarkCache,
    action: Load<Done>,
    editing_title: Option<String>,
    editing_body: Option<Composer>,
    editing_comment: Option<(u64, Composer)>,
    lf: String,
    af: String,
}

/// What a background action finished with, so we know what to refresh.
enum Done {
    Issue(Issue),
    Comments,
    Both(Issue),
}

impl IssueView {
    pub fn new(number: u64) -> Self {
        Self {
            number,
            issue: Load::Idle,
            comments: Load::Idle,
            labels_all: Load::Idle,
            users_all: Load::Idle,
            milestones: Load::Idle,
            composer: Composer::default(),
            md: CommonMarkCache::default(),
            action: Load::Idle,
            editing_title: None,
            editing_body: None,
            editing_comment: None,
            lf: String::new(),
            af: String::new(),
        }
    }

    fn load_comments(&mut self, ctx: &egui::Context, cx: &Cx, repo: &Repo) {
        let (g, (o, n), num) = (cx.gh.clone(), on(repo), self.number);
        self.comments = Load::spawn(ctx, move || g.issue_comments(&o, &n, num));
    }

    fn patch(&mut self, ctx: &egui::Context, cx: &Cx, repo: &Repo, body: Value) {
        let (g, (o, n), num) = (cx.gh.clone(), on(repo), self.number);
        self.action = Load::spawn(ctx, move || Ok(Done::Issue(g.update_issue(&o, &n, num, &body)?)));
    }
}

impl RepoSub for IssueView {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let ctx = ui.ctx().clone();
        let (o, n) = on(repo);
        if self.issue.is_idle() {
            let (g, o2, n2, num) = (cx.gh.clone(), o.clone(), n.clone(), self.number);
            self.issue = Load::spawn(&ctx, move || g.issue(&o2, &n2, num));
            self.load_comments(&ctx, cx, repo);
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.labels_all = Load::spawn(&ctx, move || g.labels(&o2, &n2));
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.users_all = Load::spawn(&ctx, move || g.assignees(&o2, &n2));
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.milestones = Load::spawn(&ctx, move || g.milestones(&o2, &n2));
        }
        match self.action.take() {
            Some(Ok(Done::Issue(i))) => self.issue = Load::Ready(i),
            Some(Ok(Done::Comments)) => self.load_comments(&ctx, cx, repo),
            Some(Ok(Done::Both(i))) => {
                self.issue = Load::Ready(i);
                self.load_comments(&ctx, cx, repo);
            }
            Some(Err(e)) => cx.err(e),
            None => {}
        }

        let issue = match self.issue.poll() {
            Load::Ready(i) => i.clone(),
            Load::Failed(e) => {
                let e = e.clone();
                super::section(ui, |ui| w::flash(ui, w::FlashKind::Danger, &e));
                return;
            }
            _ => {
                w::spinner(ui);
                return;
            }
        };
        // the issues endpoint happily serves PRs; send those to the PR screen
        if issue.is_pr() {
            cx.go_replace(Route::repo(&o, &n, RepoView::Pull(self.number)));
            return;
        }
        let pal = w::p(ui);
        let can_edit_issue = repo.can_push() || issue.user.login == cx.me.login;
        let can_triage = repo.permissions.as_ref().is_some_and(|p| p.triage || p.push || p.admin);
        let busy = self.action.is_loading();

        super::section(ui, |ui| {
            // title row
            match &mut self.editing_title {
                Some(t) => {
                    let mut save = false;
                    let mut cancel = false;
                    ui.horizontal(|ui| {
                        w::text_input(ui, t, "Title", (ui.available_width() - 180.0).max(200.0));
                        if w::button(ui, Variant::Default, None, "Save").clicked() {
                            save = true;
                        }
                        if w::button(ui, Variant::Invisible, None, "Cancel").clicked() {
                            cancel = true;
                        }
                    });
                    if save {
                        let title = t.trim().to_string();
                        self.editing_title = None;
                        self.patch(&ctx, cx, repo, json!({ "title": title }));
                    } else if cancel {
                        self.editing_title = None;
                    }
                }
                None => {
                    ui.horizontal_top(|ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2((ui.available_width() - 220.0).max(200.0), 0.0),
                            egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true),
                            |ui| {
                                ui.add(egui::Label::new(RichText::new(&issue.title).size(32.0)).wrap());
                                ui.label(RichText::new(format!("#{}", issue.number)).size(32.0).color(pal.fg_muted));
                            },
                        );
                        w::right(ui, |ui| {
                            if w::button(ui, Variant::Primary, None, "New issue").clicked() {
                                cx.go(Route::repo(&o, &n, RepoView::NewIssue));
                            }
                            if can_edit_issue && w::button(ui, Variant::Default, None, "Edit").clicked() {
                                self.editing_title = Some(issue.title.clone());
                            }
                        });
                    });
                }
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                w::state_label(ui, issue.item_state(), false);
                if w::title_link(ui, &issue.user.login, 14.0).clicked() {
                    cx.go(Route::User(issue.user.login.clone()));
                }
                let when = issue.created_at.as_ref().map(w::rel_time).unwrap_or_default();
                w::muted(ui, format!("opened this issue {when} · {} comments", issue.comments));
            });
            ui.add_space(16.0);
            w::divider(ui);
            ui.add_space(16.0);

            ui.horizontal_top(|ui| {
                let main_w = (ui.available_width() - 280.0).max(400.0);
                ui.allocate_ui_with_layout(egui::vec2(main_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_width(main_w);
                    self.timeline(ui, cx, repo, &issue, can_edit_issue, busy);
                });
                ui.add_space(24.0);
                ui.vertical(|ui| {
                    ui.set_width(256.0);
                    self.sidebar(ui, cx, repo, &issue, can_triage);
                });
            });
        });
    }
}

impl IssueView {
    fn timeline(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo, issue: &Issue, can_edit_issue: bool, busy: bool) {
        let ctx = ui.ctx().clone();
        let (o, n) = on(repo);
        let num = self.number;

        // issue body as the first comment
        if let Some(c) = &mut self.editing_body {
            let mut done = None;
            c.ui(ui, "Leave a comment", 8, |ui, text| {
                if w::button(ui, Variant::Primary, None, "Update comment").clicked() {
                    done = Some(Some(text.clone()));
                }
                if w::button(ui, Variant::Default, None, "Cancel").clicked() {
                    done = Some(None);
                }
            });
            if let Some(d) = done {
                self.editing_body = None;
                if let Some(body) = d {
                    self.patch(&ctx, cx, repo, json!({ "body": body }));
                }
            }
        } else {
            let act = common::comment_card(
                ui,
                &mut self.md,
                &issue.user,
                issue.created_at.as_ref(),
                issue.body.as_deref().unwrap_or(""),
                &issue.author_association,
                issue.reactions.as_ref(),
                false,
                can_edit_issue,
                cx,
            );
            if act.edit {
                self.editing_body = Some(Composer::with_text(issue.body.clone().unwrap_or_default()));
            }
            if act.quote {
                self.composer.text.push_str(&quote(issue.body.as_deref().unwrap_or("")));
            }
            if let Some(r) = act.react {
                let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
                self.action = Load::spawn(&ctx, move || {
                    g.react(&o2, &n2, &format!("issues/{num}"), r)?;
                    Ok(Done::Issue(g.issue(&o2, &n2, num)?))
                });
            }
        }
        ui.add_space(16.0);

        let comments = self.comments.ready().cloned();
        match comments {
            None => w::spinner(ui),
            Some(list) => {
                for c in list {
                    let mine = c.user.login == cx.me.login || repo.can_push();
                    if let Some((id, comp)) = &mut self.editing_comment
                        && *id == c.id
                    {
                        let mut done = None;
                        comp.ui(ui, "Leave a comment", 6, |ui, text| {
                            if w::button(ui, Variant::Primary, None, "Update comment").clicked() {
                                done = Some(Some(text.clone()));
                            }
                            if w::button(ui, Variant::Default, None, "Cancel").clicked() {
                                done = Some(None);
                            }
                        });
                        if let Some(d) = done {
                            self.editing_comment = None;
                            if let Some(body) = d {
                                let (g, o2, n2, id) = (cx.gh.clone(), o.clone(), n.clone(), c.id);
                                self.action = Load::spawn(&ctx, move || {
                                    g.edit_comment(&o2, &n2, id, &body)?;
                                    Ok(Done::Comments)
                                });
                            }
                        }
                        ui.add_space(16.0);
                        continue;
                    }
                    let act = common::comment_card(
                        ui,
                        &mut self.md,
                        &c.user,
                        c.created_at.as_ref(),
                        c.body.as_deref().unwrap_or(""),
                        &c.author_association,
                        c.reactions.as_ref(),
                        c.user.login == issue.user.login,
                        mine,
                        cx,
                    );
                    if act.edit {
                        self.editing_comment = Some((c.id, Composer::with_text(c.body.clone().unwrap_or_default())));
                    }
                    if act.quote {
                        self.composer.text.push_str(&quote(c.body.as_deref().unwrap_or("")));
                    }
                    if act.delete {
                        let (g, o2, n2, id) = (cx.gh.clone(), o.clone(), n.clone(), c.id);
                        self.action = Load::spawn(&ctx, move || {
                            g.delete_comment(&o2, &n2, id)?;
                            Ok(Done::Comments)
                        });
                    }
                    if let Some(r) = act.react {
                        let (g, o2, n2, id) = (cx.gh.clone(), o.clone(), n.clone(), c.id);
                        self.action = Load::spawn(&ctx, move || {
                            g.react(&o2, &n2, &format!("issues/comments/{id}"), r)?;
                            Ok(Done::Comments)
                        });
                    }
                    ui.add_space(16.0);
                }
            }
        }

        w::divider(ui);
        ui.add_space(16.0);
        if issue.locked && !repo.can_push() {
            w::flash(ui, w::FlashKind::Warning, "This conversation has been locked and limited to collaborators.");
            return;
        }
        ui.horizontal_top(|ui| {
            w::avatar(ui, &cx.me.avatar_url, 40.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("Add a comment").font(crate::theme::semibold(16.0)));
                ui.add_space(4.0);
                let mut submit = false;
                let mut state_change: Option<Value> = None;
                let open = issue.state == "open";
                self.composer.ui(ui, "Use Markdown to format your comment", 6, |ui, text| {
                    let has_text = !text.trim().is_empty();
                    if w::button_ex(ui, Variant::Primary, None, "Comment", None, has_text && !busy).clicked() {
                        submit = true;
                    }
                    if can_edit_issue {
                        if open {
                            let label = if has_text { "Close with comment" } else { "Close issue" };
                            if w::button_ex(ui, Variant::Default, Some(Icon::IssueClosed), label, None, !busy).clicked() {
                                state_change = Some(json!({ "state": "closed", "state_reason": "completed" }));
                            }
                            let more = w::icon_button(ui, Icon::TriangleDown, "More close options");
                            egui::Popup::menu(&more).align(egui::RectAlign::BOTTOM_END).show(|ui| {
                                if w::menu_item(ui, Some(Icon::IssueClosed), "Close as completed").clicked() {
                                    state_change = Some(json!({ "state": "closed", "state_reason": "completed" }));
                                }
                                if w::menu_item(ui, Some(Icon::Skip), "Close as not planned").clicked() {
                                    state_change = Some(json!({ "state": "closed", "state_reason": "not_planned" }));
                                }
                            });
                        } else if w::button_ex(ui, Variant::Default, Some(Icon::IssueReopened), "Reopen issue", None, !busy).clicked() {
                            state_change = Some(json!({ "state": "open" }));
                        }
                    }
                });
                if submit || state_change.is_some() {
                    let text = std::mem::take(&mut self.composer.text);
                    let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
                    self.action = Load::spawn(&ctx, move || {
                        if !text.trim().is_empty() {
                            g.add_comment(&o2, &n2, num, &text)?;
                        }
                        let issue = match state_change {
                            Some(b) => g.update_issue(&o2, &n2, num, &b)?,
                            None => g.issue(&o2, &n2, num)?,
                        };
                        Ok(Done::Both(issue))
                    });
                }
            });
        });
    }

    fn sidebar(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo, issue: &Issue, can_triage: bool) {
        let ctx = ui.ctx().clone();
        let users = self.users_all.ready().cloned().unwrap_or_default();
        let mut assignees: Vec<String> = issue.assignees.iter().map(|a| a.login.clone()).collect();
        let current = issue.assignees.clone();
        let mut changed_assignees = false;
        common::sidebar_section(
            ui,
            "Assignees",
            can_triage,
            |ui| {
                if current.is_empty() {
                    w::muted_small(ui, "No one assigned");
                }
                for a in &current {
                    ui.horizontal(|ui| {
                        w::avatar(ui, &a.avatar_url, 20.0);
                        if w::title_link(ui, &a.login, 14.0).clicked() {
                            cx.go(Route::User(a.login.clone()));
                        }
                    });
                }
            },
            |ui| {
                changed_assignees = common::user_picker(ui, "Assign up to 10 people", &users, &mut assignees, &mut self.af, 10);
            },
        );
        if changed_assignees {
            self.patch(&ctx, cx, repo, json!({ "assignees": assignees }));
        }

        let all_labels = self.labels_all.ready().cloned().unwrap_or_default();
        let mut labels: Vec<String> = issue.labels.iter().map(|l| l.name.clone()).collect();
        let cur_labels = issue.labels.clone();
        let mut changed_labels = false;
        common::sidebar_section(
            ui,
            "Labels",
            can_triage,
            |ui| {
                if cur_labels.is_empty() {
                    w::muted_small(ui, "None yet");
                }
                ui.horizontal_wrapped(|ui| {
                    for l in &cur_labels {
                        w::issue_label(ui, &l.name, &l.color);
                    }
                });
            },
            |ui| {
                changed_labels = common::label_picker(ui, &all_labels, &mut labels, &mut self.lf);
            },
        );
        if changed_labels {
            self.patch(&ctx, cx, repo, json!({ "labels": labels }));
        }

        let ms = self.milestones.ready().cloned().unwrap_or_default();
        let cur_ms = issue.milestone.clone();
        let mut pick_ms: Option<Option<u64>> = None;
        common::sidebar_section(
            ui,
            "Milestone",
            can_triage,
            |ui| match &cur_ms {
                Some(m) => {
                    w::meta(ui, Icon::Milestone, m.title.as_str());
                    let total = (m.open_issues + m.closed_issues).max(1);
                    let frac = m.closed_issues as f32 / total as f32;
                    ui.add(egui::ProgressBar::new(frac).desired_height(8.0).fill(w::p(ui).success_emphasis));
                }
                None => {
                    w::muted_small(ui, "No milestone");
                }
            },
            |ui| {
                if w::menu_item_ex(ui, None, "Clear milestone", cur_ms.is_none(), false).clicked() {
                    pick_ms = Some(None);
                }
                for m in &ms {
                    if w::menu_item_ex(ui, Some(Icon::Milestone), &m.title, cur_ms.as_ref().map(|x| x.number) == Some(m.number), false)
                        .clicked()
                    {
                        pick_ms = Some(Some(m.number));
                    }
                }
            },
        );
        if let Some(m) = pick_ms {
            self.patch(&ctx, cx, repo, json!({ "milestone": m }));
        }

        ui.horizontal(|ui| {
            if w::button(ui, Variant::Invisible, Some(Icon::LinkExternal), "Open on github.com").clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(&issue.html_url));
            }
        });
        ui.horizontal(|ui| {
            if w::button(ui, Variant::Invisible, Some(Icon::Copy), "Copy link").clicked() {
                ui.ctx().copy_text(issue.html_url.clone());
                cx.ok("Copied!");
            }
        });
    }
}

fn quote(text: &str) -> String {
    let mut q: String = text.lines().map(|l| format!("> {l}\n")).collect();
    q.push('\n');
    q
}
