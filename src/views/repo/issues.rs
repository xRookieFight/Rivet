//! Issue / pull request index with GitHub's query filter bar, and the new issue
//! flow including the template chooser and YAML issue forms.

use super::{RepoSub, on};
use crate::app::Cx;
use crate::github::models::{Content, Issue, Label, Milestone, Repo, SearchResult, User};
use crate::icons::{Icon, icon};
use crate::route::{RepoView, Route};
use crate::task::Load;
use crate::templates::{self, IssueForm};
use crate::theme::semibold;
use crate::views::common::{self, Composer};
use crate::widgets::{self as w, FlashKind, Variant};
use egui::{CornerRadius, Frame, Margin, RichText, Stroke, Ui};
use serde_json::json;

pub struct IssueList {
    pulls: bool,
    q: String,
    page: u32,
    items: Load<SearchResult<Issue>>,
    counts: Load<(u64, u64)>,
}

impl IssueList {
    pub fn new(q: Option<String>, pulls: bool) -> Self {
        let kind = if pulls { "is:pr" } else { "is:issue" };
        Self { pulls, q: q.unwrap_or_else(|| format!("{kind} is:open")), page: 1, items: Load::Idle, counts: Load::Idle }
    }

    fn load(&mut self, ctx: &egui::Context, cx: &Cx, repo: &Repo) {
        let scope = format!("repo:{}", repo.full_name);
        let (g, q, p) = (cx.gh.clone(), format!("{scope} {}", self.q), self.page);
        self.items = Load::spawn(ctx, move || g.search_issues(&q, "created", p));
        // counts for the Open / Closed toggles ignore the state part of the query
        let base: String = self
            .q
            .split_whitespace()
            .filter(|t| !t.starts_with("is:open") && !t.starts_with("is:closed") && !t.starts_with("state:"))
            .collect::<Vec<_>>()
            .join(" ");
        let g = cx.gh.clone();
        self.counts = Load::spawn(ctx, move || {
            let open = g.search_issues(&format!("{scope} {base} is:open"), "", 1)?.total_count;
            let closed = g.search_issues(&format!("{scope} {base} is:closed"), "", 1)?.total_count;
            Ok((open, closed))
        });
    }

    fn set_state(&mut self, open: bool) {
        let mut parts: Vec<String> = self
            .q
            .split_whitespace()
            .filter(|t| !t.starts_with("is:open") && !t.starts_with("is:closed") && !t.starts_with("state:"))
            .map(str::to_string)
            .collect();
        parts.insert(1.min(parts.len()), if open { "is:open".into() } else { "is:closed".into() });
        self.q = parts.join(" ");
        self.page = 1;
    }
}

impl RepoSub for IssueList {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let ctx = ui.ctx().clone();
        if self.items.is_idle() {
            self.load(&ctx, cx, repo);
        }
        let (o, n) = on(repo);
        let mut reload = false;
        super::section(ui, |ui| {
            let kind = if self.pulls { "is:pr" } else { "is:issue" };
            let presets_issue = [
                ("Open issues", format!("{kind} is:open")),
                ("Your issues", format!("{kind} is:open author:@me")),
                ("Everything assigned to you", format!("{kind} is:open assignee:@me")),
                ("Everything mentioning you", format!("{kind} is:open mentions:@me")),
            ];
            let presets_pr = [
                ("Open pull requests", format!("{kind} is:open")),
                ("Your pull requests", format!("{kind} is:open author:@me")),
                ("Everything assigned to you", format!("{kind} is:open assignee:@me")),
                ("Review requests", format!("{kind} is:open review-requested:@me")),
            ];
            let presets: Vec<(&str, &str)> = if self.pulls {
                presets_pr.iter().map(|(a, b)| (*a, b.as_str())).collect()
            } else {
                presets_issue.iter().map(|(a, b)| (*a, b.as_str())).collect()
            };
            ui.horizontal(|ui| {
                let avail = ui.available_width();
                ui.allocate_ui_with_layout(egui::vec2(avail - 330.0, 32.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    if common::filter_bar(ui, &mut self.q, &presets) {
                        self.page = 1;
                        reload = true;
                    }
                });
                w::right(ui, |ui| {
                    if self.pulls {
                        if w::button(ui, Variant::Primary, None, "New pull request").clicked() {
                            cx.go(Route::repo(&o, &n, RepoView::NewPull { base: None, head: None }));
                        }
                    } else if w::button_ex(ui, Variant::Primary, None, "New issue", None, repo.has_issues).clicked() {
                        cx.go(Route::repo(&o, &n, RepoView::NewIssue));
                    }
                    if w::button(ui, Variant::Default, Some(Icon::Tag), "Labels").clicked() {
                        cx.go(Route::repo(&o, &n, RepoView::Labels));
                    }
                });
            });
            ui.add_space(16.0);
            if !self.pulls && !repo.has_issues {
                w::flash(ui, FlashKind::Warning, "Issues are disabled for this repository. Enable them under Settings → Features.");
                ui.add_space(16.0);
            }
            let open_sel = !self.q.contains("is:closed");
            w::bordered(ui, |ui| {
                w::box_header(ui, |ui| {
                    // no numbers until the counts land, a fake "0" reads like an empty repo
                    let counts = self.counts.ready().copied();
                    let label = |n: Option<u64>, s: &str| n.map(|n| format!("{} {s}", w::format_count(n))).unwrap_or_else(|| s.to_string());
                    let (oi, ci) = if self.pulls { (Icon::GitPullRequest, Icon::Check) } else { (Icon::IssueOpened, Icon::Check) };
                    if w::tab_link(ui, oi, &label(counts.map(|c| c.0), "Open"), open_sel).clicked() {
                        self.set_state(true);
                        reload = true;
                    }
                    if w::tab_link(ui, ci, &label(counts.map(|c| c.1), "Closed"), !open_sel).clicked() {
                        self.set_state(false);
                        reload = true;
                    }
                });
                let mut has_next = false;
                w::show_load(ui, &mut self.items, |ui, res| {
                    if res.items.is_empty() {
                        let (ic, what) = if self.pulls { (Icon::GitPullRequest, "pull requests") } else { (Icon::IssueOpened, "issues") };
                        w::blankslate(
                            ui,
                            ic,
                            "No results matched your search.",
                            &format!("There aren't any {what} that match this query."),
                        );
                    }
                    for it in &res.items {
                        common::issue_row(ui, it, false, cx);
                    }
                    has_next = res.items.len() >= 30;
                });
                if common::pager(ui, &mut self.page, has_next) {
                    reload = true;
                }
            });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                icon(ui, Icon::Info, w::p(ui).fg_muted);
                w::muted_small(
                    ui,
                    "Tip: the filter accepts GitHub search qualifiers like label:bug, author:@me, no:assignee, sort:updated-desc.",
                );
            });
        });
        if reload {
            self.load(&ctx, cx, repo);
        }
    }
}

/// One file under .github/ISSUE_TEMPLATE, already parsed.
#[derive(Clone)]
enum IssueTemplate {
    Markdown { name: String, about: String, title: String, labels: Vec<String>, body: String },
    Form(IssueForm),
}

impl IssueTemplate {
    fn name(&self) -> &str {
        match self {
            IssueTemplate::Markdown { name, .. } => name,
            IssueTemplate::Form(f) => &f.name,
        }
    }
    fn about(&self) -> &str {
        match self {
            IssueTemplate::Markdown { about, .. } => about,
            IssueTemplate::Form(f) => &f.description,
        }
    }
}

fn parse_template(file: &Content) -> Option<IssueTemplate> {
    let text = file.text()?;
    let lower = file.name.to_lowercase();
    if lower == "config.yml" || lower == "config.yaml" {
        return None;
    }
    if lower.ends_with(".yml") || lower.ends_with(".yaml") {
        return templates::parse_issue_form(&text).ok().map(IssueTemplate::Form);
    }
    let (fm, body) = templates::split_front_matter(&text);
    let v: serde_yaml_ng::Value = fm.as_deref().and_then(|f| serde_yaml_ng::from_str(f).ok()).unwrap_or(serde_yaml_ng::Value::Null);
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string();
    let labels = match v.get("labels") {
        Some(serde_yaml_ng::Value::Sequence(seq)) => seq.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        Some(serde_yaml_ng::Value::String(s)) => s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
        _ => vec![],
    };
    let name = if s("name").is_empty() { file.name.clone() } else { s("name") };
    Some(IssueTemplate::Markdown { name, about: s("about"), title: s("title"), labels, body })
}

/// Loads issue templates from the repo's default branch.
fn load_templates(cx: &Cx, ctx: &egui::Context, repo: &Repo) -> Load<Vec<IssueTemplate>> {
    let (g, (o, n), b) = (cx.gh.clone(), on(repo), repo.default_branch.clone());
    Load::spawn(ctx, move || {
        let list = match g.contents(&o, &n, ".github/ISSUE_TEMPLATE", &b) {
            Ok(l) => l,
            Err(e) if e.to_string().contains("404") => return Ok(vec![]),
            Err(e) => return Err(e),
        };
        let mut out = vec![];
        for f in list.into_iter().filter(|f| !f.is_dir()) {
            let full = g.file(&o, &n, &f.path, &b)?;
            if let Some(t) = parse_template(&full) {
                out.push(t);
            }
        }
        Ok(out)
    })
}

#[derive(Default)]
pub struct NewIssue {
    templates: Load<Vec<IssueTemplate>>,
    chosen: Option<Option<IssueTemplate>>,
    title: String,
    composer: Composer,
    form_values: Vec<String>,
    form_checks: Vec<Vec<bool>>,
    labels_all: Load<Vec<Label>>,
    users_all: Load<Vec<User>>,
    milestones: Load<Vec<Milestone>>,
    labels: Vec<String>,
    assignees: Vec<String>,
    milestone: Option<u64>,
    lf: String,
    af: String,
    create: Load<Issue>,
    error: Option<String>,
}

impl NewIssue {
    fn choose(&mut self, t: Option<IssueTemplate>) {
        match &t {
            Some(IssueTemplate::Markdown { title, labels, body, .. }) => {
                self.title = title.clone();
                self.composer = Composer::with_text(body.clone());
                self.labels = labels.clone();
            }
            Some(IssueTemplate::Form(f)) => {
                self.title = f.title.clone();
                self.labels = f.labels.clone();
                self.form_values = f.fields.iter().map(|x| x.value.clone()).collect();
                self.form_checks = f.fields.iter().map(|x| vec![false; x.options.len()]).collect();
            }
            None => {}
        }
        self.chosen = Some(t);
    }

    /// Issue forms submit as "### Label" sections, which is what github.com produces.
    fn form_body(&self, f: &IssueForm) -> String {
        let mut out = String::new();
        for (i, field) in f.fields.iter().enumerate() {
            if field.kind == "markdown" {
                continue;
            }
            out.push_str(&format!("### {}\n\n", field.label));
            let v = match field.kind.as_str() {
                "checkboxes" => field
                    .options
                    .iter()
                    .zip(self.form_checks.get(i).cloned().unwrap_or_default())
                    .map(|(o, c)| format!("- [{}] {o}", if c { "x" } else { " " }))
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => {
                    let v = self.form_values.get(i).cloned().unwrap_or_default();
                    match (&field.render, v.trim().is_empty()) {
                        (_, true) => "_No response_".to_string(),
                        (Some(lang), false) => format!("```{lang}\n{v}\n```"),
                        (None, false) => v,
                    }
                }
            };
            out.push_str(&v);
            out.push_str("\n\n");
        }
        out
    }

    fn missing_required(&self, f: &IssueForm) -> Option<String> {
        f.fields.iter().enumerate().find_map(|(i, x)| {
            let empty = match x.kind.as_str() {
                "checkboxes" | "markdown" => false,
                _ => self.form_values.get(i).is_none_or(|v| v.trim().is_empty()),
            };
            (x.required && empty).then(|| x.label.clone())
        })
    }
}

impl RepoSub for NewIssue {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let ctx = ui.ctx().clone();
        let (o, n) = on(repo);
        if self.templates.is_idle() {
            self.templates = load_templates(cx, &ctx, repo);
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.labels_all = Load::spawn(&ctx, move || g.labels(&o2, &n2));
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.users_all = Load::spawn(&ctx, move || g.assignees(&o2, &n2));
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.milestones = Load::spawn(&ctx, move || g.milestones(&o2, &n2));
        }
        match self.create.take() {
            Some(Ok(i)) => {
                cx.ok(format!("Created issue #{}", i.number));
                cx.go(Route::repo(&o, &n, RepoView::Issue(i.number)));
            }
            Some(Err(e)) => self.error = Some(e),
            None => {}
        }
        let pal = w::p(ui);
        super::section(ui, |ui| {
            // template chooser comes first when the repo has templates
            if self.chosen.is_none() {
                match self.templates.poll() {
                    Load::Ready(list) if !list.is_empty() => {
                        let list = list.clone();
                        w::heading(ui, "Create new issue", 24.0);
                        ui.add_space(4.0);
                        w::muted(ui, "Select a template to get started.");
                        ui.add_space(16.0);
                        w::bordered(ui, |ui| {
                            for t in list {
                                w::list_row(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            ui.label(RichText::new(t.name()).font(semibold(14.0)));
                                            if !t.about().is_empty() {
                                                w::muted_small(ui, t.about());
                                            }
                                        });
                                        w::right(ui, |ui| {
                                            if w::button(ui, Variant::Primary, None, "Get started").clicked() {
                                                self.choose(Some(t.clone()));
                                            }
                                        });
                                    });
                                });
                                w::divider(ui);
                            }
                        });
                        ui.add_space(12.0);
                        if w::link(ui, "Open a blank issue").clicked() {
                            self.choose(None);
                        }
                        return;
                    }
                    Load::Ready(_) | Load::Failed(_) => self.choose(None),
                    _ => {
                        w::spinner(ui);
                        return;
                    }
                }
            }
            let template = self.chosen.clone().flatten();
            ui.horizontal(|ui| {
                w::heading(ui, "Create new issue", 24.0);
                if let Some(t) = &template {
                    w::outline_label(ui, t.name(), None);
                    if w::link(ui, "Choose a different template").clicked() {
                        self.chosen = None;
                    }
                }
            });
            ui.add_space(16.0);
            if let Some(e) = &self.error {
                w::flash(ui, FlashKind::Danger, e);
                ui.add_space(12.0);
            }
            ui.horizontal_top(|ui| {
                let main_w = (ui.available_width() - 280.0).max(400.0);
                ui.allocate_ui_with_layout(egui::vec2(main_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_width(main_w);
                    ui.horizontal_top(|ui| {
                        w::avatar(ui, &cx.me.avatar_url, 40.0);
                        ui.vertical(|ui| {
                            w::form_label(ui, "Add a title", true);
                            w::text_input(ui, &mut self.title, "Title", ui.available_width());
                            ui.add_space(12.0);
                            let mut submit = false;
                            match &template {
                                Some(IssueTemplate::Form(f)) => {
                                    let f = f.clone();
                                    self.form_ui(ui, &f);
                                    ui.add_space(12.0);
                                    ui.horizontal(|ui| {
                                        w::right(ui, |ui| {
                                            let ok = !self.title.trim().is_empty() && !self.create.is_loading();
                                            if w::button_ex(ui, Variant::Primary, None, "Create", None, ok).clicked() {
                                                if let Some(missing) = self.missing_required(&f) {
                                                    self.error = Some(format!("\"{missing}\" is required."));
                                                } else {
                                                    submit = true;
                                                }
                                            }
                                        });
                                    });
                                }
                                _ => {
                                    w::form_label(ui, "Add a description", false);
                                    let ok = !self.title.trim().is_empty() && !self.create.is_loading();
                                    self.composer.ui(ui, "Type your description here…", 12, |ui, _| {
                                        if w::button_ex(ui, Variant::Primary, None, "Create", None, ok).clicked() {
                                            submit = true;
                                        }
                                    });
                                }
                            }
                            if submit {
                                let body = match &template {
                                    Some(IssueTemplate::Form(f)) => self.form_body(f),
                                    _ => self.composer.text.clone(),
                                };
                                let mut payload = json!({
                                    "title": self.title.trim(),
                                    "body": body,
                                    "labels": self.labels,
                                    "assignees": self.assignees,
                                });
                                if let Some(m) = self.milestone {
                                    payload["milestone"] = json!(m);
                                }
                                let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
                                self.error = None;
                                self.create = Load::spawn(&ctx, move || g.create_issue(&o2, &n2, &payload));
                            }
                        });
                    });
                });
                ui.add_space(24.0);
                ui.vertical(|ui| {
                    ui.set_width(256.0);
                    let can_triage = repo.permissions.as_ref().is_some_and(|p| p.triage || p.push || p.admin);
                    let users = self.users_all.ready().cloned().unwrap_or_default();
                    let cur_assignees = self.assignees.clone();
                    let cur_labels = self.labels.clone();
                    let cur_ms = self.milestone;
                    common::sidebar_section(
                        ui,
                        "Assignees",
                        can_triage,
                        |ui| {
                            if cur_assignees.is_empty() {
                                w::muted_small(ui, "No one assigned");
                            }
                            for a in &cur_assignees {
                                w::meta(ui, Icon::Person, a.as_str());
                            }
                        },
                        |ui| {
                            common::user_picker(ui, "Assign up to 10 people", &users, &mut self.assignees, &mut self.af, 10);
                        },
                    );
                    let labels = self.labels_all.ready().cloned().unwrap_or_default();
                    common::sidebar_section(
                        ui,
                        "Labels",
                        can_triage,
                        |ui| {
                            if cur_labels.is_empty() {
                                w::muted_small(ui, "None yet");
                            }
                            ui.horizontal_wrapped(|ui| {
                                for l in &cur_labels {
                                    let color = labels.iter().find(|x| &x.name == l).map(|x| x.color.clone()).unwrap_or_default();
                                    w::issue_label(ui, l, &color);
                                }
                            });
                        },
                        |ui| {
                            common::label_picker(ui, &labels, &mut self.labels, &mut self.lf);
                        },
                    );
                    let ms = self.milestones.ready().cloned().unwrap_or_default();
                    common::sidebar_section(
                        ui,
                        "Milestone",
                        can_triage,
                        |ui| match ms.iter().find(|m| Some(m.number) == cur_ms) {
                            Some(m) => {
                                w::meta(ui, Icon::Milestone, m.title.as_str());
                            }
                            None => {
                                w::muted_small(ui, "No milestone");
                            }
                        },
                        |ui| {
                            if w::menu_item_ex(ui, None, "No milestone", self.milestone.is_none(), false).clicked() {
                                self.milestone = None;
                            }
                            for m in &ms {
                                if w::menu_item_ex(ui, Some(Icon::Milestone), &m.title, self.milestone == Some(m.number), false).clicked() {
                                    self.milestone = Some(m.number);
                                }
                            }
                        },
                    );
                    if !can_triage {
                        w::muted_small(ui, "Only people with triage access can set metadata.");
                    }
                });
            });
            let _ = pal;
        });
    }
}

impl NewIssue {
    fn form_ui(&mut self, ui: &mut Ui, f: &IssueForm) {
        let pal = w::p(ui);
        let mut md = egui_commonmark::CommonMarkCache::default();
        for (i, field) in f.fields.iter().enumerate() {
            if self.form_values.len() <= i {
                self.form_values.resize(i + 1, String::new());
                self.form_checks.resize(i + 1, vec![]);
            }
            match field.kind.as_str() {
                "markdown" => {
                    Frame::new().inner_margin(Margin::symmetric(0, 4)).show(ui, |ui| {
                        common::markdown(ui, &mut md, &field.value, None);
                    });
                }
                "input" | "textarea" | "dropdown" | "checkboxes" => {
                    w::form_label(ui, &field.label, field.required);
                    if !field.description.is_empty() {
                        w::muted_small(ui, &field.description);
                    }
                    ui.add_space(4.0);
                    match field.kind.as_str() {
                        "input" => {
                            w::text_input(ui, &mut self.form_values[i], &field.placeholder, ui.available_width());
                        }
                        "textarea" => {
                            w::text_area(ui, &mut self.form_values[i], &field.placeholder, 5, field.render.is_some());
                        }
                        "dropdown" => {
                            let cur =
                                if self.form_values[i].is_empty() { "Selections: None".to_string() } else { self.form_values[i].clone() };
                            let b = w::button_ex(ui, Variant::Default, None, &cur, Some(Icon::TriangleDown), true);
                            egui::Popup::menu(&b).show(|ui| {
                                for o in &field.options {
                                    if w::menu_item_ex(ui, None, o, &self.form_values[i] == o, false).clicked() {
                                        self.form_values[i] = o.clone();
                                    }
                                }
                            });
                        }
                        _ => {
                            if self.form_checks[i].len() != field.options.len() {
                                self.form_checks[i] = vec![false; field.options.len()];
                            }
                            for (j, o) in field.options.iter().enumerate() {
                                ui.checkbox(&mut self.form_checks[i][j], o.as_str());
                            }
                        }
                    }
                }
                other => {
                    Frame::new()
                        .stroke(Stroke::new(1.0, pal.border))
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(Margin::same(8))
                        .show(ui, |ui| w::muted_small(ui, format!("Unsupported field type \"{other}\" ({})", field.label)));
                }
            }
            ui.add_space(12.0);
        }
    }
}
