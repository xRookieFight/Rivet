//! GitHub Actions: workflow list, runs, manual dispatch with inputs, and the
//! run page with jobs, steps and logs (auto refreshing while it runs).

use super::{RepoSub, on};
use crate::app::Cx;
use crate::github::models::{Job, Repo, Workflow, WorkflowRun};
use crate::icons::{Icon, icon};
use crate::route::{RepoView, Route};
use crate::task::Load;
use crate::theme::semibold;
use crate::views::common;
use crate::widgets::{self as w, FlashKind, Variant};
use egui::{CornerRadius, Frame, Margin, RichText, Stroke, Ui};
use serde_json::{Map, Value};
use std::time::{Duration, Instant};

/// A `workflow_dispatch` input from the workflow file.
#[derive(Clone, Debug)]
struct DispatchInput {
    name: String,
    description: String,
    kind: String,
    options: Vec<String>,
    required: bool,
    value: String,
}

/// Reads the workflow YAML and returns its dispatch inputs, or None when the
/// workflow can't be triggered by hand.
fn dispatch_inputs(src: &str) -> Option<Vec<DispatchInput>> {
    use serde_yaml_ng::Value as Y;
    let v: Y = serde_yaml_ng::from_str(src).ok()?;
    // yeah, YAML 1.1 parses a bare `on` key as the boolean true
    let on = v.get("on").or_else(|| v.get(Y::Bool(true)))?;
    let wd = match on {
        Y::String(s) if s == "workflow_dispatch" => return Some(vec![]),
        Y::Sequence(seq) => return seq.iter().any(|x| x.as_str() == Some("workflow_dispatch")).then(Vec::new),
        Y::Mapping(m) => m.get(Y::String("workflow_dispatch".into()))?,
        _ => return None,
    };
    let mut out = vec![];
    if let Some(Y::Mapping(inputs)) = wd.get("inputs") {
        for (k, def) in inputs {
            let s = |key: &str| {
                def.get(key)
                    .map(|x| match x {
                        Y::String(s) => s.clone(),
                        Y::Bool(b) => b.to_string(),
                        Y::Number(n) => n.to_string(),
                        _ => String::new(),
                    })
                    .unwrap_or_default()
            };
            out.push(DispatchInput {
                name: k.as_str().unwrap_or_default().to_string(),
                description: s("description"),
                kind: if s("type").is_empty() { "string".into() } else { s("type") },
                options: def
                    .get("options")
                    .and_then(|o| o.as_sequence())
                    .map(|seq| seq.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                    .unwrap_or_default(),
                required: def.get("required").and_then(|x| x.as_bool()).unwrap_or(false),
                value: s("default"),
            });
        }
    }
    Some(out)
}

pub struct ActionsView {
    workflow: Option<u64>,
    workflows: Load<Vec<Workflow>>,
    runs: Load<(u64, Vec<WorkflowRun>)>,
    page: u32,
    dispatch: Load<Option<Vec<DispatchInput>>>,
    dispatch_ref: String,
    action: Load<()>,
    last_refresh: Instant,
}

impl ActionsView {
    pub fn new(workflow: Option<u64>) -> Self {
        Self {
            workflow,
            workflows: Load::Idle,
            runs: Load::Idle,
            page: 1,
            dispatch: Load::Idle,
            dispatch_ref: String::new(),
            action: Load::Idle,
            last_refresh: Instant::now(),
        }
    }

    fn load_runs(&mut self, ctx: &egui::Context, cx: &Cx, repo: &Repo) {
        let (g, (o, n), wf, p) = (cx.gh.clone(), on(repo), self.workflow, self.page);
        self.runs = Load::spawn(ctx, move || g.runs(&o, &n, wf, p));
        self.last_refresh = Instant::now();
    }
}

impl RepoSub for ActionsView {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let ctx = ui.ctx().clone();
        let (o, n) = on(repo);
        if self.workflows.is_idle() {
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.workflows = Load::spawn(&ctx, move || g.workflows(&o2, &n2));
            self.load_runs(&ctx, cx, repo);
            self.dispatch_ref = repo.default_branch.clone();
        }
        // load the selected workflow file to see if it can be dispatched
        if self.dispatch.is_idle()
            && let Some(id) = self.workflow
            && let Some(wf) = self.workflows.ready().and_then(|l| l.iter().find(|w| w.id == id).cloned())
        {
            let (g, o2, n2, b) = (cx.gh.clone(), o.clone(), n.clone(), repo.default_branch.clone());
            self.dispatch = Load::spawn(&ctx, move || Ok(g.file(&o2, &n2, &wf.path, &b)?.text().and_then(|t| dispatch_inputs(&t))));
        }
        match self.action.take() {
            Some(Ok(())) => {
                cx.ok("Workflow run was successfully requested.");
                self.load_runs(&ctx, cx, repo);
            }
            Some(Err(e)) => cx.err(e),
            None => {}
        }
        // keep in-progress runs fresh
        let active = self.runs.ready().is_some_and(|(_, r)| r.iter().any(|x| x.status.as_deref() != Some("completed")));
        if active && self.last_refresh.elapsed() > Duration::from_secs(10) {
            self.load_runs(&ctx, cx, repo);
        }
        if active {
            ctx.request_repaint_after(Duration::from_secs(10));
        }

        let pal = w::p(ui);
        let mut nav_wf: Option<Option<u64>> = None;
        egui::Panel::left("actions-left").resizable(false).exact_size(280.0).frame(Frame::new().inner_margin(Margin::same(16))).show(
            ui,
            |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Actions").font(semibold(20.0)));
                    w::right(ui, |ui| {
                        if repo.can_push() && w::button(ui, Variant::Default, None, "New workflow").clicked() {
                            cx.go(Route::repo(&o, &n, RepoView::CommunityFiles));
                        }
                    });
                });
                ui.add_space(12.0);
                let r = w::menu_item(ui, None, "All workflows");
                if self.workflow.is_none() {
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(r.rect.left_top() + egui::vec2(-8.0, 6.0), egui::vec2(4.0, 20.0)),
                        CornerRadius::same(2),
                        pal.accent_emphasis,
                    );
                }
                if r.clicked() {
                    nav_wf = Some(None);
                }
                ui.add_space(4.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    w::show_load(ui, &mut self.workflows, |ui, list| {
                        for wf in list.iter() {
                            let r = w::menu_item(ui, Some(Icon::Workflow), &wf.name);
                            if self.workflow == Some(wf.id) {
                                ui.painter().rect_filled(
                                    egui::Rect::from_min_size(r.rect.left_top() + egui::vec2(-8.0, 6.0), egui::vec2(4.0, 20.0)),
                                    CornerRadius::same(2),
                                    pal.accent_emphasis,
                                );
                            }
                            if r.clicked() {
                                nav_wf = Some(Some(wf.id));
                            }
                        }
                    });
                });
            },
        );
        if let Some(wf) = nav_wf {
            cx.go_replace(Route::repo(&o, &n, RepoView::Actions { workflow: wf }));
        }

        egui::CentralPanel::no_frame().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                Frame::new().inner_margin(Margin::same(24)).show(ui, |ui| {
                    let title = self
                        .workflow
                        .and_then(|id| self.workflows.ready().and_then(|l| l.iter().find(|w| w.id == id).map(|w| w.name.clone())))
                        .unwrap_or_else(|| "All workflows".into());
                    ui.label(RichText::new(&title).font(semibold(20.0)));
                    ui.add_space(12.0);
                    if let Some(Some(inputs)) = self.dispatch.ready().cloned() {
                        self.dispatch_ui(ui, cx, repo, inputs);
                        ui.add_space(16.0);
                    }
                    let mut reload = false;
                    let mut has_next = false;
                    w::bordered(ui, |ui| {
                        w::box_header(ui, |ui| {
                            let total = self.runs.ready().map(|r| r.0).unwrap_or(0);
                            ui.label(RichText::new(format!("{} workflow runs", w::format_count(total))).font(semibold(14.0)));
                            w::right(ui, |ui| {
                                if w::icon_button_invisible(ui, Icon::Sync, "Refresh").clicked() {
                                    reload = true;
                                }
                            });
                        });
                        w::show_load(ui, &mut self.runs, |ui, (_, runs)| {
                            if runs.is_empty() {
                                w::blankslate(ui, Icon::Play, "No workflow runs yet", "Runs show up here once a workflow is triggered.");
                            }
                            for r in runs.iter() {
                                let resp = w::list_row(ui, |ui| run_row(ui, r));
                                if resp.clicked() {
                                    cx.go(Route::repo(&o, &n, RepoView::Run(r.id)));
                                }
                                w::divider(ui);
                            }
                            has_next = runs.len() >= 30;
                        });
                    });
                    if common::pager(ui, &mut self.page, has_next) {
                        reload = true;
                    }
                    if reload {
                        self.load_runs(&ctx, cx, repo);
                    }
                });
            });
        });
    }
}

impl ActionsView {
    fn dispatch_ui(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo, inputs: Vec<DispatchInput>) {
        let ctx = ui.ctx().clone();
        let pal = w::p(ui);
        Frame::new()
            .fill(pal.canvas_subtle)
            .stroke(Stroke::new(1.0, pal.border))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    w::muted(ui, "This workflow has a workflow_dispatch event trigger.");
                    w::right(ui, |ui| {
                        if !repo.can_push() {
                            return;
                        }
                        let b =
                            w::button_ex(ui, Variant::Default, None, "Run workflow", Some(Icon::TriangleDown), !self.action.is_loading());
                        egui::Popup::menu(&b)
                            .width(340.0)
                            .align(egui::RectAlign::BOTTOM_END)
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                            .show(|ui| {
                                let mut values: Vec<DispatchInput> =
                                    ui.data(|d| d.get_temp(egui::Id::unique("dispatch-values"))).unwrap_or_else(|| inputs.clone());
                                if values.len() != inputs.len() {
                                    values = inputs.clone();
                                }
                                w::form_label(ui, "Use workflow from", false);
                                w::text_input_ex(ui, &mut self.dispatch_ref, "branch or tag", ui.available_width(), Some(Icon::GitBranch));
                                for inp in values.iter_mut() {
                                    ui.add_space(6.0);
                                    w::form_label(ui, if inp.description.is_empty() { &inp.name } else { &inp.description }, inp.required);
                                    match inp.kind.as_str() {
                                        "boolean" => {
                                            let mut b = inp.value == "true";
                                            if ui.checkbox(&mut b, inp.name.as_str()).changed() {
                                                inp.value = b.to_string();
                                            }
                                        }
                                        "choice" => {
                                            let bb = w::button_ex(ui, Variant::Default, None, &inp.value, Some(Icon::TriangleDown), true);
                                            egui::Popup::menu(&bb).show(|ui| {
                                                for o in inp.options.clone() {
                                                    if w::menu_item_ex(ui, None, &o, inp.value == o, false).clicked() {
                                                        inp.value = o;
                                                    }
                                                }
                                            });
                                        }
                                        _ => {
                                            w::text_input(ui, &mut inp.value, "", ui.available_width());
                                        }
                                    }
                                }
                                ui.add_space(8.0);
                                let missing = values.iter().any(|i| i.required && i.value.trim().is_empty());
                                if w::button_ex(ui, Variant::Primary, None, "Run workflow", None, !missing).clicked()
                                    && let Some(id) = self.workflow
                                {
                                    let mut map = Map::new();
                                    for i in &values {
                                        map.insert(i.name.clone(), Value::String(i.value.clone()));
                                    }
                                    let (g, (o, n), r) = (cx.gh.clone(), on(repo), self.dispatch_ref.clone());
                                    self.action = Load::spawn(&ctx, move || g.dispatch(&o, &n, id, &r, Value::Object(map)));
                                    ui.close();
                                }
                                ui.data_mut(|d| d.insert_temp(egui::Id::unique("dispatch-values"), values));
                            });
                    });
                });
            });
    }
}

fn run_row(ui: &mut Ui, r: &WorkflowRun) {
    let pal = w::p(ui);
    ui.horizontal(|ui| {
        common::run_status_icon(ui, r.status.as_deref().unwrap_or(""), r.conclusion.as_deref());
        // fixed columns like github: title/meta, branch, time
        let left_w = (ui.available_width() - 330.0).max(200.0);
        ui.allocate_ui_with_layout(egui::vec2(left_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(left_w);
            ui.add(egui::Label::new(RichText::new(&r.display_title).font(semibold(14.0))).truncate());
            let who = r.actor.as_ref().map(|a| a.login.clone()).unwrap_or_default();
            let meta = format!("{} #{}: {} by {who}", r.name.clone().unwrap_or_default(), r.run_number, r.event.replace('_', " "));
            ui.add(egui::Label::new(RichText::new(meta).size(12.0).color(pal.fg_muted)).truncate());
        });
        ui.allocate_ui_with_layout(egui::vec2(180.0, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_width(180.0);
            if let Some(b) = &r.head_branch {
                w::branch_pill(ui, b);
            }
        });
        ui.vertical(|ui| {
            if let Some(t) = &r.created_at {
                w::meta(ui, Icon::Clock, RichText::new(w::rel_time(t)).size(12.0).color(pal.fg_muted));
            }
            if let (Some(s), Some(e)) = (r.run_started_at, r.updated_at)
                && r.status.as_deref() == Some("completed")
            {
                w::meta(ui, Icon::Stopwatch, RichText::new(fmt_duration((e - s).num_seconds())).size(12.0).color(pal.fg_muted));
            }
        });
    });
}

fn fmt_duration(s: i64) -> String {
    if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m {}s", s / 60, s % 60)
    } else {
        format!("{}h {}m", s / 3600, (s % 3600) / 60)
    }
}

pub struct RunView {
    id: u64,
    run: Load<WorkflowRun>,
    jobs: Load<Vec<Job>>,
    selected: Option<u64>,
    log: Load<String>,
    log_filter: String,
    action: Load<&'static str>,
    last_refresh: Instant,
}

impl RunView {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            run: Load::Idle,
            jobs: Load::Idle,
            selected: None,
            log: Load::Idle,
            log_filter: String::new(),
            action: Load::Idle,
            last_refresh: Instant::now(),
        }
    }

    fn load(&mut self, ctx: &egui::Context, cx: &Cx, repo: &Repo) {
        let (g, (o, n), id) = (cx.gh.clone(), on(repo), self.id);
        let (g2, o2, n2) = (g.clone(), o.clone(), n.clone());
        self.run = Load::spawn(ctx, move || g.run(&o, &n, id));
        self.jobs = Load::spawn(ctx, move || g2.jobs(&o2, &n2, id));
        self.last_refresh = Instant::now();
    }

    fn load_log(&mut self, ctx: &egui::Context, cx: &Cx, repo: &Repo, job: u64) {
        let (g, (o, n)) = (cx.gh.clone(), on(repo));
        self.selected = Some(job);
        self.log = Load::spawn(ctx, move || g.job_log(&o, &n, job));
    }
}

impl RepoSub for RunView {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let ctx = ui.ctx().clone();
        let (o, n) = on(repo);
        if self.run.is_idle() {
            self.load(&ctx, cx, repo);
        }
        match self.action.take() {
            Some(Ok(msg)) => {
                cx.ok(msg);
                self.load(&ctx, cx, repo);
            }
            Some(Err(e)) => cx.err(e),
            None => {}
        }
        let running = self.run.ready().is_some_and(|r| r.status.as_deref() != Some("completed"));
        if running {
            if self.last_refresh.elapsed() > Duration::from_secs(5) {
                self.load(&ctx, cx, repo);
            }
            ctx.request_repaint_after(Duration::from_secs(5));
        }
        if self.selected.is_none()
            && let Some(first) = self.jobs.ready().and_then(|j| j.first().map(|j| j.id))
        {
            self.load_log(&ctx, cx, repo, first);
        }
        let pal = w::p(ui);
        let Some(run) = self.run.ready().cloned() else {
            if let Load::Failed(e) = self.run.poll() {
                let e = e.clone();
                super::section(ui, |ui| w::flash(ui, FlashKind::Danger, &e));
            } else {
                w::spinner(ui);
            }
            return;
        };

        egui::Panel::top("run-head").frame(Frame::new().inner_margin(Margin::same(16))).show(ui, |ui| {
            if super::back_link(ui, &run.name.clone().unwrap_or_else(|| "Actions".into())) {
                cx.go(Route::repo(&o, &n, RepoView::Actions { workflow: Some(run.workflow_id) }));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                common::run_status_icon(ui, run.status.as_deref().unwrap_or(""), run.conclusion.as_deref());
                ui.label(RichText::new(&run.display_title).size(24.0));
                ui.label(RichText::new(format!("#{}", run.run_number)).size(24.0).color(pal.fg_muted));
                w::right(ui, |ui| {
                    if !repo.can_push() {
                        return;
                    }
                    let busy = self.action.is_loading();
                    if running {
                        if w::button_ex(ui, Variant::Danger, Some(Icon::Stop), "Cancel workflow", None, !busy).clicked() {
                            let (g, o2, n2, id) = (cx.gh.clone(), o.clone(), n.clone(), self.id);
                            self.action = Load::spawn(&ctx, move || g.cancel_run(&o2, &n2, id).map(|_| "Cancellation requested"));
                        }
                    } else {
                        let b = w::button_ex(ui, Variant::Default, Some(Icon::Sync), "Re-run jobs", Some(Icon::TriangleDown), !busy);
                        egui::Popup::menu(&b).align(egui::RectAlign::BOTTOM_END).show(|ui| {
                            if w::menu_item(ui, Some(Icon::Sync), "Re-run all jobs").clicked() {
                                let (g, o2, n2, id) = (cx.gh.clone(), o.clone(), n.clone(), self.id);
                                self.action = Load::spawn(&ctx, move || g.rerun(&o2, &n2, id, false).map(|_| "Re-run requested"));
                            }
                            if run.conclusion.as_deref() == Some("failure")
                                && w::menu_item(ui, Some(Icon::Sync), "Re-run failed jobs").clicked()
                            {
                                let (g, o2, n2, id) = (cx.gh.clone(), o.clone(), n.clone(), self.id);
                                self.action =
                                    Load::spawn(&ctx, move || g.rerun(&o2, &n2, id, true).map(|_| "Re-run of failed jobs requested"));
                            }
                        });
                    }
                    if w::button(ui, Variant::Invisible, Some(Icon::LinkExternal), "github.com").clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(&run.html_url));
                    }
                });
            });
            ui.horizontal(|ui| {
                let who = run.actor.as_ref().map(|a| a.login.clone()).unwrap_or_default();
                w::muted(
                    ui,
                    format!(
                        "Triggered via {} {} by {who}",
                        run.event.replace('_', " "),
                        run.created_at.as_ref().map(w::rel_time).unwrap_or_default()
                    ),
                );
                if let Some(b) = &run.head_branch {
                    w::branch_pill(ui, b);
                }
                if w::link(ui, &run.head_sha[..7.min(run.head_sha.len())]).clicked() {
                    cx.go(Route::repo(&o, &n, RepoView::Commit(run.head_sha.clone())));
                }
            });
        });

        let mut pick = None;
        egui::Panel::left("run-jobs").resizable(false).exact_size(280.0).frame(Frame::new().inner_margin(Margin::same(16))).show(
            ui,
            |ui| {
                ui.label(RichText::new("Jobs").font(semibold(14.0)).color(pal.fg_muted));
                ui.add_space(4.0);
                if let Some(jobs) = self.jobs.ready() {
                    for j in jobs.iter() {
                        let sel = self.selected == Some(j.id);
                        let resp = Frame::new()
                            .fill(if sel { pal.control_hover } else { egui::Color32::TRANSPARENT })
                            .corner_radius(CornerRadius::same(6))
                            .inner_margin(Margin::symmetric(8, 6))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    common::run_status_icon(ui, &j.status, j.conclusion.as_deref());
                                    ui.add(egui::Label::new(RichText::new(&j.name).font(semibold(14.0))).truncate());
                                });
                            })
                            .response;
                        if ui.interact(resp.rect, ui.scope_id().with(("job", j.id)), egui::Sense::click()).clicked() {
                            pick = Some(j.id);
                        }
                    }
                } else {
                    w::spinner(ui);
                }
            },
        );
        if let Some(j) = pick {
            self.load_log(&ctx, cx, repo, j);
        }

        egui::CentralPanel::no_frame().show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("run-main").show(ui, |ui| {
                Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                    let Some(job) = self.jobs.ready().and_then(|js| js.iter().find(|j| Some(j.id) == self.selected).cloned()) else {
                        w::blankslate(ui, Icon::Workflow, "No jobs", "This run has no jobs yet.");
                        return;
                    };
                    let dark_bg = if pal.dark { pal.canvas_inset } else { egui::Color32::from_rgb(0x24, 0x29, 0x2f) };
                    Frame::new().fill(dark_bg).corner_radius(CornerRadius::same(6)).inner_margin(Margin::same(16)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let light = egui::Color32::from_rgb(0xf0, 0xf6, 0xfc);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&job.name).font(semibold(16.0)).color(light));
                            w::right(ui, |ui| {
                                w::text_input_ex(ui, &mut self.log_filter, "Search logs", 220.0, Some(Icon::Search));
                            });
                        });
                        if let (Some(s), Some(e)) = (job.started_at, job.completed_at) {
                            ui.label(
                                RichText::new(format!(
                                    "{} in {}",
                                    job.conclusion.clone().unwrap_or_default(),
                                    fmt_duration((e - s).num_seconds())
                                ))
                                .size(12.0)
                                .color(egui::Color32::from_rgb(0x91, 0x98, 0xa1)),
                            );
                        }
                        ui.add_space(8.0);
                        for s in &job.steps {
                            ui.horizontal(|ui| {
                                let (ic, c) = common::run_status(pal, &s.status, s.conclusion.as_deref());
                                icon(ui, ic, c);
                                ui.label(RichText::new(&s.name).color(light));
                                if let (Some(a), Some(b)) = (s.started_at, s.completed_at) {
                                    w::right(ui, |ui| {
                                        ui.label(
                                            RichText::new(fmt_duration((b - a).num_seconds()))
                                                .size(12.0)
                                                .color(egui::Color32::from_rgb(0x91, 0x98, 0xa1)),
                                        )
                                    });
                                }
                            });
                        }
                        ui.add_space(8.0);
                        match self.log.poll() {
                            Load::Ready(text) => {
                                let f = self.log_filter.to_lowercase();
                                let mut out = String::new();
                                for line in text.lines() {
                                    // strip the ISO timestamp github prefixes every line with
                                    let l =
                                        line.get(29..).filter(|_| line.len() > 29 && line.as_bytes().get(4) == Some(&b'-')).unwrap_or(line);
                                    if f.is_empty() || l.to_lowercase().contains(&f) {
                                        out.push_str(l);
                                        out.push('\n');
                                    }
                                }
                                ui.add(egui::Label::new(RichText::new(out).monospace().size(12.0).color(light)).selectable(true));
                            }
                            Load::Failed(e) => {
                                let msg = if running { "Logs become available once the job finishes.".to_string() } else { e.clone() };
                                ui.label(RichText::new(msg).color(light));
                            }
                            _ => {
                                ui.add(egui::Spinner::new().color(light));
                            }
                        }
                    });
                });
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::dispatch_inputs;

    #[test]
    fn reads_dispatch_inputs() {
        let y = "on:\n  workflow_dispatch:\n    inputs:\n      env:\n        type: choice\n        options: [dev, prod]\n        required: true\n        default: dev\n";
        let i = dispatch_inputs(y).unwrap();
        assert_eq!(i.len(), 1);
        assert_eq!(i[0].options, vec!["dev", "prod"]);
        assert_eq!(i[0].value, "dev");
        assert!(i[0].required);
    }

    #[test]
    fn non_dispatch_workflow() {
        assert!(dispatch_inputs("on: push\n").is_none());
        assert_eq!(dispatch_inputs("on: [push, workflow_dispatch]\n").map(|v| v.len()), Some(0));
    }
}
