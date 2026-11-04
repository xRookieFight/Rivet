//! The global "Issues" and "Pull requests" pages: created, assigned,
//! mentioned and review requests across every repository.

use super::Page;
use super::common;
use crate::app::Cx;
use crate::github::models::{Issue, SearchResult};
use crate::icons::Icon;
use crate::task::Load;
use crate::widgets::{self as w};
use egui::{Frame, Margin, Ui};

pub struct MyItems {
    pulls: bool,
    tab: usize,
    open: bool,
    filter: String,
    page: u32,
    items: Load<SearchResult<Issue>>,
}

impl MyItems {
    pub fn new(pulls: bool) -> Self {
        Self { pulls, tab: 0, open: true, filter: String::new(), page: 1, items: Load::Idle }
    }

    fn tabs(&self) -> Vec<(&'static str, &'static str)> {
        let mut t = vec![("Created", "author:@me"), ("Assigned", "assignee:@me"), ("Mentioned", "mentions:@me")];
        if self.pulls {
            t.push(("Review requests", "review-requested:@me"));
        }
        t
    }

    fn query(&self) -> String {
        let kind = if self.pulls { "is:pr" } else { "is:issue" };
        let state = if self.open { "is:open" } else { "is:closed" };
        let who = self.tabs()[self.tab].1;
        format!("{kind} {state} {who} archived:false {}", self.filter.trim()).trim().to_string()
    }

    fn load(&mut self, ctx: &egui::Context, cx: &Cx) {
        let (g, q, p) = (cx.gh.clone(), self.query(), self.page);
        self.items = Load::spawn(ctx, move || g.search_issues(&q, "updated", p));
    }
}

impl Page for MyItems {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        if self.items.is_idle() {
            self.load(ui.ctx(), cx);
        }
        let mut reload = false;
        egui::ScrollArea::vertical().show(ui, |ui| {
            Frame::new().inner_margin(Margin::same(24)).show(ui, |ui| {
                w::page(ui, 1280.0, |ui| {
                    ui.horizontal(|ui| {
                        let tabs = self.tabs();
                        for (i, (label, _)) in tabs.iter().enumerate() {
                            let v = if i == self.tab { w::Variant::Default } else { w::Variant::Invisible };
                            if w::button(ui, v, None, label).clicked() && i != self.tab {
                                self.tab = i;
                                reload = true;
                            }
                        }
                    });
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        let r = w::text_input_ex(
                            ui,
                            &mut self.filter,
                            "Filter, e.g. repo:owner/name label:bug",
                            ui.available_width(),
                            Some(Icon::Search),
                        );
                        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            reload = true;
                        }
                    });
                    ui.add_space(16.0);
                    w::bordered(ui, |ui| {
                        w::box_header(ui, |ui| {
                            let (oi, ci) = if self.pulls { (Icon::GitPullRequest, Icon::Check) } else { (Icon::IssueOpened, Icon::Check) };
                            if w::tab_link(ui, oi, "Open", self.open).clicked() && !self.open {
                                self.open = true;
                                reload = true;
                            }
                            if w::tab_link(ui, ci, "Closed", !self.open).clicked() && self.open {
                                self.open = false;
                                reload = true;
                            }
                        });
                        let mut has_next = false;
                        w::show_load(ui, &mut self.items, |ui, res| {
                            if res.items.is_empty() {
                                let what = if self.pulls { "pull requests" } else { "issues" };
                                w::blankslate(
                                    ui,
                                    if self.pulls { Icon::GitPullRequest } else { Icon::IssueOpened },
                                    &format!("No {what} matched"),
                                    "Try a different filter.",
                                );
                            }
                            for it in &res.items {
                                common::issue_row(ui, it, true, cx);
                            }
                            has_next = res.items.len() >= 30;
                        });
                        if has_next || self.page > 1 {
                            ui.add_space(4.0);
                        }
                        if common::pager(ui, &mut self.page, has_next) {
                            reload = true;
                        }
                    });
                });
            });
        });
        if reload {
            self.load(ui.ctx(), cx);
        }
    }
}
