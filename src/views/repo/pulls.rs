//! Compare & open a pull request: base / head pickers, commits and file diff
//! preview, and pull request templates (single file or the multi template folder).

use super::{RepoSub, on};
use crate::app::Cx;
use crate::github::models::{Compare, Pull, Repo};
use crate::icons::{Icon, icon};
use crate::route::{RepoView, Route};
use crate::task::Load;
use crate::theme::semibold;
use crate::views::common::{self, Composer};
use crate::widgets::{self as w, FlashKind, Variant};
use egui::{RichText, Ui};
use serde_json::json;
use std::collections::HashSet;

pub struct NewPull {
    base: Option<String>,
    head: Option<String>,
    branches: Load<Vec<String>>,
    compare: Load<Compare>,
    templates: Load<Vec<(String, String)>>,
    template_applied: bool,
    title: String,
    composer: Composer,
    draft: bool,
    create: Load<Pull>,
    error: Option<String>,
    bf: String,
    hf: String,
    collapsed: HashSet<String>,
    tab: usize,
}

impl NewPull {
    pub fn new(base: Option<String>, head: Option<String>) -> Self {
        Self {
            base,
            head,
            branches: Load::Idle,
            compare: Load::Idle,
            templates: Load::Idle,
            template_applied: false,
            title: String::new(),
            composer: Composer::default(),
            draft: false,
            create: Load::Idle,
            error: None,
            bf: String::new(),
            hf: String::new(),
            collapsed: HashSet::new(),
            tab: 0,
        }
    }
}

/// GitHub looks for PR templates in a handful of places; check all of them.
fn load_pr_templates(cx: &Cx, ctx: &egui::Context, repo: &Repo) -> Load<Vec<(String, String)>> {
    let (g, (o, n), b) = (cx.gh.clone(), on(repo), repo.default_branch.clone());
    Load::spawn(ctx, move || {
        let mut out = vec![];
        for dir in [".github/PULL_REQUEST_TEMPLATE", "PULL_REQUEST_TEMPLATE", "docs/PULL_REQUEST_TEMPLATE"] {
            if let Ok(list) = g.contents(&o, &n, dir, &b) {
                for f in list.into_iter().filter(|f| !f.is_dir() && f.name.to_lowercase().ends_with(".md")) {
                    if let Some(t) = g.file(&o, &n, &f.path, &b)?.text() {
                        out.push((f.name.clone(), t));
                    }
                }
            }
        }
        for path in [
            ".github/pull_request_template.md",
            "pull_request_template.md",
            "docs/pull_request_template.md",
            ".github/PULL_REQUEST_TEMPLATE.md",
        ] {
            if let Some(f) = g.file_opt(&o, &n, path, &b)?
                && let Some(t) = f.text()
            {
                out.insert(0, ("Default".into(), t));
                break;
            }
        }
        Ok(out)
    })
}

impl RepoSub for NewPull {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let ctx = ui.ctx().clone();
        let (o, n) = on(repo);
        let base = self.base.clone().unwrap_or_else(|| repo.default_branch.clone());
        if self.branches.is_idle() {
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.branches = Load::spawn(&ctx, move || Ok(g.branches(&o2, &n2)?.into_iter().map(|b| b.name).collect()));
            self.templates = load_pr_templates(cx, &ctx, repo);
        }
        if let Some(head) = self.head.clone()
            && self.compare.is_idle()
            && head != base
        {
            let (g, o2, n2, b) = (cx.gh.clone(), o.clone(), n.clone(), base.clone());
            self.compare = Load::spawn(&ctx, move || g.compare(&o2, &n2, &b, &head));
        }
        if !self.template_applied
            && let Some(t) = self.templates.ready()
        {
            if let Some((_, body)) = t.first()
                && self.composer.text.is_empty()
            {
                self.composer.text = body.clone();
            }
            self.template_applied = true;
        }
        // title defaults to the single commit's message like github does
        if self.title.is_empty()
            && let Some(c) = self.compare.ready()
        {
            self.title = match c.commits.len() {
                1 => c.commits[0].title().to_string(),
                _ => self.head.clone().unwrap_or_default().replace(['-', '_'], " "),
            };
        }
        match self.create.take() {
            Some(Ok(p)) => {
                cx.ok(format!("Opened pull request #{}", p.number));
                cx.go(Route::repo(&o, &n, RepoView::Pull(p.number)));
            }
            Some(Err(e)) => self.error = Some(e),
            None => {}
        }

        let pal = w::p(ui);
        super::section(ui, |ui| {
            w::heading(ui, if self.head.is_some() { "Open a pull request" } else { "Compare changes" }, 24.0);
            ui.add_space(4.0);
            w::muted(ui, "Choose two branches to see what's changed or to start a new pull request.");
            ui.add_space(16.0);
            let branches = self.branches.ready().cloned().unwrap_or_default();
            let mut new_base = None;
            let mut new_head = None;
            w::bordered(ui, |ui| {
                egui::Frame::new().fill(pal.canvas_subtle).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        icon(ui, Icon::GitCompare, pal.fg_muted);
                        new_base = branch_select(ui, "base", &base, &branches, &mut self.bf);
                        icon(ui, Icon::ArrowLeft, pal.fg_muted);
                        let head_label = self.head.clone().unwrap_or_else(|| "compare: choose…".into());
                        new_head = branch_select(ui, "compare", &head_label, &branches, &mut self.hf);
                        if let Some(c) = self.compare.ready() {
                            let (ic, color, text) = if c.status == "diverged" || c.status == "ahead" {
                                (Icon::Check, pal.success, "Able to merge. These branches can be automatically merged.")
                            } else if c.status == "identical" {
                                (Icon::Info, pal.fg_muted, "These branches are identical.")
                            } else {
                                (Icon::Info, pal.fg_muted, "Base is ahead of compare.")
                            };
                            icon(ui, ic, color);
                            ui.label(RichText::new(text).size(12.0).color(color));
                        }
                    });
                });
            });
            if let Some(b) = new_base {
                cx.go_replace(Route::repo(&o, &n, RepoView::NewPull { base: Some(b), head: self.head.clone() }));
            }
            if let Some(h) = new_head {
                cx.go_replace(Route::repo(&o, &n, RepoView::NewPull { base: Some(base.clone()), head: Some(h) }));
            }
            ui.add_space(16.0);
            if self.head.is_none() {
                w::blankslate(
                    ui,
                    Icon::GitCompare,
                    "Compare and review just about anything",
                    "Pick a compare branch above to see the changes.",
                );
                return;
            }
            if self.head.as_deref() == Some(base.as_str()) {
                w::flash(ui, FlashKind::Info, "Choose different branches to compare.");
                return;
            }
            if let Some(e) = &self.error {
                w::flash(ui, FlashKind::Danger, e);
                ui.add_space(12.0);
            }
            let Some(cmp) = self.compare.ready().cloned() else {
                if let Load::Failed(e) = self.compare.poll() {
                    w::flash(ui, FlashKind::Danger, &e.clone());
                } else {
                    w::spinner(ui);
                }
                return;
            };
            if cmp.ahead_by == 0 {
                w::flash(
                    ui,
                    FlashKind::Info,
                    &format!(
                        "There isn't anything to compare. {base} is up to date with all commits from {}.",
                        self.head.clone().unwrap_or_default()
                    ),
                );
                return;
            }

            // PR form
            ui.horizontal_top(|ui| {
                w::avatar(ui, &cx.me.avatar_url, 40.0);
                ui.vertical(|ui| {
                    w::form_label(ui, "Add a title", true);
                    w::text_input(ui, &mut self.title, "Title", ui.available_width());
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        w::form_label(ui, "Add a description", false);
                        if let Some(ts) = self.templates.ready().cloned()
                            && ts.len() > 1
                        {
                            w::right(ui, |ui| {
                                let b =
                                    w::button_ex(ui, Variant::Default, Some(Icon::FileBadge), "Template", Some(Icon::TriangleDown), true);
                                egui::Popup::menu(&b).align(egui::RectAlign::BOTTOM_END).show(|ui| {
                                    for (name, body) in &ts {
                                        if w::menu_item(ui, Some(Icon::File), name).clicked() {
                                            self.composer.text = body.clone();
                                        }
                                    }
                                });
                            });
                        }
                    });
                    let mut submit = false;
                    let ok = !self.title.trim().is_empty() && !self.create.is_loading();
                    let draft = &mut self.draft;
                    self.composer.ui(ui, "Add your description here…", 10, |ui, _| {
                        let label = if *draft { "Create draft pull request" } else { "Create pull request" };
                        if w::button_ex(ui, Variant::Primary, None, label, None, ok).clicked() {
                            submit = true;
                        }
                        let more = w::icon_button(ui, Icon::TriangleDown, "Select type of pull request");
                        egui::Popup::menu(&more).align(egui::RectAlign::BOTTOM_END).width(300.0).show(|ui| {
                            if w::menu_item_ex(ui, None, "Create pull request", !*draft, false).clicked() {
                                *draft = false;
                            }
                            if w::menu_item_ex(ui, None, "Create draft pull request", *draft, false).clicked() {
                                *draft = true;
                            }
                        });
                    });
                    if submit {
                        let body = json!({
                            "title": self.title.trim(),
                            "body": self.composer.text,
                            "head": self.head,
                            "base": base,
                            "draft": self.draft,
                        });
                        let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
                        self.error = None;
                        self.create = Load::spawn(&ctx, move || g.create_pull(&o2, &n2, &body));
                    }
                });
            });
            ui.add_space(24.0);
            changes_tabs(ui, cx, repo, &cmp, &mut self.tab, &mut self.collapsed);
        });
    }
}

fn branch_select(ui: &mut Ui, prefix: &str, current: &str, all: &[String], filter: &mut String) -> Option<String> {
    let mut picked = None;
    let b = w::button_ex(ui, Variant::Default, None, &format!("{prefix}: {current}"), Some(Icon::TriangleDown), true);
    egui::Popup::menu(&b).width(280.0).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        w::text_input_ex(ui, filter, "Find a branch", ui.available_width(), Some(Icon::Search));
        egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
            let f = filter.to_lowercase();
            for br in all.iter().filter(|b| f.is_empty() || b.to_lowercase().contains(&f)) {
                if w::menu_item_ex(ui, Some(Icon::GitBranch), br, br == current, false).clicked() {
                    picked = Some(br.clone());
                    ui.close();
                }
            }
        });
    });
    picked
}

/// Commits / Files changed tabs shared by the compare view and commit pages.
pub fn changes_tabs(ui: &mut Ui, cx: &mut Cx, repo: &Repo, cmp: &Compare, tab: &mut usize, collapsed: &mut HashSet<String>) {
    let pal = w::p(ui);
    let (o, n) = on(repo);
    let add: u64 = cmp.files.iter().map(|f| f.additions).sum();
    let del: u64 = cmp.files.iter().map(|f| f.deletions).sum();
    if let Some(t) = w::underline_nav(
        ui,
        &[(Icon::GitCommit, "Commits", Some(cmp.commits.len() as u64)), (Icon::FileDiff, "Files changed", Some(cmp.files.len() as u64))],
        *tab,
    ) {
        *tab = t;
    }
    w::divider(ui);
    ui.add_space(16.0);
    if *tab == 0 {
        w::bordered(ui, |ui| {
            for c in &cmp.commits {
                let resp = w::list_row(ui, |ui| {
                    ui.horizontal(|ui| {
                        icon(ui, Icon::GitCommit, pal.fg_muted);
                        let (login, avatar) = c
                            .author
                            .as_ref()
                            .map(|a| (a.login.clone(), a.avatar_url.clone()))
                            .unwrap_or((c.commit.author.name.clone(), String::new()));
                        w::avatar(ui, &avatar, 20.0);
                        ui.label(RichText::new(c.title()).font(semibold(14.0)));
                        w::muted_small(ui, login);
                        w::right(ui, |ui| ui.label(RichText::new(c.short_sha()).monospace().color(pal.fg_muted)));
                    });
                });
                if resp.clicked() {
                    cx.go(Route::repo(&o, &n, RepoView::Commit(c.sha.clone())));
                }
                w::divider(ui);
            }
        });
    } else {
        ui.horizontal(|ui| {
            w::muted(ui, format!("Showing {} changed files with {add} additions and {del} deletions.", cmp.files.len()));
        });
        ui.add_space(12.0);
        for f in &cmp.files {
            let mut open = !collapsed.contains(&f.filename);
            common::file_diff_box(ui, f, &mut open, |_| {});
            if open {
                collapsed.remove(&f.filename);
            } else {
                collapsed.insert(f.filename.clone());
            }
        }
    }
}
