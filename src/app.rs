//! The top level app: session handling, the global header, navigation history,
//! command palette and toasts. Screens live in `views`.

use crate::auth::{self, Account};
use crate::github::Client;
use crate::github::models::{Repo, User};
use crate::icons::{Icon, icon};
use crate::route::{RepoView, Route, SearchKind};
use crate::task::Load;
use crate::theme::{self, ThemePref, semibold};
use crate::views::{self, Page};
use crate::widgets::{self as w};
use egui::{Align, CornerRadius, Frame, Key, KeyboardShortcut, Layout, Margin, Modifiers, RichText, Sense, Stroke, Ui, Vec2};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const PREFS_KEY: &str = "rivet.prefs";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Prefs {
    pub theme: ThemePref,
    pub accounts: Vec<Account>,
    pub active: Option<String>,
    pub recent_local: Vec<PathBuf>,
    pub recent_repos: Vec<String>,
    pub clone_dir: Option<PathBuf>,
    /// Stored inverted so the overview is on by default, serde's bool default is false.
    pub hide_activity_overview: bool,
    /// Feed filter categories the user switched off; empty means everything shows.
    pub feed_hidden: Vec<String>,
    /// Index into the Home pull request / issue filter menus.
    pub home_pr_filter: usize,
    pub home_issue_filter: usize,
}

impl Prefs {
    pub fn remember_repo(&mut self, slug: &str) {
        self.recent_repos.retain(|r| r != slug);
        self.recent_repos.insert(0, slug.to_string());
        self.recent_repos.truncate(12);
    }

    pub fn remember_local(&mut self, p: PathBuf) {
        self.recent_local.retain(|r| r != &p);
        self.recent_local.insert(0, p);
        self.recent_local.truncate(15);
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum ToastKind {
    Success,
    Error,
    Info,
}

struct Toast {
    text: String,
    kind: ToastKind,
    at: Instant,
}

/// What a page gets to talk to the rest of the app.
pub struct Cx<'a> {
    pub gh: &'a Client,
    pub me: &'a User,
    pub prefs: &'a mut Prefs,
    pub my_repos: &'a [Repo],
    nav: &'a mut Option<Route>,
    replace: &'a mut bool,
    toasts: &'a mut Vec<Toast>,
    pub refresh_unread: &'a mut bool,
}

impl Cx<'_> {
    pub fn go(&mut self, r: Route) {
        *self.nav = Some(r);
    }

    /// Navigate without pushing history, e.g. tab switches inside a page.
    pub fn go_replace(&mut self, r: Route) {
        *self.nav = Some(r);
        *self.replace = true;
    }

    pub fn ok(&mut self, text: impl Into<String>) {
        self.toasts.push(Toast { text: text.into(), kind: ToastKind::Success, at: Instant::now() });
    }

    pub fn err(&mut self, text: impl Into<String>) {
        self.toasts.push(Toast { text: text.into(), kind: ToastKind::Error, at: Instant::now() });
    }

    pub fn info(&mut self, text: impl Into<String>) {
        self.toasts.push(Toast { text: text.into(), kind: ToastKind::Info, at: Instant::now() });
    }
}

struct Session {
    gh: Client,
    me: Load<User>,
    route: Route,
    page: Box<dyn Page>,
    back: Vec<Route>,
    fwd: Vec<Route>,
    unread: Load<bool>,
    last_unread_poll: Instant,
    window_title: String,
    my_repos: Load<Vec<Repo>>,
    drawer_open: bool,
    palette: Option<CommandPalette>,
    search_text: String,
    focus_search: bool,
    chord: Option<(char, Instant)>,
    /// Home and Feed stay alive between visits so they open instantly.
    kept: std::collections::HashMap<&'static str, Box<dyn Page>>,
    warmed: bool,
}

/// Pages worth keeping around instead of rebuilding on every visit.
fn keep_key(r: &Route) -> Option<&'static str> {
    match r {
        Route::Dashboard => Some("home"),
        Route::Feed => Some("feed"),
        _ => None,
    }
}

impl Session {
    fn new(ctx: &egui::Context, token: String) -> Self {
        let gh = Client::new(token);
        let g = gh.clone();
        let me = Load::spawn(ctx, move || g.me());
        let g = gh.clone();
        let my_repos = Load::spawn(ctx, move || g.my_repos("pushed", 1));
        Self {
            gh,
            me,
            route: Route::Dashboard,
            page: views::make(&Route::Dashboard),
            back: vec![],
            fwd: vec![],
            unread: Load::Idle,
            last_unread_poll: Instant::now() - Duration::from_secs(3600),
            window_title: String::new(),
            my_repos,
            drawer_open: false,
            palette: None,
            search_text: String::new(),
            focus_search: false,
            chord: None,
            kept: std::collections::HashMap::new(),
            warmed: false,
        }
    }

    /// Swaps in the page for `r`, parking the current one if it's a kept page.
    fn show(&mut self, r: &Route) {
        let next = keep_key(r).and_then(|k| self.kept.remove(k)).unwrap_or_else(|| views::make(r));
        let old = std::mem::replace(&mut self.page, next);
        if let Some(k) = keep_key(&self.route) {
            self.kept.insert(k, old);
        }
    }

    fn navigate(&mut self, r: Route, push: bool) {
        if r == self.route {
            // clicking the same place again reloads it, like a browser
            self.page = views::make(&r);
            return;
        }
        self.show(&r);
        if push {
            self.back.push(std::mem::replace(&mut self.route, r));
            self.fwd.clear();
        } else {
            self.route = r;
        }
        self.drawer_open = false;
    }

    fn go_back(&mut self) {
        if let Some(r) = self.back.pop() {
            self.show(&r);
            self.fwd.push(std::mem::replace(&mut self.route, r));
        }
    }

    fn go_forward(&mut self) {
        if let Some(r) = self.fwd.pop() {
            self.show(&r);
            self.back.push(std::mem::replace(&mut self.route, r));
        }
    }
}

enum State {
    SignedOut(views::login::LoginPage),
    SignedIn(Box<Session>),
}

pub struct RivetApp {
    prefs: Prefs,
    state: State,
    toasts: Vec<Toast>,
    switch_to: Option<(Account, String)>,
    dev: Option<crate::devtools::Dev>,
    input_bridge: crate::platform::InputBridge,
}

impl RivetApp {
    pub fn new(cc: &eframe::CreationContext<'_>, input_bridge: crate::platform::InputBridge) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        theme::install_fonts(&cc.egui_ctx);
        theme::install_style(&cc.egui_ctx);
        let prefs: Prefs = cc.storage.and_then(|s| eframe::get_value(s, PREFS_KEY)).unwrap_or_default();
        theme::apply_pref(&cc.egui_ctx, prefs.theme);

        let mut toasts = vec![];
        let dev = crate::devtools::Dev::from_env();
        if let Some(t) = dev.as_ref().and_then(|d| d.theme) {
            theme::apply_pref(&cc.egui_ctx, t);
        }
        let dev_token = dev.as_ref().and_then(|d| d.token.clone());
        let state = match dev_token.map(Ok).or_else(|| prefs.active.as_deref().map(auth::load_token)) {
            Some(Ok(tok)) => State::SignedIn(Box::new(Session::new(&cc.egui_ctx, tok))),
            Some(Err(e)) => {
                toasts.push(Toast { text: format!("Could not read the saved token: {e}"), kind: ToastKind::Error, at: Instant::now() });
                State::SignedOut(views::login::LoginPage::default())
            }
            None => State::SignedOut(views::login::LoginPage::default()),
        };
        let mut state = state;
        if let (State::SignedIn(s), Some(r)) = (&mut state, dev.as_ref().and_then(|d| d.route.clone())) {
            s.navigate(r, false);
        }
        Self { prefs, state, toasts, switch_to: None, dev, input_bridge }
    }

    fn sign_in(&mut self, ctx: &egui::Context, user: User, token: String) {
        if let Err(e) = auth::store_token(&user.login, &token) {
            self.push_toast(format!("Signed in, but the token couldn't be saved to the keychain: {e}"), ToastKind::Error);
        }
        let acc = Account { login: user.login.clone(), avatar_url: user.avatar_url.clone() };
        self.prefs.accounts.retain(|a| a.login != acc.login);
        self.prefs.accounts.push(acc);
        self.prefs.active = Some(user.login.clone());
        self.state = State::SignedIn(Box::new(Session::new(ctx, token)));
        self.push_toast(format!("Signed in as {}", user.login), ToastKind::Success);
    }

    fn sign_out(&mut self, login: &str) {
        let _ = auth::forget_token(login);
        self.prefs.accounts.retain(|a| a.login != login);
        if self.prefs.active.as_deref() == Some(login) {
            self.prefs.active = None;
            self.state = State::SignedOut(views::login::LoginPage::default());
        }
    }

    fn push_toast(&mut self, text: String, kind: ToastKind) {
        self.toasts.push(Toast { text, kind, at: Instant::now() });
    }
}

impl eframe::App for RivetApp {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        if let Some(d) = &mut self.dev {
            d.input(raw);
        }
        self.input_bridge.feed(ctx, raw);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, PREFS_KEY, &self.prefs);
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some((acc, tok)) = self.switch_to.take() {
            self.prefs.active = Some(acc.login.clone());
            self.state = State::SignedIn(Box::new(Session::new(&ctx, tok)));
        }
        crate::titlebar::show(ui);
        crate::titlebar::resize_edges(&ctx);

        match &mut self.state {
            State::SignedOut(login) => {
                let mut out = None;
                egui::CentralPanel::no_frame()
                    .frame(Frame::new().fill(theme::pal(&ctx).canvas_inset))
                    .show(ui, |ui| out = login.ui(ui, &self.prefs.accounts));
                match out {
                    Some(views::login::LoginOutcome::SignedIn(user, tok)) => self.sign_in(&ctx, user, tok),
                    Some(views::login::LoginOutcome::UseSaved(acc)) => match auth::load_token(&acc.login) {
                        Ok(tok) => self.switch_to = Some((acc, tok)),
                        Err(e) => self.push_toast(format!("No saved token for {}: {e}", acc.login), ToastKind::Error),
                    },
                    None => {}
                }
            }
            State::SignedIn(_) => self.session_ui(ui),
        }
        draw_toasts(&ctx, &mut self.toasts);
        crate::titlebar::outline(&ctx);
        if let Some(d) = &mut self.dev {
            d.tick(&ctx);
        }
    }
}

#[derive(Default)]
struct AppAction {
    sign_out: Option<String>,
    switch: Option<Account>,
    add_account: bool,
    theme: Option<ThemePref>,
}

impl RivetApp {
    fn session_ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let State::SignedIn(s) = &mut self.state else { return };
        let pal = theme::pal(&ctx);

        // the token may be revoked or expired; fall back to the login screen
        if let Load::Failed(e) = s.me.poll() {
            let e = e.clone();
            // only a rejected token ends the session; being offline shouldn't sign you out
            if e.contains("401") {
                let login = self.prefs.active.clone().unwrap_or_default();
                self.push_toast(format!("Session for {login} is no longer valid: {e}"), ToastKind::Error);
                self.prefs.active = None;
                self.state = State::SignedOut(views::login::LoginPage::default());
                return;
            }
            let pal = theme::pal(&ctx);
            let mut retry = false;
            egui::CentralPanel::no_frame().frame(Frame::new().fill(pal.canvas)).show(ui, |ui| {
                ui.add_space(ui.available_height() / 3.0);
                ui.vertical_centered(|ui| {
                    w::blankslate(ui, Icon::Globe, "Can't reach GitHub", "Check your connection. Rivet will keep your session.");
                    ui.add(egui::Label::new(RichText::new(&e).size(12.0).color(pal.fg_muted)).wrap());
                    ui.add_space(12.0);
                    if w::button(ui, w::Variant::Primary, Some(Icon::Sync), "Retry").clicked() {
                        retry = true;
                    }
                });
            });
            if retry {
                let g = s.gh.clone();
                s.me = Load::spawn(&ctx, move || g.me());
            }
            return;
        }
        let Some(me) = s.me.ready().cloned() else {
            egui::CentralPanel::no_frame().frame(Frame::new().fill(pal.canvas)).show(ui, |ui| {
                ui.add_space(ui.available_height() / 2.0 - 40.0);
                w::spinner(ui);
            });
            return;
        };
        if !s.warmed {
            s.warmed = true;
            if s.route != Route::Feed {
                s.kept.insert("feed", Box::new(views::dashboard::FeedPage::warm(&ctx, &s.gh, &me.login)));
            }
        }
        if let Some(acc) = self.prefs.accounts.iter_mut().find(|a| a.login == me.login) {
            acc.avatar_url = me.avatar_url.clone();
        }

        // keep the inbox dot fresh without hammering the api
        if s.last_unread_poll.elapsed() > Duration::from_secs(60) && !s.unread.is_loading() {
            let g = s.gh.clone();
            s.unread = Load::spawn(&ctx, move || Ok(!g.notifications(false, false, 1)?.is_empty()));
            s.last_unread_poll = Instant::now();
        }
        ctx.request_repaint_after(Duration::from_secs(60));

        let mut nav: Option<Route> = None;
        let mut replace = false;
        let mut action = AppAction::default();
        handle_shortcuts(&ctx, s, &mut nav);

        let has_unread = matches!(s.unread.poll(), Load::Ready(true));
        egui::Panel::top("header")
            .frame(Frame::new().fill(pal.header_bg).inner_margin(Margin::symmetric(16, 16)))
            .show_separator_line(!matches!(s.route, Route::Repo(_)))
            .show(ui, |ui| header(ui, s, &me, &self.prefs, has_unread, &mut nav, &mut action));

        if s.drawer_open {
            drawer(&ctx, s, &me, &mut nav);
        }

        let my_repos: Vec<Repo> = s.my_repos.ready().cloned().unwrap_or_default();
        let mut refresh_unread = false;
        egui::CentralPanel::no_frame().frame(Frame::new().fill(pal.canvas)).show(ui, |ui| {
            let mut cx = Cx {
                gh: &s.gh,
                me: &me,
                prefs: &mut self.prefs,
                my_repos: &my_repos,
                nav: &mut nav,
                replace: &mut replace,
                toasts: &mut self.toasts,
                refresh_unread: &mut refresh_unread,
            };
            s.page.ui(ui, &mut cx);
        });
        if refresh_unread {
            s.last_unread_poll = Instant::now() - Duration::from_secs(3600);
        }

        if let Some(pal_state) = &mut s.palette {
            let mut close = false;
            command_palette(&ctx, pal_state, &my_repos, &self.prefs, &mut nav, &mut close, &mut action);
            if close {
                s.palette = None;
            }
        }

        if let Some(r) = nav {
            if let Route::Repo(rr) = &r {
                self.prefs.remember_repo(&format!("{}/{}", rr.owner, rr.name));
            }
            if let Route::Local(Some(p)) = &r {
                self.prefs.remember_local(p.clone());
            }
            s.navigate(r, !replace);
            s.palette = None;
        }
        // every viewport command schedules another frame, so only send the title when it changes
        let title = format!("{} · Rivet", s.route.title());
        if title != s.window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            s.window_title = title;
        }

        if let Some(t) = action.theme {
            self.prefs.theme = t;
            theme::apply_pref(&ctx, t);
        }
        if let Some(login) = action.sign_out {
            self.sign_out(&login);
        }
        if let Some(acc) = action.switch {
            match auth::load_token(&acc.login) {
                Ok(tok) => self.switch_to = Some((acc, tok)),
                Err(e) => self.push_toast(format!("No saved token for {}: {e}", acc.login), ToastKind::Error),
            }
        }
        if action.add_account {
            self.state = State::SignedOut(views::login::LoginPage::default());
        }
    }
}

fn handle_shortcuts(ctx: &egui::Context, s: &mut Session, nav: &mut Option<Route>) {
    let typing = ctx.egui_wants_keyboard_input();
    let palette_sc = KeyboardShortcut::new(Modifiers::COMMAND, Key::K);
    let palette_sc2 = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::P);
    if ctx.input_mut(|i| i.consume_shortcut(&palette_sc) || i.consume_shortcut(&palette_sc2)) {
        s.palette = if s.palette.is_some() { None } else { Some(CommandPalette::default()) };
    }
    // mouse back / forward side buttons, like a browser
    let (side_back, side_fwd) =
        ctx.input(|i| (i.pointer.button_pressed(egui::PointerButton::Extra1), i.pointer.button_pressed(egui::PointerButton::Extra2)));
    if side_back {
        s.go_back();
    }
    if side_fwd {
        s.go_forward();
    }
    if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::ALT, Key::ArrowLeft))) {
        s.go_back();
    }
    if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::ALT, Key::ArrowRight))) {
        s.go_forward();
    }
    if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::R))) || ctx.input(|i| i.key_pressed(Key::F5)) {
        s.page = views::make(&s.route);
    }
    if typing || s.palette.is_some() {
        return;
    }
    // github style single key and "g x" chords
    let text: Vec<char> =
        ctx.input(|i| i.events.iter().filter_map(|e| if let egui::Event::Text(t) = e { t.chars().next() } else { None }).collect());
    for c in text {
        if let Some((prev, at)) = s.chord.take()
            && prev == 'g'
            && at.elapsed() < Duration::from_millis(1500)
        {
            let repo = if let Route::Repo(r) = &s.route { Some((r.owner.clone(), r.name.clone())) } else { None };
            *nav = match (c, repo) {
                ('d', _) => Some(Route::Dashboard),
                ('n', _) => Some(Route::Notifications),
                ('c', Some((o, n))) => Some(Route::repo_home(&o, &n)),
                ('i', Some((o, n))) => Some(Route::repo(&o, &n, RepoView::Issues { q: None })),
                ('p', Some((o, n))) => Some(Route::repo(&o, &n, RepoView::Pulls { q: None })),
                ('a', Some((o, n))) => Some(Route::repo(&o, &n, RepoView::Actions { workflow: None })),
                ('i', None) => Some(Route::MyItems { pulls: false }),
                ('p', None) => Some(Route::MyItems { pulls: true }),
                _ => None,
            };
            continue;
        }
        match c {
            '/' => s.focus_search = true,
            'g' => s.chord = Some(('g', Instant::now())),
            _ => {}
        }
    }
}

fn header(ui: &mut Ui, s: &mut Session, me: &User, prefs: &Prefs, unread: bool, nav: &mut Option<Route>, action: &mut AppAction) {
    let pal = w::p(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if header_icon(ui, Icon::ThreeBars, "Open global navigation menu").clicked() {
            s.drawer_open = !s.drawer_open;
        }
        // no back / forward buttons here: github's header has none, history lives
        // on Alt+← / Alt+→ and the mouse side buttons
        let mark = ui
            .add(Icon::MarkGithub.image(32.0, pal.fg).sense(Sense::click()))
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Dashboard");
        if mark.clicked() {
            *nav = Some(Route::Dashboard);
        }
        ui.add_space(4.0);
        // context breadcrumb, same as the github header
        match &s.route {
            Route::Repo(r) => {
                let (o, n) = (r.owner.clone(), r.name.clone());
                if crumb(ui, &o, false).clicked() {
                    *nav = Some(Route::User(o.clone()));
                }
                ui.label(RichText::new("/").size(14.0).color(pal.fg_muted));
                if crumb(ui, &n, true).clicked() {
                    *nav = Some(Route::repo_home(&o, &n));
                }
            }
            Route::User(u) => {
                crumb(ui, u, true);
            }
            other => {
                crumb(ui, &other.title(), true);
            }
        }

        w::right(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let av = w::avatar(ui, &me.avatar_url, 32.0).on_hover_text("Open user navigation menu");
            egui::Popup::menu(&av).width(300.0).align(egui::RectAlign::BOTTOM_END).show(|ui| user_menu(ui, me, prefs, nav, action));

            let inbox =
                header_icon(ui, Icon::Inbox, if unread { "You have unread notifications" } else { "You have no unread notifications" });
            if unread {
                let c = inbox.rect.right_top() + Vec2::new(-4.0, 4.0);
                ui.painter().circle(c, 4.5, pal.accent_emphasis, Stroke::new(2.0, pal.header_bg));
            }
            if inbox.clicked() {
                *nav = Some(Route::Notifications);
            }
            if header_icon(ui, Icon::Repo, "Your repositories").clicked() {
                *nav = Some(Route::User(me.login.clone()));
            }
            if header_icon(ui, Icon::GitPullRequest, "Your pull requests").clicked() {
                *nav = Some(Route::MyItems { pulls: true });
            }
            if header_icon(ui, Icon::IssueOpened, "Your issues").clicked() {
                *nav = Some(Route::MyItems { pulls: false });
            }
            let plus = create_button(ui).on_hover_text("Create something new");
            egui::Popup::menu(&plus).align(egui::RectAlign::BOTTOM_END).width(220.0).show(|ui| {
                if w::menu_item(ui, Some(Icon::Repo), "New repository").clicked() {
                    *nav = Some(Route::NewRepo);
                }
                if crate::platform::LOCAL_GIT {
                    if w::menu_item(ui, Some(Icon::DesktopDownload), "Clone repository").clicked() {
                        *nav = Some(Route::Clone { url: String::new() });
                    }
                    if w::menu_item(ui, Some(Icon::FileDirectory), "Open local repository").clicked() {
                        *nav = Some(Route::Local(None));
                    }
                }
                if let Route::Repo(r) = &s.route {
                    ui.separator();
                    if w::menu_item(ui, Some(Icon::IssueOpened), "New issue").clicked() {
                        *nav = Some(Route::repo(&r.owner, &r.name, RepoView::NewIssue));
                    }
                    if w::menu_item(ui, Some(Icon::GitPullRequest), "New pull request").clicked() {
                        *nav = Some(Route::repo(&r.owner, &r.name, RepoView::NewPull { base: None, head: None }));
                    }
                }
            });
            header_divider(ui);
            // search box, "Type [/] to search" with the command palette button inside
            let width = (ui.available_width() - 16.0).clamp(200.0, 350.0);
            let scoped = matches!(s.route, Route::Repo(_));
            let (resp, open_palette) = search_box(ui, &mut s.search_text, width, scoped);
            if open_palette {
                s.palette = Some(CommandPalette::default());
            }
            if s.focus_search {
                resp.request_focus();
                s.focus_search = false;
            }
            if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) && !s.search_text.trim().is_empty() {
                let mut q = s.search_text.trim().to_string();
                if let Route::Repo(r) = &s.route
                    && !q.contains("repo:")
                {
                    q = format!("repo:{}/{} {q}", r.owner, r.name);
                }
                *nav = Some(Route::Search { q, kind: SearchKind::Repositories });
            }
        });
    });
}

fn crumb(ui: &mut Ui, text: &str, strong: bool) -> egui::Response {
    let pal = w::p(ui);
    let font = if strong { semibold(14.0) } else { egui::FontId::proportional(14.0) };
    let id = ui.next_auto_id();
    let hovered = ui.ctx().read_response(id).is_some_and(|r| r.hovered());
    let r = Frame::new()
        .fill(if hovered { pal.control_hover } else { egui::Color32::TRANSPARENT })
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(6, 4))
        .show(ui, |ui| ui.label(RichText::new(text).font(font).color(pal.fg)))
        .response;
    ui.interact(r.rect, id, Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// GitHub's header search: magnifier, "Type [/] to search" placeholder with the
/// slash in a key cap, and the command palette button at the right edge.
fn search_box(ui: &mut Ui, text: &mut String, width: f32, scoped: bool) -> (egui::Response, bool) {
    let pal = w::p(ui);
    let id = ui.make_persistent_id("global-search");
    let focused = ui.ctx().memory(|m| m.has_focus(id));
    let stroke = if focused { Stroke::new(2.0, pal.accent_emphasis) } else { Stroke::new(1.0, pal.border) };
    let mut palette = false;
    let resp = Frame::new()
        .fill(pal.canvas)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin { left: 8, right: 2, top: 0, bottom: 0 })
        .show(ui, |ui| {
            ui.set_width(width - 10.0 - stroke.width * 2.0);
            ui.set_height(30.0);
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                icon(ui, Icon::Search, pal.fg_muted);
                let edit_w = ui.available_width() - 34.0;
                let r = ui.add_sized(
                    Vec2::new(edit_w, 28.0),
                    egui::TextEdit::singleline(text).id(id).frame(Frame::NONE).align(egui::Align2::LEFT_CENTER).desired_width(edit_w),
                );
                if text.is_empty() && !focused {
                    hint_with_key(ui, r.rect, scoped);
                }
                let pb = ui
                    .add(egui::Button::image(Icon::CommandPalette.image(16.0, pal.fg_muted)).frame(false).min_size(Vec2::splat(28.0)))
                    .on_hover_text("Command palette (Ctrl+K)");
                if pb.clicked() {
                    palette = true;
                }
                r
            })
            .inner
        })
        .inner;
    (resp, palette)
}

fn hint_with_key(ui: &Ui, rect: egui::Rect, scoped: bool) {
    let pal = w::p(ui);
    let painter = ui.painter();
    let font = egui::FontId::proportional(14.0);
    let mut x = rect.left() + 2.0;
    let y = rect.center().y;
    let before = painter.layout_no_wrap("Type".into(), font.clone(), pal.fg_muted);
    painter.galley(egui::pos2(x, y - before.size().y / 2.0), before.clone(), pal.fg_muted);
    x += before.size().x + 4.0;
    let key = painter.layout_no_wrap("/".into(), egui::FontId::proportional(12.0), pal.fg_muted);
    let cap = egui::Rect::from_min_size(egui::pos2(x, y - 9.0), Vec2::new(key.size().x + 10.0, 18.0));
    painter.rect(cap, CornerRadius::same(4), egui::Color32::TRANSPARENT, Stroke::new(1.0, pal.border), egui::StrokeKind::Inside);
    painter.galley(cap.center() - key.size() / 2.0, key, pal.fg_muted);
    x = cap.right() + 4.0;
    let after = if scoped { "to search this repository" } else { "to search" };
    let g = painter.layout_no_wrap(after.into(), font, pal.fg_muted);
    painter.galley(egui::pos2(x, y - g.size().y / 2.0), g, pal.fg_muted);
}

/// Header icon buttons are outlined with a transparent fill, unlike regular buttons.
fn header_icon(ui: &mut Ui, ic: Icon, tip: &str) -> egui::Response {
    let pal = w::p(ui);
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
    let fill = if resp.is_pointer_button_down_on() {
        pal.btn_active
    } else if resp.hovered() {
        pal.btn_hover
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter().rect(rect, CornerRadius::same(6), fill, Stroke::new(1.0, pal.border), egui::StrokeKind::Inside);
    ic.image(16.0, pal.fg_muted).paint_at(ui, egui::Rect::from_center_size(rect.center(), Vec2::splat(16.0)));
    resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The bordered "+ ▾" create menu button.
fn create_button(ui: &mut Ui) -> egui::Response {
    let pal = w::p(ui);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(48.0, 32.0), Sense::click());
    let fill = if resp.is_pointer_button_down_on() {
        pal.btn_active
    } else if resp.hovered() {
        pal.btn_hover
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter().rect(rect, CornerRadius::same(6), fill, Stroke::new(1.0, pal.border), egui::StrokeKind::Inside);
    let plus = egui::Rect::from_min_size(egui::pos2(rect.left() + 8.0, rect.center().y - 8.0), Vec2::splat(16.0));
    Icon::Plus.image(16.0, pal.fg_muted).paint_at(ui, plus);
    let tri = egui::Rect::from_min_size(egui::pos2(rect.right() - 22.0, rect.center().y - 8.0), Vec2::splat(16.0));
    Icon::TriangleDown.image(16.0, pal.fg_muted).paint_at(ui, tri);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The thin vertical rule github puts between header groups.
fn header_divider(ui: &mut Ui) {
    let pal = w::p(ui);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 20.0), Sense::hover());
    ui.painter().vline(rect.center().x, rect.y_range(), Stroke::new(1.0, pal.border));
}

fn user_menu(ui: &mut Ui, me: &User, prefs: &Prefs, nav: &mut Option<Route>, action: &mut AppAction) {
    let pal = w::p(ui);
    ui.horizontal(|ui| {
        w::avatar(ui, &me.avatar_url, 32.0);
        ui.vertical(|ui| {
            ui.label(RichText::new(&me.login).font(semibold(14.0)));
            if let Some(n) = &me.name {
                w::muted_small(ui, n);
            }
        });
    });
    ui.separator();
    if w::menu_item(ui, Some(Icon::Person), "Profile").clicked() {
        *nav = Some(Route::User(me.login.clone()));
    }
    if w::menu_item(ui, Some(Icon::Repo), "Repositories").clicked() {
        *nav = Some(Route::User(me.login.clone()));
    }
    if w::menu_item(ui, Some(Icon::Star), "Stars").clicked() {
        *nav = Some(Route::Search { q: format!("user:{} stars:>0", me.login), kind: SearchKind::Repositories });
    }
    if crate::platform::LOCAL_GIT && w::menu_item(ui, Some(Icon::FileDirectory), "Local repositories").clicked() {
        *nav = Some(Route::Local(None));
    }
    ui.separator();
    if w::menu_item(ui, Some(Icon::Gear), "Settings").clicked() {
        *nav = Some(Route::Settings);
    }
    ui.separator();
    if prefs.accounts.len() > 1 {
        ui.label(RichText::new("Switch account").size(12.0).font(semibold(12.0)).color(pal.fg_muted));
        for acc in prefs.accounts.iter().filter(|a| a.login != me.login) {
            if w::menu_item(ui, Some(Icon::Person), &acc.login).clicked() {
                action.switch = Some(acc.clone());
            }
        }
    }
    if w::menu_item(ui, Some(Icon::PlusCircle), "Add account").clicked() {
        action.add_account = true;
    }
    ui.separator();
    if w::menu_item_ex(ui, Some(Icon::SignOut), "Sign out", false, true).clicked() {
        action.sign_out = Some(me.login.clone());
    }
}

fn drawer(ctx: &egui::Context, s: &mut Session, me: &User, nav: &mut Option<Route>) {
    let pal = theme::pal(ctx);
    let modal = egui::Modal::new(egui::Id::unique("global-drawer"))
        .backdrop_color(pal.overlay_shadow)
        .area(
            egui::Area::new(egui::Id::unique("global-drawer-area"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(0.0, crate::titlebar::HEIGHT)),
        )
        .frame(
            Frame::new()
                .fill(pal.canvas)
                .stroke(Stroke::new(1.0, pal.border))
                .corner_radius(CornerRadius { nw: 0, sw: 0, ne: 12, se: 12 })
                .inner_margin(Margin::same(16)),
        )
        .show(ctx, |ui| {
            ui.set_width(304.0);
            ui.set_min_height(ctx.content_rect().height() - 34.0 - crate::titlebar::HEIGHT);
            ui.horizontal(|ui| {
                ui.add(Icon::MarkGithub.image(32.0, pal.fg));
                w::right(ui, |ui| {
                    if w::icon_button_invisible(ui, Icon::X, "Close").clicked() {
                        s.drawer_open = false;
                    }
                });
            });
            ui.add_space(8.0);
            let items: [(Icon, &str, Route); 7] = [
                (Icon::Home, "Home", Route::Dashboard),
                (Icon::IssueOpened, "Issues", Route::MyItems { pulls: false }),
                (Icon::GitPullRequest, "Pull requests", Route::MyItems { pulls: true }),
                (Icon::Inbox, "Notifications", Route::Notifications),
                (Icon::Search, "Search", Route::Search { q: String::new(), kind: SearchKind::Repositories }),
                (Icon::FileDirectory, "Local repositories", Route::Local(None)),
                (Icon::Person, "Your profile", Route::User(me.login.clone())),
            ];
            for (ic, label, r) in items {
                if r.needs_local_git() && !crate::platform::LOCAL_GIT {
                    continue;
                }
                if w::menu_item(ui, Some(ic), label).clicked() {
                    *nav = Some(r);
                }
            }
            ui.separator();
            ui.label(RichText::new("Repositories").size(12.0).font(semibold(12.0)).color(pal.fg_muted));
            egui::ScrollArea::vertical().max_height(ui.available_height() - 8.0).show(ui, |ui| {
                if let Some(repos) = s.my_repos.ready() {
                    for r in repos.iter().take(25) {
                        let resp = ui
                            .horizontal(|ui| {
                                w::avatar(ui, &r.owner.avatar_url, 16.0);
                                w::plain_link(ui, &r.full_name)
                            })
                            .inner;
                        if resp.clicked() {
                            *nav = Some(Route::repo_home(&r.owner.login, &r.name));
                        }
                    }
                }
            });
        });
    if modal.should_close() {
        s.drawer_open = false;
    }
}

#[derive(Default)]
struct CommandPalette {
    query: String,
    selected: usize,
    focused: bool,
}

struct Command {
    icon: Icon,
    label: String,
    hint: &'static str,
    run: CommandRun,
}

enum CommandRun {
    Go(Route),
    Theme(ThemePref),
}

fn palette_commands(q: &str, repos: &[Repo], prefs: &Prefs) -> Vec<Command> {
    let mut out = vec![];
    let ql = q.to_lowercase();
    let matches = |s: &str| ql.is_empty() || s.to_lowercase().contains(&ql);

    // a pasted github.com link opens the matching screen
    if let Some(r) = Route::from_github_url(q.trim()) {
        out.push(Command { icon: Icon::LinkExternal, label: q.trim().to_string(), hint: "Open link", run: CommandRun::Go(r) });
    }
    // "owner/name" typed directly jumps to that repo, even if we've never seen it
    if let Some(r) = Route::from_slug(q) {
        out.push(Command { icon: Icon::Repo, label: q.trim().to_string(), hint: "Open repository", run: CommandRun::Go(r) });
    }
    let pages: [(Icon, &str, Route); 9] = [
        (Icon::Home, "Dashboard", Route::Dashboard),
        (Icon::Inbox, "Notifications", Route::Notifications),
        (Icon::IssueOpened, "Your issues", Route::MyItems { pulls: false }),
        (Icon::GitPullRequest, "Your pull requests", Route::MyItems { pulls: true }),
        (Icon::Repo, "New repository", Route::NewRepo),
        (Icon::DesktopDownload, "Clone repository", Route::Clone { url: String::new() }),
        (Icon::FileDirectory, "Open local repository", Route::Local(None)),
        (Icon::Gear, "Settings", Route::Settings),
        (Icon::Search, "Search GitHub", Route::Search { q: q.to_string(), kind: SearchKind::Repositories }),
    ];
    for (ic, label, r) in pages {
        if r.needs_local_git() && !crate::platform::LOCAL_GIT {
            continue;
        }
        if matches(label) || label == "Search GitHub" && !q.is_empty() {
            out.push(Command { icon: ic, label: label.into(), hint: "Jump to", run: CommandRun::Go(r) });
        }
    }
    for (t, label) in
        [(ThemePref::Light, "Switch theme: Light"), (ThemePref::Dark, "Switch theme: Dark"), (ThemePref::System, "Switch theme: System")]
    {
        if !ql.is_empty() && matches(label) {
            out.push(Command { icon: Icon::Sun, label: label.into(), hint: "Command", run: CommandRun::Theme(t) });
        }
    }
    let mut seen = std::collections::HashSet::new();
    for slug in prefs.recent_repos.iter().chain(repos.iter().map(|r| &r.full_name)) {
        if matches(slug)
            && seen.insert(slug.clone())
            && let Some(r) = Route::from_slug(slug)
        {
            out.push(Command { icon: Icon::Repo, label: slug.clone(), hint: "Repository", run: CommandRun::Go(r) });
        }
    }
    for p in prefs.recent_local.iter().filter(|_| crate::platform::LOCAL_GIT) {
        let s = p.display().to_string();
        if matches(&s) {
            out.push(Command { icon: Icon::FileDirectory, label: s, hint: "Local", run: CommandRun::Go(Route::Local(Some(p.clone()))) });
        }
    }
    out.truncate(40);
    out
}

fn command_palette(
    ctx: &egui::Context,
    st: &mut CommandPalette,
    repos: &[Repo],
    prefs: &Prefs,
    nav: &mut Option<Route>,
    close: &mut bool,
    action: &mut AppAction,
) {
    let pal = theme::pal(ctx);
    let cmds = palette_commands(&st.query, repos, prefs);
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
        st.selected = (st.selected + 1).min(cmds.len().saturating_sub(1));
    }
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
        st.selected = st.selected.saturating_sub(1);
    }
    let mut run: Option<usize> = None;
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) && !cmds.is_empty() {
        run = Some(st.selected.min(cmds.len() - 1));
    }
    let width = 640.0_f32.min(ctx.content_rect().width() - 32.0);
    let modal = egui::Modal::new(egui::Id::unique("command-palette"))
        .backdrop_color(pal.overlay_shadow)
        .area(
            egui::Area::new(egui::Id::unique("command-palette-area"))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 80.0)),
        )
        .frame(Frame::new().fill(pal.canvas).stroke(Stroke::new(1.0, pal.border)).corner_radius(CornerRadius::same(12)))
        .show(ctx, |ui| {
            ui.set_width(width);
            Frame::new().inner_margin(Margin::symmetric(16, 12)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    icon(ui, Icon::Search, pal.fg_muted);
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut st.query)
                            .frame(Frame::NONE)
                            .hint_text(RichText::new("Search or jump to… (try owner/repo)").color(pal.fg_muted))
                            .desired_width(f32::INFINITY),
                    );
                    if !st.focused {
                        r.request_focus();
                        st.focused = true;
                    }
                    if r.changed() {
                        st.selected = 0;
                    }
                });
            });
            w::divider(ui);
            egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                Frame::new().inner_margin(Margin::same(8)).show(ui, |ui| {
                    if cmds.is_empty() {
                        w::muted(ui, "No results");
                    }
                    for (i, c) in cmds.iter().enumerate() {
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::click());
                        if resp.hovered() {
                            st.selected = i;
                        }
                        if i == st.selected {
                            ui.painter().rect_filled(rect, CornerRadius::same(6), pal.control_hover);
                            resp.scroll_to_me(None);
                        }
                        let mut child = ui.new_child(
                            egui::UiBuilder::new().max_rect(rect.shrink2(Vec2::new(8.0, 0.0))).layout(Layout::left_to_right(Align::Center)),
                        );
                        icon(&mut child, c.icon, pal.fg_muted);
                        child.label(&c.label);
                        child.with_layout(Layout::right_to_left(Align::Center), |ui| w::muted_small(ui, c.hint));
                        if resp.clicked() {
                            run = Some(i);
                        }
                    }
                });
            });
        });
    if modal.should_close() {
        *close = true;
    }
    if let Some(i) = run
        && let Some(c) = cmds.into_iter().nth(i)
    {
        match c.run {
            CommandRun::Go(r) => *nav = Some(r),
            CommandRun::Theme(t) => action.theme = Some(t),
        }
        *close = true;
    }
}

fn draw_toasts(ctx: &egui::Context, toasts: &mut Vec<Toast>) {
    toasts.retain(|t| t.at.elapsed() < Duration::from_secs(if t.kind == ToastKind::Error { 8 } else { 4 }));
    if toasts.is_empty() {
        return;
    }
    ctx.request_repaint_after(Duration::from_millis(250));
    let pal = theme::pal(ctx);
    egui::Area::new(egui::Id::unique("toasts")).order(egui::Order::Tooltip).anchor(egui::Align2::LEFT_BOTTOM, Vec2::new(16.0, -16.0)).show(
        ctx,
        |ui| {
            for t in toasts.iter() {
                let (bg, ic) = match t.kind {
                    ToastKind::Success => (pal.success_emphasis, Icon::Check),
                    ToastKind::Error => (pal.danger_emphasis, Icon::Stop),
                    ToastKind::Info => (pal.accent_emphasis, Icon::Info),
                };
                Frame::new()
                    .fill(pal.canvas)
                    .stroke(Stroke::new(1.0, pal.border))
                    .corner_radius(CornerRadius::same(6))
                    .shadow(egui::Shadow { offset: [0, 4], blur: 12, spread: 0, color: pal.overlay_shadow })
                    .show(ui, |ui| {
                        ui.set_max_width(420.0);
                        ui.horizontal(|ui| {
                            Frame::new()
                                .fill(bg)
                                .corner_radius(CornerRadius { nw: 6, sw: 6, ne: 0, se: 0 })
                                .inner_margin(Margin::symmetric(12, 12))
                                .show(ui, |ui| icon(ui, ic, pal.fg_on_emphasis));
                            Frame::new().inner_margin(Margin::symmetric(4, 8)).show(ui, |ui| {
                                ui.add(egui::Label::new(&t.text).wrap());
                            });
                        });
                    });
                ui.add_space(8.0);
            }
        },
    );
}
