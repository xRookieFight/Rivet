//! Notification inbox with GitHub's left filter column, bulk actions and
//! per-row mark read / done / unsubscribe.

use super::Page;
use super::dashboard::notification_route;
use crate::app::Cx;
use crate::github::models::Notification;
use crate::icons::{Icon, icon};
use crate::task::Load;
use crate::theme::semibold;
use crate::widgets::{self as w, Variant};
use egui::{CornerRadius, Frame, Margin, RichText, Ui};
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Folder {
    Inbox,
    All,
}

pub struct NotificationsPage {
    folder: Folder,
    participating: bool,
    reason: Option<&'static str>,
    repo: Option<String>,
    filter: String,
    page: u32,
    items: Load<Vec<Notification>>,
    selected: HashSet<String>,
    action: Load<Vec<String>>,
    hidden: HashSet<String>,
    read_local: HashSet<String>,
}

impl Default for NotificationsPage {
    fn default() -> Self {
        Self {
            folder: Folder::Inbox,
            participating: false,
            reason: None,
            repo: None,
            filter: String::new(),
            page: 1,
            items: Load::Idle,
            selected: HashSet::new(),
            action: Load::Idle,
            hidden: HashSet::new(),
            read_local: HashSet::new(),
        }
    }
}

const REASONS: [(&str, &str, Icon); 6] = [
    ("assign", "Assigned", Icon::Person),
    ("participating", "Participating", Icon::CommentDiscussion),
    ("mention", "Mentioned", Icon::Mail),
    ("team_mention", "Team mentioned", Icon::People),
    ("review_requested", "Review requested", Icon::CodeReview),
    ("ci_activity", "CI activity", Icon::Workflow),
];

fn reason_label(r: &str) -> &str {
    match r {
        "assign" => "assigned",
        "author" => "author",
        "comment" => "comment",
        "mention" => "mention",
        "team_mention" => "team mention",
        "review_requested" => "review requested",
        "subscribed" => "subscribed",
        "state_change" => "state change",
        "ci_activity" => "ci activity",
        "manual" => "manual",
        "security_alert" => "security alert",
        other => other,
    }
}

impl NotificationsPage {
    fn reload(&mut self, ctx: &egui::Context, cx: &Cx) {
        let g = cx.gh.clone();
        let (all, part, page) = (self.folder == Folder::All, self.participating, self.page);
        self.items = Load::spawn(ctx, move || g.notifications(all, part, page));
        self.selected.clear();
    }

    fn visible<'a>(&self, items: &'a [Notification]) -> Vec<&'a Notification> {
        let f = self.filter.to_lowercase();
        items
            .iter()
            .filter(|n| !self.hidden.contains(&n.id))
            .filter(|n| self.repo.as_ref().is_none_or(|r| &n.repository.full_name == r))
            .filter(|n| match self.reason {
                None => true,
                Some("participating") => {
                    matches!(n.reason.as_str(), "author" | "comment" | "mention" | "assign" | "state_change" | "review_requested")
                }
                Some(r) => n.reason == r,
            })
            .filter(|n| f.is_empty() || n.subject.title.to_lowercase().contains(&f) || n.repository.full_name.to_lowercase().contains(&f))
            .collect()
    }
}

impl Page for NotificationsPage {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        if self.items.is_idle() {
            self.reload(ui.ctx(), cx);
        }
        match self.action.take() {
            Some(Ok(ids)) => {
                for id in ids {
                    self.read_local.insert(id);
                }
                *cx.refresh_unread = true;
            }
            Some(Err(e)) => cx.err(e),
            None => {}
        }
        let pal = w::p(ui);
        let mut reload = false;

        egui::Panel::left("notif-left").resizable(false).exact_size(260.0).frame(Frame::new().inner_margin(Margin::same(16))).show(
            ui,
            |ui| {
                let nav_item = |ui: &mut Ui, ic: Icon, label: &str, sel: bool| -> bool {
                    let r = w::menu_item_ex(ui, Some(ic), label, false, false);
                    if sel {
                        let bar = egui::Rect::from_min_size(r.rect.left_top() + egui::vec2(-8.0, 6.0), egui::vec2(4.0, 20.0));
                        ui.painter().rect_filled(bar, CornerRadius::same(2), pal.accent_emphasis);
                    }
                    r.clicked()
                };
                if nav_item(ui, Icon::Inbox, "Inbox", self.folder == Folder::Inbox && self.reason.is_none() && self.repo.is_none()) {
                    self.folder = Folder::Inbox;
                    self.reason = None;
                    self.repo = None;
                    reload = true;
                }
                if nav_item(ui, Icon::Check, "All (read too)", self.folder == Folder::All) {
                    self.folder = Folder::All;
                    reload = true;
                }
                ui.add_space(8.0);
                ui.label(RichText::new("Filters").font(semibold(12.0)).color(pal.fg_muted));
                for (key, label, ic) in REASONS {
                    if nav_item(ui, ic, label, self.reason == Some(key)) {
                        self.reason = if self.reason == Some(key) { None } else { Some(key) };
                    }
                }
                ui.add_space(8.0);
                ui.label(RichText::new("Repositories").font(semibold(12.0)).color(pal.fg_muted));
                if let Load::Ready(items) = &self.items {
                    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
                    for n in items.iter().filter(|n| !self.hidden.contains(&n.id)) {
                        *counts.entry(n.repository.full_name.clone()).or_default() += 1;
                    }
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (repo, n) in counts {
                            let sel = self.repo.as_deref() == Some(repo.as_str());
                            if nav_item(ui, Icon::Repo, &format!("{repo}  ({n})"), sel) {
                                self.repo = if sel { None } else { Some(repo.clone()) };
                            }
                        }
                    });
                }
            },
        );

        egui::CentralPanel::no_frame().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                Frame::new().inner_margin(Margin::same(24)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        w::text_input_ex(
                            ui,
                            &mut self.filter,
                            "Filter notifications",
                            (ui.available_width() - 260.0).max(200.0),
                            Some(Icon::Filter),
                        );
                        let label = if self.participating { "Participating" } else { "All" };
                        let b = w::button_ex(ui, Variant::Default, None, label, Some(Icon::TriangleDown), true);
                        egui::Popup::menu(&b).show(|ui| {
                            if w::menu_item_ex(ui, None, "All", !self.participating, false).clicked() {
                                self.participating = false;
                                reload = true;
                            }
                            if w::menu_item_ex(ui, None, "Participating", self.participating, false).clicked() {
                                self.participating = true;
                                reload = true;
                            }
                        });
                        if w::icon_button(ui, Icon::Sync, "Refresh").clicked() {
                            reload = true;
                        }
                    });
                    ui.add_space(16.0);
                    self.list(ui, cx);
                });
            });
        });
        if reload {
            self.page = 1;
            self.reload(ui.ctx(), cx);
        }
    }
}

impl NotificationsPage {
    fn list(&mut self, ui: &mut Ui, cx: &mut Cx) {
        let pal = w::p(ui);
        let ctx = ui.ctx().clone();
        let Some(items) = (match self.items.poll() {
            Load::Ready(v) => Some(v.clone()),
            Load::Failed(e) => {
                w::flash(ui, w::FlashKind::Danger, &e.clone());
                None
            }
            _ => {
                w::spinner(ui);
                None
            }
        }) else {
            return;
        };
        let visible = self.visible(&items);
        let mut bulk_read: Vec<String> = vec![];
        let mut bulk_done: Vec<String> = vec![];
        let mut mark_all = false;

        w::bordered(ui, |ui| {
            w::box_header(ui, |ui| {
                let all_sel = !visible.is_empty() && visible.iter().all(|n| self.selected.contains(&n.id));
                let mut toggle = all_sel;
                if ui.checkbox(&mut toggle, "").changed() {
                    if toggle {
                        self.selected.extend(visible.iter().map(|n| n.id.clone()));
                    } else {
                        self.selected.clear();
                    }
                }
                if self.selected.is_empty() {
                    ui.label(RichText::new("Select all").color(pal.fg_muted));
                    w::right(ui, |ui| {
                        if w::button(ui, Variant::Invisible, Some(Icon::Check), "Mark all as read").clicked() {
                            mark_all = true;
                        }
                    });
                } else {
                    ui.label(RichText::new(format!("{} selected", self.selected.len())).font(semibold(14.0)));
                    if w::button(ui, Variant::Default, Some(Icon::Check), "Done").clicked() {
                        bulk_done = self.selected.iter().cloned().collect();
                    }
                    if w::button(ui, Variant::Default, Some(Icon::Mail), "Mark as read").clicked() {
                        bulk_read = self.selected.iter().cloned().collect();
                    }
                }
            });
            if visible.is_empty() {
                w::blankslate(ui, Icon::Inbox, "All caught up!", "Take a break, write some code, do what you do best.");
            }
            for n in &visible {
                let unread = n.unread && !self.read_local.contains(&n.id);
                let resp = w::list_row(ui, |ui| {
                    ui.horizontal(|ui| {
                        let mut sel = self.selected.contains(&n.id);
                        if ui.checkbox(&mut sel, "").changed() {
                            if sel {
                                self.selected.insert(n.id.clone());
                            } else {
                                self.selected.remove(&n.id);
                            }
                        }
                        let (dot_rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                        if unread {
                            ui.painter().circle_filled(dot_rect.center(), 4.0, pal.accent_emphasis);
                        }
                        let ic = match n.subject.kind.as_str() {
                            "PullRequest" => Icon::GitPullRequest,
                            "Issue" => Icon::IssueOpened,
                            "Release" => Icon::Tag,
                            "Discussion" => Icon::CommentDiscussion,
                            "CheckSuite" => Icon::Workflow,
                            "Commit" => Icon::GitCommit,
                            "RepositoryVulnerabilityAlert" => Icon::Alert,
                            _ => Icon::Bell,
                        };
                        icon(ui, ic, if unread { pal.fg } else { pal.fg_muted });
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                w::muted_small(ui, &n.repository.full_name);
                                if let Some(num) = n.number() {
                                    w::muted_small(ui, format!("#{num}"));
                                }
                            });
                            let title = RichText::new(&n.subject.title).color(pal.fg);
                            ui.add(egui::Label::new(if unread { title.font(semibold(14.0)) } else { title }).truncate());
                        });
                        w::right(ui, |ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            if w::icon_button_invisible(ui, Icon::Check, "Done").clicked() {
                                bulk_done.push(n.id.clone());
                            }
                            if unread && w::icon_button_invisible(ui, Icon::Mail, "Mark as read").clicked() {
                                bulk_read.push(n.id.clone());
                            }
                            let more = w::icon_button_invisible(ui, Icon::KebabHorizontal, "More");
                            egui::Popup::menu(&more).align(egui::RectAlign::BOTTOM_END).show(|ui| {
                                if w::menu_item(ui, Some(Icon::Bell), "Unsubscribe").clicked() {
                                    let (g, id) = (cx.gh.clone(), n.id.clone());
                                    self.action = Load::spawn(&ctx, move || {
                                        g.unsubscribe_thread(&id)?;
                                        Ok(vec![])
                                    });
                                    cx.ok("Unsubscribed from this thread");
                                }
                                if w::menu_item(ui, Some(Icon::LinkExternal), "Open on github.com").clicked() {
                                    ui.ctx().open_url(egui::OpenUrl::new_tab(&n.repository.html_url));
                                }
                            });
                            if let Some(t) = &n.updated_at {
                                w::muted_small(ui, w::rel_time(t));
                            }
                            w::outline_label(ui, reason_label(&n.reason), None);
                        });
                    });
                });
                if resp.clicked() {
                    // opening a thread marks it read, same as the website
                    if unread {
                        bulk_read.push(n.id.clone());
                    }
                    cx.go(notification_route(n));
                }
                w::divider(ui);
            }
        });
        if super::common::pager(ui, &mut self.page, items.len() >= 50) {
            self.reload(&ctx, cx);
        }

        if mark_all {
            let g = cx.gh.clone();
            let ids: Vec<String> = items.iter().map(|n| n.id.clone()).collect();
            self.action = Load::spawn(&ctx, move || {
                g.mark_all_read()?;
                Ok(ids)
            });
        }
        if !bulk_read.is_empty() {
            let g = cx.gh.clone();
            self.action = Load::spawn(&ctx, move || {
                for id in &bulk_read {
                    g.mark_thread_read(id)?;
                }
                Ok(bulk_read)
            });
            self.selected.clear();
        }
        if !bulk_done.is_empty() {
            for id in &bulk_done {
                self.hidden.insert(id.clone());
            }
            let g = cx.gh.clone();
            self.action = Load::spawn(&ctx, move || {
                for id in &bulk_done {
                    g.mark_thread_done(id)?;
                }
                Ok(bulk_done)
            });
            self.selected.clear();
        }
    }
}
