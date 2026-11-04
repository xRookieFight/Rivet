//! User and organization profiles, laid out like github.com: the tab bar lined
//! up with the main column, the vcard sidebar (bio, details, socials,
//! achievements, highlights, organizations), the profile README, pinned
//! repositories, the contribution graph with its year picker and activity
//! overview, and the month by month contribution activity timeline.

use super::Page;
use super::common;
use crate::app::Cx;
use crate::github::models::{Content, ContributionCalendar, Org, PinnedRepo, ProfileExtras, Repo, SocialAccount, User};
use crate::icons::{Icon, icon};
use crate::route::Route;
use crate::task::Load;
use crate::theme::semibold;
use crate::widgets::{self as w, Variant};
use chrono::{Datelike, NaiveDate};
use egui::{CornerRadius, Frame, Margin, RichText, Sense, Stroke, Ui, Vec2};
use egui_commonmark::CommonMarkCache;

const SIDEBAR: f32 = 296.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Repositories,
    Stars,
}

/// One loaded month of the contribution activity timeline.
struct Month {
    label: String,
    data: ContributionCalendar,
}

pub struct Profile {
    login: String,
    tab: Tab,
    user: Load<User>,
    repos: Load<Vec<Repo>>,
    repos_page: u32,
    stars: Load<Vec<Repo>>,
    stars_page: u32,
    pinned: Load<Vec<PinnedRepo>>,
    calendar: Load<ContributionCalendar>,
    year: Option<i32>,
    years: Vec<i32>,
    following: Load<bool>,
    follow_action: Load<bool>,
    filter: String,
    readme: Load<Option<Content>>,
    md: CommonMarkCache,
    extras: Load<ProfileExtras>,
    socials: Load<Vec<SocialAccount>>,
    orgs: Load<Vec<Org>>,
    months: Vec<Month>,
    month_load: Load<Month>,
    /// First day of the next (older) month the timeline should load.
    month_cursor: NaiveDate,
}

impl Profile {
    pub fn new(login: String) -> Self {
        let today = chrono::Utc::now().date_naive();
        Self {
            login,
            tab: Tab::Overview,
            user: Load::Idle,
            repos: Load::Idle,
            repos_page: 1,
            stars: Load::Idle,
            stars_page: 1,
            pinned: Load::Idle,
            calendar: Load::Idle,
            year: None,
            years: vec![],
            following: Load::Idle,
            follow_action: Load::Idle,
            filter: String::new(),
            readme: Load::Idle,
            md: CommonMarkCache::default(),
            extras: Load::Idle,
            socials: Load::Idle,
            orgs: Load::Idle,
            months: vec![],
            month_load: Load::Idle,
            month_cursor: NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today),
        }
    }

    fn load_repos(&mut self, ctx: &egui::Context, cx: &Cx, org: bool) {
        let (g, l, p) = (cx.gh.clone(), self.login.clone(), self.repos_page);
        let mine = l == cx.me.login;
        self.repos = Load::spawn(ctx, move || {
            if org {
                g.org_repos(&l, p)
            } else if mine {
                // own profile: include private repos, but only the ones we own
                Ok(g.my_repos("updated", p)?.into_iter().filter(|r| r.owner.login == l).collect())
            } else {
                g.user_repos(&l, p)
            }
        });
    }

    fn load_calendar(&mut self, ctx: &egui::Context, cx: &Cx) {
        let (g, l) = (cx.gh.clone(), self.login.clone());
        let range = self.year.map(|y| (format!("{y}-01-01T00:00:00Z"), format!("{y}-12-31T23:59:59Z")));
        self.calendar = Load::spawn(ctx, move || match range {
            Some((from, to)) => g.contributions_between(&l, Some(from), Some(to)),
            None => g.contributions(&l),
        });
    }

    /// Loads the month at `month_cursor` for the activity timeline.
    fn load_month(&mut self, ctx: &egui::Context, cx: &Cx) {
        let start = self.month_cursor;
        let next = if start.month() == 12 {
            NaiveDate::from_ymd_opt(start.year() + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(start.year(), start.month() + 1, 1)
        }
        .unwrap_or(start);
        let end = next.pred_opt().unwrap_or(start);
        let label = start.format("%B %Y").to_string();
        let (g, l) = (cx.gh.clone(), self.login.clone());
        let (from, to) = (format!("{start}T00:00:00Z"), format!("{end}T23:59:59Z"));
        self.month_load = Load::spawn(ctx, move || Ok(Month { label, data: g.contributions_between(&l, Some(from), Some(to))? }));
        self.month_cursor = start.checked_sub_months(chrono::Months::new(1)).unwrap_or(start);
    }

    /// Restarts the timeline at the newest month of the selected year.
    fn reset_months(&mut self, ctx: &egui::Context, cx: &Cx) {
        let today = chrono::Utc::now().date_naive();
        self.month_cursor = match self.year {
            Some(y) if y < today.year() => NaiveDate::from_ymd_opt(y, 12, 1),
            _ => NaiveDate::from_ymd_opt(today.year(), today.month(), 1),
        }
        .unwrap_or(today);
        self.months.clear();
        self.load_month(ctx, cx);
    }
}

impl Page for Profile {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        let ctx = ui.ctx().clone();
        if self.user.is_idle() {
            let (g, l) = (cx.gh.clone(), self.login.clone());
            self.user = Load::spawn(&ctx, move || g.user(&l));
            let (g, l) = (cx.gh.clone(), self.login.clone());
            self.pinned = Load::spawn(&ctx, move || g.pinned_repos(&l));
            self.load_calendar(&ctx, cx);
            self.reset_months(&ctx, cx);
            // the special login/login repo holds the profile README
            let (g, l) = (cx.gh.clone(), self.login.clone());
            self.readme = Load::spawn(&ctx, move || match g.readme(&l, &l, "HEAD") {
                Ok(r) => Ok(r),
                Err(e) if e.to_string().contains("404") => Ok(None),
                Err(e) => Err(e),
            });
            let l = self.login.clone();
            self.extras = Load::spawn(&ctx, move || crate::github::web::profile_extras(&l));
            let (g, l) = (cx.gh.clone(), self.login.clone());
            self.socials = Load::spawn(&ctx, move || g.social_accounts(&l));
            let (g, l) = (cx.gh.clone(), self.login.clone());
            self.orgs = Load::spawn(&ctx, move || g.user_orgs(&l));
            if self.login != cx.me.login {
                let (g, l) = (cx.gh.clone(), self.login.clone());
                self.following = Load::spawn(&ctx, move || g.is_following(&l));
            }
        }
        match self.follow_action.take() {
            Some(Ok(now)) => {
                self.following = Load::Ready(now);
                cx.ok(if now { format!("You are now following {}", self.login) } else { format!("Unfollowed {}", self.login) });
            }
            Some(Err(e)) => cx.err(e),
            None => {}
        }
        if let Some(Ok(m)) = self.month_load.take() {
            self.months.push(m);
        }
        if let Load::Ready(cal) = self.calendar.poll()
            && self.years.is_empty()
        {
            self.years = cal.years.clone();
        }
        let Some(user) = (match self.user.poll() {
            Load::Ready(u) => Some(u.clone()),
            Load::Failed(e) => {
                let e = e.clone();
                Frame::new().inner_margin(Margin::same(24)).show(ui, |ui| w::flash(ui, w::FlashKind::Danger, &e));
                None
            }
            _ => {
                w::spinner(ui);
                None
            }
        }) else {
            return;
        };
        let is_org = user.kind == "Organization";
        if self.repos.is_idle() {
            self.load_repos(&ctx, cx, is_org);
        }
        let pal = w::p(ui);

        // tabs line up with the main column, the sidebar column above stays empty
        let inner = ui.available_width() - 48.0;
        let side = ((inner - inner.min(1280.0)) / 2.0).max(0.0);
        let narrow = inner < 900.0;
        let sidebar_w = if narrow { 220.0 } else { SIDEBAR };
        let mut tab_click = None;
        let extras = match self.extras.poll() {
            Load::Ready(e) => Some(e.clone()),
            _ => None,
        };
        egui::Panel::top("profile-tabs").frame(Frame::new().fill(pal.header_bg)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(24.0 + side + sidebar_w + 24.0 - 8.0);
                let stars = extras.as_ref().and_then(|e| e.stars);
                let mut tabs = vec![(Icon::Book, "Overview", None), (Icon::Repo, "Repositories", Some(user.public_repos))];
                if !is_org {
                    tabs.push((Icon::Star, "Stars", stars));
                }
                let current = match self.tab {
                    Tab::Overview => 0,
                    Tab::Repositories => 1,
                    Tab::Stars => 2,
                };
                tab_click = w::underline_nav(ui, &tabs, current);
            });
        });
        match tab_click {
            Some(0) => self.tab = Tab::Overview,
            Some(1) => self.tab = Tab::Repositories,
            Some(2) => {
                self.tab = Tab::Stars;
                if self.stars.is_idle() {
                    let (g, l, p) = (cx.gh.clone(), self.login.clone(), self.stars_page);
                    self.stars = Load::spawn(&ctx, move || g.starred_by(&l, p));
                }
            }
            _ => {}
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            Frame::new().inner_margin(Margin { left: 24, right: 24, top: 24, bottom: 48 }).show(ui, |ui| {
                w::page(ui, 1280.0, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.allocate_ui_with_layout(Vec2::new(sidebar_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                            ui.set_width(sidebar_w);
                            self.sidebar(ui, &user, is_org, extras.as_ref(), cx);
                        });
                        ui.add_space(24.0);
                        ui.vertical(|ui| match self.tab {
                            Tab::Overview => self.overview(ui, cx),
                            Tab::Repositories => self.repositories(ui, cx, is_org),
                            Tab::Stars => self.stars_tab(ui, cx),
                        });
                    });
                });
            });
        });
    }
}

/// Thin rule plus spacing between sidebar sections, like github's border-top blocks.
fn section_rule(ui: &mut Ui) {
    ui.add_space(16.0);
    w::divider(ui);
    ui.add_space(16.0);
}

fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// "21:53 (UTC +03:00)" for a profile's time zone offset.
fn local_time(offset_hours: f32) -> String {
    let secs = (offset_hours * 3600.0).round() as i32;
    let tz = chrono::FixedOffset::east_opt(secs).unwrap_or(chrono::FixedOffset::east_opt(0).expect("utc"));
    let now = chrono::Utc::now().with_timezone(&tz);
    let sign = if secs < 0 { '-' } else { '+' };
    let (h, m) = (secs.abs() / 3600, (secs.abs() % 3600) / 60);
    format!("{} (UTC {sign}{h:02}:{m:02})", now.format("%H:%M"))
}

/// "@handle" from a social profile url, falling back to the host + path.
fn social_label(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    let last = trimmed.rsplit('/').next().unwrap_or(trimmed);
    if last.starts_with('@') {
        last.to_string()
    } else if trimmed.contains("x.com/") || trimmed.contains("twitter.com/") || trimmed.contains("instagram.com/") {
        format!("@{last}")
    } else {
        trimmed.trim_start_matches("https://").trim_start_matches("http://").trim_start_matches("www.").to_string()
    }
}

impl Profile {
    fn sidebar(&mut self, ui: &mut Ui, user: &User, is_org: bool, extras: Option<&ProfileExtras>, cx: &mut Cx) {
        let pal = w::p(ui);
        let ctx = ui.ctx().clone();
        let size = ui.available_width();
        if is_org {
            w::org_avatar(ui, &user.avatar_url, size.min(120.0));
        } else {
            w::avatar(ui, &user.avatar_url, size);
        }
        ui.add_space(16.0);
        if let Some(n) = user.name.as_ref().filter(|n| !n.is_empty()) {
            ui.add(egui::Label::new(RichText::new(n).font(semibold(24.0))).wrap());
        }
        ui.label(RichText::new(&user.login).size(20.0).color(pal.fg_muted));
        ui.add_space(16.0);
        if !is_org {
            ui.vertical_centered_justified(|ui| {
                if user.login == cx.me.login {
                    if w::button(ui, Variant::Default, None, "Edit profile").clicked() {
                        ctx.open_url(egui::OpenUrl::new_tab("https://github.com/settings/profile"));
                    }
                } else {
                    let following = matches!(self.following.poll(), Load::Ready(true));
                    let busy = self.follow_action.is_loading();
                    let label = if following { "Unfollow" } else { "Follow" };
                    if w::button_ex(ui, Variant::Default, None, label, None, !busy).clicked() {
                        let (g, l) = (cx.gh.clone(), self.login.clone());
                        self.follow_action = Load::spawn(&ctx, move || {
                            g.set_following(&l, !following)?;
                            Ok(!following)
                        });
                    }
                }
            });
            ui.add_space(16.0);
        }
        if let Some(b) = user.bio.as_ref().filter(|b| !b.trim().is_empty()) {
            ui.add(egui::Label::new(RichText::new(b.trim()).size(16.0)).wrap());
            ui.add_space(16.0);
        }
        if !is_org {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                icon(ui, Icon::People, pal.fg_muted);
                ui.label(RichText::new(w::format_count(user.followers)).font(semibold(14.0)));
                w::muted(ui, "followers ·");
                ui.label(RichText::new(w::format_count(user.following)).font(semibold(14.0)));
                w::muted(ui, "following");
            });
            ui.add_space(12.0);
        }

        // vcard details
        let detail = |ui: &mut Ui, ic: Icon, text: &str| -> egui::Response {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                icon(ui, ic, pal.fg_muted);
                ui.add(egui::Label::new(RichText::new(text).size(14.0)).truncate())
            })
            .inner
        };
        if let Some(c) = user.company.as_ref().filter(|s| !s.is_empty()) {
            // "@org" mentions link to the org, like on github
            let r = detail(ui, Icon::Organization, c);
            if let Some(org) = c.trim().strip_prefix('@').filter(|o| !o.contains(' ')) {
                let r = r.interact(Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                if r.clicked() {
                    cx.go(Route::User(org.to_string()));
                }
            }
        }
        if let Some(l) = user.location.as_ref().filter(|s| !s.is_empty()) {
            detail(ui, Icon::Location, l);
        }
        if let Some(h) = extras.and_then(|e| e.utc_offset_hours) {
            detail(ui, Icon::Clock, &local_time(h));
        }
        if let Some(b) = user.blog.as_ref().filter(|s| !s.is_empty()) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                icon(ui, Icon::Link, pal.fg_muted);
                if w::link(ui, b.trim_start_matches("https://").trim_start_matches("http://")).clicked() {
                    let url = if b.starts_with("http") { b.clone() } else { format!("https://{b}") };
                    ctx.open_url(egui::OpenUrl::new_tab(url));
                }
            });
        }
        if let Load::Ready(socials) = self.socials.poll() {
            for s in socials.clone() {
                let ic = match s.provider.as_str() {
                    "twitter" => Icon::SocialX,
                    "youtube" => Icon::SocialYoutube,
                    _ => Icon::Link,
                };
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    icon(ui, ic, pal.fg_muted);
                    if w::link(ui, social_label(&s.url)).clicked() {
                        ctx.open_url(egui::OpenUrl::new_tab(&s.url));
                    }
                });
            }
        } else if let Some(t) = user.twitter_username.as_ref().filter(|s| !s.is_empty()) {
            detail(ui, Icon::SocialX, &format!("@{t}"));
        }

        if let Some(e) = extras
            && !e.achievements.is_empty()
        {
            section_rule(ui);
            ui.label(RichText::new("Achievements").font(semibold(16.0)));
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                for a in &e.achievements {
                    let r = ui.add(egui::Image::new(a.image.as_str()).fit_to_exact_size(Vec2::splat(64.0))).on_hover_text(&a.name);
                    if let Some(t) = &a.tier {
                        // tier pill sits on the badge's bottom right corner
                        let font = egui::FontId::new(11.0, egui::FontFamily::Name(crate::theme::SEMIBOLD.into()));
                        let g = ui.painter().layout_no_wrap(t.clone(), font, pal.fg);
                        let pill = egui::Rect::from_min_size(
                            r.rect.right_bottom() - Vec2::new(g.size().x + 12.0, 18.0),
                            Vec2::new(g.size().x + 12.0, 18.0),
                        );
                        ui.painter().rect(
                            pill,
                            CornerRadius::same(255),
                            pal.canvas,
                            Stroke::new(1.0, pal.border),
                            egui::StrokeKind::Inside,
                        );
                        ui.painter().galley(pill.center() - g.size() / 2.0, g, pal.fg);
                    }
                }
            });
        }
        if let Some(e) = extras
            && !e.highlights.is_empty()
        {
            section_rule(ui);
            ui.label(RichText::new("Highlights").font(semibold(16.0)));
            ui.add_space(8.0);
            for h in &e.highlights {
                let ic = if h.contains("PRO") { Icon::Star } else { Icon::Cpu };
                detail(ui, ic, h);
            }
        }
        if let Load::Ready(orgs) = self.orgs.poll()
            && !orgs.is_empty()
        {
            let orgs = orgs.clone();
            section_rule(ui);
            ui.label(RichText::new("Organizations").font(semibold(16.0)));
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                for o in &orgs {
                    if w::org_avatar(ui, &o.avatar_url, 32.0)
                        .on_hover_text(format!("@{}", o.login))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        cx.go(Route::User(o.login.clone()));
                    }
                }
            });
        }
    }

    fn overview(&mut self, ui: &mut Ui, cx: &mut Cx) {
        let ctx = ui.ctx().clone();
        let pal = w::p(ui);
        if let Load::Ready(Some(rm)) = self.readme.poll() {
            let rm = rm.clone();
            let text = rm.text().unwrap_or_default();
            let login = self.login.clone();
            w::bordered(ui, |ui| {
                Frame::new().inner_margin(Margin::same(24)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        if w::link(ui, &login).clicked() {
                            cx.go(Route::repo_home(&login, &login));
                        }
                        ui.label(RichText::new("/").size(12.0).color(pal.fg_muted));
                        ui.label(RichText::new(&rm.name).size(12.0).color(pal.fg_muted));
                        if login == cx.me.login {
                            w::right(ui, |ui| {
                                if w::icon_button_invisible(ui, Icon::Pencil, "Edit README").clicked() {
                                    cx.go(Route::repo(&login, &login, crate::route::RepoView::Readme));
                                }
                            });
                        }
                    });
                    ui.add_space(12.0);
                    let base = common::raw_base(&login, &login, "HEAD", "");
                    common::markdown(ui, &mut self.md, &text, Some(&base));
                });
            });
            ui.add_space(24.0);
        }

        if let Some(pins) = self.pinned.ready().cloned()
            && !pins.is_empty()
        {
            ui.label(RichText::new("Pinned").size(16.0));
            ui.add_space(8.0);
            let col_w = (ui.available_width() - 16.0) / 2.0;
            for row in pins.chunks(2) {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    for p in row {
                        ui.allocate_ui_with_layout(Vec2::new(col_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                            pinned_card(ui, p, &self.login, col_w, cx);
                        });
                    }
                });
                ui.add_space(16.0);
            }
            ui.add_space(8.0);
        }

        let mut year_pick = None;
        match self.calendar.poll() {
            Load::Ready(cal) => {
                let cal = cal.clone();
                ui.horizontal(|ui| {
                    let period = self.year.map(|y| format!("in {y}")).unwrap_or_else(|| "in the last year".into());
                    ui.label(RichText::new(format!("{} contributions {period}", thousands(cal.total))).size(16.0));
                    w::right(ui, |ui| {
                        let b = w::button_ex(ui, Variant::Invisible, None, "Contribution settings", Some(Icon::TriangleDown), true);
                        egui::Popup::menu(&b).width(280.0).align(egui::RectAlign::BOTTOM_END).show(|ui| {
                            let on = !cx.prefs.hide_activity_overview;
                            if w::menu_item_ex(ui, None, "Activity overview", on, false).clicked() {
                                cx.prefs.hide_activity_overview = on;
                            }
                            w::muted_small(ui, "Show the commit, review, issue and pull request split.");
                        });
                    });
                });
                ui.add_space(8.0);
                ui.horizontal_top(|ui| {
                    let years_w = if self.years.is_empty() { 0.0 } else { 104.0 };
                    let box_w = ui.available_width() - years_w;
                    ui.allocate_ui_with_layout(Vec2::new(box_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.set_width(box_w);
                        w::bordered(ui, |ui| {
                            Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                super::dashboard::contribution_graph(ui, &cal);
                                ui.horizontal(|ui| {
                                    let r = ui.add(
                                        egui::Label::new(RichText::new("Learn how we count contributions").size(12.0).color(pal.fg_muted))
                                            .sense(Sense::click()),
                                    );
                                    if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                        ctx.open_url(egui::OpenUrl::new_tab(
                                            "https://docs.github.com/articles/why-are-my-contributions-not-showing-up-on-my-profile",
                                        ));
                                    }
                                });
                                if !cx.prefs.hide_activity_overview {
                                    ui.add_space(12.0);
                                    w::divider(ui);
                                    ui.add_space(12.0);
                                    activity_overview(ui, &cal, cx);
                                }
                            });
                        });
                    });
                    if !self.years.is_empty() {
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.set_width(96.0);
                            for y in self.years.clone() {
                                let sel = self.year == Some(y) || (self.year.is_none() && Some(&y) == self.years.first());
                                let (rect, r) = ui.allocate_exact_size(Vec2::new(96.0, 32.0), Sense::click());
                                let fill = if sel {
                                    pal.accent_emphasis
                                } else if r.hovered() {
                                    pal.control_hover
                                } else {
                                    egui::Color32::TRANSPARENT
                                };
                                ui.painter().rect_filled(rect, CornerRadius::same(6), fill);
                                ui.painter().text(
                                    rect.left_center() + Vec2::new(16.0, 0.0),
                                    egui::Align2::LEFT_CENTER,
                                    y.to_string(),
                                    egui::FontId::proportional(12.0),
                                    if sel { pal.fg_on_emphasis } else { pal.fg_muted },
                                );
                                if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && !sel {
                                    year_pick = Some(y);
                                }
                            }
                        });
                    }
                });
            }
            Load::Failed(_) => {}
            _ => w::spinner(ui),
        }
        if let Some(y) = year_pick {
            // the newest year is github's rolling "last year" view
            self.year = if Some(&y) == self.years.first() { None } else { Some(y) };
            self.load_calendar(&ctx, cx);
            self.reset_months(&ctx, cx);
        }

        ui.add_space(24.0);
        self.activity_timeline(ui, cx);
    }

    /// Month by month timeline, newest first, with "Show more activity".
    fn activity_timeline(&mut self, ui: &mut Ui, cx: &mut Cx) {
        let ctx = ui.ctx().clone();
        let pal = w::p(ui);
        ui.label(RichText::new("Contribution activity").size(16.0));
        ui.add_space(8.0);
        for m in &self.months {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&m.label).font(semibold(12.0)));
                let rect = ui.available_rect_before_wrap();
                ui.painter().hline(rect.left() + 8.0..=rect.right(), rect.center().y, Stroke::new(1.0, pal.border_muted));
            });
            ui.add_space(12.0);
            if !month_items(ui, &m.data, cx) {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    w::muted(ui, format!("{} has no activity yet for this period.", self.login));
                });
            }
            ui.add_space(16.0);
        }
        let loading = self.month_load.is_loading();
        if loading {
            ui.add(egui::Spinner::new().size(16.0));
        }
        // stop at the start of the selected year (or a year back for the rolling view)
        let today = chrono::Utc::now().date_naive();
        let floor = match self.year {
            Some(y) => NaiveDate::from_ymd_opt(y, 1, 1),
            None => today.checked_sub_months(chrono::Months::new(12)),
        }
        .unwrap_or(today);
        if !loading && self.month_cursor >= floor {
            ui.add_space(8.0);
            ui.vertical_centered_justified(|ui| {
                if w::button(ui, Variant::Default, None, "Show more activity").clicked() {
                    self.load_month(&ctx, cx);
                }
            });
        }
    }

    fn repositories(&mut self, ui: &mut Ui, cx: &mut Cx, is_org: bool) {
        let mut reload = false;
        ui.horizontal(|ui| {
            w::text_input_ex(ui, &mut self.filter, "Find a repository…", (ui.available_width() - 120.0).max(200.0), Some(Icon::Search));
            if self.login == cx.me.login && w::button(ui, Variant::Primary, Some(Icon::Repo), "New").clicked() {
                cx.go(Route::NewRepo);
            }
        });
        ui.add_space(8.0);
        w::divider(ui);
        let f = self.filter.to_lowercase();
        let mut has_next = false;
        w::show_load(ui, &mut self.repos, |ui, repos| {
            let shown: Vec<&Repo> = repos.iter().filter(|r| f.is_empty() || r.name.to_lowercase().contains(&f)).collect();
            if shown.is_empty() {
                w::blankslate(ui, Icon::Repo, "No repositories", "Nothing matched here.");
            }
            for r in shown {
                common::repo_row(ui, r, cx);
            }
            has_next = repos.len() >= 50;
        });
        if common::pager(ui, &mut self.repos_page, has_next) {
            reload = true;
        }
        if reload {
            self.load_repos(ui.ctx(), cx, is_org);
        }
    }

    fn stars_tab(&mut self, ui: &mut Ui, cx: &mut Cx) {
        let ctx = ui.ctx().clone();
        let mut has_next = false;
        w::show_load(ui, &mut self.stars, |ui, repos| {
            if repos.is_empty() {
                w::blankslate(ui, Icon::Star, "No starred repositories", "");
            }
            for r in repos.iter() {
                common::repo_row(ui, r, cx);
            }
            has_next = repos.len() >= 30;
        });
        if common::pager(ui, &mut self.stars_page, has_next) {
            let (g, l, p) = (cx.gh.clone(), self.login.clone(), self.stars_page);
            self.stars = Load::spawn(&ctx, move || g.starred_by(&l, p));
        }
    }
}

/// A pinned repository card: "owner/ **name**" for repos you don't own,
/// visibility label, fork source, two lines of description, then stats.
fn pinned_card(ui: &mut Ui, p: &PinnedRepo, login: &str, width: f32, cx: &mut Cx) {
    let pal = w::p(ui);
    Frame::new().stroke(Stroke::new(1.0, pal.border)).corner_radius(CornerRadius::same(6)).inner_margin(Margin::same(16)).show(ui, |ui| {
        ui.set_width(width - 34.0);
        ui.set_min_height(104.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            icon(ui, Icon::Repo, pal.fg_muted);
            ui.add_space(8.0);
            let mut clicked = false;
            if !p.owner.eq_ignore_ascii_case(login) {
                clicked |= w::link(ui, format!("{}/", p.owner)).clicked();
            }
            clicked |= w::link_strong(ui, &p.name, 14.0).clicked();
            if clicked {
                cx.go(Route::repo_home(&p.owner, &p.name));
            }
            ui.add_space(8.0);
            let label = match (p.private, p.archived) {
                (true, true) => "Private archive",
                (true, false) => "Private",
                (false, true) => "Public archive",
                (false, false) => "Public",
            };
            w::outline_label(ui, label, if p.archived { Some(pal.attention) } else { None });
        });
        if let Some(parent) = &p.parent {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                w::muted_small(ui, "Forked from");
                if w::link(ui, parent).clicked()
                    && let Some(r) = Route::from_slug(parent)
                {
                    cx.go(r);
                }
            });
        }
        if let Some(d) = p.description.as_ref().filter(|d| !d.is_empty()) {
            ui.add_space(4.0);
            // github clamps pinned descriptions to two lines
            let short: String = if d.chars().count() > 150 { d.chars().take(150).collect::<String>() + "…" } else { d.clone() };
            ui.add(egui::Label::new(RichText::new(short).size(12.0).color(pal.fg_muted)).wrap());
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            if let Some(l) = &p.language {
                w::language_dot(ui, l);
            }
            if p.stars > 0 {
                w::meta(ui, Icon::Star, RichText::new(w::format_count(p.stars)).size(12.0).color(pal.fg_muted));
            }
            if p.forks > 0 {
                w::meta(ui, Icon::RepoForked, RichText::new(w::format_count(p.forks)).size(12.0).color(pal.fg_muted));
            }
        });
    });
}

/// Timeline entries for one month: icon in a grey circle on a vertical rule,
/// then the summary and per-repo breakdown. Returns false when there was nothing.
fn month_items(ui: &mut Ui, cal: &ContributionCalendar, cx: &mut Cx) -> bool {
    let pal = w::p(ui);
    let sections: [(Icon, &str, &str, &Vec<(String, u64)>); 4] = [
        (Icon::RepoPush, "Created", "commits", &cal.commit_repos),
        (Icon::GitPullRequest, "Opened", "pull requests", &cal.pull_repos),
        (Icon::Eye, "Reviewed", "pull requests", &cal.review_repos),
        (Icon::IssueOpened, "Opened", "issues", &cal.issue_repos),
    ];
    let mut any = false;
    let mut entry = |ui: &mut Ui, ic: Icon, add: &mut dyn FnMut(&mut Ui)| {
        any = true;
        let top = ui.cursor().top();
        ui.horizontal_top(|ui| {
            ui.add_space(8.0);
            Frame::new()
                .fill(pal.neutral_muted)
                .stroke(Stroke::new(2.0, pal.canvas))
                .corner_radius(CornerRadius::same(255))
                .inner_margin(Margin::same(8))
                .show(ui, |ui| icon(ui, ic, pal.fg_muted));
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.add_space(6.0);
                add(ui);
            });
        });
        // the timeline's vertical rule runs behind the icons
        let x = ui.min_rect().left() + 8.0 + 16.0;
        ui.painter().with_clip_rect(ui.clip_rect()).vline(x, top + 34.0..=ui.cursor().top(), Stroke::new(2.0, pal.border_muted));
        ui.add_space(16.0);
    };
    for (ic, verb, what, repos) in sections {
        if repos.is_empty() {
            continue;
        }
        let total: u64 = repos.iter().map(|r| r.1).sum();
        let max = repos.iter().map(|r| r.1).max().unwrap_or(1).max(1);
        entry(ui, ic, &mut |ui| {
            let n = repos.len();
            ui.label(
                RichText::new(format!("{verb} {} {what} in {n} repositor{}", thousands(total), if n == 1 { "y" } else { "ies" }))
                    .size(16.0),
            );
            ui.add_space(6.0);
            for (r, c) in repos.iter().take(10) {
                ui.horizontal(|ui| {
                    let name_w = (ui.available_width() * 0.5).max(160.0);
                    ui.allocate_ui_with_layout(Vec2::new(name_w, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_width(name_w);
                        if w::link(ui, r).clicked()
                            && let Some(route) = Route::from_slug(r)
                        {
                            cx.go(route);
                        }
                    });
                    w::muted_small(ui, format!("{} {what}", thousands(*c)));
                    w::right(ui, |ui| {
                        let bar_w = 100.0;
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_w, 8.0), Sense::hover());
                        ui.painter().rect_filled(rect, CornerRadius::same(4), pal.neutral_muted);
                        let filled = egui::Rect::from_min_size(rect.min, Vec2::new(bar_w * (*c as f32 / max as f32).max(0.04), 8.0));
                        ui.painter().rect_filled(filled, CornerRadius::same(4), pal.success_emphasis);
                    });
                });
            }
            if n > 10 {
                w::muted_small(ui, format!("and {} more", n - 10));
            }
        });
    }
    if !cal.created_repos.is_empty() {
        entry(ui, Icon::Repo, &mut |ui| {
            let n = cal.created_repos.len();
            ui.label(RichText::new(format!("Created {n} repositor{}", if n == 1 { "y" } else { "ies" })).size(16.0));
            ui.add_space(6.0);
            for r in cal.created_repos.iter().take(10) {
                if w::link(ui, r).clicked()
                    && let Some(route) = Route::from_slug(r)
                {
                    cx.go(route);
                }
            }
        });
    }
    any
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_separator() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(9279), "9,279");
        assert_eq!(thousands(1234567), "1,234,567");
    }

    #[test]
    fn social_labels() {
        assert_eq!(social_label("https://x.com/xRookieFightt"), "@xRookieFightt");
        assert_eq!(social_label("https://www.youtube.com/@xrookiefight"), "@xrookiefight");
        assert_eq!(social_label("https://example.com/me"), "example.com/me");
    }

    #[test]
    fn local_time_format() {
        assert!(local_time(3.0).ends_with("(UTC +03:00)"));
        assert!(local_time(-5.5).ends_with("(UTC -05:30)"));
    }
}

/// GitHub's "Activity overview": the repos you touched most on the left and the
/// commits / code review / issues / pull requests cross on the right.
fn activity_overview(ui: &mut Ui, cal: &ContributionCalendar, cx: &mut Cx) {
    let pal = w::p(ui);
    let total = (cal.commits + cal.issues + cal.pulls + cal.reviews).max(1) as f32;
    let pct = |n: u64| (n as f32 / total * 100.0).round() as u32;
    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        ui.label(RichText::new("Activity overview").font(semibold(14.0)));
        ui.add_space(8.0);
        let mut repos: Vec<(String, u64)> = cal.commit_repos.clone();
        for (r, n) in cal.pull_repos.iter().chain(&cal.issue_repos).chain(&cal.review_repos) {
            match repos.iter_mut().find(|x| &x.0 == r) {
                Some(x) => x.1 += n,
                None => repos.push((r.clone(), *n)),
            }
        }
        repos.sort_by_key(|r| std::cmp::Reverse(r.1));
        if repos.is_empty() {
            w::muted(ui, "No public activity in this period.");
        } else {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                icon(ui, Icon::Repo, pal.fg_muted);
                w::muted(ui, "Contributed to");
                for (i, (r, _)) in repos.iter().take(3).enumerate() {
                    if w::link_strong(ui, r, 14.0).clicked()
                        && let Some(route) = Route::from_slug(r)
                    {
                        cx.go(route);
                    }
                    if i < repos.len().min(3) - 1 {
                        w::muted(ui, ",");
                    }
                }
                if repos.len() > 3 {
                    w::muted(ui, format!("and {} other repositories", repos.len() - 3));
                }
            });
        }

        let ui = &mut cols[1];
        let size = ui.available_width().min(260.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), size * 0.8), Sense::hover());
        let c = rect.center();
        let arm = size * 0.28;
        let p = ui.painter();
        let line = Stroke::new(2.0, pal.success_emphasis);
        let axis = Stroke::new(1.0, pal.success_emphasis.gamma_multiply(0.6));
        p.line_segment([c - Vec2::new(arm, 0.0), c + Vec2::new(arm, 0.0)], axis);
        p.line_segment([c - Vec2::new(0.0, arm), c + Vec2::new(0.0, arm)], axis);
        // up: code review, right: issues, down: pull requests, left: commits
        let r = |n: u64| arm * (n as f32 / total).max(0.03);
        let pts = vec![
            c - Vec2::new(0.0, r(cal.reviews)),
            c + Vec2::new(r(cal.issues), 0.0),
            c + Vec2::new(0.0, r(cal.pulls)),
            c - Vec2::new(r(cal.commits), 0.0),
        ];
        p.add(egui::Shape::convex_polygon(pts.clone(), pal.success_emphasis.gamma_multiply(0.35), line));
        for q in &pts {
            p.circle(*q, 3.0, pal.canvas, line);
        }
        let font = egui::FontId::proportional(12.0);
        let label = |pos: egui::Pos2, align: egui::Align2, pct: u32, name: &str| {
            p.text(pos, align, format!("{pct}%\n{name}"), font.clone(), pal.fg_muted);
        };
        label(c - Vec2::new(0.0, arm + 6.0), egui::Align2::CENTER_BOTTOM, pct(cal.reviews), "Code review");
        label(c + Vec2::new(arm + 8.0, 0.0), egui::Align2::LEFT_CENTER, pct(cal.issues), "Issues");
        label(c + Vec2::new(0.0, arm + 6.0), egui::Align2::CENTER_TOP, pct(cal.pulls), "Pull requests");
        label(c - Vec2::new(arm + 8.0, 0.0), egui::Align2::RIGHT_CENTER, pct(cal.commits), "Commits");
    });
}
