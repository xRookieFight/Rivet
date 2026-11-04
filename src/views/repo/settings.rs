//! Repository settings: general details, features, pull request options,
//! branch protection rules and the danger zone.

use super::{RepoSub, on};
use crate::app::Cx;
use crate::github::models::{Branch, BranchProtection, Repo};
use crate::icons::Icon;
use crate::route::{RepoView, Route};
use crate::task::Load;
use crate::theme::semibold;
use crate::widgets::{self as w, FlashKind, Variant};
use egui::{CornerRadius, Frame, Margin, RichText, Stroke, Ui};
use serde_json::{Value, json};

#[derive(Default)]
pub struct RepoSettings {
    section: usize,
    init: bool,
    name: String,
    description: String,
    homepage: String,
    topics: String,
    default_branch: String,
    has_issues: bool,
    has_wiki: bool,
    has_projects: bool,
    has_discussions: bool,
    is_template: bool,
    allow_merge: bool,
    allow_squash: bool,
    allow_rebase: bool,
    delete_on_merge: bool,
    branches: Load<Vec<Branch>>,
    action: Load<Option<Repo>>,
    protect_branch: String,
    protection: Load<Option<BranchProtection>>,
    rules: Rules,
    danger: Option<Danger>,
    confirm_text: String,
}

#[derive(Default, Clone)]
struct Rules {
    require_pr: bool,
    approvals: u32,
    dismiss_stale: bool,
    code_owners: bool,
    require_checks: bool,
    strict: bool,
    contexts: String,
    enforce_admins: bool,
    linear: bool,
    force_push: bool,
    deletions: bool,
    conversation: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Danger {
    Visibility,
    Archive,
    Delete,
}

impl Rules {
    fn from(p: &BranchProtection) -> Self {
        let enabled = |v: &Option<Value>| v.as_ref().and_then(|x| x["enabled"].as_bool()).unwrap_or(false);
        let reviews = p.required_pull_request_reviews.clone().unwrap_or(Value::Null);
        let checks = p.required_status_checks.clone().unwrap_or(Value::Null);
        Rules {
            require_pr: p.required_pull_request_reviews.is_some(),
            approvals: reviews["required_approving_review_count"].as_u64().unwrap_or(1) as u32,
            dismiss_stale: reviews["dismiss_stale_reviews"].as_bool().unwrap_or(false),
            code_owners: reviews["require_code_owner_reviews"].as_bool().unwrap_or(false),
            require_checks: p.required_status_checks.is_some(),
            strict: checks["strict"].as_bool().unwrap_or(false),
            contexts: checks["contexts"]
                .as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(", "))
                .unwrap_or_default(),
            enforce_admins: enabled(&p.enforce_admins),
            linear: enabled(&p.required_linear_history),
            force_push: enabled(&p.allow_force_pushes),
            deletions: enabled(&p.allow_deletions),
            conversation: enabled(&p.required_conversation_resolution),
        }
    }

    /// The PUT protection endpoint wants every top level key, nulls included.
    fn to_body(&self) -> Value {
        let contexts: Vec<String> = self.contexts.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        json!({
            "required_status_checks": if self.require_checks { json!({ "strict": self.strict, "contexts": contexts }) } else { Value::Null },
            "enforce_admins": self.enforce_admins,
            "required_pull_request_reviews": if self.require_pr {
                json!({ "required_approving_review_count": self.approvals, "dismiss_stale_reviews": self.dismiss_stale, "require_code_owner_reviews": self.code_owners })
            } else { Value::Null },
            "restrictions": Value::Null,
            "required_linear_history": self.linear,
            "allow_force_pushes": self.force_push,
            "allow_deletions": self.deletions,
            "required_conversation_resolution": self.conversation,
        })
    }
}

const SECTIONS: [(Icon, &str); 3] = [(Icon::Gear, "General"), (Icon::GitBranch, "Branches"), (Icon::Alert, "Danger zone")];

impl RepoSettings {
    fn fill(&mut self, r: &Repo) {
        self.name = r.name.clone();
        self.description = r.description.clone().unwrap_or_default();
        self.homepage = r.homepage.clone().unwrap_or_default();
        self.topics = r.topics.join(", ");
        self.default_branch = r.default_branch.clone();
        self.has_issues = r.has_issues;
        self.has_wiki = r.has_wiki;
        self.has_projects = r.has_projects;
        self.has_discussions = r.has_discussions;
        self.is_template = r.is_template;
        self.allow_merge = r.allow_merge_commit.unwrap_or(true);
        self.allow_squash = r.allow_squash_merge.unwrap_or(true);
        self.allow_rebase = r.allow_rebase_merge.unwrap_or(true);
        self.delete_on_merge = r.delete_branch_on_merge.unwrap_or(false);
        self.protect_branch = r.default_branch.clone();
        self.init = true;
    }

    fn save(&mut self, ctx: &egui::Context, cx: &Cx, repo: &Repo, body: Value, topics: Option<Vec<String>>) {
        let (g, (o, n)) = (cx.gh.clone(), on(repo));
        self.action = Load::spawn(ctx, move || {
            let r = g.update_repo(&o, &n, &body)?;
            if let Some(t) = topics {
                g.replace_topics(&r.owner.login, &r.name, &t)?;
            }
            Ok(Some(r))
        });
    }
}

impl RepoSub for RepoSettings {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let ctx = ui.ctx().clone();
        if !self.init {
            self.fill(repo);
            let (g, (o, n)) = (cx.gh.clone(), on(repo));
            self.branches = Load::spawn(&ctx, move || g.branches(&o, &n));
        }
        match self.action.take() {
            Some(Ok(Some(r))) => {
                cx.ok("Settings saved");
                // renamed or otherwise changed: reload the repo from its (maybe new) route
                cx.go_replace(Route::repo(&r.owner.login, &r.name, RepoView::Settings));
            }
            Some(Ok(None)) => {
                cx.ok("Done");
                self.protection = Load::Idle;
            }
            Some(Err(e)) => cx.err(e),
            None => {}
        }
        if !repo.is_admin() {
            super::section(ui, |ui| w::flash(ui, FlashKind::Warning, "You need admin access to change this repository's settings."));
            return;
        }
        let pal = w::p(ui);
        super::section(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(240.0);
                    for (i, (ic, label)) in SECTIONS.iter().enumerate() {
                        let r = w::menu_item(ui, Some(*ic), label);
                        if i == self.section {
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(r.rect.left_top() + egui::vec2(-8.0, 6.0), egui::vec2(4.0, 20.0)),
                                CornerRadius::same(2),
                                pal.accent_emphasis,
                            );
                        }
                        if r.clicked() {
                            self.section = i;
                        }
                    }
                });
                ui.add_space(24.0);
                ui.vertical(|ui| match self.section {
                    0 => self.general(ui, cx, repo),
                    1 => self.branch_rules(ui, cx, repo),
                    _ => self.danger_zone(ui, cx, repo),
                });
            });
        });
        if let Some(d) = self.danger {
            self.danger_modal(&ctx, cx, repo, d);
        }
    }
}

fn title(ui: &mut Ui, t: &str) {
    ui.label(RichText::new(t).size(24.0));
    ui.add_space(8.0);
    w::divider(ui);
    ui.add_space(16.0);
}

fn check(ui: &mut Ui, v: &mut bool, label: &str, desc: &str) {
    ui.checkbox(v, RichText::new(label).font(semibold(14.0)));
    if !desc.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(26.0);
            w::muted_small(ui, desc);
        });
    }
    ui.add_space(8.0);
}

impl RepoSettings {
    fn general(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let ctx = ui.ctx().clone();
        let busy = self.action.is_loading();
        title(ui, "General");
        w::form_label(ui, "Repository name", false);
        ui.horizontal(|ui| {
            w::text_input(ui, &mut self.name, "", 300.0);
            if w::button_ex(
                ui,
                Variant::Default,
                None,
                "Rename",
                None,
                !busy && self.name.trim() != repo.name && !self.name.trim().is_empty(),
            )
            .clicked()
            {
                self.save(&ctx, cx, repo, json!({ "name": self.name.trim() }), None);
            }
        });
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            w::form_label(ui, "Default branch", false);
        });
        let b = w::button_ex(ui, Variant::Default, Some(Icon::GitBranch), &self.default_branch, Some(Icon::TriangleDown), true);
        egui::Popup::menu(&b).width(260.0).show(|ui| {
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                if let Some(list) = self.branches.ready().cloned() {
                    for br in list {
                        if w::menu_item_ex(ui, Some(Icon::GitBranch), &br.name, br.name == self.default_branch, false).clicked() {
                            self.default_branch = br.name.clone();
                        }
                    }
                }
            });
        });
        ui.add_space(12.0);
        w::form_label(ui, "Description", false);
        w::text_input(ui, &mut self.description, "", ui.available_width());
        ui.add_space(8.0);
        w::form_label(ui, "Website", false);
        w::text_input(ui, &mut self.homepage, "https://", ui.available_width());
        ui.add_space(8.0);
        w::form_label(ui, "Topics (comma separated)", false);
        w::text_input(ui, &mut self.topics, "rust, gui, github", ui.available_width());
        ui.add_space(12.0);
        check(
            ui,
            &mut self.is_template,
            "Template repository",
            "Template repositories let users generate new repositories with the same structure and files.",
        );
        ui.add_space(8.0);
        ui.label(RichText::new("Features").size(20.0));
        ui.add_space(8.0);
        check(ui, &mut self.has_wiki, "Wikis", "Wikis host documentation for your repository.");
        check(ui, &mut self.has_issues, "Issues", "Issues integrate lightweight task tracking into your repository.");
        check(ui, &mut self.has_discussions, "Discussions", "Discussions is the space for your community to have conversations.");
        check(ui, &mut self.has_projects, "Projects", "Projects on GitHub are created at the repository owner's level.");
        ui.add_space(8.0);
        ui.label(RichText::new("Pull Requests").size(20.0));
        ui.add_space(8.0);
        check(
            ui,
            &mut self.allow_merge,
            "Allow merge commits",
            "Add all commits from the head branch to the base branch with a merge commit.",
        );
        check(
            ui,
            &mut self.allow_squash,
            "Allow squash merging",
            "Combine all commits from the head branch into a single commit in the base branch.",
        );
        check(
            ui,
            &mut self.allow_rebase,
            "Allow rebase merging",
            "Add all commits from the head branch onto the base branch individually.",
        );
        check(ui, &mut self.delete_on_merge, "Automatically delete head branches", "Deleted branches will still be able to be restored.");
        if !(self.allow_merge || self.allow_squash || self.allow_rebase) {
            w::flash(ui, FlashKind::Danger, "You must select at least one merge method.");
        }
        ui.add_space(12.0);
        let ok = !busy && (self.allow_merge || self.allow_squash || self.allow_rebase);
        if w::button_ex(ui, Variant::Primary, None, if busy { "Saving…" } else { "Save changes" }, None, ok).clicked() {
            let topics: Vec<String> =
                self.topics.split(',').map(|t| t.trim().to_lowercase().replace(' ', "-")).filter(|t| !t.is_empty()).collect();
            let body = json!({
                "description": self.description,
                "homepage": self.homepage,
                "default_branch": self.default_branch,
                "is_template": self.is_template,
                "has_issues": self.has_issues,
                "has_wiki": self.has_wiki,
                "has_projects": self.has_projects,
                "has_discussions": self.has_discussions,
                "allow_merge_commit": self.allow_merge,
                "allow_squash_merge": self.allow_squash,
                "allow_rebase_merge": self.allow_rebase,
                "delete_branch_on_merge": self.delete_on_merge,
            });
            self.save(&ctx, cx, repo, body, Some(topics));
        }
    }

    fn branch_rules(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let ctx = ui.ctx().clone();
        title(ui, "Branch protection rules");
        w::muted(
            ui,
            "Define branch protection rules to disable force pushing, prevent branches from being deleted, and optionally require status checks before merging.",
        );
        ui.add_space(12.0);
        let mut picked = None;
        let b = w::button_ex(ui, Variant::Default, Some(Icon::GitBranch), &self.protect_branch, Some(Icon::TriangleDown), true);
        egui::Popup::menu(&b).width(260.0).show(|ui| {
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                if let Some(list) = self.branches.ready().cloned() {
                    for br in list {
                        let label = if br.protected { format!("{} (protected)", br.name) } else { br.name.clone() };
                        if w::menu_item_ex(ui, Some(Icon::GitBranch), &label, br.name == self.protect_branch, false).clicked() {
                            picked = Some(br.name.clone());
                        }
                    }
                }
            });
        });
        if let Some(p) = picked {
            self.protect_branch = p;
            self.protection = Load::Idle;
        }
        if self.protection.is_idle() {
            let (g, (o, n), b) = (cx.gh.clone(), on(repo), self.protect_branch.clone());
            self.protection = Load::spawn(&ctx, move || g.branch_protection(&o, &n, &b));
            self.rules = Rules::default();
        }
        ui.add_space(12.0);
        let state = match self.protection.poll() {
            Load::Ready(p) => Some(p.clone()),
            Load::Failed(e) => {
                w::flash(ui, FlashKind::Danger, &e.clone());
                return;
            }
            _ => {
                w::spinner(ui);
                return;
            }
        };
        let existing = state.flatten();
        if let Some(p) = &existing
            && !self.rules.require_pr
            && !self.rules.require_checks
            && self.rules.approvals == 0
        {
            self.rules = Rules::from(p);
        }
        if self.rules.approvals == 0 {
            self.rules.approvals = 1;
        }
        let pal = w::p(ui);
        Frame::new().stroke(Stroke::new(1.0, pal.border)).corner_radius(CornerRadius::same(6)).inner_margin(Margin::same(16)).show(
            ui,
            |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("Rules for {}", self.protect_branch)).font(semibold(16.0)));
                    if existing.is_some() {
                        w::outline_label(ui, "Protected", Some(pal.success));
                    }
                });
                ui.add_space(12.0);
                let r = &mut self.rules;
                check(
                    ui,
                    &mut r.require_pr,
                    "Require a pull request before merging",
                    "All commits must be made to a non-protected branch and submitted via a pull request.",
                );
                if r.require_pr {
                    ui.horizontal(|ui| {
                        ui.add_space(26.0);
                        ui.label("Required approvals");
                        ui.add(egui::DragValue::new(&mut r.approvals).range(0..=6));
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(26.0);
                        ui.checkbox(&mut r.dismiss_stale, "Dismiss stale approvals when new commits are pushed");
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(26.0);
                        ui.checkbox(&mut r.code_owners, "Require review from Code Owners");
                    });
                    ui.add_space(8.0);
                }
                check(
                    ui,
                    &mut r.require_checks,
                    "Require status checks to pass before merging",
                    "Choose which status checks must pass before branches can be merged.",
                );
                if r.require_checks {
                    ui.horizontal(|ui| {
                        ui.add_space(26.0);
                        ui.checkbox(&mut r.strict, "Require branches to be up to date before merging");
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(26.0);
                        w::text_input(ui, &mut r.contexts, "check names, comma separated", 360.0);
                    });
                    ui.add_space(8.0);
                }
                check(ui, &mut r.conversation, "Require conversation resolution before merging", "");
                check(ui, &mut r.linear, "Require linear history", "Prevent merge commits from being pushed.");
                check(ui, &mut r.enforce_admins, "Do not allow bypassing the above settings", "The settings apply to administrators too.");
                check(ui, &mut r.force_push, "Allow force pushes", "Permit force pushes for all users with push access.");
                check(ui, &mut r.deletions, "Allow deletions", "Allow users with push access to delete matching branches.");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let busy = self.action.is_loading();
                    if w::button_ex(ui, Variant::Primary, None, if existing.is_some() { "Save changes" } else { "Create" }, None, !busy)
                        .clicked()
                    {
                        let (g, (o, n), b, body) = (cx.gh.clone(), on(repo), self.protect_branch.clone(), self.rules.to_body());
                        self.action = Load::spawn(&ctx, move || {
                            g.set_branch_protection(&o, &n, &b, &body)?;
                            Ok(None)
                        });
                    }
                    if existing.is_some() && w::button_ex(ui, Variant::Danger, None, "Delete rule", None, !busy).clicked() {
                        let (g, (o, n), b) = (cx.gh.clone(), on(repo), self.protect_branch.clone());
                        self.action = Load::spawn(&ctx, move || {
                            g.remove_branch_protection(&o, &n, &b)?;
                            Ok(None)
                        });
                    }
                });
            },
        );
    }

    fn danger_zone(&mut self, ui: &mut Ui, _cx: &mut Cx, repo: &Repo) {
        let pal = w::p(ui);
        title(ui, "Danger Zone");
        Frame::new().stroke(Stroke::new(1.0, pal.danger_emphasis)).corner_radius(CornerRadius::same(6)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let vis = if repo.private { "private" } else { "public" };
            let items = [
                (
                    Danger::Visibility,
                    "Change repository visibility",
                    format!("This repository is currently {vis}."),
                    if repo.private { "Make public" } else { "Make private" },
                ),
                (
                    Danger::Archive,
                    if repo.archived { "Unarchive this repository" } else { "Archive this repository" },
                    if repo.archived {
                        "Make this repository writable again.".to_string()
                    } else {
                        "Mark this repository archived and read-only.".to_string()
                    },
                    if repo.archived { "Unarchive this repository" } else { "Archive this repository" },
                ),
                (
                    Danger::Delete,
                    "Delete this repository",
                    "Once you delete a repository, there is no going back. Please be certain.".to_string(),
                    "Delete this repository",
                ),
            ];
            for (i, (d, t, desc, btn)) in items.into_iter().enumerate() {
                if i > 0 {
                    w::divider(ui);
                }
                Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new(t).font(semibold(14.0)));
                            w::muted_small(ui, desc);
                        });
                        w::right(ui, |ui| {
                            if w::button(ui, Variant::Danger, None, btn).clicked() {
                                self.danger = Some(d);
                                self.confirm_text.clear();
                            }
                        });
                    });
                });
            }
        });
    }

    fn danger_modal(&mut self, ctx: &egui::Context, cx: &mut Cx, repo: &Repo, d: Danger) {
        let pal = crate::theme::pal(ctx);
        let modal = egui::Modal::new(egui::Id::unique("danger-modal"))
            .backdrop_color(pal.overlay_shadow)
            .frame(Frame::new().fill(pal.canvas).stroke(Stroke::new(1.0, pal.border)).corner_radius(CornerRadius::same(12)).inner_margin(Margin::same(16)))
            .show(ctx, |ui| {
                ui.set_width(460.0);
                let t = match d {
                    Danger::Visibility => "Change repository visibility",
                    Danger::Archive => "Archive repository",
                    Danger::Delete => "Delete repository",
                };
                ui.label(RichText::new(t).font(semibold(16.0)));
                ui.add_space(8.0);
                if d == Danger::Delete {
                    w::flash(ui, FlashKind::Danger, "This will permanently delete the repository, wiki, issues, comments, packages, secrets, workflow runs, and remove all collaborator associations.");
                    ui.add_space(8.0);
                }
                ui.label(format!("To confirm, type \"{}\" in the box below", repo.full_name));
                w::text_input(ui, &mut self.confirm_text, "", ui.available_width());
                ui.add_space(12.0);
                let ok = self.confirm_text.trim() == repo.full_name;
                ui.horizontal(|ui| {
                    w::right(ui, |ui| {
                        if w::button_ex(ui, Variant::Danger, None, "I understand, continue", None, ok).clicked() {
                            let (g, (o, n)) = (cx.gh.clone(), on(repo));
                            match d {
                                Danger::Visibility => {
                                    let private = !repo.private;
                                    self.action = Load::spawn(ctx, move || Ok(Some(g.update_repo(&o, &n, &json!({ "private": private }))?)));
                                }
                                Danger::Archive => {
                                    let archived = !repo.archived;
                                    self.action = Load::spawn(ctx, move || Ok(Some(g.update_repo(&o, &n, &json!({ "archived": archived }))?)));
                                }
                                Danger::Delete => match cx.gh.delete_repo(&o, &n) {
                                    // done inline so we can leave the page right away
                                    Ok(()) => {
                                        cx.ok(format!("{} was deleted", repo.full_name));
                                        cx.go(Route::Dashboard);
                                    }
                                    Err(e) => cx.err(format!("{e:#}")),
                                },
                            }
                            self.danger = None;
                        }
                        if w::button(ui, Variant::Default, None, "Cancel").clicked() {
                            self.danger = None;
                        }
                    });
                });
            });
        if modal.should_close() {
            self.danger = None;
        }
    }
}
