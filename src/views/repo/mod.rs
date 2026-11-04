//! Repository pages: the shared header (title, watch/fork/star, tabs) and the
//! sub view selected by the route.

mod actions;
mod code;
mod commits;
mod issue;
mod issues;
mod labels;
mod manage;
mod pull;
mod pulls;
mod settings;

use super::Page;
use crate::app::Cx;
use crate::github::models::Repo;
use crate::icons::{Icon, icon};
use crate::route::{RepoRoute, RepoView, Route};
use crate::task::Load;
use crate::theme::semibold;
use crate::widgets::{self as w, Variant};
use egui::{Frame, Margin, RichText, Ui};

/// Implemented by every screen under a repository.
pub trait RepoSub {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo);
}

pub struct RepoPage {
    route: RepoRoute,
    repo: Load<Repo>,
    starred: Load<bool>,
    watching: Load<String>,
    action: Load<RepoAction>,
    fork_dialog: bool,
    fork_owner: Option<String>,
    fork_name: String,
    fork_orgs: Load<Vec<crate::github::models::Org>>,
    open_prs: Load<u64>,
    sub: Box<dyn RepoSub>,
}

enum RepoAction {
    Star(bool),
    Watch(String),
    Forked(Repo),
}

fn make_sub(r: &RepoRoute) -> Box<dyn RepoSub> {
    match &r.view {
        RepoView::Code { git_ref, path } => Box::new(code::CodeView::new(git_ref.clone(), path.clone())),
        RepoView::Blob { git_ref, path } => Box::new(code::BlobView::new(git_ref.clone(), path.clone())),
        RepoView::Issues { q } => Box::new(issues::IssueList::new(q.clone(), false)),
        RepoView::Pulls { q } => Box::new(issues::IssueList::new(q.clone(), true)),
        RepoView::Issue(n) => Box::new(issue::IssueView::new(*n)),
        RepoView::NewIssue => Box::new(issues::NewIssue::default()),
        RepoView::Pull(n) => Box::new(pull::PullView::new(*n)),
        RepoView::NewPull { base, head } => Box::new(pulls::NewPull::new(base.clone(), head.clone())),
        RepoView::Compare { base, head } => Box::new(pulls::NewPull::new(Some(base.clone()), Some(head.clone()))),
        RepoView::Actions { workflow } => Box::new(actions::ActionsView::new(*workflow)),
        RepoView::Run(id) => Box::new(actions::RunView::new(*id)),
        RepoView::Commits { git_ref } => Box::new(commits::CommitsView::new(git_ref.clone())),
        RepoView::Commit(sha) => Box::new(commits::CommitView::new(sha.clone())),
        RepoView::Branches => Box::new(commits::BranchesView::default()),
        RepoView::Tags => Box::new(commits::TagsView::default()),
        RepoView::Settings => Box::new(settings::RepoSettings::default()),
        RepoView::Readme => Box::new(manage::ReadmeManager::default()),
        RepoView::Templates => Box::new(manage::TemplateManager::default()),
        RepoView::CommunityFiles => Box::new(manage::CommunityFiles::default()),
        RepoView::Labels => Box::new(labels::LabelsView::default()),
    }
}

impl RepoPage {
    pub fn new(route: RepoRoute) -> Self {
        let sub = make_sub(&route);
        Self {
            route,
            repo: Load::Idle,
            starred: Load::Idle,
            watching: Load::Idle,
            action: Load::Idle,
            fork_dialog: false,
            fork_owner: None,
            fork_name: String::new(),
            fork_orgs: Load::Idle,
            open_prs: Load::Idle,
            sub,
        }
    }

    fn go(&self, cx: &mut Cx, view: RepoView) {
        cx.go(Route::repo(&self.route.owner, &self.route.name, view));
    }
}

impl Page for RepoPage {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        let ctx = ui.ctx().clone();
        let (o, n) = (self.route.owner.clone(), self.route.name.clone());
        if self.repo.is_idle() {
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.repo = Load::spawn(&ctx, move || g.repo(&o2, &n2));
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.starred = Load::spawn(&ctx, move || g.is_starred(&o2, &n2));
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.watching = Load::spawn(&ctx, move || g.subscription(&o2, &n2));
            // open_issues_count lumps PRs in, so count PRs separately for the tab badges
            let (g, slug) = (cx.gh.clone(), format!("{o}/{n}"));
            self.open_prs = Load::spawn(&ctx, move || Ok(g.search_issues(&format!("repo:{slug} is:pr is:open"), "", 1)?.total_count));
        }
        match self.action.take() {
            Some(Ok(RepoAction::Star(s))) => {
                if let Load::Ready(r) = &mut self.repo {
                    if s {
                        r.stargazers_count += 1;
                    } else {
                        r.stargazers_count = r.stargazers_count.saturating_sub(1);
                    }
                }
                self.starred = Load::Ready(s);
            }
            Some(Ok(RepoAction::Watch(m))) => {
                cx.ok(match m.as_str() {
                    "watching" => "You're now watching this repository",
                    "ignoring" => "You're ignoring this repository",
                    _ => "Participating and @mentions only",
                });
                self.watching = Load::Ready(m);
            }
            Some(Ok(RepoAction::Forked(r))) => {
                cx.ok(format!("Forked to {}", r.full_name));
                cx.go(Route::repo_home(&r.owner.login, &r.name));
            }
            Some(Err(e)) => cx.err(e),
            None => {}
        }

        let pal = w::p(ui);
        let repo = match self.repo.poll() {
            Load::Ready(r) => r.clone(),
            Load::Failed(e) => {
                let e = e.clone();
                Frame::new().inner_margin(Margin::same(32)).show(ui, |ui| {
                    w::page(ui, 900.0, |ui| {
                        if e.contains("404") {
                            w::blankslate(
                                ui,
                                Icon::Repo,
                                "Repository not found",
                                "It may be private, renamed, or you may not have access with this account.",
                            );
                        } else {
                            w::flash(ui, w::FlashKind::Danger, &e);
                        }
                    });
                });
                return;
            }
            _ => {
                w::spinner(ui);
                return;
            }
        };

        egui::Panel::top("repo-header")
            .frame(Frame::new().fill(pal.header_bg).inner_margin(Margin { left: 16, right: 16, top: 0, bottom: 0 }))
            .show(ui, |ui| self.tabs(ui, cx, &repo));
        // like github, the name + Watch / Fork / Star row only sits on the repository home
        if matches!(&self.route.view, RepoView::Code { path, .. } if path.is_empty()) {
            egui::Panel::top("repo-title")
                .frame(Frame::new().fill(pal.canvas).inner_margin(Margin { left: 24, right: 24, top: 20, bottom: 16 }))
                .show_separator_line(true)
                .show(ui, |ui| w::page(ui, 1280.0, |ui| self.title_row(ui, cx, &repo)));
        }

        if self.fork_dialog {
            self.fork_modal(&ctx, cx, &repo);
        }
        self.sub.ui(ui, cx, &repo);
    }
}

impl RepoPage {
    fn title_row(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let pal = w::p(ui);
        let ctx = ui.ctx().clone();
        let (o, n) = (self.route.owner.clone(), self.route.name.clone());
        ui.horizontal(|ui| {
            if repo.owner.kind == "Organization" {
                w::org_avatar(ui, &repo.owner.avatar_url, 24.0);
            } else {
                w::avatar(ui, &repo.owner.avatar_url, 24.0);
            }
            ui.label(RichText::new(&repo.name).font(semibold(20.0)));
            let vis = repo.visibility.clone().unwrap_or_else(|| if repo.private { "private".into() } else { "public".into() });
            let mut vis_label = vis[..1].to_uppercase() + &vis[1..];
            if repo.is_template {
                vis_label.push_str(" template");
            }
            w::outline_label(ui, &vis_label, None);
            if repo.archived {
                w::outline_label(ui, "Public archive", Some(pal.attention));
            }

            w::right(ui, |ui| {
                let busy = self.action.is_loading();
                // star
                let starred = matches!(self.starred.poll(), Load::Ready(true));
                let star_icon = if starred { Icon::StarFill } else { Icon::Star };
                if w::button_counter(ui, star_icon, if starred { "Starred" } else { "Star" }, repo.stargazers_count, false, !busy).clicked()
                {
                    let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
                    self.action = Load::spawn(&ctx, move || {
                        g.set_starred(&o2, &n2, !starred)?;
                        Ok(RepoAction::Star(!starred))
                    });
                }
                // fork
                if w::button_counter(ui, Icon::RepoForked, "Fork", repo.forks_count, false, !busy).clicked() {
                    self.fork_dialog = true;
                    self.fork_name = repo.name.clone();
                }
                // watch
                let mode = match self.watching.poll() {
                    Load::Ready(m) => m.clone(),
                    _ => "participating".into(),
                };
                let watch_label = match mode.as_str() {
                    "watching" => "Unwatch",
                    "ignoring" => "Stop ignoring",
                    _ => "Watch",
                };
                let wb = w::button_counter(ui, Icon::Eye, watch_label, repo.subscribers_count, true, !busy);
                egui::Popup::menu(&wb).width(300.0).align(egui::RectAlign::BOTTOM_END).show(|ui| {
                    for (m, title, desc) in [
                        (
                            "participating",
                            "Participating and @mentions",
                            "Only receive notifications from this repository when participating or @mentioned.",
                        ),
                        ("watching", "All Activity", "Notified of all notifications on this repository."),
                        ("ignoring", "Ignore", "Never be notified."),
                    ] {
                        let r = ui
                            .horizontal_top(|ui| {
                                let (rr, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                                if mode == m {
                                    Icon::Check.image(16.0, pal.fg).paint_at(ui, rr);
                                }
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(title).font(semibold(14.0)));
                                    ui.add(egui::Label::new(RichText::new(desc).size(12.0).color(pal.fg_muted)).wrap());
                                });
                            })
                            .response;
                        if ui.interact(r.rect, ui.scope_id().with(("watch", m)), egui::Sense::click()).clicked() {
                            let (g, o2, n2, m2) = (cx.gh.clone(), o.clone(), n.clone(), m.to_string());
                            self.action = Load::spawn(&ctx, move || {
                                g.set_subscription(&o2, &n2, &m2)?;
                                Ok(RepoAction::Watch(m2))
                            });
                            ui.close();
                        }
                        ui.add_space(6.0);
                    }
                });
                if w::icon_button(ui, Icon::DesktopDownload, "Clone this repository locally").clicked() {
                    cx.go(Route::Clone { url: repo.clone_url.clone() });
                }
            });
        });
    }

    /// The repository UnderlineNav that sits in the header area, with open counts.
    fn tabs(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let prs = match self.open_prs.poll() {
            Load::Ready(n) => Some(*n),
            _ => None,
        };
        // the issue count is only known once we know how many of the "issues" are PRs
        let issues = prs.map(|p| repo.open_issues_count.saturating_sub(p));
        let mut tabs: Vec<(Icon, &str, Option<u64>)> = vec![
            (Icon::Code, "Code", None),
            (Icon::IssueOpened, "Issues", issues),
            (Icon::GitPullRequest, "Pull requests", prs),
            (Icon::Play, "Actions", None),
            (Icon::FileBadge, "Files", None),
        ];
        if !repo.has_issues {
            tabs[1].1 = "Issues (off)";
        }
        if repo.is_admin() {
            tabs.push((Icon::Gear, "Settings", None));
        }
        if let Some(i) = w::underline_nav(ui, &tabs, self.route.view.tab()) {
            let view = match i {
                0 => RepoView::Code { git_ref: None, path: String::new() },
                1 => RepoView::Issues { q: None },
                2 => RepoView::Pulls { q: None },
                3 => RepoView::Actions { workflow: None },
                4 => RepoView::Readme,
                _ => RepoView::Settings,
            };
            self.go(cx, view);
        }
    }

    fn fork_modal(&mut self, ctx: &egui::Context, cx: &mut Cx, repo: &Repo) {
        let pal = crate::theme::pal(ctx);
        let modal = egui::Modal::new(egui::Id::unique("fork-modal"))
            .backdrop_color(pal.overlay_shadow)
            .frame(
                Frame::new()
                    .fill(pal.canvas)
                    .stroke(egui::Stroke::new(1.0, pal.border))
                    .corner_radius(egui::CornerRadius::same(12))
                    .inner_margin(Margin::same(16)),
            )
            .show(ctx, |ui| {
                ui.set_width(460.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Create a new fork").font(semibold(16.0)));
                    w::right(ui, |ui| {
                        if w::icon_button_invisible(ui, Icon::X, "Close").clicked() {
                            self.fork_dialog = false;
                        }
                    });
                });
                ui.add_space(4.0);
                w::muted_small(ui, format!("A fork is a copy of {}. Only the default branch is copied.", repo.full_name));
                ui.add_space(12.0);
                w::form_label(ui, "Owner", true);
                if self.fork_orgs.is_idle() {
                    let g = cx.gh.clone();
                    self.fork_orgs = Load::spawn(ctx, move || g.my_orgs());
                }
                let owner_label = self.fork_owner.clone().unwrap_or_else(|| cx.me.login.clone());
                let b = w::button_ex(ui, Variant::Default, None, &owner_label, Some(Icon::TriangleDown), true);
                egui::Popup::menu(&b).width(240.0).show(|ui| {
                    if w::menu_item_ex(ui, Some(Icon::Person), &cx.me.login, self.fork_owner.is_none(), false).clicked() {
                        self.fork_owner = None;
                    }
                    if let Some(orgs) = self.fork_orgs.ready() {
                        for o in orgs.iter() {
                            if w::menu_item_ex(ui, Some(Icon::Organization), &o.login, self.fork_owner.as_deref() == Some(&o.login), false)
                                .clicked()
                            {
                                self.fork_owner = Some(o.login.clone());
                            }
                        }
                    }
                });
                ui.add_space(8.0);
                w::form_label(ui, "Repository name", true);
                w::text_input(ui, &mut self.fork_name, "", ui.available_width());
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    w::right(ui, |ui| {
                        let busy = self.action.is_loading();
                        if w::button_ex(
                            ui,
                            Variant::Primary,
                            None,
                            if busy { "Forking…" } else { "Create fork" },
                            None,
                            !busy && !self.fork_name.trim().is_empty(),
                        )
                        .clicked()
                        {
                            let (g, o, n) = (cx.gh.clone(), repo.owner.login.clone(), repo.name.clone());
                            let (org, name) = (self.fork_owner.clone(), self.fork_name.trim().to_string());
                            let rename = if name != n { Some(name) } else { None };
                            self.action =
                                Load::spawn(ctx, move || Ok(RepoAction::Forked(g.fork(&o, &n, org.as_deref(), rename.as_deref())?)));
                            self.fork_dialog = false;
                        }
                        if w::button(ui, Variant::Default, None, "Cancel").clicked() {
                            self.fork_dialog = false;
                        }
                    });
                });
            });
        if modal.should_close() {
            self.fork_dialog = false;
        }
    }
}

/// Branch / tag picker button used by code, commits and PR screens.
pub fn ref_picker(
    ui: &mut Ui,
    cx: &Cx,
    repo: &Repo,
    current: &str,
    branches: &mut Load<Vec<String>>,
    filter: &mut String,
) -> Option<String> {
    let pal = w::p(ui);
    let ctx = ui.ctx().clone();
    let mut picked = None;
    let b = w::button_ex(ui, Variant::Default, Some(Icon::GitBranch), current, Some(Icon::TriangleDown), true);
    if b.clicked() && branches.is_idle() {
        let (g, o, n) = (cx.gh.clone(), repo.owner.login.clone(), repo.name.clone());
        *branches = Load::spawn(&ctx, move || {
            let mut out: Vec<String> = g.branches(&o, &n)?.into_iter().map(|b| b.name).collect();
            out.extend(g.tags(&o, &n)?.into_iter().map(|t| format!("tags/{}", t.name)));
            Ok(out)
        });
    }
    egui::Popup::menu(&b).width(300.0).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        ui.label(RichText::new("Switch branches/tags").font(semibold(14.0)));
        w::text_input_ex(ui, filter, "Find a branch or tag…", ui.available_width(), Some(Icon::Search));
        ui.add_space(4.0);
        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
            w::show_load(ui, branches, |ui, list| {
                let f = filter.to_lowercase();
                for b in list.iter().filter(|b| f.is_empty() || b.to_lowercase().contains(&f)) {
                    let (ic, name) = match b.strip_prefix("tags/") {
                        Some(t) => (Icon::Tag, t),
                        None => (Icon::GitBranch, b.as_str()),
                    };
                    let r = w::menu_item_ex(ui, Some(ic), name, name == current, false);
                    if b == &repo.default_branch {
                        let rr = r.rect;
                        ui.painter().text(
                            rr.right_center() - egui::vec2(8.0, 0.0),
                            egui::Align2::RIGHT_CENTER,
                            "default",
                            egui::FontId::proportional(11.0),
                            pal.fg_muted,
                        );
                    }
                    if r.clicked() {
                        picked = Some(name.to_string());
                        ui.close();
                    }
                }
            });
        });
    });
    picked
}

/// Small "owner/name" helper since almost every API call needs both.
pub fn on(repo: &Repo) -> (String, String) {
    (repo.owner.login.clone(), repo.name.clone())
}

pub fn section<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::ScrollArea::vertical()
        .show(ui, |ui| Frame::new().inner_margin(Margin::same(24)).show(ui, |ui| w::page(ui, 1280.0, add)).inner)
        .inner
}

pub fn back_link(ui: &mut Ui, label: &str) -> bool {
    let pal = w::p(ui);
    ui.horizontal(|ui| {
        icon(ui, Icon::ArrowLeft, pal.accent);
        w::link(ui, label).clicked()
    })
    .inner
}
