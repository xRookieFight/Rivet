//! The Feed tab in GitHub's card style. Like github.com's feed it only shows
//! the "discovery" events: stars, forks, new or newly public repositories and
//! releases. Pushes, reviews, comments and labels stay out; that's what the
//! Home lists and notifications are for.
//!
//! Stars, forks and new repositories only count from people you follow (the
//! events API also returns activity on repos you merely watch, github's feed
//! doesn't show that), and releases skip drafts and pre-releases.
//!
//! Several people starring or forking the same repository fold into one card.
//! Repository card details (description, language, stars) come from one
//! batched GraphQL lookup since the events API doesn't carry them.

use crate::app::Cx;
use crate::github::Client;
use crate::github::models::{Event, RepoSummary, User};
use crate::icons::{Icon, icon};
use crate::route::{RepoView, Route};
use crate::task::Load;
use crate::theme::semibold;
use crate::widgets::{self as w, Variant};
use egui::{CornerRadius, Frame, Margin, RichText, Stroke, Ui, Vec2};
use std::collections::{HashMap, HashSet};

pub struct Feed {
    events: Load<Vec<Event>>,
    following: Load<HashSet<String>>,
    page: u32,
    more: Load<Vec<Event>>,
    repos: HashMap<String, RepoSummary>,
    requested: HashSet<String>,
    lookup: Load<HashMap<String, RepoSummary>>,
    stars: HashMap<String, bool>,
    star_action: Load<(String, bool)>,
}

pub const CATEGORIES: [(&str, &str); 3] = [("stars", "Stars"), ("repos", "Repositories"), ("releases", "Releases")];

/// Feed category of an event, or None for events the feed doesn't show.
fn category(e: &Event) -> Option<&'static str> {
    match e.kind.as_str() {
        "WatchEvent" => Some("stars"),
        "ForkEvent" | "PublicEvent" => Some("repos"),
        "CreateEvent" if e.payload["ref_type"] == "repository" => Some("repos"),
        "ReleaseEvent"
            if e.payload["action"] != "deleted"
                && e.payload["release"]["prerelease"].as_bool() != Some(true)
                && e.payload["release"]["draft"].as_bool() != Some(true) =>
        {
            Some("releases")
        }
        _ => None,
    }
}

/// One card: the newest event plus whoever else did the same thing to the same repo.
struct Entry<'a> {
    event: &'a Event,
    others: Vec<&'a User>,
}

/// Folds stars and forks of the same repository into the newest one.
fn group<'a>(events: &'a [Event], hidden: &[String], following: &HashSet<String>, me: &str) -> Vec<Entry<'a>> {
    let mut out: Vec<Entry> = vec![];
    let mut index: HashMap<(String, String), usize> = HashMap::new();
    for e in events {
        let Some(cat) = category(e) else { continue };
        if hidden.iter().any(|h| h == cat) {
            continue;
        }
        // people-driven events only from people you follow; releases come from watched repos
        if cat != "releases" && (e.actor.login == me || !following.contains(&e.actor.login)) {
            continue;
        }
        let foldable = matches!(e.kind.as_str(), "WatchEvent" | "ForkEvent");
        let key = (e.kind.clone(), e.repo.name.clone());
        if foldable && let Some(&i) = index.get(&key) {
            let entry: &mut Entry = &mut out[i];
            if entry.event.actor.login != e.actor.login && !entry.others.iter().any(|u| u.login == e.actor.login) {
                entry.others.push(&e.actor);
            }
            continue;
        }
        if foldable {
            index.insert(key, out.len());
        }
        out.push(Entry { event: e, others: vec![] });
    }
    out
}

fn split(slug: &str) -> (&str, &str) {
    slug.split_once('/').unwrap_or(("", ""))
}

impl Default for Feed {
    fn default() -> Self {
        Self {
            events: Load::Idle,
            following: Load::Idle,
            page: 1,
            more: Load::Idle,
            repos: HashMap::new(),
            requested: HashSet::new(),
            lookup: Load::Idle,
            stars: HashMap::new(),
            star_action: Load::Idle,
        }
    }
}

impl Feed {
    /// Starts the event and following requests so the tab opens warm.
    pub fn prefetch(&mut self, ctx: &egui::Context, gh: &Client, login: &str) {
        if self.events.is_idle() {
            let (g, l) = (gh.clone(), login.to_string());
            self.events = Load::spawn(ctx, move || g.received_events(&l, 1));
        }
        if self.following.is_idle() {
            let g = gh.clone();
            self.following = Load::spawn(ctx, move || g.following());
        }
    }

    /// Queues card lookups for repos we haven't asked about yet. One job at a time.
    fn fetch_missing(&mut self, ctx: &egui::Context, gh: &Client) {
        if self.lookup.is_loading() {
            return;
        }
        let Load::Ready(events) = &self.events else { return };
        let mut slugs: Vec<String> = events
            .iter()
            .filter(|e| matches!(category(e), Some("stars" | "repos")))
            .map(|e| e.repo.name.clone())
            .filter(|s| !self.requested.contains(s))
            .collect();
        slugs.sort();
        slugs.dedup();
        if slugs.is_empty() {
            return;
        }
        self.requested.extend(slugs.iter().cloned());
        let g = gh.clone();
        self.lookup = Load::spawn(ctx, move || g.repo_summaries(&slugs));
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        let ctx = ui.ctx().clone();
        self.prefetch(&ctx, cx.gh, &cx.me.login);
        if let Some(Ok(more)) = self.more.take()
            && let Load::Ready(list) = &mut self.events
        {
            list.extend(more);
        }
        // a failed lookup just leaves repo cards without details
        if let Some(Ok(repos)) = self.lookup.take() {
            self.repos.extend(repos);
        }
        match self.star_action.take() {
            Some(Ok((slug, on))) => {
                self.stars.insert(slug.clone(), on);
                if let Some(r) = self.repos.get_mut(&slug) {
                    r.stars = if on { r.stars + 1 } else { r.stars.saturating_sub(1) };
                }
            }
            Some(Err(e)) => cx.err(e),
            None => {}
        }
        self.fetch_missing(&ctx, cx.gh);

        ui.horizontal(|ui| {
            ui.label(RichText::new("Feed").size(20.0).font(semibold(20.0)));
            w::right(ui, |ui| {
                let b = w::button_ex(ui, Variant::Default, Some(Icon::Filter), "Filter", None, true);
                egui::Popup::menu(&b)
                    .width(240.0)
                    .align(egui::RectAlign::BOTTOM_END)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        ui.label(RichText::new("Events").font(semibold(12.0)).color(w::p(ui).fg_muted));
                        for (key, label) in CATEGORIES {
                            let mut on = !cx.prefs.feed_hidden.iter().any(|k| k == key);
                            if ui.checkbox(&mut on, label).changed() {
                                if on {
                                    cx.prefs.feed_hidden.retain(|k| k != key);
                                } else {
                                    cx.prefs.feed_hidden.push(key.to_string());
                                }
                            }
                        }
                    });
            });
        });
        ui.add_space(16.0);

        // without the following list we can't tell feed events from watch noise
        let following = match self.following.poll() {
            Load::Ready(f) => f.clone(),
            Load::Failed(_) => HashSet::new(),
            _ => {
                w::spinner(ui);
                return;
            }
        };
        let mut events = match self.events.poll() {
            Load::Ready(v) => v.clone(),
            Load::Failed(e) => {
                w::flash(ui, w::FlashKind::Danger, &e.clone());
                return;
            }
            _ => {
                w::spinner(ui);
                return;
            }
        };
        // received_events isn't strictly ordered, the feed is newest first
        events.sort_by_key(|e| std::cmp::Reverse(e.created_at));
        let hidden = cx.prefs.feed_hidden.clone();
        let entries = group(&events, &hidden, &following, &cx.me.login);
        let mut star_click: Option<(String, bool)> = None;
        if entries.is_empty() {
            w::bordered(ui, |ui| {
                w::blankslate(
                    ui,
                    Icon::Rss,
                    "Nothing new yet",
                    "Stars, forks, new repositories and releases from people you follow show up here.",
                )
            });
        }
        for entry in &entries {
            // the same repo can appear in several events, so scope ids per event
            if let Some(s) = ui.push_id(&entry.event.id, |ui| self.item(ui, entry, cx)).inner {
                star_click = Some(s);
            }
            ui.add_space(20.0);
        }

        // github serves up to 300 events (3 pages of 100)
        let mut load_more = false;
        if events.len() >= 100 * self.page as usize && self.page < 3 {
            ui.vertical_centered_justified(|ui| {
                let busy = self.more.is_loading();
                if w::button_ex(ui, Variant::Default, None, if busy { "Loading…" } else { "More" }, None, !busy).clicked() {
                    load_more = true;
                }
            });
        }
        if load_more {
            self.page += 1;
            let (g, l, p) = (cx.gh.clone(), cx.me.login.clone(), self.page);
            self.more = Load::spawn(&ctx, move || g.received_events(&l, p));
        }
        if let Some((slug, on)) = star_click {
            let g = cx.gh.clone();
            self.star_action = Load::spawn(&ctx, move || {
                let (o, n) = split(&slug);
                g.set_starred(o, n, on)?;
                Ok((slug, on))
            });
        }
    }

    /// One feed entry. Returns a star toggle request when the card's button was hit.
    fn item(&self, ui: &mut Ui, entry: &Entry, cx: &mut Cx) -> Option<(String, bool)> {
        let pal = w::p(ui);
        let e = entry.event;
        let verb = match e.kind.as_str() {
            "WatchEvent" => "starred a repository",
            "ForkEvent" => "forked a repository",
            "PublicEvent" => "made a repository public",
            "CreateEvent" => "created a repository",
            _ => "published a release",
        };
        let mut star = None;
        ui.horizontal_top(|ui| {
            if w::avatar(ui, &e.actor.avatar_url, 32.0).clicked() {
                cx.go(Route::User(e.actor.login.clone()));
            }
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    let head_w = (ui.available_width() - 90.0).max(120.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(head_w, 20.0),
                        egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(true),
                        |ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            if w::title_link(ui, &e.actor.display_name(), 14.0).clicked() {
                                cx.go(Route::User(e.actor.login.clone()));
                            }
                            match entry.others.as_slice() {
                                [] => {}
                                [one] => {
                                    ui.label(RichText::new("and").color(pal.fg_muted));
                                    if w::title_link(ui, &one.display_name(), 14.0).clicked() {
                                        cx.go(Route::User(one.login.clone()));
                                    }
                                }
                                many => {
                                    let names: Vec<String> = many.iter().map(|u| u.display_name()).collect();
                                    ui.label(RichText::new(format!("and {} others", many.len())).font(semibold(14.0)))
                                        .on_hover_text(names.join(", "));
                                }
                            }
                            ui.label(RichText::new(verb).color(pal.fg_muted));
                        },
                    );
                    w::right(ui, |ui| {
                        if let Some(t) = &e.created_at {
                            w::muted_small(ui, w::rel_time(t));
                        }
                    });
                });
                ui.add_space(8.0);
                if e.kind == "ReleaseEvent" {
                    release_card(ui, e, cx);
                } else {
                    star = self.repo_card(ui, &e.repo.name, cx);
                    if e.kind == "ForkEvent"
                        && let Some(fork) = e.payload["forkee"]["full_name"].as_str()
                    {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            icon(ui, Icon::RepoForked, pal.fg_muted);
                            w::muted_small(ui, "Forked to");
                            if w::link(ui, fork).clicked()
                                && let Some(r) = Route::from_slug(fork)
                            {
                                cx.go(r);
                            }
                        });
                    }
                }
            });
        });
        star
    }

    fn repo_card(&self, ui: &mut Ui, slug: &str, cx: &mut Cx) -> Option<(String, bool)> {
        let pal = w::p(ui);
        let info = self.repos.get(slug).cloned();
        let starred = self.stars.get(slug).copied().or(info.as_ref().map(|i| i.starred)).unwrap_or(false);
        let mut out = None;
        let (o, n) = split(slug);
        let (o, n) = (o.to_string(), n.to_string());
        card(ui, |ui| {
            ui.horizontal(|ui| {
                let avatar = info.as_ref().map(|i| i.owner_avatar.clone()).unwrap_or_default();
                w::org_avatar(ui, &avatar, 20.0);
                // renamed repos keep their old name in events; show the current one
                let shown = info.as_ref().map(|i| i.full_name.clone()).unwrap_or_else(|| slug.to_string());
                if w::link_strong(ui, shown, 16.0).clicked() {
                    cx.go(Route::repo_home(&o, &n));
                }
                if info.as_ref().is_some_and(|i| i.private) {
                    w::outline_label(ui, "Private", None);
                }
                w::right(ui, |ui| {
                    let (ic, label) = if starred { (Icon::StarFill, "Starred") } else { (Icon::Star, "Star") };
                    if info.is_some() && w::button(ui, Variant::Default, Some(ic), label).clicked() {
                        out = Some((slug.to_string(), !starred));
                    }
                });
            });
            match &info {
                Some(i) => {
                    if let Some(d) = i.description.as_ref().filter(|d| !d.is_empty()) {
                        ui.add_space(4.0);
                        ui.add(egui::Label::new(RichText::new(d).color(pal.fg_muted)).wrap());
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 16.0;
                        if let Some(l) = &i.language {
                            w::language_dot(ui, l);
                        }
                        w::meta(ui, Icon::Star, RichText::new(w::format_count(i.stars)).size(12.0).color(pal.fg_muted));
                        if let Some(t) = &i.pushed_at {
                            w::muted_small(ui, format!("Updated {}", w::rel_time(t)));
                        }
                    });
                }
                None if self.requested.contains(slug) && !self.lookup.is_idle() => {
                    ui.add_space(4.0);
                    ui.add(egui::Spinner::new().size(14.0));
                }
                // looked up but gone: deleted, renamed away or private to us
                None => {}
            }
        });
        out
    }
}

fn card(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let pal = w::p(ui);
    Frame::new()
        .fill(pal.canvas)
        .stroke(Stroke::new(1.0, pal.border))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        });
}

fn release_card(ui: &mut Ui, e: &Event, cx: &mut Cx) {
    let pal = w::p(ui);
    let rel = &e.payload["release"];
    let name = rel["name"].as_str().filter(|s| !s.is_empty()).or(rel["tag_name"].as_str()).unwrap_or("Release").to_string();
    let tag = rel["tag_name"].as_str().unwrap_or_default().to_string();
    let body = rel["body"].as_str().unwrap_or_default().to_string();
    let url = rel["html_url"].as_str().unwrap_or_default().to_string();
    let (o, n) = split(&e.repo.name);
    let (o, n) = (o.to_string(), n.to_string());
    card(ui, |ui| {
        ui.horizontal(|ui| {
            icon(ui, Icon::Tag, pal.success);
            if w::title_link(ui, &name, 16.0).clicked() && !url.is_empty() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(&url));
            }
            if rel["prerelease"].as_bool() == Some(true) {
                w::outline_label(ui, "Pre-release", Some(pal.attention));
            }
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if w::link(ui, &e.repo.name).clicked() {
                cx.go(Route::repo_home(&o, &n));
            }
            w::branch_pill(ui, &tag);
            if w::button(ui, Variant::Invisible, Some(Icon::Code), "Browse files").clicked() {
                cx.go(Route::repo(&o, &n, RepoView::Code { git_ref: Some(tag.clone()), path: String::new() }));
            }
        });
        snippet_text(ui, &body);
    });
}

/// First couple of lines of a body, markdown noise stripped, like the feed preview.
fn snippet_text(ui: &mut Ui, body: &str) {
    let pal = w::p(ui);
    let clean: String = body
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("<!--") && !l.starts_with("```") && !l.starts_with('|'))
        .map(|l| strip_markdown(l.trim_start_matches(['#', '>', '-', '*', ' '])))
        .filter(|l| !l.trim().is_empty())
        .take(3)
        .collect::<Vec<_>>()
        .join(" ");
    if clean.is_empty() {
        return;
    }
    let short: String = if clean.chars().count() > 240 { clean.chars().take(240).collect::<String>() + "…" } else { clean };
    ui.add_space(6.0);
    ui.add(egui::Label::new(RichText::new(short).color(pal.fg_muted)).wrap());
}

/// Turns `[text](url)` into `text`, drops images, html tags and `:emoji:` codes.
fn strip_markdown(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let image = c == '!' && chars.get(i + 1) == Some(&'[');
        if c == '[' || image {
            let start = if image { i + 2 } else { i + 1 };
            if let Some(close) = chars[start..].iter().position(|&x| x == ']').map(|p| p + start)
                && chars.get(close + 1) == Some(&'(')
                && let Some(end) = chars[close + 2..].iter().position(|&x| x == ')').map(|p| p + close + 2)
            {
                if !image {
                    out.extend(&chars[start..close]);
                }
                i = end + 1;
                continue;
            }
        }
        if c == '<'
            && let Some(end) = chars[i..].iter().position(|&x| x == '>')
        {
            i += end + 1;
            continue;
        }
        if c == ':'
            && let Some(end) = chars[i + 1..].iter().position(|&x| x == ':')
            && end > 0
            && chars[i + 1..i + 1 + end].iter().all(|x| x.is_ascii_alphanumeric() || *x == '_')
        {
            i += end + 2;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out.replace("**", "").replace('`', "")
}

impl User {
    /// Bots show up as "name[bot]" in events; github drops the suffix in the feed.
    pub fn display_name(&self) -> String {
        self.login.trim_end_matches("[bot]").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn ev(id: &str, kind: &str, actor: &str, repo: &str, payload: Value) -> Event {
        let mut e = Event { id: id.into(), kind: kind.into(), payload, ..Default::default() };
        e.actor.login = actor.into();
        e.repo.name = repo.into();
        e
    }

    #[test]
    fn only_discovery_events() {
        assert_eq!(category(&ev("1", "WatchEvent", "a", "r", Value::Null)), Some("stars"));
        assert_eq!(category(&ev("1", "CreateEvent", "a", "r", json!({ "ref_type": "repository" }))), Some("repos"));
        assert_eq!(category(&ev("1", "CreateEvent", "a", "r", json!({ "ref_type": "branch" }))), None);
        assert_eq!(category(&ev("1", "ReleaseEvent", "a", "r", json!({ "action": "published" }))), Some("releases"));
        assert_eq!(category(&ev("1", "ReleaseEvent", "a", "r", json!({ "action": "published", "release": { "prerelease": true } }))), None);
        for kind in ["PushEvent", "PullRequestEvent", "IssueCommentEvent", "PullRequestReviewEvent", "IssuesEvent"] {
            assert_eq!(category(&ev("1", kind, "a", "r", Value::Null)), None, "{kind}");
        }
    }

    #[test]
    fn stars_on_one_repo_fold_together() {
        let events = vec![
            ev("1", "WatchEvent", "ann", "o/x", Value::Null),
            ev("2", "PushEvent", "ann", "o/x", Value::Null),
            ev("3", "WatchEvent", "bob", "o/x", Value::Null),
            ev("4", "WatchEvent", "ann", "o/x", Value::Null),
            ev("5", "WatchEvent", "cat", "o/y", Value::Null),
        ];
        let follows: HashSet<String> = ["ann", "bob", "cat"].iter().map(|s| s.to_string()).collect();
        let g = group(&events, &[], &follows, "me");
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].event.id, "1");
        assert_eq!(g[0].others.iter().map(|u| u.login.as_str()).collect::<Vec<_>>(), vec!["bob"]);
        assert_eq!(g[1].event.repo.name, "o/y");
        assert!(group(&events, &["stars".to_string()], &follows, "me").is_empty());
        // strangers starring a repo you watch stay out of the feed
        assert!(group(&events, &[], &HashSet::new(), "me").is_empty());
    }

    #[test]
    fn markdown_is_flattened() {
        assert_eq!(strip_markdown("[Codecov](https://x.y) Report :white_check_mark: All"), "Codecov Report  All");
        assert_eq!(strip_markdown("![badge](a.svg) **bold** `code` <br>"), " bold code ");
    }
}
