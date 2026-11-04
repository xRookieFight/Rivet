//! The Files tab: dedicated managers for README, issue / PR templates and
//! community health + config files, all sharing one editor that commits
//! through the contents API.

use super::{RepoSub, on};
use crate::app::Cx;
use crate::github::models::{Content, LicenseTemplate, Repo};
use crate::icons::{Icon, icon};
use crate::route::{RepoView, Route};
use crate::task::Load;
use crate::templates::{self, Template};
use crate::theme::semibold;
use crate::views::common::{markdown, raw_base};
use crate::widgets::{self as w, FlashKind, Variant};
use egui::{CornerRadius, Frame, Margin, RichText, Stroke, Ui};
use egui_commonmark::CommonMarkCache;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Markdown,
    IssueForm,
    Yaml,
    Text,
}

fn kind_of(path: &str) -> Kind {
    let l = path.to_lowercase();
    if l.ends_with(".md") {
        Kind::Markdown
    } else if (l.ends_with(".yml") || l.ends_with(".yaml")) && l.contains("issue_template/") && !l.ends_with("config.yml") {
        Kind::IssueForm
    } else if l.ends_with(".yml") || l.ends_with(".yaml") {
        Kind::Yaml
    } else {
        Kind::Text
    }
}

/// Edit + preview + commit for one file in the repo.
struct FileEditor {
    path: String,
    existing: Load<Option<Content>>,
    loaded: bool,
    buffer: String,
    original: String,
    mode: usize,
    message: String,
    new_branch: bool,
    branch: String,
    save: Load<String>,
    md: CommonMarkCache,
}

impl FileEditor {
    fn new(path: String) -> Self {
        Self {
            path,
            existing: Load::Idle,
            loaded: false,
            buffer: String::new(),
            original: String::new(),
            mode: 2,
            message: String::new(),
            new_branch: false,
            branch: String::new(),
            save: Load::Idle,
            md: CommonMarkCache::default(),
        }
    }

    fn with_text(path: String, text: String) -> Self {
        let mut e = Self::new(path);
        e.buffer = text;
        e
    }

    fn dirty(&self) -> bool {
        self.buffer != self.original
    }

    /// Returns true after a successful commit so the owner can refresh lists.
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) -> bool {
        let ctx = ui.ctx().clone();
        let (o, n) = on(repo);
        let pal = w::p(ui);
        if self.existing.is_idle() {
            let (g, o2, n2, p, b) = (cx.gh.clone(), o.clone(), n.clone(), self.path.clone(), repo.default_branch.clone());
            self.existing = Load::spawn(&ctx, move || g.file_opt(&o2, &n2, &p, &b));
        }
        if !self.loaded
            && let Load::Ready(c) = self.existing.poll()
        {
            if let Some(text) = c.as_ref().and_then(|c| c.text()) {
                self.original = text.clone();
                if self.buffer.is_empty() {
                    self.buffer = text;
                }
            }
            self.loaded = true;
        }
        let mut saved = false;
        match self.save.take() {
            Some(Ok(m)) => {
                cx.ok(m);
                if self.new_branch {
                    cx.go(Route::repo(
                        &o,
                        &n,
                        RepoView::NewPull { base: Some(repo.default_branch.clone()), head: Some(self.branch.clone()) },
                    ));
                }
                self.original = self.buffer.clone();
                self.existing = Load::Idle;
                self.loaded = false;
                saved = true;
            }
            Some(Err(e)) => cx.err(e),
            None => {}
        }
        let exists = matches!(self.existing.poll(), Load::Ready(Some(_)));
        ui.horizontal(|ui| {
            icon(ui, Icon::File, pal.fg_muted);
            ui.label(RichText::new(&self.path).monospace().strong());
            if !self.loaded {
                ui.add(egui::Spinner::new().size(14.0));
            } else if exists {
                w::outline_label(ui, "In repository", Some(pal.success));
            } else {
                w::outline_label(ui, "New file", Some(pal.accent));
            }
            if self.dirty() {
                w::outline_label(ui, "Unsaved changes", Some(pal.attention));
            }
            w::right(ui, |ui| {
                for (i, l) in ["Preview", "Split", "Edit"].iter().enumerate().rev() {
                    let idx = 2 - i;
                    let v = if self.mode == idx { Variant::Default } else { Variant::Invisible };
                    if w::button(ui, v, None, l).clicked() {
                        self.mode = idx;
                    }
                }
            });
        });
        ui.add_space(8.0);
        let kind = kind_of(&self.path);
        let editor_h = 520.0;
        let lang = super::code::lang_for(&self.path).to_string();
        let edit = |ui: &mut Ui, buf: &mut String| {
            w::bordered(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("fe-edit").max_height(editor_h).show(ui, |ui| {
                    let theme = egui_extras::syntax_highlighting::CodeTheme::from_style(ui.style());
                    let mut layouter = |ui: &Ui, b: &dyn egui::TextBuffer, wrap: f32| {
                        let mut job = egui_extras::syntax_highlighting::highlight(ui.ctx(), ui.style(), &theme, b.as_str(), &lang);
                        job.wrap.max_width = wrap;
                        ui.fonts_mut(|f| f.layout_job(job))
                    };
                    ui.add(
                        egui::TextEdit::multiline(buf)
                            .code_editor()
                            .frame(Frame::NONE)
                            .desired_width(f32::INFINITY)
                            .desired_rows(26)
                            .layouter(&mut layouter),
                    );
                });
            });
        };
        let base = raw_base(&o, &n, &repo.default_branch, "");
        let preview = |ui: &mut Ui, buf: &str, md: &mut CommonMarkCache| {
            w::bordered(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("fe-prev").max_height(editor_h).show(ui, |ui| {
                    Frame::new().inner_margin(Margin::same(24)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        render_preview(ui, kind, buf, md, &base);
                    });
                });
            });
        };
        match self.mode {
            0 => preview(ui, &self.buffer, &mut self.md),
            1 => {
                ui.columns(2, |cols| {
                    edit(&mut cols[0], &mut self.buffer);
                    preview(&mut cols[1], &self.buffer, &mut self.md);
                });
            }
            _ => edit(ui, &mut self.buffer),
        }
        ui.add_space(12.0);
        if !repo.can_push() {
            w::flash(ui, FlashKind::Warning, "You need write access to commit changes to this repository.");
            return saved;
        }
        Frame::new().stroke(Stroke::new(1.0, pal.border)).corner_radius(CornerRadius::same(6)).inner_margin(Margin::same(12)).show(
            ui,
            |ui| {
                ui.set_width(ui.available_width());
                let default_msg = if exists { format!("Update {}", self.path) } else { format!("Create {}", self.path) };
                ui.horizontal(|ui| {
                    w::text_input(ui, &mut self.message, &default_msg, (ui.available_width() - 200.0).max(200.0));
                    w::right(ui, |ui| {
                        let busy = self.save.is_loading();
                        let ok = self.loaded && !busy && (self.dirty() || !exists) && !self.buffer.is_empty();
                        if w::button_ex(ui, Variant::Primary, None, if busy { "Committing…" } else { "Commit changes" }, None, ok).clicked()
                        {
                            if self.new_branch && self.branch.trim().is_empty() {
                                self.branch =
                                    format!("update-{}", self.path.rsplit('/').next().unwrap_or("file").to_lowercase().replace('.', "-"));
                            }
                            let msg = if self.message.trim().is_empty() { default_msg.clone() } else { self.message.trim().to_string() };
                            let sha = match self.existing.poll() {
                                Load::Ready(Some(c)) => Some(c.sha.clone()),
                                _ => None,
                            };
                            let (g, o2, n2, p, content) = (cx.gh.clone(), o.clone(), n.clone(), self.path.clone(), self.buffer.clone());
                            let (base, nb) = (repo.default_branch.clone(), self.new_branch.then(|| self.branch.trim().to_string()));
                            self.save = Load::spawn(&ctx, move || {
                                let target = match &nb {
                                    Some(b) => {
                                        let head =
                                            g.commits(&o2, &n2, &base, None, 1)?.into_iter().next().map(|c| c.sha).unwrap_or_default();
                                        g.create_branch(&o2, &n2, b, &head)?;
                                        b.clone()
                                    }
                                    None => base,
                                };
                                g.put_file(&o2, &n2, &p, content.as_bytes(), &msg, &target, sha.as_deref())?;
                                Ok(format!("Committed {p} to {target}"))
                            });
                        }
                        if self.dirty() && w::button(ui, Variant::Invisible, None, "Discard").clicked() {
                            self.buffer = self.original.clone();
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.new_branch, "Commit to a new branch and open a pull request");
                    if self.new_branch {
                        w::text_input(ui, &mut self.branch, "branch name", 240.0);
                    }
                });
            },
        );
        saved
    }
}

fn render_preview(ui: &mut Ui, kind: Kind, buf: &str, md: &mut CommonMarkCache, base: &str) {
    let pal = w::p(ui);
    match kind {
        Kind::Markdown => {
            let (fm, body) = templates::split_front_matter(buf);
            if let Some(fm) = fm {
                Frame::new().fill(pal.canvas_subtle).corner_radius(CornerRadius::same(6)).inner_margin(Margin::same(8)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(fm).monospace().size(12.0).color(pal.fg_muted));
                });
                ui.add_space(8.0);
            }
            markdown(ui, md, &body, Some(base));
        }
        Kind::IssueForm => match templates::parse_issue_form(buf) {
            Ok(f) => {
                ui.label(RichText::new(&f.name).font(semibold(20.0)));
                if !f.description.is_empty() {
                    w::muted(ui, &f.description);
                }
                ui.add_space(12.0);
                for field in &f.fields {
                    match field.kind.as_str() {
                        "markdown" => markdown(ui, md, &field.value, None),
                        _ => {
                            w::form_label(ui, &field.label, field.required);
                            if !field.description.is_empty() {
                                w::muted_small(ui, &field.description);
                            }
                            let mut ph = field.placeholder.clone();
                            match field.kind.as_str() {
                                "textarea" => {
                                    w::text_area(ui, &mut ph, "", 3, field.render.is_some());
                                }
                                "dropdown" => {
                                    ui.horizontal_wrapped(|ui| {
                                        for o in &field.options {
                                            w::outline_label(ui, o, None);
                                        }
                                    });
                                }
                                "checkboxes" => {
                                    for o in &field.options {
                                        let mut b = false;
                                        ui.checkbox(&mut b, o.as_str());
                                    }
                                }
                                _ => {
                                    w::text_input(ui, &mut ph, "", ui.available_width());
                                }
                            }
                        }
                    }
                    ui.add_space(10.0);
                }
            }
            Err(e) => w::flash(ui, FlashKind::Danger, &format!("Invalid issue form: {e}")),
        },
        Kind::Yaml => match serde_yaml_ng::from_str::<serde_yaml_ng::Value>(buf) {
            Ok(_) => {
                w::flash(ui, FlashKind::Success, "Valid YAML.");
                ui.add_space(8.0);
                ui.label(RichText::new(buf).monospace().size(12.0));
            }
            Err(e) => w::flash(ui, FlashKind::Danger, &format!("YAML error: {e}")),
        },
        Kind::Text => {
            ui.label(RichText::new(buf).monospace().size(12.0));
        }
    }
}

/// Left nav shared by the three managers.
fn files_nav(ui: &mut Ui, cx: &mut Cx, repo: &Repo, current: usize) {
    let pal = w::p(ui);
    let (o, n) = on(repo);
    let items = [
        (Icon::Book, "README", RepoView::Readme),
        (Icon::FileBadge, "Templates", RepoView::Templates),
        (Icon::People, "Community & config", RepoView::CommunityFiles),
        (Icon::Tag, "Labels", RepoView::Labels),
    ];
    for (i, (ic, label, view)) in items.into_iter().enumerate() {
        let r = w::menu_item(ui, Some(ic), label);
        if i == current {
            ui.painter().rect_filled(
                egui::Rect::from_min_size(r.rect.left_top() + egui::vec2(-8.0, 6.0), egui::vec2(4.0, 20.0)),
                CornerRadius::same(2),
                pal.accent_emphasis,
            );
        }
        if r.clicked() && i != current {
            cx.go_replace(Route::repo(&o, &n, view));
        }
    }
}

fn layout(ui: &mut Ui, cx: &mut Cx, repo: &Repo, current: usize, add: impl FnOnce(&mut Ui, &mut Cx)) {
    super::section(ui, |ui| {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(220.0);
                files_nav(ui, cx, repo, current);
            });
            ui.add_space(24.0);
            ui.vertical(|ui| add(ui, cx));
        });
    });
}

#[derive(Default)]
pub struct ReadmeManager {
    editor: Option<FileEditor>,
    readme_path: Load<String>,
}

impl RepoSub for ReadmeManager {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let ctx = ui.ctx().clone();
        if self.readme_path.is_idle() {
            let (g, (o, n), b) = (cx.gh.clone(), on(repo), repo.default_branch.clone());
            self.readme_path = Load::spawn(&ctx, move || Ok(g.readme(&o, &n, &b)?.map(|c| c.path).unwrap_or_else(|| "README.md".into())));
        }
        if self.editor.is_none()
            && let Some(p) = self.readme_path.ready()
        {
            self.editor = Some(FileEditor::new(p.clone()));
        }
        let (o, n) = on(repo);
        let desc = repo.description.clone().unwrap_or_default();
        layout(ui, cx, repo, 0, |ui, cx| {
            w::heading(ui, "README", 24.0);
            w::muted(ui, "Edit the README with a live GitHub Flavored Markdown preview, apply templates and insert badges.");
            ui.add_space(12.0);
            let Some(ed) = &mut self.editor else {
                w::spinner(ui);
                return;
            };
            ui.horizontal_wrapped(|ui| {
                let t = w::button_ex(ui, Variant::Default, Some(Icon::FileBadge), "Templates", Some(Icon::TriangleDown), true);
                egui::Popup::menu(&t).width(240.0).show(|ui| {
                    for tpl in templates::README {
                        if w::menu_item(ui, Some(Icon::File), tpl.name).clicked() {
                            ed.buffer = templates::fill(tpl.body, &o, &n, &desc, &repo.default_branch);
                        }
                    }
                });
                let b = w::button_ex(ui, Variant::Default, Some(Icon::Verified), "Badges", Some(Icon::TriangleDown), true);
                egui::Popup::menu(&b).width(240.0).show(|ui| {
                    for (name, snippet) in templates::BADGES {
                        if w::menu_item(ui, None, name).clicked() {
                            let badge = templates::fill(snippet, &o, &n, &desc, &repo.default_branch);
                            insert_badge(&mut ed.buffer, &badge);
                        }
                    }
                });
                w::muted_small(ui, "Badges are inserted under the first heading.");
            });
            ui.add_space(12.0);
            ed.ui(ui, cx, repo);
        });
    }
}

/// Badges go on the line after the first heading, grouped together.
fn insert_badge(buf: &mut String, badge: &str) {
    if buf.contains(badge) {
        return;
    }
    let mut lines: Vec<String> = buf.lines().map(str::to_string).collect();
    let pos = lines.iter().position(|l| l.starts_with('#')).map(|i| i + 1).unwrap_or(0);
    // keep badges in a single row with the ones already there
    let mut at = pos;
    while at < lines.len() && (lines[at].trim().is_empty() && at == pos || lines[at].starts_with("[![")) {
        at += 1;
    }
    if at == pos {
        lines.insert(pos, String::new());
        lines.insert(pos + 1, badge.to_string());
    } else {
        lines.insert(at, badge.to_string());
    }
    *buf = lines.join("\n");
    if !buf.ends_with('\n') {
        buf.push('\n');
    }
}

#[derive(Clone)]
struct RepoTemplate {
    path: String,
    category: &'static str,
}

#[derive(Default)]
pub struct TemplateManager {
    list: Load<Vec<RepoTemplate>>,
    editor: Option<FileEditor>,
    new_name: String,
    new_kind: usize,
    new_starter: usize,
}

fn load_template_files(cx: &Cx, ctx: &egui::Context, repo: &Repo) -> Load<Vec<RepoTemplate>> {
    let (g, (o, n), b) = (cx.gh.clone(), on(repo), repo.default_branch.clone());
    Load::spawn(ctx, move || {
        let mut out = vec![];
        if let Ok(list) = g.contents(&o, &n, ".github/ISSUE_TEMPLATE", &b) {
            for f in list.into_iter().filter(|f| !f.is_dir()) {
                let l = f.name.to_lowercase();
                let category = if l == "config.yml" || l == "config.yaml" {
                    "Template chooser config"
                } else if l.ends_with(".yml") || l.ends_with(".yaml") {
                    "Issue forms"
                } else {
                    "Issue templates"
                };
                out.push(RepoTemplate { path: f.path, category });
            }
        }
        if let Ok(list) = g.contents(&o, &n, ".github/PULL_REQUEST_TEMPLATE", &b) {
            for f in list.into_iter().filter(|f| !f.is_dir()) {
                out.push(RepoTemplate { path: f.path, category: "Pull request templates" });
            }
        }
        for p in [".github/pull_request_template.md", "pull_request_template.md", "docs/pull_request_template.md"] {
            if g.file_opt(&o, &n, p, &b)?.is_some() {
                out.push(RepoTemplate { path: p.into(), category: "Pull request templates" });
            }
        }
        Ok(out)
    })
}

const CATEGORIES: [&str; 4] = ["Issue forms", "Issue templates", "Template chooser config", "Pull request templates"];

impl RepoSub for TemplateManager {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let ctx = ui.ctx().clone();
        if self.list.is_idle() {
            self.list = load_template_files(cx, &ctx, repo);
        }
        let (o, n) = on(repo);
        let desc = repo.description.clone().unwrap_or_default();
        let pal = w::p(ui);
        layout(ui, cx, repo, 1, |ui, cx| {
            if let Some(ed) = &mut self.editor {
                if super::back_link(ui, "All templates") {
                    self.editor = None;
                    return;
                }
                ui.add_space(8.0);
                if ed.ui(ui, cx, repo) {
                    self.list = Load::Idle;
                }
                return;
            }
            w::heading(ui, "Templates", 24.0);
            w::muted(ui, "Issue templates, YAML issue forms and pull request templates for this repository.");
            ui.add_space(16.0);
            let list = self.list.ready().cloned();
            match list {
                None => w::spinner(ui),
                Some(list) => {
                    for cat in CATEGORIES {
                        let items: Vec<&RepoTemplate> = list.iter().filter(|t| t.category == cat).collect();
                        w::bordered(ui, |ui| {
                            w::box_header(ui, |ui| {
                                ui.label(RichText::new(cat).font(semibold(14.0)));
                                w::counter(ui, items.len() as u64);
                            });
                            if items.is_empty() {
                                Frame::new().inner_margin(Margin::same(12)).show(ui, |ui| w::muted_small(ui, "None yet"));
                            }
                            for t in items {
                                let resp = w::list_row(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        icon(ui, Icon::File, pal.fg_muted);
                                        ui.label(RichText::new(&t.path).monospace());
                                        w::right(ui, |ui| w::muted_small(ui, "Edit"));
                                    });
                                });
                                if resp.clicked() {
                                    self.editor = Some(FileEditor::new(t.path.clone()));
                                }
                                w::divider(ui);
                            }
                        });
                        ui.add_space(12.0);
                    }
                }
            }
            ui.add_space(8.0);
            ui.label(RichText::new("Add a template").font(semibold(16.0)));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.new_kind, 0, "Issue template / form");
                ui.radio_value(&mut self.new_kind, 1, "Pull request template");
            });
            let starters: &[Template] = if self.new_kind == 0 { templates::ISSUE_TEMPLATES } else { templates::PR_TEMPLATES };
            if self.new_starter >= starters.len() {
                self.new_starter = 0;
            }
            ui.horizontal(|ui| {
                let b = w::button_ex(
                    ui,
                    Variant::Default,
                    None,
                    &format!("Starter: {}", starters[self.new_starter].name),
                    Some(Icon::TriangleDown),
                    true,
                );
                egui::Popup::menu(&b).width(260.0).show(|ui| {
                    for (i, s) in starters.iter().enumerate() {
                        if w::menu_item_ex(ui, None, s.name, i == self.new_starter, false).clicked() {
                            self.new_starter = i;
                        }
                    }
                });
                let hint = if self.new_kind == 0 { "bug_report.yml" } else { "feature.md" };
                w::text_input(ui, &mut self.new_name, hint, 220.0);
                let name_ok = !self.new_name.trim().is_empty() && !self.new_name.contains('/');
                if w::button_ex(ui, Variant::Primary, None, "Create", None, name_ok).clicked() {
                    let dir = if self.new_kind == 0 { ".github/ISSUE_TEMPLATE" } else { ".github/PULL_REQUEST_TEMPLATE" };
                    let path = format!("{dir}/{}", self.new_name.trim());
                    let body = templates::fill(starters[self.new_starter].body, &o, &n, &desc, &repo.default_branch);
                    self.editor = Some(FileEditor::with_text(path, body));
                    self.new_name.clear();
                }
            });
            ui.add_space(4.0);
            w::muted_small(
                ui,
                "Multiple pull request templates live in .github/PULL_REQUEST_TEMPLATE/ and are picked with ?template=name.md, or from the Template menu when opening a PR in Rivet.",
            );
        });
    }
}

/// One known file in the community / config list.
struct Known {
    path: &'static str,
    title: &'static str,
    desc: &'static str,
    starter: Starter,
}

enum Starter {
    Text(&'static str),
    License,
    Gitignore,
    Workflow,
}

const KNOWN: &[Known] = &[
    Known { path: "LICENSE", title: "License", desc: "Tell people how they can use your code.", starter: Starter::License },
    Known {
        path: "CONTRIBUTING.md",
        title: "Contributing",
        desc: "How people can contribute to the project.",
        starter: Starter::Text(templates::CONTRIBUTING),
    },
    Known {
        path: "CODE_OF_CONDUCT.md",
        title: "Code of conduct",
        desc: "Standards for how to engage in the community.",
        starter: Starter::Text(templates::CODE_OF_CONDUCT),
    },
    Known {
        path: "SECURITY.md",
        title: "Security policy",
        desc: "How to report security vulnerabilities.",
        starter: Starter::Text(templates::SECURITY),
    },
    Known {
        path: ".github/FUNDING.yml",
        title: "Funding",
        desc: "Shows a Sponsor button on the repository.",
        starter: Starter::Text(templates::FUNDING),
    },
    Known { path: ".gitignore", title: ".gitignore", desc: "Files git should ignore.", starter: Starter::Gitignore },
    Known {
        path: ".editorconfig",
        title: ".editorconfig",
        desc: "Consistent editor settings across contributors.",
        starter: Starter::Text(templates::EDITORCONFIG),
    },
    Known {
        path: ".github/CODEOWNERS",
        title: "CODEOWNERS",
        desc: "Automatically request reviews from owners.",
        starter: Starter::Text(templates::CODEOWNERS),
    },
    Known {
        path: ".github/dependabot.yml",
        title: "Dependabot",
        desc: "Keep dependencies up to date automatically.",
        starter: Starter::Text(templates::DEPENDABOT),
    },
    Known {
        path: ".github/workflows/ci.yml",
        title: "Actions workflow",
        desc: "Build and test on every push.",
        starter: Starter::Workflow,
    },
];

#[derive(Default)]
pub struct CommunityFiles {
    present: Load<Vec<bool>>,
    health: Load<u64>,
    workflows: Load<Vec<String>>,
    editor: Option<FileEditor>,
    licenses: Load<Vec<LicenseTemplate>>,
    gitignores: Load<Vec<String>>,
    starter_job: Load<(String, String)>,
    gi_filter: String,
}

impl RepoSub for CommunityFiles {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let ctx = ui.ctx().clone();
        let (o, n) = on(repo);
        if self.present.is_idle() {
            let (g, o2, n2, b) = (cx.gh.clone(), o.clone(), n.clone(), repo.default_branch.clone());
            self.present = Load::spawn(&ctx, move || KNOWN.iter().map(|k| Ok(g.file_opt(&o2, &n2, k.path, &b)?.is_some())).collect());
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.health = Load::spawn(&ctx, move || Ok(g.community_profile(&o2, &n2)?["health_percentage"].as_u64().unwrap_or(0)));
            let (g, o2, n2, b) = (cx.gh.clone(), o.clone(), n.clone(), repo.default_branch.clone());
            self.workflows = Load::spawn(&ctx, move || match g.contents(&o2, &n2, ".github/workflows", &b) {
                Ok(l) => Ok(l.into_iter().filter(|f| !f.is_dir()).map(|f| f.path).collect()),
                Err(e) if e.to_string().contains("404") => Ok(vec![]),
                Err(e) => Err(e),
            });
            let g = cx.gh.clone();
            self.licenses = Load::spawn(&ctx, move || g.license_templates());
            let g = cx.gh.clone();
            self.gitignores = Load::spawn(&ctx, move || g.gitignore_templates());
        }
        match self.starter_job.take() {
            Some(Ok((path, text))) => self.editor = Some(FileEditor::with_text(path, text)),
            Some(Err(e)) => cx.err(e),
            None => {}
        }
        let desc = repo.description.clone().unwrap_or_default();
        let pal = w::p(ui);
        layout(ui, cx, repo, 2, |ui, cx| {
            if let Some(ed) = &mut self.editor {
                if super::back_link(ui, "Community & config files") {
                    self.editor = None;
                    return;
                }
                ui.add_space(8.0);
                if ed.ui(ui, cx, repo) {
                    self.present = Load::Idle;
                }
                return;
            }
            ui.horizontal(|ui| {
                w::heading(ui, "Community standards & config", 24.0);
                if let Some(h) = self.health.ready() {
                    w::right(ui, |ui| {
                        ui.add(
                            egui::ProgressBar::new(*h as f32 / 100.0)
                                .desired_width(160.0)
                                .text(format!("{h}% health"))
                                .fill(pal.success_emphasis),
                        );
                    });
                }
            });
            w::muted(ui, "Files that help people use and contribute to the project, plus repository configuration.");
            ui.add_space(16.0);
            let present = self.present.ready().cloned();
            w::bordered(ui, |ui| {
                let Some(present) = present else {
                    w::spinner(ui);
                    return;
                };
                for (k, has) in KNOWN.iter().zip(present) {
                    w::list_row(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (ic, c) = if has { (Icon::CheckCircleFill, pal.success) } else { (Icon::Circle, pal.fg_muted) };
                            icon(ui, ic, c);
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(k.title).font(semibold(14.0)));
                                    ui.label(RichText::new(k.path).monospace().size(11.0).color(pal.fg_muted));
                                });
                                w::muted_small(ui, k.desc);
                            });
                            w::right(ui, |ui| {
                                if has {
                                    if w::button(ui, Variant::Default, Some(Icon::Pencil), "Edit").clicked() {
                                        self.editor = Some(FileEditor::new(k.path.to_string()));
                                    }
                                    return;
                                }
                                match &k.starter {
                                    Starter::Text(t) => {
                                        if w::button(ui, Variant::Default, Some(Icon::Plus), "Add").clicked() {
                                            self.editor = Some(FileEditor::with_text(
                                                k.path.into(),
                                                templates::fill(t, &o, &n, &desc, &repo.default_branch),
                                            ));
                                        }
                                    }
                                    Starter::Workflow => {
                                        let b = w::button_ex(ui, Variant::Default, Some(Icon::Plus), "Add", Some(Icon::TriangleDown), true);
                                        egui::Popup::menu(&b).align(egui::RectAlign::BOTTOM_END).show(|ui| {
                                            for t in templates::WORKFLOWS {
                                                if w::menu_item(ui, Some(Icon::Workflow), t.name).clicked() {
                                                    self.editor = Some(FileEditor::with_text(
                                                        k.path.into(),
                                                        templates::fill(t.body, &o, &n, &desc, &repo.default_branch),
                                                    ));
                                                }
                                            }
                                        });
                                    }
                                    Starter::License => {
                                        let b = w::button_ex(
                                            ui,
                                            Variant::Default,
                                            Some(Icon::Plus),
                                            "Choose a license",
                                            Some(Icon::TriangleDown),
                                            true,
                                        );
                                        egui::Popup::menu(&b).width(300.0).align(egui::RectAlign::BOTTOM_END).show(|ui| {
                                            if let Some(list) = self.licenses.ready().cloned() {
                                                for l in list {
                                                    if w::menu_item(ui, Some(Icon::Law), &l.name).clicked() {
                                                        let (g, key, owner) = (
                                                            cx.gh.clone(),
                                                            l.key.clone(),
                                                            cx.me.name.clone().unwrap_or_else(|| cx.me.login.clone()),
                                                        );
                                                        self.starter_job = Load::spawn(&ctx, move || {
                                                            let body = g.license_template(&key)?.body.unwrap_or_default();
                                                            let year = chrono::Utc::now().format("%Y").to_string();
                                                            Ok((
                                                                "LICENSE".into(),
                                                                body.replace("[year]", &year).replace("[fullname]", &owner),
                                                            ))
                                                        });
                                                    }
                                                }
                                            }
                                        });
                                    }
                                    Starter::Gitignore => {
                                        let b = w::button_ex(
                                            ui,
                                            Variant::Default,
                                            Some(Icon::Plus),
                                            "Choose a template",
                                            Some(Icon::TriangleDown),
                                            true,
                                        );
                                        egui::Popup::menu(&b)
                                            .width(260.0)
                                            .align(egui::RectAlign::BOTTOM_END)
                                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                                            .show(|ui| {
                                                w::text_input(ui, &mut self.gi_filter, "Filter…", ui.available_width());
                                                egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                                                    if let Some(list) = self.gitignores.ready().cloned() {
                                                        let f = self.gi_filter.to_lowercase();
                                                        for t in list.into_iter().filter(|t| f.is_empty() || t.to_lowercase().contains(&f))
                                                        {
                                                            if w::menu_item(ui, None, &t).clicked() {
                                                                let g = cx.gh.clone();
                                                                self.starter_job = Load::spawn(&ctx, move || {
                                                                    Ok((".gitignore".into(), g.gitignore_template(&t)?))
                                                                });
                                                                ui.close();
                                                            }
                                                        }
                                                    }
                                                });
                                            });
                                    }
                                }
                            });
                        });
                    });
                    w::divider(ui);
                }
            });
            ui.add_space(16.0);
            ui.label(RichText::new("Workflow files").font(semibold(16.0)));
            ui.add_space(8.0);
            w::bordered(ui, |ui| {
                w::show_load(ui, &mut self.workflows, |ui, list| {
                    if list.is_empty() {
                        Frame::new()
                            .inner_margin(Margin::same(12))
                            .show(ui, |ui| w::muted_small(ui, "No workflows yet. Add one from the list above."));
                    }
                    for p in list.iter() {
                        let resp = w::list_row(ui, |ui| {
                            ui.horizontal(|ui| {
                                icon(ui, Icon::Workflow, pal.fg_muted);
                                ui.label(RichText::new(p).monospace());
                            });
                        });
                        if resp.clicked() {
                            self.editor = Some(FileEditor::new(p.clone()));
                        }
                        w::divider(ui);
                    }
                });
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{Kind, insert_badge, kind_of};

    #[test]
    fn badge_goes_under_heading() {
        let mut s = "# Title\n\nText\n".to_string();
        insert_badge(&mut s, "[![A](a)](a)");
        assert!(s.starts_with("# Title\n\n[![A](a)](a)"));
        insert_badge(&mut s, "[![B](b)](b)");
        let a = s.find("[![A]").unwrap();
        let b = s.find("[![B]").unwrap();
        assert!(b > a && s[a..b].lines().count() <= 2);
    }

    #[test]
    fn file_kinds() {
        assert!(kind_of(".github/ISSUE_TEMPLATE/bug.yml") == Kind::IssueForm);
        assert!(kind_of(".github/ISSUE_TEMPLATE/config.yml") == Kind::Yaml);
        assert!(kind_of("README.md") == Kind::Markdown);
        assert!(kind_of(".editorconfig") == Kind::Text);
    }
}
