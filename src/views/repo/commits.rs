//! Commit history, single commit diff, branches and tags.

use super::{RepoSub, on, ref_picker};
use crate::app::Cx;
use crate::github::models::{Branch, Commit, Release, Repo, Tag};
use crate::icons::{Icon, icon};
use crate::route::{RepoView, Route};
use crate::task::Load;
use crate::theme::semibold;
use crate::views::common;
use crate::widgets::{self as w, Variant};
use egui::{Frame, Margin, RichText, Ui};
use std::collections::HashSet;

pub struct CommitsView {
    git_ref: Option<String>,
    page: u32,
    commits: Load<Vec<Commit>>,
    branches: Load<Vec<String>>,
    bf: String,
}

impl CommitsView {
    pub fn new(git_ref: Option<String>) -> Self {
        Self { git_ref, page: 1, commits: Load::Idle, branches: Load::Idle, bf: String::new() }
    }

    fn load(&mut self, ctx: &egui::Context, cx: &Cx, repo: &Repo) {
        let r = self.git_ref.clone().unwrap_or_else(|| repo.default_branch.clone());
        let (g, (o, n), p) = (cx.gh.clone(), on(repo), self.page);
        self.commits = Load::spawn(ctx, move || g.commits(&o, &n, &r, None, p));
    }
}

impl RepoSub for CommitsView {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let ctx = ui.ctx().clone();
        if self.commits.is_idle() {
            self.load(&ctx, cx, repo);
        }
        let (o, n) = on(repo);
        let git_ref = self.git_ref.clone().unwrap_or_else(|| repo.default_branch.clone());
        let pal = w::p(ui);
        let mut reload = false;
        super::section(ui, |ui| {
            ui.horizontal(|ui| {
                w::heading(ui, "Commits", 24.0);
            });
            ui.add_space(12.0);
            if let Some(r) = ref_picker(ui, cx, repo, &git_ref, &mut self.branches, &mut self.bf) {
                cx.go_replace(Route::repo(&o, &n, RepoView::Commits { git_ref: Some(r) }));
            }
            ui.add_space(16.0);
            let mut has_next = false;
            w::show_load(ui, &mut self.commits, |ui, commits| {
                has_next = commits.len() >= 35;
                let mut last_day = String::new();
                // group by day like github's "Commits on Nov 4, 2026"
                let mut groups: Vec<(String, Vec<&Commit>)> = vec![];
                for c in commits.iter() {
                    let day = c.commit.author.date.map(|d| d.format("%b %-d, %Y").to_string()).unwrap_or_default();
                    if day != last_day {
                        groups.push((day.clone(), vec![]));
                        last_day = day;
                    }
                    if let Some(g) = groups.last_mut() {
                        g.1.push(c);
                    }
                }
                for (day, list) in groups {
                    ui.horizontal(|ui| {
                        icon(ui, Icon::GitCommit, pal.fg_muted);
                        w::muted(ui, format!("Commits on {day}"));
                    });
                    ui.add_space(8.0);
                    w::bordered(ui, |ui| {
                        for c in list {
                            let resp = w::list_row(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.add(egui::Label::new(RichText::new(c.title()).font(semibold(14.0))).truncate());
                                        ui.horizontal(|ui| {
                                            let (login, avatar) = c
                                                .author
                                                .as_ref()
                                                .map(|a| (a.login.clone(), a.avatar_url.clone()))
                                                .unwrap_or((c.commit.author.name.clone(), String::new()));
                                            w::avatar(ui, &avatar, 16.0);
                                            w::muted_small(
                                                ui,
                                                format!(
                                                    "{login} committed {}",
                                                    c.commit.author.date.as_ref().map(w::rel_time).unwrap_or_default()
                                                ),
                                            );
                                        });
                                    });
                                    w::right(ui, |ui| {
                                        if w::icon_button_invisible(ui, Icon::Code, "Browse the repository at this point").clicked() {
                                            cx.go(Route::repo(
                                                &o,
                                                &n,
                                                RepoView::Code { git_ref: Some(c.sha.clone()), path: String::new() },
                                            ));
                                        }
                                        if w::icon_button_invisible(ui, Icon::Copy, "Copy full SHA").clicked() {
                                            ui.ctx().copy_text(c.sha.clone());
                                        }
                                        ui.label(RichText::new(c.short_sha()).monospace().color(pal.fg_muted));
                                    });
                                });
                            });
                            if resp.clicked() {
                                cx.go(Route::repo(&o, &n, RepoView::Commit(c.sha.clone())));
                            }
                            w::divider(ui);
                        }
                    });
                    ui.add_space(16.0);
                }
            });
            if common::pager(ui, &mut self.page, has_next) {
                reload = true;
            }
        });
        if reload {
            self.load(&ctx, cx, repo);
        }
    }
}

pub struct CommitView {
    sha: String,
    commit: Load<Commit>,
    collapsed: HashSet<String>,
}

impl CommitView {
    pub fn new(sha: String) -> Self {
        Self { sha, commit: Load::Idle, collapsed: HashSet::new() }
    }
}

impl RepoSub for CommitView {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let ctx = ui.ctx().clone();
        let (o, n) = on(repo);
        if self.commit.is_idle() {
            let (g, o2, n2, s) = (cx.gh.clone(), o.clone(), n.clone(), self.sha.clone());
            self.commit = Load::spawn(&ctx, move || g.commit(&o2, &n2, &s));
        }
        let pal = w::p(ui);
        super::section(ui, |ui| {
            let Some(c) = self.commit.ready().cloned() else {
                if let Load::Failed(e) = self.commit.poll() {
                    w::flash(ui, w::FlashKind::Danger, &e.clone());
                } else {
                    w::spinner(ui);
                }
                return;
            };
            w::bordered(ui, |ui| {
                Frame::new().fill(pal.canvas_subtle).inner_margin(Margin::same(16)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(c.title()).font(semibold(20.0)));
                        w::right(ui, |ui| {
                            if w::button(ui, Variant::Default, None, "Browse files").clicked() {
                                cx.go(Route::repo(&o, &n, RepoView::Code { git_ref: Some(c.sha.clone()), path: String::new() }));
                            }
                        });
                    });
                    let rest: String = c.commit.message.lines().skip(1).collect::<Vec<_>>().join("\n");
                    if !rest.trim().is_empty() {
                        ui.add_space(8.0);
                        ui.label(RichText::new(rest.trim()).monospace().size(12.0));
                    }
                });
                w::divider(ui);
                Frame::new().inner_margin(Margin::symmetric(16, 10)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let (login, avatar) = c
                            .author
                            .as_ref()
                            .map(|a| (a.login.clone(), a.avatar_url.clone()))
                            .unwrap_or((c.commit.author.name.clone(), String::new()));
                        w::avatar(ui, &avatar, 20.0);
                        ui.label(RichText::new(&login).font(semibold(14.0)));
                        w::muted(ui, format!("committed {}", c.commit.author.date.as_ref().map(w::rel_time).unwrap_or_default()));
                        w::right(ui, |ui| {
                            ui.label(RichText::new(&c.sha).monospace().size(12.0));
                            w::muted_small(ui, "commit");
                            for p in c.parents.iter().rev() {
                                if w::link(ui, &p.sha[..7.min(p.sha.len())]).clicked() {
                                    cx.go(Route::repo(&o, &n, RepoView::Commit(p.sha.clone())));
                                }
                            }
                            w::muted_small(ui, if c.parents.len() > 1 { "parents" } else { "parent" });
                        });
                    });
                });
            });
            ui.add_space(16.0);
            if let Some(s) = &c.stats {
                w::muted(
                    ui,
                    format!("Showing {} changed files with {} additions and {} deletions.", c.files.len(), s.additions, s.deletions),
                );
                ui.add_space(12.0);
            }
            for f in &c.files {
                let mut open = !self.collapsed.contains(&f.filename);
                common::file_diff_box(ui, f, &mut open, |ui| {
                    if f.status != "removed" && w::icon_button_invisible(ui, Icon::File, "View file").clicked() {
                        cx.go(Route::repo(&o, &n, RepoView::Blob { git_ref: Some(c.sha.clone()), path: f.filename.clone() }));
                    }
                });
                if open {
                    self.collapsed.remove(&f.filename);
                } else {
                    self.collapsed.insert(f.filename.clone());
                }
            }
        });
    }
}

#[derive(Default)]
pub struct BranchesView {
    branches: Load<Vec<Branch>>,
    filter: String,
    new_name: String,
    action: Load<String>,
    confirm: Option<String>,
}

impl RepoSub for BranchesView {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let ctx = ui.ctx().clone();
        let (o, n) = on(repo);
        if self.branches.is_idle() {
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.branches = Load::spawn(&ctx, move || g.branches(&o2, &n2));
        }
        match self.action.take() {
            Some(Ok(m)) => {
                cx.ok(m);
                self.branches = Load::Idle;
            }
            Some(Err(e)) => cx.err(e),
            None => {}
        }
        let pal = w::p(ui);
        super::section(ui, |ui| {
            ui.horizontal(|ui| {
                w::heading(ui, "Branches", 24.0);
                w::right(ui, |ui| {
                    if repo.can_push() {
                        let b = w::button(ui, Variant::Primary, None, "New branch");
                        egui::Popup::menu(&b)
                            .width(320.0)
                            .align(egui::RectAlign::BOTTOM_END)
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                            .show(|ui| {
                                ui.label(RichText::new("Create a branch").font(semibold(14.0)));
                                w::text_input(ui, &mut self.new_name, "Branch name", ui.available_width());
                                w::muted_small(ui, format!("Source: {}", repo.default_branch));
                                let ok = !self.new_name.trim().is_empty() && !self.action.is_loading();
                                if w::button_ex(ui, Variant::Primary, None, "Create new branch", None, ok).clicked() {
                                    let source = self
                                        .branches
                                        .ready()
                                        .and_then(|l| l.iter().find(|b| b.name == repo.default_branch).map(|b| b.commit.sha.clone()))
                                        .unwrap_or_default();
                                    let (g, o2, n2, name) = (cx.gh.clone(), o.clone(), n.clone(), self.new_name.trim().to_string());
                                    self.new_name.clear();
                                    self.action = Load::spawn(&ctx, move || {
                                        g.create_branch(&o2, &n2, &name, &source)?;
                                        Ok(format!("Branch {name} created"))
                                    });
                                    ui.close();
                                }
                            });
                    }
                });
            });
            ui.add_space(12.0);
            w::text_input_ex(ui, &mut self.filter, "Search branches…", ui.available_width(), Some(Icon::Search));
            ui.add_space(16.0);
            let f = self.filter.to_lowercase();
            w::bordered(ui, |ui| {
                w::box_header(ui, |ui| {
                    ui.label(RichText::new("All branches").font(semibold(14.0)));
                });
                w::show_load(ui, &mut self.branches, |ui, list| {
                    for b in list.iter().filter(|b| f.is_empty() || b.name.to_lowercase().contains(&f)) {
                        w::list_row(ui, |ui| {
                            ui.horizontal(|ui| {
                                icon(ui, Icon::GitBranch, pal.fg_muted);
                                if w::link_strong(ui, &b.name, 14.0).clicked() {
                                    cx.go(Route::repo(&o, &n, RepoView::Code { git_ref: Some(b.name.clone()), path: String::new() }));
                                }
                                if b.name == repo.default_branch {
                                    w::outline_label(ui, "Default", None);
                                }
                                if b.protected {
                                    icon(ui, Icon::Shield, pal.fg_muted).on_hover_text("Protected branch");
                                }
                                w::right(ui, |ui| {
                                    if repo.can_push()
                                        && b.name != repo.default_branch
                                        && !b.protected
                                        && w::icon_button_invisible(ui, Icon::Trash, "Delete branch").clicked()
                                    {
                                        self.confirm = Some(b.name.clone());
                                    }
                                    if b.name != repo.default_branch
                                        && w::button(ui, Variant::Default, Some(Icon::GitPullRequest), "New pull request").clicked()
                                    {
                                        cx.go(Route::repo(&o, &n, RepoView::NewPull { base: None, head: Some(b.name.clone()) }));
                                    }
                                    ui.label(
                                        RichText::new(&b.commit.sha[..7.min(b.commit.sha.len())])
                                            .monospace()
                                            .size(12.0)
                                            .color(pal.fg_muted),
                                    );
                                });
                            });
                        });
                        w::divider(ui);
                    }
                });
            });
        });
        if let Some(name) = self.confirm.clone() {
            let modal = egui::Modal::new(egui::Id::unique("del-branch"))
                .frame(
                    Frame::new()
                        .fill(pal.canvas)
                        .stroke(egui::Stroke::new(1.0, pal.border))
                        .corner_radius(egui::CornerRadius::same(12))
                        .inner_margin(Margin::same(16)),
                )
                .show(&ctx, |ui| {
                    ui.set_width(380.0);
                    ui.label(RichText::new(format!("Delete branch {name}?")).font(semibold(16.0)));
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        w::right(ui, |ui| {
                            if w::button(ui, Variant::Danger, None, "Delete").clicked() {
                                let (g, o2, n2, b) = (cx.gh.clone(), o.clone(), n.clone(), name.clone());
                                self.action = Load::spawn(&ctx, move || {
                                    g.delete_branch(&o2, &n2, &b)?;
                                    Ok(format!("Deleted {b}"))
                                });
                                self.confirm = None;
                            }
                            if w::button(ui, Variant::Default, None, "Cancel").clicked() {
                                self.confirm = None;
                            }
                        });
                    });
                });
            if modal.should_close() {
                self.confirm = None;
            }
        }
    }
}

#[derive(Default)]
pub struct TagsView {
    tags: Load<Vec<Tag>>,
    releases: Load<Vec<Release>>,
    md: egui_commonmark::CommonMarkCache,
}

impl RepoSub for TagsView {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let ctx = ui.ctx().clone();
        let (o, n) = on(repo);
        if self.tags.is_idle() {
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.tags = Load::spawn(&ctx, move || g.tags(&o2, &n2));
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.releases = Load::spawn(&ctx, move || g.releases(&o2, &n2));
        }
        let pal = w::p(ui);
        super::section(ui, |ui| {
            w::heading(ui, "Releases", 24.0);
            ui.add_space(12.0);
            if let Some(rels) = self.releases.ready().cloned() {
                if rels.is_empty() {
                    w::muted(ui, "There aren't any releases here.");
                }
                for r in rels.iter().take(10) {
                    w::bordered(ui, |ui| {
                        Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(r.name.clone().unwrap_or_else(|| r.tag_name.clone())).font(semibold(20.0)));
                                if r.prerelease {
                                    w::outline_label(ui, "Pre-release", Some(pal.attention));
                                }
                                if r.draft {
                                    w::outline_label(ui, "Draft", None);
                                }
                                w::right(ui, |ui| {
                                    if w::button(ui, Variant::Invisible, Some(Icon::LinkExternal), "View").clicked() {
                                        ui.ctx().open_url(egui::OpenUrl::new_tab(&r.html_url));
                                    }
                                });
                            });
                            ui.horizontal(|ui| {
                                w::meta(ui, Icon::Tag, r.tag_name.as_str());
                                if let Some(d) = &r.published_at {
                                    w::muted_small(ui, format!("{} released this {}", r.author.login, w::rel_time(d)));
                                }
                            });
                            if let Some(b) = r.body.as_ref().filter(|b| !b.is_empty()) {
                                ui.add_space(8.0);
                                common::markdown(ui, &mut self.md, b, None);
                            }
                        });
                    });
                    ui.add_space(16.0);
                }
            }
            ui.add_space(8.0);
            w::heading(ui, "Tags", 20.0);
            ui.add_space(8.0);
            w::bordered(ui, |ui| {
                w::show_load(ui, &mut self.tags, |ui, tags| {
                    if tags.is_empty() {
                        w::blankslate(ui, Icon::Tag, "No tags", "");
                    }
                    for t in tags.iter() {
                        w::list_row(ui, |ui| {
                            ui.horizontal(|ui| {
                                icon(ui, Icon::Tag, pal.fg_muted);
                                if w::link_strong(ui, &t.name, 14.0).clicked() {
                                    cx.go(Route::repo(&o, &n, RepoView::Code { git_ref: Some(t.name.clone()), path: String::new() }));
                                }
                                w::right(ui, |ui| {
                                    if w::link(ui, &t.commit.sha[..7.min(t.commit.sha.len())]).clicked() {
                                        cx.go(Route::repo(&o, &n, RepoView::Commit(t.commit.sha.clone())));
                                    }
                                    if w::icon_button_invisible(ui, Icon::FileZip, "Download zip").clicked() {
                                        ui.ctx().open_url(egui::OpenUrl::new_tab(&t.zipball_url));
                                    }
                                });
                            });
                        });
                        w::divider(ui);
                    }
                });
            });
        });
    }
}
