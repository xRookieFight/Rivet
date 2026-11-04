//! Local repositories: working tree changes, staging, commits, branches,
//! remotes, push/pull/fetch and a visual merge conflict resolver.

use super::Page;
use super::common;
use crate::app::Cx;
use crate::gitops::{self, Change, ConflictHunk, LocalBranch, LogEntry, MergeOutcome, WorkingState};
use crate::icons::{Icon, icon};
use crate::platform;
use crate::route::Route;
use crate::task::Load;
use crate::theme::semibold;
use crate::widgets::{self as w, FlashKind, Variant};
use egui::{CornerRadius, Frame, Margin, RichText, Stroke, Ui};
use std::path::PathBuf;

pub struct LocalPage {
    path: Option<PathBuf>,
    state: Load<WorkingState>,
    branches: Load<Vec<LocalBranch>>,
    history: Load<Vec<LogEntry>>,
    remotes: Load<Vec<(String, String)>>,
    tab: usize,
    selected: Option<(String, bool)>,
    diff: Load<String>,
    summary: String,
    description: String,
    op: Load<String>,
    op_label: &'static str,
    new_branch: String,
    branch_filter: String,
    remote_name: String,
    remote_url: String,
    show_remotes: bool,
    resolver: Option<Resolver>,
}

/// One conflicted file being resolved hunk by hunk.
struct Resolver {
    file: String,
    hunks: Vec<ConflictHunk>,
    choice: Vec<Option<u8>>,
    result: String,
    manual: bool,
}

impl Resolver {
    fn new(file: String, working: &str) -> Self {
        let hunks = gitops::parse_conflict_markers(working);
        let choice = vec![None; hunks.len()];
        let mut r = Self { file, hunks, choice, result: String::new(), manual: false };
        r.rebuild();
        r
    }

    fn unresolved(&self) -> usize {
        self.hunks.iter().zip(&self.choice).filter(|(h, c)| matches!(h, ConflictHunk::Conflict { .. }) && c.is_none()).count()
    }

    /// 1 = current (ours), 2 = incoming (theirs), 3 = both.
    fn rebuild(&mut self) {
        if self.manual {
            return;
        }
        let mut out = String::new();
        for (h, c) in self.hunks.iter().zip(&self.choice) {
            match h {
                ConflictHunk::Common(t) => out.push_str(t),
                ConflictHunk::Conflict { ours, theirs } => match c {
                    Some(1) => out.push_str(ours),
                    Some(2) => out.push_str(theirs),
                    Some(3) => {
                        out.push_str(ours);
                        out.push_str(theirs);
                    }
                    _ => {
                        out.push_str("<<<<<<< current\n");
                        out.push_str(ours);
                        out.push_str("=======\n");
                        out.push_str(theirs);
                        out.push_str(">>>>>>> incoming\n");
                    }
                },
            }
        }
        self.result = out;
    }
}

impl LocalPage {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            path,
            state: Load::Idle,
            branches: Load::Idle,
            history: Load::Idle,
            remotes: Load::Idle,
            tab: 0,
            selected: None,
            diff: Load::Idle,
            summary: String::new(),
            description: String::new(),
            op: Load::Idle,
            op_label: "",
            new_branch: String::new(),
            branch_filter: String::new(),
            remote_name: String::new(),
            remote_url: String::new(),
            show_remotes: false,
            resolver: None,
        }
    }

    fn refresh(&mut self, ctx: &egui::Context) {
        let Some(p) = self.path.clone() else { return };
        let p1 = p.clone();
        self.state = Load::spawn(ctx, move || gitops::working_state(&p1));
        let p2 = p.clone();
        self.branches = Load::spawn(ctx, move || gitops::branches(&p2));
        let p3 = p.clone();
        self.history = Load::spawn(ctx, move || gitops::log(&p3, 200));
        self.remotes = Load::spawn(ctx, move || gitops::remotes(&p));
        if let Some((f, staged)) = self.selected.clone() {
            self.load_diff(ctx, f, staged);
        }
    }

    fn load_diff(&mut self, ctx: &egui::Context, file: String, staged: bool) {
        let Some(p) = self.path.clone() else { return };
        self.selected = Some((file.clone(), staged));
        self.diff = Load::spawn(ctx, move || gitops::file_diff(&p, &file, staged));
    }

    /// Runs a git operation in the background; the label shows on the busy button.
    fn run(&mut self, ctx: &egui::Context, label: &'static str, job: impl FnOnce(PathBuf) -> anyhow::Result<String> + Send + 'static) {
        let Some(p) = self.path.clone() else { return };
        self.op_label = label;
        self.op = Load::spawn(ctx, move || job(p));
    }
}

impl Page for LocalPage {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        let ctx = ui.ctx().clone();
        let Some(path) = self.path.clone() else {
            self.picker(ui, cx);
            return;
        };
        if self.state.is_idle() {
            self.refresh(&ctx);
        }
        match self.op.take() {
            Some(Ok(msg)) => {
                if !msg.is_empty() {
                    cx.ok(msg);
                }
                self.refresh(&ctx);
            }
            Some(Err(e)) => {
                cx.err(e);
                self.refresh(&ctx);
            }
            None => {}
        }
        if let Load::Failed(e) = self.state.poll() {
            let e = e.clone();
            Frame::new().inner_margin(Margin::same(24)).show(ui, |ui| {
                w::flash(ui, FlashKind::Danger, &e);
                ui.add_space(8.0);
                if w::button(ui, Variant::Default, None, "Choose another folder").clicked() {
                    self.path = None;
                }
            });
            return;
        }
        let Some(st) = self.state.ready().cloned() else {
            w::spinner(ui);
            return;
        };
        let pal = w::p(ui);
        let busy = self.op.is_loading();

        egui::Panel::top("local-toolbar")
            .frame(Frame::new().fill(pal.header_bg).inner_margin(Margin::symmetric(16, 10)))
            .show(ui, |ui| self.toolbar(ui, cx, &path, &st, busy));

        egui::Panel::left("local-sidebar")
            .resizable(true)
            .default_size(340.0)
            // capped so a stale saved width or wide content can't swallow the diff area
            .size_range(240.0..=560.0)
            .frame(Frame::new().inner_margin(Margin::same(0)))
            .show(ui, |ui| self.sidebar(ui, cx, &st, busy));

        egui::CentralPanel::no_frame().show(ui, |ui| {
            // vertical only: with both axes the inner margin feeds back into the width
            // every frame and the page keeps growing to the right
            egui::ScrollArea::vertical().id_salt("local-main").show(ui, |ui| {
                Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                    if self.resolver.is_some() {
                        self.resolver_ui(ui, cx);
                    } else if self.tab == 0 {
                        self.diff_ui(ui);
                    } else {
                        w::blankslate(
                            ui,
                            Icon::History,
                            "History",
                            "Pick a commit on the left to copy its SHA, or open it on GitHub when the repository has a GitHub remote.",
                        );
                    }
                });
            });
        });

        if self.show_remotes {
            self.remotes_modal(&ctx, cx);
        }
    }
}

impl LocalPage {
    fn picker(&mut self, ui: &mut Ui, cx: &mut Cx) {
        let pal = w::p(ui);
        egui::ScrollArea::vertical().show(ui, |ui| {
            Frame::new().inner_margin(Margin::same(32)).show(ui, |ui| {
                w::page(ui, 760.0, |ui| {
                    w::heading(ui, "Local repositories", 24.0);
                    ui.add_space(4.0);
                    w::muted(ui, "Work with repositories on this computer: review changes, commit, push and resolve conflicts.");
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        if platform::NATIVE_FILES
                            && w::button(ui, Variant::Primary, Some(Icon::FileDirectory), "Open folder…").clicked()
                            && let Some(dir) = platform::pick_folder("Open a git repository", None)
                        {
                            match gitops::open(&dir) {
                                Ok(repo) => {
                                    let root = repo.workdir().map(|p| p.to_path_buf()).unwrap_or(dir);
                                    cx.go(Route::Local(Some(root)));
                                }
                                Err(e) => cx.err(format!("{e:#}")),
                            }
                        }
                        if w::button(ui, Variant::Default, Some(Icon::DesktopDownload), "Clone a repository").clicked() {
                            cx.go(Route::Clone { url: String::new() });
                        }
                    });
                    ui.add_space(24.0);
                    w::bordered(ui, |ui| {
                        w::box_header(ui, |ui| {
                            ui.label(RichText::new("Recent").font(semibold(14.0)));
                        });
                        if cx.prefs.recent_local.is_empty() {
                            w::blankslate(
                                ui,
                                Icon::FileDirectory,
                                "No local repositories yet",
                                "Open a folder or clone a repository to get started.",
                            );
                        }
                        for p in cx.prefs.recent_local.clone() {
                            let resp = w::list_row(ui, |ui| {
                                ui.horizontal(|ui| {
                                    icon(ui, Icon::Repo, pal.fg_muted);
                                    ui.vertical(|ui| {
                                        let name = p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                                        ui.label(RichText::new(name).font(semibold(14.0)));
                                        w::muted_small(ui, p.display().to_string());
                                    });
                                    if !p.exists() {
                                        w::right(ui, |ui| w::outline_label(ui, "Missing", Some(pal.danger)));
                                    }
                                });
                            });
                            if resp.clicked() {
                                cx.go(Route::Local(Some(p.clone())));
                            }
                            w::divider(ui);
                        }
                    });
                });
            });
        });
    }

    fn toolbar(&mut self, ui: &mut Ui, cx: &mut Cx, path: &std::path::Path, st: &WorkingState, busy: bool) {
        let ctx = ui.ctx().clone();
        let token = Some(cx.gh.token().to_string());
        ui.horizontal(|ui| {
            icon(ui, Icon::Repo, w::p(ui).fg_muted);
            let name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            ui.label(RichText::new(name).font(semibold(16.0)));
            ui.add_space(8.0);

            // branch menu
            let b = w::button_ex(ui, Variant::Default, Some(Icon::GitBranch), &st.branch, Some(Icon::TriangleDown), !busy);
            egui::Popup::menu(&b).width(320.0).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                ui.label(RichText::new("Switch branches").font(semibold(14.0)));
                w::text_input(ui, &mut self.branch_filter, "Filter branches", ui.available_width());
                egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                    if let Some(list) = self.branches.ready().cloned() {
                        let f = self.branch_filter.to_lowercase();
                        for br in list.iter().filter(|b| f.is_empty() || b.name.to_lowercase().contains(&f)) {
                            ui.horizontal(|ui| {
                                let ic = if br.remote { Icon::Globe } else { Icon::GitBranch };
                                if w::menu_item_ex(ui, Some(ic), &br.name, br.is_head, false).clicked() && !br.is_head {
                                    let n = br.name.clone();
                                    self.run(&ctx, "Switching…", move |p| {
                                        gitops::switch_branch(&p, &n)?;
                                        Ok(format!("Switched to {n}"))
                                    });
                                    ui.close();
                                }
                            });
                            if !br.is_head {
                                ui.horizontal(|ui| {
                                    ui.add_space(32.0);
                                    if w::link(ui, format!("Merge into {}", st.branch)).clicked() {
                                        let n = br.name.clone();
                                        self.run(&ctx, "Merging…", move |p| {
                                            Ok(match gitops::merge_branch(&p, &n)? {
                                                MergeOutcome::UpToDate => "Already up to date".into(),
                                                MergeOutcome::FastForward => format!("Fast-forwarded to {n}"),
                                                MergeOutcome::Merged => format!("Merged {n}"),
                                                MergeOutcome::Conflicts(c) => {
                                                    format!("Merge has {c} conflicted file(s), resolve them below")
                                                }
                                            })
                                        });
                                        ui.close();
                                    }
                                    if !br.remote && w::link(ui, "Delete").clicked() {
                                        let n = br.name.clone();
                                        self.run(&ctx, "Deleting…", move |p| {
                                            gitops::delete_branch(&p, &n)?;
                                            Ok(format!("Deleted {n}"))
                                        });
                                        ui.close();
                                    }
                                });
                            }
                        }
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    w::text_input(ui, &mut self.new_branch, "new-branch-name", ui.available_width() - 90.0);
                    let ok = !self.new_branch.trim().is_empty() && git2::Branch::name_is_valid(self.new_branch.trim()).unwrap_or(false);
                    if w::button_ex(ui, Variant::Default, None, "Create", None, ok).clicked() {
                        let n = self.new_branch.trim().to_string();
                        self.new_branch.clear();
                        self.run(&ctx, "Creating…", move |p| {
                            gitops::create_branch(&p, &n, true)?;
                            Ok(format!("Created and switched to {n}"))
                        });
                        ui.close();
                    }
                });
            });

            w::right(ui, |ui| {
                if let Some((o, n)) = gitops::github_slug(path)
                    && w::button(ui, Variant::Default, Some(Icon::MarkGithub), "View on GitHub").clicked()
                {
                    cx.go(Route::repo_home(&o, &n));
                }
                if w::button(ui, Variant::Default, Some(Icon::Globe), "Remotes").clicked() {
                    self.show_remotes = true;
                }
                if platform::NATIVE_FILES && w::icon_button(ui, Icon::FileDirectory, "Show in file manager").clicked() {
                    platform::open_path(path);
                }
                if w::icon_button(ui, Icon::Sync, "Refresh").clicked() {
                    self.refresh(&ctx);
                }
                let label = if busy {
                    self.op_label.to_string()
                } else if st.ahead > 0 {
                    format!("Push origin  ↑{}", st.ahead)
                } else if st.behind > 0 {
                    format!("Pull origin  ↓{}", st.behind)
                } else if st.upstream.is_none() {
                    "Publish branch".into()
                } else {
                    "Fetch origin".into()
                };
                let ic = if st.ahead > 0 || st.upstream.is_none() {
                    Icon::Upload
                } else if st.behind > 0 {
                    Icon::Download
                } else {
                    Icon::Sync
                };
                if w::button_ex(ui, Variant::Default, Some(ic), &label, None, !busy).clicked() {
                    let t = token.clone();
                    if st.ahead > 0 || st.upstream.is_none() {
                        self.run(&ctx, "Pushing…", move |p| {
                            gitops::push(&p, t)?;
                            Ok("Pushed".into())
                        });
                    } else if st.behind > 0 {
                        self.run(&ctx, "Pulling…", move |p| {
                            Ok(match gitops::pull(&p, t)? {
                                MergeOutcome::UpToDate => "Already up to date".into(),
                                MergeOutcome::FastForward | MergeOutcome::Merged => "Pulled".into(),
                                MergeOutcome::Conflicts(c) => format!("Pull produced {c} conflicted file(s)"),
                            })
                        });
                    } else {
                        self.run(&ctx, "Fetching…", move |p| {
                            gitops::fetch(&p, t)?;
                            Ok("Fetched".into())
                        });
                    }
                }
                if st.ahead == 0 && st.upstream.is_some() {
                    let more = w::icon_button(ui, Icon::TriangleDown, "More sync options");
                    egui::Popup::menu(&more).align(egui::RectAlign::BOTTOM_END).show(|ui| {
                        if w::menu_item(ui, Some(Icon::Download), "Pull").clicked() {
                            let t = token.clone();
                            self.run(&ctx, "Pulling…", move |p| {
                                gitops::pull(&p, t)?;
                                Ok("Pulled".into())
                            });
                        }
                        if w::menu_item(ui, Some(Icon::Upload), "Push").clicked() {
                            let t = token.clone();
                            self.run(&ctx, "Pushing…", move |p| {
                                gitops::push(&p, t)?;
                                Ok("Pushed".into())
                            });
                        }
                    });
                }
            });
        });
    }

    fn sidebar(&mut self, ui: &mut Ui, cx: &mut Cx, st: &WorkingState, busy: bool) {
        let ctx = ui.ctx().clone();
        let pal = w::p(ui);
        let changes = st.staged.len() + st.unstaged.len() + st.conflicts.len();
        let tab = w::underline_nav(ui, &[(Icon::Diff, "Changes", Some(changes as u64)), (Icon::History, "History", None)], self.tab);
        if let Some(t) = tab {
            self.tab = t;
        }
        w::divider(ui);
        if self.tab == 1 {
            egui::ScrollArea::vertical().show(ui, |ui| {
                if let Some(list) = self.history.ready().cloned() {
                    if list.is_empty() {
                        w::blankslate(ui, Icon::GitCommit, "No commits yet", "");
                    }
                    let slug = self.path.as_deref().and_then(gitops::github_slug);
                    for e in list {
                        let resp = w::list_row(ui, |ui| {
                            ui.label(RichText::new(&e.summary).font(semibold(14.0)));
                            ui.horizontal(|ui| {
                                w::muted_small(ui, format!("{} · {}", e.author, w::rel_time(&e.time)));
                                w::right(ui, |ui| ui.label(RichText::new(&e.sha[..7]).monospace().size(11.0).color(pal.fg_muted)));
                            });
                        });
                        resp.context_menu(|ui| {
                            if w::menu_item(ui, Some(Icon::Copy), "Copy SHA").clicked() {
                                ui.ctx().copy_text(e.sha.clone());
                            }
                            if let Some((o, n)) = &slug
                                && w::menu_item(ui, Some(Icon::MarkGithub), "View on GitHub").clicked()
                            {
                                cx.go(Route::repo(o, n, crate::route::RepoView::Commit(e.sha.clone())));
                            }
                        });
                        if resp.clicked()
                            && let Some((o, n)) = &slug
                        {
                            cx.go(Route::repo(o, n, crate::route::RepoView::Commit(e.sha.clone())));
                        }
                        w::divider(ui);
                    }
                }
            });
            return;
        }

        let commit_h = 210.0;
        egui::ScrollArea::vertical().max_height(ui.available_height() - commit_h).show(ui, |ui| {
            if st.merging || !st.conflicts.is_empty() {
                Frame::new().inner_margin(Margin::same(12)).show(ui, |ui| {
                    w::flash(ui, FlashKind::Warning, &format!("Merge in progress. {} conflicted file(s).", st.conflicts.len()));
                    ui.add_space(8.0);
                    if w::button_ex(ui, Variant::Danger, None, "Abort merge", None, !busy).clicked() {
                        self.resolver = None;
                        self.run(&ctx, "Aborting…", |p| {
                            gitops::abort_merge(&p)?;
                            Ok("Merge aborted".into())
                        });
                    }
                });
                for f in &st.conflicts {
                    let resp = w::list_row(ui, |ui| {
                        ui.horizontal(|ui| {
                            icon(ui, Icon::Alert, pal.attention);
                            ui.label(RichText::new(f).monospace().size(12.0));
                        });
                    });
                    if resp.clicked()
                        && let Some(p) = &self.path
                    {
                        match gitops::conflict_text(p, f) {
                            Ok(text) => self.resolver = Some(Resolver::new(f.clone(), &text)),
                            Err(e) => cx.err(format!("{e:#}")),
                        }
                    }
                }
                w::divider(ui);
            }
            self.file_group(ui, "Staged changes", &st.staged, true, busy);
            self.file_group(ui, "Changes", &st.unstaged, false, busy);
            if st.staged.is_empty() && st.unstaged.is_empty() && st.conflicts.is_empty() {
                w::blankslate(ui, Icon::CheckCircle, "No local changes", "There are no uncommitted changes in this repository.");
            }
        });

        // commit box pinned to the bottom of the sidebar
        w::divider(ui);
        Frame::new().inner_margin(Margin::same(12)).show(ui, |ui| {
            ui.horizontal(|ui| {
                w::avatar(ui, &cx.me.avatar_url, 20.0);
                w::text_input(ui, &mut self.summary, "Summary (required)", ui.available_width());
            });
            ui.add_space(6.0);
            w::text_area(ui, &mut self.description, "Description", 3, false);
            ui.add_space(6.0);
            let can = !self.summary.trim().is_empty() && !st.staged.is_empty() && st.conflicts.is_empty() && !busy;
            ui.vertical_centered_justified(|ui| {
                let label = format!("Commit to {}", st.branch);
                if w::button_ex(ui, Variant::Primary, None, &label, None, can).clicked() {
                    let msg = if self.description.trim().is_empty() {
                        self.summary.trim().to_string()
                    } else {
                        format!("{}\n\n{}", self.summary.trim(), self.description.trim())
                    };
                    self.summary.clear();
                    self.description.clear();
                    self.run(&ctx, "Committing…", move |p| {
                        let sha = gitops::commit(&p, &msg)?;
                        Ok(format!("Committed {}", &sha[..7]))
                    });
                }
            });
        });
    }

    fn file_group(&mut self, ui: &mut Ui, title: &str, files: &[gitops::FileStatus], staged: bool, busy: bool) {
        if files.is_empty() {
            return;
        }
        let ctx = ui.ctx().clone();
        let pal = w::p(ui);
        Frame::new().fill(pal.canvas_subtle).inner_margin(Margin::symmetric(12, 6)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{title} ({})", files.len())).font(semibold(12.0)).color(pal.fg_muted));
                w::right(ui, |ui| {
                    let label = if staged { "Unstage all" } else { "Stage all" };
                    if w::button_ex(ui, Variant::Invisible, None, label, None, !busy).clicked() {
                        let all: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
                        if staged {
                            self.run(&ctx, "Unstaging…", move |p| gitops::unstage(&p, &all).map(|_| String::new()));
                        } else {
                            self.run(&ctx, "Staging…", move |p| gitops::stage_all(&p).map(|_| String::new()));
                        }
                    }
                });
            });
        });
        for f in files {
            let is_sel = self.selected.as_ref() == Some(&(f.path.clone(), staged));
            let (letter, color) = match f.change {
                Change::Added | Change::Untracked => ("+", pal.success),
                Change::Modified | Change::TypeChange => ("•", pal.attention),
                Change::Deleted => ("−", pal.danger),
                Change::Renamed => ("→", pal.accent),
                Change::Conflicted => ("!", pal.danger),
            };
            let row = Frame::new()
                .fill(if is_sel { pal.accent_subtle } else { egui::Color32::TRANSPARENT })
                .inner_margin(Margin::symmetric(12, 4))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let mut checked = staged;
                        if ui.checkbox(&mut checked, "").changed() && !busy {
                            let file = vec![f.path.clone()];
                            if staged {
                                self.run(&ctx, "Unstaging…", move |p| gitops::unstage(&p, &file).map(|_| String::new()));
                            } else {
                                self.run(&ctx, "Staging…", move |p| gitops::stage(&p, &file).map(|_| String::new()));
                            }
                        }
                        ui.add(egui::Label::new(RichText::new(&f.path).monospace().size(12.0)).truncate());
                        w::right(ui, |ui| ui.label(RichText::new(letter).font(semibold(14.0)).color(color)));
                    });
                })
                .response;
            let resp = ui.interact(row.rect, ui.scope_id().with(("file", &f.path, staged)), egui::Sense::click());
            if resp.clicked() {
                self.load_diff(&ctx, f.path.clone(), staged);
            }
            resp.context_menu(|ui| {
                if !staged
                    && f.change != Change::Untracked
                    && w::menu_item_ex(ui, Some(Icon::Trash), "Discard changes", false, true).clicked()
                {
                    let file = vec![f.path.clone()];
                    self.run(&ctx, "Discarding…", move |p| gitops::discard(&p, &file).map(|_| "Changes discarded".into()));
                    ui.close();
                }
                if w::menu_item(ui, Some(Icon::Copy), "Copy file path").clicked() {
                    ui.ctx().copy_text(f.path.clone());
                    ui.close();
                }
                if let Some(root) = &self.path
                    && platform::NATIVE_FILES
                    && w::menu_item(ui, Some(Icon::FileDirectory), "Open file").clicked()
                {
                    platform::open_path(&root.join(&f.path));
                    ui.close();
                }
            });
        }
    }

    fn diff_ui(&mut self, ui: &mut Ui) {
        let Some((file, staged)) = self.selected.clone() else {
            w::blankslate(ui, Icon::Diff, "Select a file", "Choose a changed file on the left to see its diff.");
            return;
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(&file).monospace().strong());
            w::outline_label(ui, if staged { "Staged" } else { "Unstaged" }, None);
        });
        ui.add_space(8.0);
        w::show_load(ui, &mut self.diff, |ui, d| {
            w::bordered(ui, |ui| {
                if d.trim().is_empty() {
                    Frame::new()
                        .inner_margin(Margin::same(16))
                        .show(ui, |ui| w::muted(ui, "No textual changes (binary file or mode change)."));
                } else {
                    egui::ScrollArea::horizontal().id_salt("local-diff-h").show(ui, |ui| common::diff_view(ui, d));
                }
            });
        });
    }

    fn resolver_ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        let ctx = ui.ctx().clone();
        let pal = w::p(ui);
        let Some(r) = &mut self.resolver else { return };
        let mut close = false;
        let mut save: Option<(String, String)> = None;
        ui.horizontal(|ui| {
            icon(ui, Icon::Alert, pal.attention);
            ui.label(RichText::new(format!("Resolve conflicts in {}", r.file)).font(semibold(16.0)));
            w::right(ui, |ui| {
                if w::button(ui, Variant::Invisible, None, "Close").clicked() {
                    close = true;
                }
                let left = r.unresolved();
                let ok = r.manual || left == 0;
                if w::button_ex(ui, Variant::Primary, Some(Icon::Check), "Mark as resolved", None, ok).clicked() {
                    save = Some((r.file.clone(), r.result.clone()));
                }
                if !r.manual {
                    w::muted_small(ui, format!("{left} conflict(s) left"));
                }
            });
        });
        ui.add_space(12.0);
        let mut changed = false;
        let mut conflict_no = 0;
        for i in 0..r.hunks.len() {
            let ConflictHunk::Conflict { ours, theirs } = r.hunks[i].clone() else { continue };
            conflict_no += 1;
            Frame::new().stroke(Stroke::new(1.0, pal.border)).corner_radius(CornerRadius::same(6)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                Frame::new().fill(pal.canvas_subtle).inner_margin(Margin::symmetric(12, 6)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("Conflict {conflict_no}")).font(semibold(14.0)));
                        w::right(ui, |ui| {
                            for (val, label) in [(3u8, "Accept both"), (2, "Accept incoming"), (1, "Accept current")] {
                                let v = if r.choice[i] == Some(val) { Variant::Primary } else { Variant::Default };
                                if w::button(ui, v, None, label).clicked() {
                                    r.choice[i] = Some(val);
                                    r.manual = false;
                                    changed = true;
                                }
                            }
                        });
                    });
                });
                ui.columns(2, |cols| {
                    for (col, (title, text, bg)) in
                        cols.iter_mut().zip([("Current (HEAD)", &ours, pal.diff_del_bg), ("Incoming", &theirs, pal.diff_add_bg)])
                    {
                        Frame::new().fill(bg).inner_margin(Margin::same(8)).show(col, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(RichText::new(title).size(12.0).font(semibold(12.0)).color(pal.fg_muted));
                            ui.label(RichText::new(text.as_str()).monospace().size(12.0));
                        });
                    }
                });
            });
            ui.add_space(12.0);
        }
        if changed {
            r.rebuild();
        }
        ui.label(RichText::new("Result").font(semibold(14.0)));
        w::muted_small(ui, "Edit the merged result directly if neither side is right. Editing switches to manual mode.");
        ui.add_space(4.0);
        let resp = w::text_area(ui, &mut r.result, "", 16, true);
        if resp.changed() {
            r.manual = true;
        }
        if close {
            self.resolver = None;
        }
        if let Some((file, content)) = save {
            if content.contains("<<<<<<<") || content.contains(">>>>>>>") {
                cx.err("The result still contains conflict markers");
                return;
            }
            self.resolver = None;
            self.run(&ctx, "Resolving…", move |p| {
                gitops::resolve_conflict(&p, &file, &content)?;
                Ok(format!("Resolved {file}"))
            });
        }
    }

    fn remotes_modal(&mut self, ctx: &egui::Context, _cx: &mut Cx) {
        let pal = crate::theme::pal(ctx);
        let modal = egui::Modal::new(egui::Id::unique("remotes-modal"))
            .backdrop_color(pal.overlay_shadow)
            .frame(
                Frame::new()
                    .fill(pal.canvas)
                    .stroke(Stroke::new(1.0, pal.border))
                    .corner_radius(CornerRadius::same(12))
                    .inner_margin(Margin::same(16)),
            )
            .show(ctx, |ui| {
                ui.set_width(560.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Remotes").font(semibold(16.0)));
                    w::right(ui, |ui| {
                        if w::icon_button_invisible(ui, Icon::X, "Close").clicked() {
                            self.show_remotes = false;
                        }
                    });
                });
                ui.add_space(8.0);
                if let Some(list) = self.remotes.ready().cloned() {
                    w::bordered(ui, |ui| {
                        if list.is_empty() {
                            Frame::new().inner_margin(Margin::same(12)).show(ui, |ui| w::muted(ui, "No remotes configured."));
                        }
                        for (name, url) in list {
                            w::list_row(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(&name).font(semibold(14.0)));
                                    ui.label(RichText::new(&url).monospace().size(12.0).color(pal.fg_muted));
                                    w::right(ui, |ui| {
                                        if w::icon_button_invisible(ui, Icon::Trash, "Remove remote").clicked() {
                                            let n = name.clone();
                                            self.run(ctx, "Removing…", move |p| {
                                                gitops::remove_remote(&p, &n)?;
                                                Ok(format!("Removed remote {n}"))
                                            });
                                        }
                                        if w::icon_button_invisible(ui, Icon::Pencil, "Use the URL below for this remote").clicked()
                                            && !self.remote_url.trim().is_empty()
                                        {
                                            let (n, u) = (name.clone(), self.remote_url.trim().to_string());
                                            self.run(ctx, "Updating…", move |p| {
                                                gitops::set_remote_url(&p, &n, &u)?;
                                                Ok(format!("Updated {n}"))
                                            });
                                        }
                                    });
                                });
                            });
                            w::divider(ui);
                        }
                    });
                }
                ui.add_space(12.0);
                ui.label(RichText::new("Add remote").font(semibold(14.0)));
                ui.horizontal(|ui| {
                    w::text_input(ui, &mut self.remote_name, "name", 120.0);
                    w::text_input(ui, &mut self.remote_url, "https://github.com/owner/repo.git", ui.available_width() - 80.0);
                    let ok = !self.remote_name.trim().is_empty() && !self.remote_url.trim().is_empty();
                    if w::button_ex(ui, Variant::Default, None, "Add", None, ok).clicked() {
                        let (n, u) = (self.remote_name.trim().to_string(), self.remote_url.trim().to_string());
                        self.remote_name.clear();
                        self.remote_url.clear();
                        self.run(ctx, "Adding…", move |p| {
                            gitops::add_remote(&p, &n, &u)?;
                            Ok(format!("Added remote {n}"))
                        });
                    }
                });
            });
        if modal.should_close() {
            self.show_remotes = false;
        }
    }
}

/// Clone dialog: pick one of your repos or paste a URL, choose a folder.
pub struct ClonePage {
    url: String,
    dest: Option<PathBuf>,
    filter: String,
    job: Load<PathBuf>,
}

impl ClonePage {
    pub fn new(url: String) -> Self {
        Self { url, dest: None, filter: String::new(), job: Load::Idle }
    }
}

impl Page for ClonePage {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        let ctx = ui.ctx().clone();
        match self.job.take() {
            Some(Ok(p)) => {
                cx.ok(format!("Cloned into {}", p.display()));
                cx.go(Route::Local(Some(p)));
            }
            Some(Err(e)) => cx.err(e),
            None => {}
        }
        let pal = w::p(ui);
        let base = self.dest.clone().or_else(|| cx.prefs.clone_dir.clone()).or_else(super::settings::default_clone_dir).unwrap_or_default();
        let repo_name = gitops::parse_github_url(self.url.trim())
            .map(|(_, n)| n)
            .or_else(|| self.url.trim().trim_end_matches(".git").rsplit('/').next().map(str::to_string))
            .unwrap_or_default();
        let target = base.join(&repo_name);

        egui::ScrollArea::vertical().show(ui, |ui| {
            Frame::new().inner_margin(Margin::same(32)).show(ui, |ui| {
                w::page(ui, 760.0, |ui| {
                    w::heading(ui, "Clone a repository", 24.0);
                    ui.add_space(16.0);
                    w::form_label(ui, "Repository URL", true);
                    w::text_input(ui, &mut self.url, "https://github.com/owner/repo.git", ui.available_width());
                    ui.add_space(12.0);
                    w::form_label(ui, "Local path", true);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(target.display().to_string()).monospace());
                        if platform::NATIVE_FILES
                            && w::button(ui, Variant::Default, Some(Icon::FileDirectory), "Choose…").clicked()
                            && let Some(d) = platform::pick_folder("Local path", Some(&base))
                        {
                            self.dest = Some(d);
                        }
                    });
                    if target.exists() && !repo_name.is_empty() {
                        ui.label(RichText::new("That folder already exists.").size(12.0).color(pal.danger));
                    }
                    ui.add_space(16.0);
                    let busy = self.job.is_loading();
                    let ok = !repo_name.is_empty() && !target.exists() && !busy;
                    if w::button_ex(ui, Variant::Primary, Some(Icon::DesktopDownload), if busy { "Cloning…" } else { "Clone" }, None, ok)
                        .clicked()
                    {
                        let (url, dest, tok) = (self.url.trim().to_string(), target.clone(), cx.gh.token().to_string());
                        self.job = Load::spawn(&ctx, move || {
                            if let Some(parent) = dest.parent() {
                                std::fs::create_dir_all(parent)?;
                            }
                            gitops::clone(&url, &dest, Some(tok))
                        });
                    }
                    ui.add_space(24.0);
                    ui.label(RichText::new("Your repositories").font(semibold(16.0)));
                    ui.add_space(8.0);
                    w::text_input_ex(ui, &mut self.filter, "Filter your repositories", ui.available_width(), Some(Icon::Search));
                    ui.add_space(8.0);
                    let f = self.filter.to_lowercase();
                    w::bordered(ui, |ui| {
                        for r in cx.my_repos.iter().filter(|r| f.is_empty() || r.full_name.to_lowercase().contains(&f)).take(30) {
                            let sel = self.url == r.clone_url;
                            let resp = w::list_row(ui, |ui| {
                                ui.horizontal(|ui| {
                                    icon(ui, if r.private { Icon::Lock } else { Icon::Repo }, pal.fg_muted);
                                    ui.label(RichText::new(&r.full_name).font(semibold(14.0)).color(if sel { pal.accent } else { pal.fg }));
                                });
                            });
                            if resp.clicked() {
                                self.url = r.clone_url.clone();
                            }
                            w::divider(ui);
                        }
                    });
                });
            });
        });
    }
}
