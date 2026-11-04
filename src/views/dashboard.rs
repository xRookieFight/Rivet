//! Home dashboard, laid out like github's current one: account switcher with
//! Home / Feed and top repositories on the left, a greeting with your pull
//! requests and issues in the middle, and the changelog on the right.
//!
//! GitHub's version also has a Copilot prompt and agent sessions; there's no
//! public API for those, so Rivet leaves them out rather than faking them.

use super::Page;
use crate::app::Cx;
use crate::github::models::{ChangelogEntry, ContributionCalendar, Issue, Notification, Org, Repo, SearchResult};
use crate::icons::{Icon, icon};
use crate::route::{RepoView, Route};
use crate::task::Load;
use crate::theme::semibold;
use crate::widgets::{self as w};
use egui::{CornerRadius, Frame, Margin, RichText, Sense, Stroke, Ui, Vec2};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HomeTab {
    Home,
    Feed,
}

/// Left column shared by Home and Feed.
#[derive(Default)]
pub struct Sidebar {
    repos: Load<Vec<Repo>>,
    orgs: Load<Vec<Org>>,
    filter: String,
    searching: bool,
    show_more: bool,
}

impl Sidebar {
    pub fn ui(&mut self, ui: &mut Ui, cx: &mut Cx, tab: HomeTab) {
        let ctx = ui.ctx().clone();
        if self.repos.is_idle() {
            let g = cx.gh.clone();
            self.repos = Load::spawn(&ctx, move || g.my_repos("pushed", 1));
            let g = cx.gh.clone();
            self.orgs = Load::spawn(&ctx, move || g.my_orgs());
        }
        let pal = w::p(ui);

        // account context switcher, the ⇄ opens your orgs
        ui.horizontal(|ui| {
            w::avatar(ui, &cx.me.avatar_url, 32.0);
            ui.vertical(|ui| {
                ui.label(RichText::new(&cx.me.login).font(semibold(14.0)));
                w::muted_small(ui, "Your personal account");
            });
            w::right(ui, |ui| {
                let b = w::icon_button_invisible(ui, Icon::ArrowSwitch, "Switch dashboard context");
                egui::Popup::menu(&b).width(260.0).align(egui::RectAlign::BOTTOM_END).show(|ui| {
                    ui.label(RichText::new("Switch dashboard context").font(semibold(12.0)).color(pal.fg_muted));
                    ui.horizontal(|ui| {
                        w::avatar(ui, &cx.me.avatar_url, 20.0);
                        ui.label(&cx.me.login);
                        w::right(ui, |ui| icon(ui, Icon::Check, pal.fg));
                    });
                    match self.orgs.poll() {
                        Load::Ready(orgs) => {
                            for o in orgs.clone() {
                                let r = ui
                                    .horizontal(|ui| {
                                        w::org_avatar(ui, &o.avatar_url, 20.0);
                                        w::plain_link(ui, &o.login)
                                    })
                                    .inner;
                                if r.clicked() {
                                    cx.go(Route::User(o.login.clone()));
                                }
                            }
                        }
                        Load::Failed(_) => {
                            w::muted_small(ui, "Organizations need the read:org scope.");
                        }
                        _ => {
                            ui.add(egui::Spinner::new().size(14.0));
                        }
                    }
                });
            });
        });
        ui.add_space(16.0);

        for (t, ic, label, route) in
            [(HomeTab::Home, Icon::Home, "Home", Route::Dashboard), (HomeTab::Feed, Icon::Globe, "Feed", Route::Feed)]
        {
            let r = w::menu_item(ui, Some(ic), label);
            if t == tab {
                ui.painter().rect_filled(r.rect, CornerRadius::same(6), pal.control_hover);
                let bar = egui::Rect::from_min_size(r.rect.left_top() + Vec2::new(-8.0, 6.0), Vec2::new(4.0, 20.0));
                ui.painter().rect_filled(bar, CornerRadius::same(2), pal.accent_emphasis);
                // repaint the label over the fill so it stays readable
                let mut x = r.rect.left() + 8.0;
                ic.image(16.0, pal.fg_muted)
                    .paint_at(ui, egui::Rect::from_min_size(egui::pos2(x, r.rect.center().y - 8.0), Vec2::splat(16.0)));
                x += 24.0;
                ui.painter().text(egui::pos2(x, r.rect.center().y), egui::Align2::LEFT_CENTER, label, semibold(14.0), pal.fg);
            }
            if r.clicked() && t != tab {
                cx.go(route);
            }
        }
        ui.add_space(8.0);
        w::divider(ui);
        ui.add_space(12.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new("Top repositories").font(semibold(12.0)).color(pal.fg_muted));
            w::right(ui, |ui| {
                if w::icon_button_invisible(ui, Icon::Search, "Find a repository").clicked() {
                    self.searching = !self.searching;
                }
            });
        });
        if self.searching {
            w::text_input(ui, &mut self.filter, "Find a repository…", ui.available_width());
            ui.add_space(4.0);
        }
        let f = self.filter.to_lowercase();
        let limit = if self.show_more || !f.is_empty() { usize::MAX } else { 10 };
        let mut total = 0;
        w::show_load(ui, &mut self.repos, |ui, repos| {
            let list: Vec<&Repo> = repos.iter().filter(|r| f.is_empty() || r.full_name.to_lowercase().contains(&f)).collect();
            total = list.len();
            for r in list.into_iter().take(limit) {
                ui.horizontal(|ui| {
                    w::avatar(ui, &r.owner.avatar_url, 16.0);
                    if w::plain_link(ui, &r.full_name).clicked() {
                        cx.go(Route::repo_home(&r.owner.login, &r.name));
                    }
                });
                ui.add_space(2.0);
            }
            if total == 0 {
                w::muted_small(ui, "No repositories match.");
            }
        });
        if total > 10 && f.is_empty() {
            ui.add_space(4.0);
            let label = if self.show_more { "Show less" } else { "Show more" };
            if ui
                .add(egui::Label::new(RichText::new(label).size(12.0).color(pal.fg_muted)).sense(Sense::click()))
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                self.show_more = !self.show_more;
            }
        }
    }
}

/// Two-column shell (sidebar + content) used by Home and Feed.
pub fn shell(
    ui: &mut Ui,
    cx: &mut Cx,
    sidebar: &mut Sidebar,
    tab: HomeTab,
    right: Option<&mut dyn FnMut(&mut Ui, &mut Cx)>,
    center: &mut dyn FnMut(&mut Ui, &mut Cx),
) {
    let pal = w::p(ui);
    let total = ui.available_width();
    egui::Panel::left("home-left")
        .resizable(false)
        .exact_size(320.0_f32.min(total * 0.28))
        .frame(Frame::new().fill(if pal.dark { pal.canvas_inset } else { pal.canvas }).inner_margin(Margin::same(16)))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("home-left-scroll").show(ui, |ui| sidebar.ui(ui, cx, tab));
        });
    if let Some(right) = right
        && total > 1150.0
    {
        egui::Panel::right("home-right")
            .resizable(false)
            .exact_size(320.0)
            .show_separator_line(false)
            .frame(Frame::new().inner_margin(Margin { left: 0, right: 24, top: 32, bottom: 16 }))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("home-right-scroll").show(ui, |ui| right(ui, cx));
            });
    }
    egui::CentralPanel::no_frame().show(ui, |ui| {
        egui::ScrollArea::vertical().id_salt("home-center").show(ui, |ui| {
            Frame::new().inner_margin(Margin::symmetric(32, 32)).show(ui, |ui| {
                w::page(ui, 768.0, |ui| center(ui, cx));
            });
        });
    });
}

/// Which slice of your work a Home section shows. Mirrors the filter menu.
const PR_FILTERS: [(&str, &str); 4] = [
    ("Created by you", "author:@me"),
    ("Assigned to you", "assignee:@me"),
    ("Review requested", "review-requested:@me"),
    ("Mentioning you", "mentions:@me"),
];
const HOME_LIST_LEN: usize = 3;
const ISSUE_FILTERS: [(&str, &str); 3] =
    [("Created by you", "author:@me"), ("Assigned to you", "assignee:@me"), ("Mentioning you", "mentions:@me")];

#[derive(Default)]
pub struct Dashboard {
    sidebar: Sidebar,
    prs: Load<SearchResult<Issue>>,
    issues: Load<SearchResult<Issue>>,
    changelog: Load<Vec<ChangelogEntry>>,
}

impl Dashboard {
    fn load_section(&mut self, ctx: &egui::Context, cx: &Cx, pulls: bool) {
        let (kind, filters, pick) = if pulls {
            ("is:pr", &PR_FILTERS[..], cx.prefs.home_pr_filter)
        } else {
            ("is:issue", &ISSUE_FILTERS[..], cx.prefs.home_issue_filter)
        };
        let who = filters.get(pick).unwrap_or(&filters[0]).1;
        let q = format!("{kind} is:open archived:false {who}");
        let g = cx.gh.clone();
        // short glance lists; View all opens the full page
        let job = move || -> anyhow::Result<SearchResult<Issue>> {
            let mut r = g.search_issues(&q, "updated", 1)?;
            r.items.truncate(HOME_LIST_LEN);
            Ok(r)
        };
        if pulls {
            self.prs = Load::spawn(ctx, job);
        } else {
            self.issues = Load::spawn(ctx, job);
        }
    }
}

fn greeting() -> &'static str {
    use chrono::Timelike;
    match chrono::Local::now().hour() {
        5..=11 => "Good morning",
        12..=17 => "Good afternoon",
        _ => "Good evening",
    }
}

impl Page for Dashboard {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        let ctx = ui.ctx().clone();
        if self.prs.is_idle() {
            self.load_section(&ctx, cx, true);
            self.load_section(&ctx, cx, false);
            self.changelog = Load::spawn(&ctx, || crate::github::changelog(5));
        }
        let mut sidebar = std::mem::take(&mut self.sidebar);
        let mut reload: Option<bool> = None;
        let changelog = &mut self.changelog;
        let (prs, issues) = (&mut self.prs, &mut self.issues);
        let mut right = |ui: &mut Ui, _cx: &mut Cx| changelog_card(ui, changelog);
        let mut center = |ui: &mut Ui, cx: &mut Cx| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{}, {}!", greeting(), cx.me.login)).size(24.0).font(semibold(24.0)));
            });
            ui.add_space(24.0);
            if section(ui, cx, "Pull requests", true, prs) {
                reload = Some(true);
            }
            ui.add_space(24.0);
            if section(ui, cx, "Issues", false, issues) {
                reload = Some(false);
            }
        };
        shell(ui, cx, &mut sidebar, HomeTab::Home, Some(&mut right), &mut center);
        self.sidebar = sidebar;
        if let Some(pulls) = reload {
            self.load_section(&ctx, cx, pulls);
        }
    }
}

/// One Home module: heading with View all + filter menu, then up to 12 rows.
/// Returns true when the filter changed and the list needs reloading.
fn section(ui: &mut Ui, cx: &mut Cx, title: &str, pulls: bool, data: &mut Load<SearchResult<Issue>>) -> bool {
    let pal = w::p(ui);
    let mut changed = false;
    let filters: &[(&str, &str)] = if pulls { &PR_FILTERS } else { &ISSUE_FILTERS };
    let current = if pulls { cx.prefs.home_pr_filter } else { cx.prefs.home_issue_filter };
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).font(semibold(14.0)).color(pal.fg_muted));
        w::muted_small(ui, format!("· {}", filters.get(current).unwrap_or(&filters[0]).0));
        w::right(ui, |ui| {
            let f = w::icon_button_invisible(ui, Icon::Filter, "Filter this list");
            egui::Popup::menu(&f).width(220.0).align(egui::RectAlign::BOTTOM_END).show(|ui| {
                for (i, (label, _)) in filters.iter().enumerate() {
                    if w::menu_item_ex(ui, None, label, i == current, false).clicked() {
                        if pulls {
                            cx.prefs.home_pr_filter = i;
                        } else {
                            cx.prefs.home_issue_filter = i;
                        }
                        changed = true;
                    }
                }
            });
            if w::link(ui, "View all").clicked() {
                cx.go(Route::MyItems { pulls });
            }
        });
    });
    ui.add_space(8.0);
    w::bordered(ui, |ui| {
        w::show_load(ui, data, |ui, res| {
            if res.items.is_empty() {
                let what = if pulls { "pull requests" } else { "issues" };
                Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                    ui.vertical_centered(|ui| w::muted(ui, format!("No open {what} here.")));
                });
            }
            let n = res.items.len();
            for (i, it) in res.items.iter().enumerate() {
                home_row(ui, it, cx);
                if i + 1 < n {
                    w::divider(ui);
                }
            }
        });
    });
    changed
}

fn home_row(ui: &mut Ui, it: &Issue, cx: &mut Cx) {
    let pal = w::p(ui);
    let resp = w::list_row(ui, |ui| {
        ui.horizontal_top(|ui| {
            let (ic, color) = it.item_state().list_icon(pal, it.is_pr());
            icon(ui, ic, color);
            let text_w = (ui.available_width() - 60.0).max(120.0);
            ui.allocate_ui_with_layout(Vec2::new(text_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_width(text_w);
                ui.add(egui::Label::new(RichText::new(&it.title).font(semibold(14.0))).truncate());
                let mut meta = format!("{}#{} · Opened by {}", it.repo_full_name(), it.number, it.user.login);
                if let Some(t) = &it.updated_at {
                    meta.push_str(&format!(" · Updated {}", w::rel_time(t)));
                }
                ui.add(egui::Label::new(RichText::new(meta).size(12.0).color(pal.fg_muted)).truncate());
            });
            w::right(ui, |ui| {
                if it.comments > 0 {
                    ui.label(RichText::new(it.comments.to_string()).size(12.0).color(pal.fg_muted));
                    icon(ui, Icon::Comment, pal.fg_muted);
                }
            });
        });
    });
    if resp.clicked()
        && let Some((o, n)) = it.repo_full_name().split_once('/')
    {
        let view = if it.is_pr() { RepoView::Pull(it.number) } else { RepoView::Issue(it.number) };
        cx.go(Route::repo(o, n, view));
    }
}

/// "Latest from our changelog" timeline card.
fn changelog_card(ui: &mut Ui, data: &mut Load<Vec<ChangelogEntry>>) {
    let pal = w::p(ui);
    Frame::new()
        .fill(if pal.dark { pal.canvas_subtle } else { pal.canvas })
        .stroke(Stroke::new(1.0, pal.border))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Latest from our changelog").font(semibold(14.0)));
            ui.add_space(12.0);
            match data.poll() {
                Load::Ready(entries) => {
                    let entries = entries.clone();
                    for e in &entries {
                        ui.horizontal_top(|ui| {
                            let (dot, _) = ui.allocate_exact_size(Vec2::new(12.0, 16.0), Sense::hover());
                            ui.painter().circle_filled(dot.center(), 3.0, pal.fg_subtle);
                            ui.vertical(|ui| {
                                if let Some(d) = &e.date {
                                    w::muted_small(ui, d.format("%b %-d, %Y").to_string());
                                }
                                let r =
                                    ui.add(egui::Label::new(RichText::new(&e.title).size(13.0).color(pal.fg)).wrap().sense(Sense::click()));
                                if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                    ui.ctx().open_url(egui::OpenUrl::new_tab(&e.link));
                                }
                            });
                        });
                        ui.add_space(10.0);
                    }
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        // the arrow is an octicon, the bundled fonts have no → glyph
                        let r = ui
                            .horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                ui.label(RichText::new("View changelog").size(12.0).color(pal.fg_muted));
                                icon(ui, Icon::ArrowRight, pal.fg_muted);
                            })
                            .response;
                        if ui
                            .interact(r.rect, ui.scope_id().with("changelog-link"), Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            ui.ctx().open_url(egui::OpenUrl::new_tab("https://github.blog/changelog/"));
                        }
                    });
                }
                Load::Failed(_) => {
                    w::muted_small(ui, "Couldn't load the changelog.");
                }
                _ => {
                    ui.add(egui::Spinner::new().size(16.0));
                }
            }
        });
}

/// The Feed tab: same left column as Home, feed cards in the middle.
#[derive(Default)]
pub struct FeedPage {
    sidebar: Sidebar,
    feed: super::feed::Feed,
}

impl FeedPage {
    /// A feed page that already started fetching, so the first visit is quick.
    pub fn warm(ctx: &egui::Context, gh: &crate::github::Client, login: &str) -> Self {
        let mut p = Self::default();
        p.feed.prefetch(ctx, gh, login);
        p
    }
}

impl Page for FeedPage {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        let mut sidebar = std::mem::take(&mut self.sidebar);
        let feed = &mut self.feed;
        let mut center = |ui: &mut Ui, cx: &mut Cx| feed.ui(ui, cx);
        shell(ui, cx, &mut sidebar, HomeTab::Feed, None, &mut center);
        self.sidebar = sidebar;
    }
}

pub fn kbd(ui: &mut Ui, k: &str) {
    let pal = w::p(ui);
    Frame::new()
        .fill(pal.canvas_subtle)
        .stroke(Stroke::new(1.0, pal.border))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(5, 1))
        .show(ui, |ui| ui.label(RichText::new(k).monospace().size(11.0)));
}

pub fn notification_route(n: &Notification) -> Route {
    let (o, name) = (n.repository.owner.login.clone(), n.repository.name.clone());
    match (n.subject.kind.as_str(), n.number()) {
        ("Issue", Some(num)) => Route::repo(&o, &name, RepoView::Issue(num)),
        ("PullRequest", Some(num)) => Route::repo(&o, &name, RepoView::Pull(num)),
        ("CheckSuite", _) => Route::repo(&o, &name, RepoView::Actions { workflow: None }),
        ("Commit", _) => match n.subject.url.as_deref().and_then(|u| u.rsplit('/').next()) {
            Some(sha) => Route::repo(&o, &name, RepoView::Commit(sha.into())),
            None => Route::repo_home(&o, &name),
        },
        _ => Route::repo_home(&o, &name),
    }
}

/// The green squares. Colors come from GitHub (light theme values), so in dark
/// mode we remap to the dark palette by level.
pub fn contribution_graph(ui: &mut Ui, cal: &ContributionCalendar) {
    let pal = w::p(ui);
    let levels_light = ["#ebedf0", "#9be9a8", "#40c463", "#30a14e", "#216e39"];
    let dark = ["#151b23", "#033a16", "#196c2e", "#2ea043", "#56d364"];
    let light = ["#eff2f5", "#aceebb", "#4ac26b", "#2da44e", "#116329"];
    let weeks = cal.weeks.len().max(1);
    let avail = ui.available_width();
    // stretch the squares to fill the box like github does, but keep them
    // between 8 and 14px; narrow boxes drop the oldest weeks instead
    let gap = 3.0;
    let cell = ((avail - 30.0) / weeks as f32 - gap).clamp(8.0, 14.0);
    let fit = ((avail - 30.0) / (cell + gap)).floor().max(1.0) as usize;
    let start = weeks.saturating_sub(fit);
    let shown = weeks - start;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(30.0 + shown as f32 * (cell + gap), 7.0 * (cell + gap) + 18.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let origin = rect.min + Vec2::new(30.0, 16.0);
    for (i, label) in [(1, "Mon"), (3, "Wed"), (5, "Fri")] {
        painter.text(
            egui::pos2(rect.left(), origin.y + i as f32 * (cell + gap) + cell / 2.0),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(10.0),
            pal.fg_muted,
        );
    }
    let mut last_month = String::new();
    let hover_pos = ui.input(|i| i.pointer.hover_pos());
    let mut tip = None;
    for (wi, week) in cal.weeks[start..].iter().enumerate() {
        if let Some(first) = week.first() {
            let month = first.date.get(5..7).unwrap_or("").to_string();
            if month != last_month && week.len() == 7 {
                let name = match month.as_str() {
                    "01" => "Jan",
                    "02" => "Feb",
                    "03" => "Mar",
                    "04" => "Apr",
                    "05" => "May",
                    "06" => "Jun",
                    "07" => "Jul",
                    "08" => "Aug",
                    "09" => "Sep",
                    "10" => "Oct",
                    "11" => "Nov",
                    _ => "Dec",
                };
                if wi > 0 || last_month.is_empty() {
                    painter.text(
                        egui::pos2(origin.x + wi as f32 * (cell + gap), rect.top()),
                        egui::Align2::LEFT_TOP,
                        name,
                        egui::FontId::proportional(10.0),
                        pal.fg_muted,
                    );
                }
                last_month = month;
            }
        }
        for day in week {
            let weekday = chrono::NaiveDate::parse_from_str(&day.date, "%Y-%m-%d")
                .map(|d| chrono::Datelike::weekday(&d).num_days_from_sunday())
                .unwrap_or(0);
            let level = levels_light.iter().position(|c| c.eq_ignore_ascii_case(&day.color)).unwrap_or(if day.count == 0 { 0 } else { 2 });
            let color = w::parse_hex(if pal.dark { dark[level] } else { light[level] }).unwrap_or(pal.neutral_muted);
            let r =
                egui::Rect::from_min_size(origin + Vec2::new(wi as f32 * (cell + gap), weekday as f32 * (cell + gap)), Vec2::splat(cell));
            painter.rect_filled(r, CornerRadius::same(2), color);
            if hover_pos.is_some_and(|p| r.contains(p)) {
                tip = Some(format!(
                    "{} contribution{} on {}",
                    if day.count == 0 { "No".to_string() } else { day.count.to_string() },
                    if day.count == 1 { "" } else { "s" },
                    day.date
                ));
            }
        }
    }
    if let Some(t) = tip {
        egui::Tooltip::always_open(ui.ctx().clone(), ui.layer_id(), egui::Id::unique("contrib-tip"), egui::PopupAnchor::Pointer)
            .show(|ui| ui.label(t));
    }
    ui.horizontal(|ui| {
        w::right(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            w::muted_small(ui, "More");
            for c in (if pal.dark { dark } else { light }).iter().rev() {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(cell), Sense::hover());
                ui.painter().rect_filled(r, CornerRadius::same(2), w::parse_hex(c).unwrap_or(pal.neutral_muted));
            }
            w::muted_small(ui, "Less");
        });
    });
}
