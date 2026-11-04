//! Global search with GitHub's left "Filter by" column and result types:
//! code, repositories, issues, pull requests, users, commits.

use super::Page;
use super::common;
use crate::app::Cx;
use crate::github::models::{CodeHit, CommitSearchHit, Issue, Repo, SearchResult, User};
use crate::icons::{Icon, icon};
use crate::route::{RepoView, Route, SearchKind};
use crate::task::Load;
use crate::theme::semibold;
use crate::widgets::{self as w, Variant};
use egui::{CornerRadius, Frame, Margin, RichText, Stroke, Ui};

enum Results {
    Repos(SearchResult<Repo>),
    Issues(SearchResult<Issue>),
    Users(SearchResult<User>),
    Code(SearchResult<CodeHit>),
    Commits(SearchResult<CommitSearchHit>),
}

impl Results {
    fn total(&self) -> u64 {
        match self {
            Results::Repos(r) => r.total_count,
            Results::Issues(r) => r.total_count,
            Results::Users(r) => r.total_count,
            Results::Code(r) => r.total_count,
            Results::Commits(r) => r.total_count,
        }
    }
    fn len(&self) -> usize {
        match self {
            Results::Repos(r) => r.items.len(),
            Results::Issues(r) => r.items.len(),
            Results::Users(r) => r.items.len(),
            Results::Code(r) => r.items.len(),
            Results::Commits(r) => r.items.len(),
        }
    }
}

pub struct SearchPage {
    q: String,
    input: String,
    kind: SearchKind,
    sort: &'static str,
    page: u32,
    results: Load<Results>,
}

impl SearchPage {
    pub fn new(q: String, kind: SearchKind) -> Self {
        Self { input: q.clone(), q, kind, sort: "", page: 1, results: Load::Idle }
    }

    fn run(&mut self, ctx: &egui::Context, cx: &Cx) {
        if self.q.trim().is_empty() {
            self.results = Load::Idle;
            return;
        }
        let g = cx.gh.clone();
        let (q, page, sort) = (self.q.clone(), self.page, self.sort);
        self.results = match self.kind {
            SearchKind::Repositories => Load::spawn(ctx, move || Ok(Results::Repos(g.search_repos(&q, sort, page)?))),
            SearchKind::Issues => {
                let q = if q.contains("is:issue") || q.contains("is:pr") { q } else { format!("{q} is:issue") };
                Load::spawn(ctx, move || Ok(Results::Issues(g.search_issues(&q, sort, page)?)))
            }
            SearchKind::PullRequests => {
                let q = if q.contains("is:pr") { q } else { format!("{q} is:pr") };
                Load::spawn(ctx, move || Ok(Results::Issues(g.search_issues(&q, sort, page)?)))
            }
            SearchKind::Users => Load::spawn(ctx, move || Ok(Results::Users(g.search_users(&q, page)?))),
            SearchKind::Code => Load::spawn(ctx, move || Ok(Results::Code(g.search_code(&q, page)?))),
            SearchKind::Commits => Load::spawn(ctx, move || Ok(Results::Commits(g.search_commits(&q, page)?))),
        };
    }
}

impl Page for SearchPage {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        if self.results.is_idle() && !self.q.is_empty() {
            self.run(ui.ctx(), cx);
        }
        let pal = w::p(ui);
        let mut rerun = false;

        egui::Panel::left("search-left").resizable(false).exact_size(260.0).frame(Frame::new().inner_margin(Margin::same(16))).show(
            ui,
            |ui| {
                ui.label(RichText::new("Filter by").font(semibold(14.0)));
                ui.add_space(8.0);
                for k in SearchKind::ALL {
                    let ic = match k {
                        SearchKind::Code => Icon::Code,
                        SearchKind::Repositories => Icon::Repo,
                        SearchKind::Issues => Icon::IssueOpened,
                        SearchKind::PullRequests => Icon::GitPullRequest,
                        SearchKind::Users => Icon::People,
                        SearchKind::Commits => Icon::GitCommit,
                    };
                    let r = w::menu_item(ui, Some(ic), k.label());
                    if k == self.kind {
                        let bar = egui::Rect::from_min_size(r.rect.left_top() + egui::vec2(-8.0, 6.0), egui::vec2(4.0, 20.0));
                        ui.painter().rect_filled(bar, CornerRadius::same(2), pal.accent_emphasis);
                    }
                    if r.clicked() && k != self.kind {
                        self.kind = k;
                        self.page = 1;
                        self.sort = "";
                        rerun = true;
                    }
                }
                ui.add_space(16.0);
                ui.label(RichText::new("Advanced").font(semibold(14.0)));
                ui.add_space(4.0);
                let tips: &[(&str, &str)] = match self.kind {
                    SearchKind::Repositories => &[
                        ("language:rust", "Language"),
                        ("stars:>100", "Stars"),
                        ("user:", "Owner"),
                        ("topic:", "Topic"),
                        ("archived:false", "Not archived"),
                    ],
                    SearchKind::Issues | SearchKind::PullRequests => &[
                        ("is:open", "Open"),
                        ("author:@me", "Authored by you"),
                        ("assignee:@me", "Assigned to you"),
                        ("label:bug", "Label"),
                        ("repo:", "Repository"),
                    ],
                    SearchKind::Code => {
                        &[("language:", "Language"), ("repo:", "Repository"), ("path:", "Path"), ("extension:", "Extension")]
                    }
                    SearchKind::Users => &[
                        ("type:user", "Users only"),
                        ("type:org", "Organizations only"),
                        ("followers:>100", "Followers"),
                        ("location:", "Location"),
                    ],
                    SearchKind::Commits => &[("author:", "Author"), ("repo:", "Repository"), ("committer-date:>2024-01-01", "Date")],
                };
                for (q, label) in tips {
                    ui.horizontal(|ui| {
                        if w::link(ui, *q).clicked() {
                            if !self.input.is_empty() && !self.input.ends_with(' ') {
                                self.input.push(' ');
                            }
                            self.input.push_str(q);
                        }
                        w::muted_small(ui, *label);
                    });
                }
            },
        );

        egui::CentralPanel::no_frame().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                Frame::new().inner_margin(Margin::same(24)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let r = w::text_input_ex(
                            ui,
                            &mut self.input,
                            "Search GitHub",
                            (ui.available_width() - 100.0).max(200.0),
                            Some(Icon::Search),
                        );
                        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if w::button(ui, Variant::Default, None, "Search").clicked() || enter {
                            self.q = self.input.trim().to_string();
                            self.page = 1;
                            rerun = true;
                        }
                    });
                    ui.add_space(16.0);
                    if self.q.is_empty() {
                        w::blankslate(
                            ui,
                            Icon::Search,
                            "Search GitHub",
                            "Find repositories, code, issues, pull requests, users and commits.",
                        );
                        return;
                    }
                    let mut sort_changed = false;
                    let mut has_next = false;
                    w::show_load(ui, &mut self.results, |ui, res| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{} results", w::format_count(res.total()))).font(semibold(16.0)));
                            w::right(ui, |ui| {
                                let opts: &[(&str, &str)] = match self.kind {
                                    SearchKind::Repositories => &[
                                        ("", "Best match"),
                                        ("stars", "Most stars"),
                                        ("forks", "Most forks"),
                                        ("updated", "Recently updated"),
                                    ],
                                    SearchKind::Issues | SearchKind::PullRequests => &[
                                        ("", "Best match"),
                                        ("created", "Newest"),
                                        ("comments", "Most commented"),
                                        ("updated", "Recently updated"),
                                    ],
                                    _ => &[],
                                };
                                if !opts.is_empty() {
                                    let cur = opts.iter().find(|o| o.0 == self.sort).map(|o| o.1).unwrap_or("Best match");
                                    let b = w::button_ex(
                                        ui,
                                        Variant::Default,
                                        None,
                                        &format!("Sort by: {cur}"),
                                        Some(Icon::TriangleDown),
                                        true,
                                    );
                                    egui::Popup::menu(&b).align(egui::RectAlign::BOTTOM_END).show(|ui| {
                                        for (k, label) in opts {
                                            if w::menu_item_ex(ui, None, label, *k == self.sort, false).clicked() {
                                                self.sort = k;
                                                sort_changed = true;
                                            }
                                        }
                                    });
                                }
                            });
                        });
                        ui.add_space(8.0);
                        has_next = res.len() >= 30 && (self.page as u64 * 30) < res.total().min(1000);
                        render(ui, res, cx);
                    });
                    if sort_changed {
                        self.page = 1;
                        rerun = true;
                    }
                    if common::pager(ui, &mut self.page, has_next) {
                        rerun = true;
                    }
                });
            });
        });
        if rerun {
            self.run(ui.ctx(), cx);
        }
    }
}

fn render(ui: &mut Ui, res: &Results, cx: &mut Cx) {
    let pal = w::p(ui);
    if res.len() == 0 {
        w::bordered(ui, |ui| {
            w::blankslate(ui, Icon::Search, "Your search did not match anything", "Try different keywords or remove filters.")
        });
        return;
    }
    match res {
        Results::Repos(r) => w::bordered(ui, |ui| {
            for repo in &r.items {
                common::repo_row(ui, repo, cx);
            }
        }),
        Results::Issues(r) => w::bordered(ui, |ui| {
            for it in &r.items {
                common::issue_row(ui, it, true, cx);
            }
        }),
        Results::Users(r) => w::bordered(ui, |ui| {
            for u in &r.items {
                let resp = w::list_row(ui, |ui| {
                    ui.horizontal(|ui| {
                        w::avatar(ui, &u.avatar_url, 40.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&u.login).font(semibold(16.0)).color(pal.accent));
                            w::muted_small(ui, if u.kind == "Organization" { "Organization" } else { "User" });
                        });
                    });
                });
                if resp.clicked() {
                    cx.go(Route::User(u.login.clone()));
                }
                w::divider(ui);
            }
        }),
        Results::Code(r) => {
            for hit in &r.items {
                Frame::new().stroke(Stroke::new(1.0, pal.border)).corner_radius(CornerRadius::same(6)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    Frame::new().fill(pal.canvas_subtle).inner_margin(Margin::symmetric(16, 8)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            w::avatar(ui, &hit.repository.owner.avatar_url, 20.0);
                            if w::link(ui, &hit.repository.full_name).clicked() {
                                cx.go(Route::repo_home(&hit.repository.owner.login, &hit.repository.name));
                            }
                            w::muted(ui, "·");
                            if w::link_strong(ui, &hit.path, 14.0).clicked() {
                                cx.go(Route::repo(
                                    &hit.repository.owner.login,
                                    &hit.repository.name,
                                    RepoView::Blob { git_ref: None, path: hit.path.clone() },
                                ));
                            }
                        });
                    });
                    for m in hit.text_matches.iter().filter(|m| m.property == "content").take(2) {
                        w::divider(ui);
                        Frame::new().inner_margin(Margin::symmetric(16, 8)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(RichText::new(m.fragment.trim()).monospace().size(12.0));
                        });
                    }
                });
                ui.add_space(12.0);
            }
        }
        Results::Commits(r) => w::bordered(ui, |ui| {
            for c in &r.items {
                let resp = w::list_row(ui, |ui| {
                    ui.horizontal(|ui| {
                        icon(ui, Icon::GitCommit, pal.fg_muted);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(c.commit.message.lines().next().unwrap_or("")).font(semibold(14.0)));
                            ui.horizontal(|ui| {
                                w::muted_small(ui, &c.repository.full_name);
                                w::muted_small(ui, format!("· {} committed", c.commit.author.name));
                                if let Some(d) = &c.commit.author.date {
                                    w::muted_small(ui, w::rel_time(d));
                                }
                            });
                        });
                        w::right(ui, |ui| {
                            ui.label(RichText::new(&c.sha[..7.min(c.sha.len())]).monospace().color(pal.fg_muted));
                        });
                    });
                });
                if resp.clicked() {
                    cx.go(Route::repo(&c.repository.owner.login, &c.repository.name, RepoView::Commit(c.sha.clone())));
                }
                w::divider(ui);
            }
        }),
    }
}
