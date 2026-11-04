//! Label management: list, create, edit and delete, with a live preview.

use super::{RepoSub, on};
use crate::app::Cx;
use crate::github::models::{Label, Repo};
use crate::icons::Icon;
use crate::route::{RepoView, Route};
use crate::task::Load;
use crate::theme::semibold;
use crate::widgets::{self as w, Variant};
use egui::{Frame, Margin, RichText, Ui};

#[derive(Default, Clone)]
struct Draft {
    original: Option<String>,
    name: String,
    color: String,
    description: String,
}

#[derive(Default)]
pub struct LabelsView {
    labels: Load<Vec<Label>>,
    filter: String,
    draft: Option<Draft>,
    action: Load<String>,
}

/// Same palette github picks from when you hit the refresh button.
const SUGGESTED: [&str; 12] =
    ["b60205", "d93f0b", "fbca04", "0e8a16", "006b75", "1d76db", "0052cc", "5319e7", "e99695", "f9d0c4", "c2e0c6", "bfd4f2"];

impl RepoSub for LabelsView {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let ctx = ui.ctx().clone();
        let (o, n) = on(repo);
        if self.labels.is_idle() {
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.labels = Load::spawn(&ctx, move || g.labels(&o2, &n2));
        }
        match self.action.take() {
            Some(Ok(m)) => {
                cx.ok(m);
                self.draft = None;
                self.labels = Load::Idle;
            }
            Some(Err(e)) => cx.err(e),
            None => {}
        }
        let can = repo.permissions.as_ref().is_some_and(|p| p.triage || p.push || p.admin);
        super::section(ui, |ui| {
            if super::back_link(ui, "Issues") {
                cx.go(Route::repo(&o, &n, RepoView::Issues { q: None }));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                w::text_input_ex(ui, &mut self.filter, "Search all labels", 320.0, Some(Icon::Search));
                w::right(ui, |ui| {
                    if can && w::button(ui, Variant::Primary, None, "New label").clicked() {
                        let idx = (chrono::Utc::now().timestamp() as usize) % SUGGESTED.len();
                        self.draft = Some(Draft { color: SUGGESTED[idx].into(), ..Default::default() });
                    }
                });
            });
            ui.add_space(16.0);
            if let Some(d) = &mut self.draft
                && d.original.is_none()
            {
                let mut d2 = d.clone();
                let r = editor(ui, &mut d2);
                *d = d2.clone();
                self.handle(&ctx, cx, repo, r, d2);
                ui.add_space(16.0);
            }
            let f = self.filter.to_lowercase();
            let labels = self.labels.ready().cloned();
            w::bordered(ui, |ui| {
                let Some(labels) = labels else {
                    w::spinner(ui);
                    return;
                };
                let shown: Vec<&Label> = labels.iter().filter(|l| f.is_empty() || l.name.to_lowercase().contains(&f)).collect();
                w::box_header(ui, |ui| {
                    ui.label(RichText::new(format!("{} labels", shown.len())).font(semibold(14.0)));
                });
                for l in shown {
                    let editing = self.draft.as_ref().is_some_and(|d| d.original.as_deref() == Some(&l.name));
                    if editing {
                        let mut d = self.draft.clone().unwrap_or_default();
                        Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                            let r = editor(ui, &mut d);
                            self.draft = Some(d.clone());
                            self.handle(&ctx, cx, repo, r, d);
                        });
                        w::divider(ui);
                        continue;
                    }
                    w::list_row(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.allocate_ui_with_layout(egui::vec2(220.0, 24.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                                w::issue_label(ui, &l.name, &l.color);
                            });
                            w::muted(ui, l.description.clone().unwrap_or_default());
                            w::right(ui, |ui| {
                                if can {
                                    if w::button(ui, Variant::Invisible, None, "Delete").clicked() {
                                        let (g, o2, n2, name) = (cx.gh.clone(), o.clone(), n.clone(), l.name.clone());
                                        self.action = Load::spawn(&ctx, move || {
                                            g.delete_label(&o2, &n2, &name)?;
                                            Ok(format!("Deleted label {name}"))
                                        });
                                    }
                                    if w::button(ui, Variant::Invisible, None, "Edit").clicked() {
                                        self.draft = Some(Draft {
                                            original: Some(l.name.clone()),
                                            name: l.name.clone(),
                                            color: l.color.clone(),
                                            description: l.description.clone().unwrap_or_default(),
                                        });
                                    }
                                }
                                if w::link(ui, "issues").clicked() {
                                    cx.go(Route::repo(
                                        &o,
                                        &n,
                                        RepoView::Issues { q: Some(format!("is:issue is:open label:\"{}\"", l.name)) },
                                    ));
                                }
                            });
                        });
                    });
                    w::divider(ui);
                }
            });
        });
    }
}

enum EditResult {
    None,
    Save,
    Cancel,
}

fn editor(ui: &mut Ui, d: &mut Draft) -> EditResult {
    let pal = w::p(ui);
    let mut res = EditResult::None;
    let preview = if d.name.trim().is_empty() { "Label preview" } else { d.name.trim() };
    w::issue_label(ui, preview, &d.color);
    ui.add_space(8.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            w::form_label(ui, "Label name", true);
            w::text_input(ui, &mut d.name, "Label name", 220.0);
        });
        ui.vertical(|ui| {
            w::form_label(ui, "Description", false);
            w::text_input(ui, &mut d.description, "Description (optional)", 300.0);
        });
        ui.vertical(|ui| {
            w::form_label(ui, "Color", false);
            ui.horizontal(|ui| {
                if w::icon_button(ui, Icon::Sync, "Get a new color").clicked() {
                    let cur = SUGGESTED.iter().position(|c| *c == d.color).unwrap_or(0);
                    d.color = SUGGESTED[(cur + 1) % SUGGESTED.len()].to_string();
                }
                let mut hex = format!("#{}", d.color.trim_start_matches('#'));
                if w::text_input(ui, &mut hex, "#", 100.0).changed() {
                    d.color = hex.trim_start_matches('#').chars().take(6).collect();
                }
                let mut c = w::parse_hex(&d.color).unwrap_or(pal.neutral_emphasis);
                if ui.color_edit_button_srgba(&mut c).changed() {
                    d.color = format!("{:02x}{:02x}{:02x}", c.r(), c.g(), c.b());
                }
            });
        });
        ui.vertical(|ui| {
            ui.add_space(22.0);
            ui.horizontal(|ui| {
                if w::button(ui, Variant::Default, None, "Cancel").clicked() {
                    res = EditResult::Cancel;
                }
                let ok = !d.name.trim().is_empty() && w::parse_hex(&d.color).is_some();
                let label = if d.original.is_some() { "Save changes" } else { "Create label" };
                if w::button_ex(ui, Variant::Primary, None, label, None, ok).clicked() {
                    res = EditResult::Save;
                }
            });
        });
    });
    res
}

impl LabelsView {
    fn handle(&mut self, ctx: &egui::Context, cx: &Cx, repo: &Repo, r: EditResult, d: Draft) {
        match r {
            EditResult::Cancel => self.draft = None,
            EditResult::Save => {
                let (g, (o, n)) = (cx.gh.clone(), on(repo));
                self.action = Load::spawn(ctx, move || {
                    let name = d.name.trim().to_string();
                    match &d.original {
                        Some(old) => g.update_label(&o, &n, old, &name, &d.color, &d.description).map(|_| format!("Updated {name}")),
                        None => g.create_label(&o, &n, &name, &d.color, &d.description).map(|_| format!("Created {name}")),
                    }
                });
            }
            EditResult::None => {}
        }
    }
}
