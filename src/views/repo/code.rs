//! Code tab: directory listing with the latest commit bar, README, the About
//! sidebar, and the file viewer / editor.

use super::{RepoSub, on, ref_picker};
use crate::app::Cx;
use crate::github::models::{Commit, Content, PathCommit, Release, Repo};
use crate::icons::{Icon, icon};
use crate::platform;
use crate::route::{RepoView, Route};
use crate::task::Load;
use crate::theme::semibold;
use crate::views::common::{self, markdown, raw_base};
use crate::widgets::{self as w, FlashKind, Variant};
use egui::{CornerRadius, Frame, Margin, RichText, Sense, Stroke, Ui, Vec2};
use egui_commonmark::CommonMarkCache;
use std::collections::HashMap;

pub struct CodeView {
    git_ref: Option<String>,
    path: String,
    listing: Load<Vec<Content>>,
    path_commits: Load<HashMap<String, PathCommit>>,
    readme: Load<Option<Content>>,
    last_commit: Load<Option<Commit>>,
    languages: Load<Vec<(String, u64)>>,
    releases: Load<Vec<Release>>,
    counts: Load<(usize, usize)>,
    branches: Load<Vec<String>>,
    branch_filter: String,
    md: CommonMarkCache,
    clone_tab: usize,
}

impl CodeView {
    pub fn new(git_ref: Option<String>, path: String) -> Self {
        Self {
            git_ref,
            path,
            listing: Load::Idle,
            path_commits: Load::Idle,
            readme: Load::Idle,
            last_commit: Load::Idle,
            languages: Load::Idle,
            releases: Load::Idle,
            counts: Load::Idle,
            branches: Load::Idle,
            branch_filter: String::new(),
            md: CommonMarkCache::default(),
            clone_tab: 0,
        }
    }

    fn start(&mut self, ctx: &egui::Context, cx: &Cx, repo: &Repo) {
        let r = self.git_ref.clone().unwrap_or_else(|| repo.default_branch.clone());
        let (o, n) = on(repo);
        let (g, o2, n2, r2, p2) = (cx.gh.clone(), o.clone(), n.clone(), r.clone(), self.path.clone());
        self.listing = Load::spawn(ctx, move || {
            let mut v = g.contents(&o2, &n2, &p2, &r2)?;
            // github lists folders first, then files, both alphabetically
            v.sort_by_key(|a| (!a.is_dir(), a.name.to_lowercase()));
            Ok(v)
        });
        let (g, o2, n2, r2, p2) = (cx.gh.clone(), o.clone(), n.clone(), r.clone(), self.path.clone());
        self.last_commit = Load::spawn(ctx, move || Ok(g.commits(&o2, &n2, &r2, Some(&p2), 1)?.into_iter().next()));
        if self.path.is_empty() {
            let (g, o2, n2, r2) = (cx.gh.clone(), o.clone(), n.clone(), r.clone());
            self.readme = Load::spawn(ctx, move || g.readme(&o2, &n2, &r2));
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.languages = Load::spawn(ctx, move || g.languages(&o2, &n2));
            let (g, o2, n2) = (cx.gh.clone(), o.clone(), n.clone());
            self.releases = Load::spawn(ctx, move || g.releases(&o2, &n2));
            let (g, o2, n2) = (cx.gh.clone(), o, n);
            self.counts = Load::spawn(ctx, move || Ok((g.branches(&o2, &n2)?.len(), g.tags(&o2, &n2)?.len())));
        } else {
            self.readme = Load::Ready(None);
        }
    }
}

impl RepoSub for CodeView {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let ctx = ui.ctx().clone();
        if self.listing.is_idle() {
            self.start(&ctx, cx, repo);
        }
        let git_ref = self.git_ref.clone().unwrap_or_else(|| repo.default_branch.clone());
        let (o, n) = on(repo);

        // last commit per entry, fetched once the listing is in (and again after uploads)
        if self.listing.is_loading() {
            self.path_commits = Load::Idle;
        } else if self.path_commits.is_idle()
            && let Some(list) = self.listing.ready()
        {
            let paths: Vec<String> = list.iter().map(|c| c.path.clone()).collect();
            let (g, o2, n2, r) = (cx.gh.clone(), o.clone(), n.clone(), git_ref.clone());
            self.path_commits = Load::spawn(&ctx, move || g.last_commits(&o2, &n2, &r, &paths));
        }

        // a README sitting in the listing of a subfolder gets rendered too
        if !self.path.is_empty()
            && matches!(self.readme, Load::Ready(None))
            && let Some(list) = self.listing.ready()
            && let Some(rm) = list.iter().find(|c| c.name.to_lowercase().starts_with("readme") && !c.is_dir())
        {
            let (g, o2, n2, p2, r2) = (cx.gh.clone(), o.clone(), n.clone(), rm.path.clone(), git_ref.clone());
            self.readme = Load::spawn(&ctx, move || Ok(Some(g.file(&o2, &n2, &p2, &r2)?)));
        }

        super::section(ui, |ui| {
            if repo.size == 0 && self.path.is_empty() && matches!(self.listing.poll(), Load::Failed(_)) {
                empty_repo(ui, repo, cx);
                return;
            }
            let wide = ui.available_width() > 1000.0;
            let side_w = if wide { 296.0 } else { 0.0 };
            ui.horizontal_top(|ui| {
                let main_w = ui.available_width() - side_w - if wide { 24.0 } else { 0.0 };
                ui.allocate_ui_with_layout(Vec2::new(main_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_width(main_w);
                    self.main(ui, cx, repo, &git_ref);
                });
                if wide {
                    ui.add_space(24.0);
                    ui.allocate_ui_with_layout(Vec2::new(side_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.set_width(side_w);
                        self.about(ui, cx, repo);
                    });
                }
            });
        });
    }
}

impl CodeView {
    fn main(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo, git_ref: &str) {
        let pal = w::p(ui);
        let (o, n) = on(repo);
        ui.horizontal(|ui| {
            if let Some(r) = ref_picker(ui, cx, repo, git_ref, &mut self.branches, &mut self.branch_filter) {
                cx.go(Route::repo(&o, &n, RepoView::Code { git_ref: Some(r), path: self.path.clone() }));
            }
            if self.path.is_empty() {
                if let Some((b, t)) = self.counts.ready().copied() {
                    if w::button(ui, Variant::Invisible, Some(Icon::GitBranch), &format!("{b} Branches")).clicked() {
                        cx.go(Route::repo(&o, &n, RepoView::Branches));
                    }
                    if w::button(ui, Variant::Invisible, Some(Icon::Tag), &format!("{t} Tags")).clicked() {
                        cx.go(Route::repo(&o, &n, RepoView::Tags));
                    }
                }
            } else {
                breadcrumb(ui, cx, repo, git_ref, &self.path, false);
            }
            w::right(ui, |ui| {
                let code = w::button(ui, Variant::Primary, Some(Icon::Code), "Code");
                egui::Popup::menu(&code).width(380.0).align(egui::RectAlign::BOTTOM_END).show(|ui| {
                    ui.horizontal(|ui| {
                        for (i, l) in ["HTTPS", "SSH", "GitHub CLI"].iter().enumerate() {
                            if w::tab_link(ui, Icon::Terminal, l, self.clone_tab == i).clicked() {
                                self.clone_tab = i;
                            }
                        }
                    });
                    ui.add_space(6.0);
                    let text = match self.clone_tab {
                        0 => repo.clone_url.clone(),
                        1 => repo.ssh_url.clone(),
                        _ => format!("gh repo clone {}", repo.full_name),
                    };
                    ui.horizontal(|ui| {
                        let mut t = text.clone();
                        ui.add(
                            egui::TextEdit::singleline(&mut t).font(egui::TextStyle::Monospace).desired_width(ui.available_width() - 40.0),
                        );
                        if w::icon_button(ui, Icon::Copy, "Copy").clicked() {
                            ui.ctx().copy_text(text.clone());
                        }
                    });
                    ui.add_space(6.0);
                    ui.separator();
                    if platform::LOCAL_GIT && w::menu_item(ui, Some(Icon::DesktopDownload), "Clone with Rivet").clicked() {
                        cx.go(Route::Clone { url: repo.clone_url.clone() });
                    }
                    if w::menu_item(ui, Some(Icon::FileZip), "Download ZIP").clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(format!(
                            "https://github.com/{}/archive/refs/heads/{git_ref}.zip",
                            repo.full_name
                        )));
                    }
                });
                if repo.can_push() {
                    let add = w::button_ex(ui, Variant::Default, Some(Icon::Plus), "Add file", Some(Icon::TriangleDown), true);
                    egui::Popup::menu(&add).align(egui::RectAlign::BOTTOM_END).show(|ui| {
                        if w::menu_item(ui, Some(Icon::Plus), "Create new file").clicked() {
                            let dir = if self.path.is_empty() { String::new() } else { format!("{}/", self.path) };
                            cx.go(Route::repo(
                                &o,
                                &n,
                                RepoView::Blob { git_ref: Some(git_ref.to_string()), path: format!("{dir}\u{0}new") },
                            ));
                        }
                        if platform::NATIVE_FILES
                            && w::menu_item(ui, Some(Icon::Upload), "Upload files").clicked()
                            && let Some(files) = platform::pick_files("Upload files")
                        {
                            let (g, o2, n2, b, dir) = (cx.gh.clone(), o.clone(), n.clone(), git_ref.to_string(), self.path.clone());
                            let count = files.len();
                            self.listing = Load::spawn(ui.ctx(), move || {
                                for f in &files {
                                    let name = f.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                                    let target = if dir.is_empty() { name.clone() } else { format!("{dir}/{name}") };
                                    let bytes = std::fs::read(f)?;
                                    let sha = g.file_opt(&o2, &n2, &target, &b)?.map(|c| c.sha);
                                    g.put_file(&o2, &n2, &target, &bytes, &format!("Add {name}"), &b, sha.as_deref())?;
                                }
                                let mut v = g.contents(&o2, &n2, &dir, &b)?;
                                v.sort_by_key(|a| (!a.is_dir(), a.name.to_lowercase()));
                                Ok(v)
                            });
                            cx.info(format!("Uploading {count} file(s)…"));
                        }
                    });
                }
            });
        });
        ui.add_space(16.0);

        w::bordered(ui, |ui| {
            // latest commit bar
            Frame::new()
                .fill(pal.canvas_subtle)
                .corner_radius(CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 })
                .inner_margin(Margin::symmetric(16, 12))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    match self.last_commit.poll() {
                        Load::Ready(Some(c)) => {
                            let c = c.clone();
                            commit_bar(ui, cx, repo, &c, git_ref);
                        }
                        Load::Ready(None) => {
                            w::muted(ui, "No commits yet");
                        }
                        _ => {
                            ui.add(egui::Spinner::new().size(16.0));
                        }
                    }
                });
            w::divider(ui);
            let mut go_up = false;
            if !self.path.is_empty() {
                let r = w::list_row(ui, |ui| {
                    ui.horizontal(|ui| {
                        icon(ui, Icon::FileDirectoryFill, pal.accent);
                        ui.label(RichText::new("..").color(pal.accent));
                    });
                });
                go_up = r.clicked();
                w::divider(ui);
            }
            if go_up {
                let parent = self.path.rsplit_once('/').map(|(p, _)| p.to_string()).unwrap_or_default();
                cx.go(Route::repo(&o, &n, RepoView::Code { git_ref: Some(git_ref.to_string()), path: parent }));
            }
            self.path_commits.poll();
            let commits = match &self.path_commits {
                Load::Ready(m) => Some(m),
                _ => None,
            };
            let mut open_commit = None;
            w::show_load(ui, &mut self.listing, |ui, items| {
                if items.is_empty() {
                    w::blankslate(ui, Icon::FileDirectory, "This directory is empty", "");
                }
                for it in items.iter() {
                    let mut over_msg = None;
                    let resp = w::list_row(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (ic, color) = match it.kind.as_str() {
                                "dir" => (Icon::FileDirectoryFill, pal.accent.gamma_multiply(0.85)),
                                "submodule" => (Icon::FileSymlinkFile, pal.fg_muted),
                                "symlink" => (Icon::FileSymlinkFile, pal.fg_muted),
                                _ => (Icon::File, pal.fg_muted),
                            };
                            icon(ui, ic, color);
                            // three columns like github: name, last commit message, age
                            let total = ui.available_width();
                            let name_w = (total * 0.32).clamp(140.0, 340.0);
                            let time_w = 110.0;
                            ui.allocate_ui_with_layout(Vec2::new(name_w, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.set_width(name_w);
                                w::plain_link(ui, &it.name);
                            });
                            let msg_w = (ui.available_width() - time_w).max(40.0);
                            let last = commits.and_then(|m| m.get(&it.path));
                            ui.allocate_ui_with_layout(Vec2::new(msg_w, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.set_width(msg_w);
                                if let Some(c) = last {
                                    let r = ui.add(
                                        egui::Label::new(RichText::new(&c.message).color(pal.fg_muted)).truncate().sense(Sense::click()),
                                    );
                                    // the row's click area sits on top, so remember the pointer
                                    // being over the message and route the click there
                                    if ui.rect_contains_pointer(r.rect) {
                                        over_msg = Some(c.sha.clone());
                                    }
                                }
                            });
                            w::right(ui, |ui| {
                                if let Some(d) = last.and_then(|c| c.date.as_ref()) {
                                    w::muted_small(ui, w::rel_time(d));
                                }
                            });
                        });
                    });
                    if resp.clicked()
                        && let Some(sha) = over_msg.take()
                    {
                        open_commit = Some(sha);
                    } else if resp.clicked() {
                        let view = if it.is_dir() {
                            RepoView::Code { git_ref: Some(git_ref.to_string()), path: it.path.clone() }
                        } else {
                            RepoView::Blob { git_ref: Some(git_ref.to_string()), path: it.path.clone() }
                        };
                        cx.go(Route::repo(&o, &n, view));
                    }
                    w::divider(ui);
                }
            });
            if let Some(sha) = open_commit {
                cx.go(Route::repo(&o, &n, RepoView::Commit(sha)));
            }
        });
        ui.add_space(24.0);

        if let Load::Ready(Some(rm)) = &mut self.readme {
            let rm = rm.clone();
            let text = rm.text().unwrap_or_default();
            let dir = rm.path.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
            w::bordered(ui, |ui| {
                Frame::new().inner_margin(Margin::symmetric(16, 8)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        icon(ui, Icon::Book, pal.fg_muted);
                        ui.label(RichText::new("README").font(semibold(14.0)));
                        w::right(ui, |ui| {
                            if repo.can_push() && w::icon_button_invisible(ui, Icon::Pencil, "Edit README").clicked() {
                                cx.go(Route::repo(&o, &n, RepoView::Readme));
                            }
                        });
                    });
                });
                w::divider(ui);
                Frame::new().inner_margin(Margin::same(32)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let base = raw_base(&o, &n, git_ref, &dir);
                    markdown(ui, &mut self.md, &text, Some(&base));
                });
            });
        } else if matches!(self.readme, Load::Ready(None)) && self.path.is_empty() && repo.can_push() {
            w::bordered(ui, |ui| {
                Frame::new().inner_margin(Margin::same(24)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        w::muted(ui, "Help people interested in this repository understand your project by adding a README.");
                        w::right(ui, |ui| {
                            if w::button(ui, Variant::Primary, None, "Add a README").clicked() {
                                cx.go(Route::repo(&o, &n, RepoView::Readme));
                            }
                        });
                    });
                });
            });
        }
    }

    fn about(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let pal = w::p(ui);
        let (o, n) = on(repo);
        ui.horizontal(|ui| {
            ui.label(RichText::new("About").font(semibold(16.0)));
            if repo.is_admin() {
                w::right(ui, |ui| {
                    if w::icon_button_invisible(ui, Icon::Gear, "Edit repository details").clicked() {
                        cx.go(Route::repo(&o, &n, RepoView::Settings));
                    }
                });
            }
        });
        ui.add_space(8.0);
        match &repo.description {
            Some(d) if !d.is_empty() => {
                ui.add(egui::Label::new(RichText::new(d).size(16.0)).wrap());
            }
            _ => {
                ui.label(RichText::new("No description, website, or topics provided.").italics().color(pal.fg_muted));
            }
        }
        if let Some(h) = repo.homepage.as_ref().filter(|h| !h.is_empty()) {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                icon(ui, Icon::Link, pal.fg);
                if w::link_strong(ui, h.trim_start_matches("https://").trim_start_matches("http://"), 14.0).clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(h));
                }
            });
        }
        if !repo.topics.is_empty() {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                for t in &repo.topics {
                    common::topic(ui, t);
                }
            });
        }
        ui.add_space(12.0);
        if matches!(self.readme, Load::Ready(Some(_))) {
            w::meta(ui, Icon::Book, "Readme");
        }
        if let Some(l) = &repo.license {
            w::meta(ui, Icon::Law, format!("{} license", l.spdx_id.clone().unwrap_or_else(|| l.name.clone())));
        }
        if w::meta(ui, Icon::Pulse, "Activity").interact(Sense::click()).clicked() {
            cx.go(Route::repo(&o, &n, RepoView::Commits { git_ref: None }));
        }
        w::meta(ui, Icon::Star, RichText::new(format!("{} stars", w::format_count(repo.stargazers_count))));
        w::meta(ui, Icon::Eye, RichText::new(format!("{} watching", w::format_count(repo.subscribers_count))));
        w::meta(ui, Icon::RepoForked, RichText::new(format!("{} forks", w::format_count(repo.forks_count))));
        ui.add_space(16.0);
        w::divider(ui);
        ui.add_space(16.0);

        if let Some(rels) = self.releases.ready() {
            let published: Vec<&Release> = rels.iter().filter(|r| !r.draft).collect();
            ui.horizontal(|ui| {
                ui.label(RichText::new("Releases").font(semibold(16.0)));
                w::counter(ui, published.len() as u64);
            });
            ui.add_space(4.0);
            // "Latest" is the newest full release; pre-releases never get that badge
            let pick = published.iter().find(|r| !r.prerelease).or(published.first());
            if let Some(latest) = pick {
                let latest = (*latest).clone();
                ui.horizontal(|ui| {
                    icon(ui, Icon::Tag, pal.success);
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            if w::title_link(ui, latest.name.as_deref().unwrap_or(&latest.tag_name), 14.0).clicked() {
                                ui.ctx().open_url(egui::OpenUrl::new_tab(&latest.html_url));
                            }
                            if latest.prerelease {
                                w::outline_label(ui, "Pre-release", Some(pal.attention));
                            } else {
                                w::outline_label(ui, "Latest", Some(pal.success));
                            }
                        });
                        if let Some(d) = &latest.published_at {
                            w::muted_small(ui, w::rel_time(d));
                        }
                    });
                });
            } else {
                w::muted_small(ui, "No releases published");
            }
            ui.add_space(16.0);
            w::divider(ui);
            ui.add_space(16.0);
        }

        if let Some(langs) = self.languages.ready()
            && !langs.is_empty()
        {
            ui.label(RichText::new("Languages").font(semibold(16.0)));
            ui.add_space(8.0);
            let shown = language_breakdown(langs);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 8.0), Sense::hover());
            let mut x = rect.left();
            let painter = ui.painter();
            painter.rect_filled(rect, CornerRadius::same(4), pal.neutral_muted);
            for (i, (l, pct)) in shown.iter().enumerate() {
                let wdt = rect.width() * (*pct as f32 / 100.0);
                let r = egui::Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(wdt.max(1.0), 8.0));
                let cr = if shown.len() == 1 {
                    CornerRadius::same(4)
                } else if i == 0 {
                    CornerRadius { nw: 4, sw: 4, ne: 0, se: 0 }
                } else if i == shown.len() - 1 {
                    CornerRadius { ne: 4, se: 4, nw: 0, sw: 0 }
                } else {
                    CornerRadius::ZERO
                };
                let color = if l == "Other" { pal.fg_subtle } else { w::language_color(l) };
                painter.rect_filled(r, cr, color);
                x += wdt + 1.0;
            }
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                for (l, pct) in &shown {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                        let color = if l == "Other" { pal.fg_subtle } else { w::language_color(l) };
                        ui.painter().circle_filled(r.center(), 4.0, color);
                        ui.label(RichText::new(l).size(12.0).font(semibold(12.0)));
                        w::muted_small(ui, format!("{pct:.1}%"));
                    });
                }
            });
        }
    }
}

/// Language shares the way github lists them: each rounded down to 0.1%,
/// anything under 0.1% or past the sixth language folded into "Other".
pub fn language_breakdown(langs: &[(String, u64)]) -> Vec<(String, f64)> {
    let total: u64 = langs.iter().map(|l| l.1).sum::<u64>().max(1);
    let mut out: Vec<(String, f64)> = vec![];
    for (l, b) in langs {
        let pct = (*b as f64 * 1000.0 / total as f64).floor() / 10.0;
        if pct >= 0.1 && out.len() < 6 {
            out.push((l.clone(), pct));
        }
    }
    let shown: f64 = out.iter().map(|x| x.1).sum();
    let other = ((100.0 - shown) * 10.0).round() / 10.0;
    if other >= 0.1 {
        out.push(("Other".into(), other));
    }
    out
}

fn commit_bar(ui: &mut Ui, cx: &mut Cx, repo: &Repo, c: &Commit, git_ref: &str) {
    let pal = w::p(ui);
    let (o, n) = on(repo);
    ui.horizontal(|ui| {
        let (login, avatar) =
            c.author.as_ref().map(|a| (a.login.clone(), a.avatar_url.clone())).unwrap_or((c.commit.author.name.clone(), String::new()));
        w::avatar(ui, &avatar, 24.0);
        if w::title_link(ui, &login, 14.0).clicked() && c.author.is_some() {
            cx.go(Route::User(login.clone()));
        }
        let title = c.title().to_string();
        let r = ui.add(egui::Label::new(RichText::new(&title).color(pal.fg_muted)).truncate().sense(Sense::click()));
        if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            cx.go(Route::repo(&o, &n, RepoView::Commit(c.sha.clone())));
        }
        w::right(ui, |ui| {
            if w::button(ui, Variant::Invisible, Some(Icon::History), "Commits").clicked() {
                cx.go(Route::repo(&o, &n, RepoView::Commits { git_ref: Some(git_ref.to_string()) }));
            }
            if let Some(d) = &c.commit.author.date {
                w::muted_small(ui, w::rel_time(d));
            }
            if w::link(ui, c.short_sha()).clicked() {
                cx.go(Route::repo(&o, &n, RepoView::Commit(c.sha.clone())));
            }
        });
    });
}

fn empty_repo(ui: &mut Ui, repo: &Repo, cx: &mut Cx) {
    let pal = w::p(ui);
    let (o, n) = on(repo);
    w::bordered(ui, |ui| {
        Frame::new().fill(pal.accent_subtle).inner_margin(Margin::same(16)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Quick setup — if you've done this kind of thing before").font(semibold(16.0)));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let mut url = repo.clone_url.clone();
                ui.add(egui::TextEdit::singleline(&mut url).font(egui::TextStyle::Monospace).desired_width(420.0));
                if w::icon_button(ui, Icon::Copy, "Copy").clicked() {
                    ui.ctx().copy_text(repo.clone_url.clone());
                }
                if w::button(ui, Variant::Default, Some(Icon::DesktopDownload), "Set up with Rivet").clicked() {
                    cx.go(Route::Clone { url: repo.clone_url.clone() });
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                w::muted(ui, "Get started by");
                if repo.can_push() && w::link(ui, "creating a README").clicked() {
                    cx.go(Route::repo(&o, &n, RepoView::Readme));
                }
                w::muted(ui, "or adding community files.");
            });
        });
        Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
            ui.label(RichText::new("…or push an existing repository from the command line").font(semibold(14.0)));
            ui.add_space(8.0);
            let snippet = format!(
                "git remote add origin {}\ngit branch -M {}\ngit push -u origin {}",
                repo.clone_url, repo.default_branch, repo.default_branch
            );
            Frame::new().fill(pal.canvas_subtle).corner_radius(CornerRadius::same(6)).inner_margin(Margin::same(12)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(snippet).monospace());
            });
        });
    });
}

pub fn human_size(b: u64) -> String {
    match b {
        0..1024 => format!("{b} Bytes"),
        1024..1_048_576 => format!("{:.1} KB", b as f64 / 1024.0),
        _ => format!("{:.1} MB", b as f64 / 1_048_576.0),
    }
}

/// owner / name / dir / file path breadcrumb.
fn breadcrumb(ui: &mut Ui, cx: &mut Cx, repo: &Repo, git_ref: &str, path: &str, last_is_file: bool) {
    let (o, n) = on(repo);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if w::link_strong(ui, &repo.name, 16.0).clicked() {
            cx.go(Route::repo(&o, &n, RepoView::Code { git_ref: Some(git_ref.to_string()), path: String::new() }));
        }
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        for (i, part) in parts.iter().enumerate() {
            ui.label(RichText::new("/").size(16.0).color(w::p(ui).fg_muted));
            let last = i == parts.len() - 1;
            if last {
                ui.label(RichText::new(*part).font(semibold(16.0)));
                if last_is_file && w::icon_button_invisible(ui, Icon::Copy, "Copy path").clicked() {
                    ui.ctx().copy_text(path.to_string());
                }
            } else if w::link_strong(ui, *part, 16.0).clicked() {
                cx.go(Route::repo(&o, &n, RepoView::Code { git_ref: Some(git_ref.to_string()), path: parts[..=i].join("/") }));
            }
        }
    });
}

/// File viewer with syntax highlighting, markdown preview, images, and an
/// in-place editor that commits through the contents API.
pub struct BlobView {
    git_ref: Option<String>,
    path: String,
    new_file: bool,
    file: Load<Content>,
    bytes: Option<Vec<u8>>,
    last_commit: Load<Option<Commit>>,
    md: CommonMarkCache,
    show_source: bool,
    editing: bool,
    buffer: String,
    new_name: String,
    message: String,
    branch_mode: usize,
    new_branch: String,
    save: Load<Option<String>>,
    confirm_delete: bool,
    branches: Load<Vec<String>>,
    branch_filter: String,
}

impl BlobView {
    pub fn new(git_ref: Option<String>, path: String) -> Self {
        // "dir/\0new" is how the Add file menu asks for a blank file
        let new_file = path.ends_with("\u{0}new");
        let path = path.trim_end_matches("\u{0}new").to_string();
        Self {
            git_ref,
            path,
            new_file,
            file: Load::Idle,
            bytes: None,
            last_commit: Load::Idle,
            md: CommonMarkCache::default(),
            show_source: false,
            editing: new_file,
            buffer: String::new(),
            new_name: String::new(),
            message: String::new(),
            branch_mode: 0,
            new_branch: String::new(),
            save: Load::Idle,
            confirm_delete: false,
            branches: Load::Idle,
            branch_filter: String::new(),
        }
    }
}

fn is_image(path: &str) -> bool {
    let l = path.to_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".bmp", ".ico"].iter().any(|e| l.ends_with(e))
}

fn is_markdown(path: &str) -> bool {
    let l = path.to_lowercase();
    l.ends_with(".md") || l.ends_with(".markdown") || l.ends_with(".mdx")
}

pub fn lang_for(path: &str) -> &str {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => "rs",
        "py" => "py",
        "js" | "mjs" | "cjs" => "js",
        "ts" | "tsx" => "ts",
        "jsx" => "js",
        "json" => "json",
        "toml" => "toml",
        "yml" | "yaml" => "yaml",
        "md" => "md",
        "c" | "h" => "c",
        "cpp" | "cc" | "hpp" | "cxx" => "cpp",
        "cs" => "cs",
        "go" => "go",
        "java" => "java",
        "kt" => "kotlin",
        "rb" => "rb",
        "php" => "php",
        "sh" | "bash" | "zsh" => "sh",
        "html" | "htm" => "html",
        "css" | "scss" => "css",
        "sql" => "sql",
        "lua" | "luau" => "lua",
        "swift" => "swift",
        "xml" => "xml",
        "gd" => "py",
        _ => ext,
    }
}

impl RepoSub for BlobView {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo) {
        let ctx = ui.ctx().clone();
        let git_ref = self.git_ref.clone().unwrap_or_else(|| repo.default_branch.clone());
        let (o, n) = on(repo);
        if self.file.is_idle() && !self.new_file {
            let (g, o2, n2, p, r) = (cx.gh.clone(), o.clone(), n.clone(), self.path.clone(), git_ref.clone());
            self.file = Load::spawn(&ctx, move || {
                let mut f = g.file(&o2, &n2, &p, &r)?;
                // >1MB files come without inline content
                if f.content.as_deref().is_none_or(|c| c.is_empty()) && f.size > 0 {
                    use base64::Engine;
                    let bytes = g.blob_text(&o2, &n2, &f.sha)?;
                    f.content = Some(base64::engine::general_purpose::STANDARD.encode(bytes));
                }
                Ok(f)
            });
            let (g, o2, n2, p, r) = (cx.gh.clone(), o.clone(), n.clone(), self.path.clone(), git_ref.clone());
            self.last_commit = Load::spawn(&ctx, move || Ok(g.commits(&o2, &n2, &r, Some(&p), 1)?.into_iter().next()));
        }
        if self.bytes.is_none()
            && let Some(f) = self.file.ready()
        {
            self.bytes = f.decoded();
            self.buffer = self.bytes.as_ref().and_then(|b| String::from_utf8(b.clone()).ok()).unwrap_or_default();
        }
        match self.save.take() {
            Some(Ok(Some(msg))) => {
                cx.ok(msg);
                let target = if self.new_file { self.full_new_path() } else { self.path.clone() };
                let branch = if self.branch_mode == 1 { self.new_branch.clone() } else { git_ref.clone() };
                if self.branch_mode == 1 {
                    cx.go(Route::repo(&o, &n, RepoView::NewPull { base: Some(git_ref.clone()), head: Some(branch) }));
                } else {
                    cx.go(Route::repo(&o, &n, RepoView::Blob { git_ref: Some(branch), path: target }));
                }
            }
            Some(Ok(None)) => {
                cx.ok(format!("Deleted {}", self.path));
                let parent = self.path.rsplit_once('/').map(|(p, _)| p.to_string()).unwrap_or_default();
                cx.go(Route::repo(&o, &n, RepoView::Code { git_ref: Some(git_ref.clone()), path: parent }));
            }
            Some(Err(e)) => cx.err(e),
            None => {}
        }

        super::section(ui, |ui| {
            let pal = w::p(ui);
            ui.horizontal(|ui| {
                if let Some(r) = ref_picker(ui, cx, repo, &git_ref, &mut self.branches, &mut self.branch_filter) {
                    cx.go(Route::repo(&o, &n, RepoView::Blob { git_ref: Some(r), path: self.path.clone() }));
                }
                if self.new_file {
                    breadcrumb(ui, cx, repo, &git_ref, &self.path, false);
                    ui.label(RichText::new("/").size(16.0).color(pal.fg_muted));
                    w::text_input(ui, &mut self.new_name, "Name your file…", 260.0);
                } else {
                    breadcrumb(ui, cx, repo, &git_ref, &self.path, true);
                }
            });
            ui.add_space(16.0);

            if self.editing {
                self.editor(ui, cx, repo, &git_ref);
                return;
            }
            if let Load::Failed(e) = self.file.poll() {
                w::flash(ui, FlashKind::Danger, &e.clone());
                return;
            }
            if self.bytes.is_none() {
                w::spinner(ui);
                return;
            }
            if let Some(Some(c)) = self.last_commit.ready().cloned() {
                w::bordered(ui, |ui| {
                    Frame::new().inner_margin(Margin::symmetric(16, 10)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        commit_bar(ui, cx, repo, &c, &git_ref);
                    });
                });
                ui.add_space(16.0);
            }
            let bytes = self.bytes.clone().unwrap_or_default();
            let text = String::from_utf8(bytes.clone()).ok();
            let lines = text.as_ref().map(|t| t.lines().count()).unwrap_or(0);
            let file = self.file.ready().cloned().unwrap_or_default();
            w::bordered(ui, |ui| {
                Frame::new()
                    .fill(pal.canvas_subtle)
                    .corner_radius(CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 })
                    .inner_margin(Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            if is_markdown(&self.path) {
                                if w::button(ui, if self.show_source { Variant::Invisible } else { Variant::Default }, None, "Preview")
                                    .clicked()
                                {
                                    self.show_source = false;
                                }
                                if w::button(ui, if self.show_source { Variant::Default } else { Variant::Invisible }, None, "Code")
                                    .clicked()
                                {
                                    self.show_source = true;
                                }
                            }
                            let info =
                                if text.is_some() { format!("{lines} lines · {}", human_size(file.size)) } else { human_size(file.size) };
                            w::muted_small(ui, info);
                            w::right(ui, |ui| {
                                if repo.can_push() {
                                    if w::icon_button_invisible(ui, Icon::Trash, "Delete this file").clicked() {
                                        self.confirm_delete = true;
                                    }
                                    if text.is_some() && w::icon_button_invisible(ui, Icon::Pencil, "Edit this file").clicked() {
                                        self.editing = true;
                                    }
                                }
                                if let Some(t) = &text
                                    && w::icon_button_invisible(ui, Icon::Copy, "Copy raw content").clicked()
                                {
                                    ui.ctx().copy_text(t.clone());
                                    cx.ok("Copied!");
                                }
                                if platform::NATIVE_FILES
                                    && w::icon_button_invisible(ui, Icon::Download, "Download raw file").clicked()
                                    && let Some(dest) = platform::save_file(&file.name)
                                {
                                    match std::fs::write(&dest, &bytes) {
                                        Ok(()) => cx.ok(format!("Saved to {}", dest.display())),
                                        Err(e) => cx.err(e.to_string()),
                                    }
                                }
                                if w::button(ui, Variant::Default, None, "Raw").clicked()
                                    && let Some(u) = &file.download_url
                                {
                                    ui.ctx().open_url(egui::OpenUrl::new_tab(u));
                                }
                                if w::button(ui, Variant::Default, None, "History").clicked() {
                                    cx.go(Route::repo(&o, &n, RepoView::Commits { git_ref: Some(git_ref.clone()) }));
                                }
                            });
                        });
                    });
                w::divider(ui);
                if is_image(&self.path) {
                    Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            let uri = format!("bytes://{}/{}/{}", repo.full_name, file.sha, file.name);
                            ui.add(egui::Image::from_bytes(uri, bytes.clone()).max_width(ui.available_width()).max_height(700.0));
                        });
                    });
                } else if let Some(t) = &text {
                    if is_markdown(&self.path) && !self.show_source {
                        Frame::new().inner_margin(Margin::same(32)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            let dir = self.path.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
                            markdown(ui, &mut self.md, t, Some(&raw_base(&o, &n, &git_ref, &dir)));
                        });
                    } else {
                        code_listing(ui, t, lang_for(&self.path));
                    }
                } else {
                    Frame::new().inner_margin(Margin::same(24)).show(ui, |ui| {
                        ui.vertical_centered(|ui| w::muted(ui, "Binary file not shown. Use Raw or Download to view it."));
                    });
                }
            });
        });

        if self.confirm_delete {
            self.delete_modal(&ctx, cx, repo, &git_ref);
        }
    }
}

impl BlobView {
    fn full_new_path(&self) -> String {
        let name = self.new_name.trim().trim_matches('/');
        if self.path.is_empty() { name.to_string() } else { format!("{}/{name}", self.path.trim_end_matches('/')) }
    }

    fn editor(&mut self, ui: &mut Ui, cx: &mut Cx, repo: &Repo, git_ref: &str) {
        let ctx = ui.ctx().clone();
        let pal = w::p(ui);
        let (o, n) = on(repo);
        w::bordered(ui, |ui| {
            Frame::new().fill(pal.canvas_subtle).inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(if self.new_file { "New file" } else { "Edit" }).font(semibold(14.0)));
                    w::muted_small(ui, "Spaces · UTF-8");
                });
            });
            w::divider(ui);
            let lang = lang_for(if self.new_file { &self.new_name } else { &self.path }).to_string();
            let theme = egui_extras::syntax_highlighting::CodeTheme::from_style(ui.style());
            let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, wrap: f32| {
                let mut job = egui_extras::syntax_highlighting::highlight(ui.ctx(), ui.style(), &theme, buf.as_str(), &lang);
                job.wrap.max_width = wrap;
                ui.fonts_mut(|f| f.layout_job(job))
            };
            egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.buffer)
                        .code_editor()
                        .frame(Frame::NONE)
                        .desired_width(f32::INFINITY)
                        .desired_rows(28)
                        .layouter(&mut layouter),
                );
            });
        });
        ui.add_space(16.0);
        w::bordered(ui, |ui| {
            Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                ui.label(RichText::new("Commit changes").font(semibold(16.0)));
                ui.add_space(8.0);
                let default_msg = if self.new_file {
                    format!("Create {}", self.new_name.trim())
                } else {
                    format!("Update {}", self.path.rsplit('/').next().unwrap_or(""))
                };
                w::form_label(ui, "Commit message", false);
                w::text_input(ui, &mut self.message, &default_msg, ui.available_width());
                ui.add_space(12.0);
                ui.radio_value(&mut self.branch_mode, 0, format!("Commit directly to the {git_ref} branch"));
                ui.radio_value(&mut self.branch_mode, 1, "Create a new branch for this commit and start a pull request");
                if self.branch_mode == 1 {
                    ui.horizontal(|ui| {
                        ui.add_space(24.0);
                        icon(ui, Icon::GitBranch, pal.fg_muted);
                        w::text_input(ui, &mut self.new_branch, &format!("{}-patch-1", cx.me.login), 300.0);
                    });
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    w::right(ui, |ui| {
                        let busy = self.save.is_loading();
                        let name_ok = !self.new_file || !self.new_name.trim().is_empty();
                        if w::button_ex(
                            ui,
                            Variant::Primary,
                            None,
                            if busy { "Committing…" } else { "Commit changes" },
                            None,
                            !busy && name_ok,
                        )
                        .clicked()
                        {
                            if self.branch_mode == 1 && self.new_branch.trim().is_empty() {
                                self.new_branch = format!("{}-patch-1", cx.me.login);
                            }
                            let msg = if self.message.trim().is_empty() { default_msg.clone() } else { self.message.trim().to_string() };
                            let target = if self.new_file { self.full_new_path() } else { self.path.clone() };
                            let sha = self.file.ready().map(|f| f.sha.clone());
                            let (g, o2, n2, base) = (cx.gh.clone(), o.clone(), n.clone(), git_ref.to_string());
                            let new_branch = if self.branch_mode == 1 { Some(self.new_branch.trim().to_string()) } else { None };
                            let content = self.buffer.clone();
                            self.save = Load::spawn(&ctx, move || {
                                let branch = match &new_branch {
                                    Some(b) => {
                                        let head =
                                            g.commits(&o2, &n2, &base, None, 1)?.into_iter().next().map(|c| c.sha).unwrap_or_default();
                                        g.create_branch(&o2, &n2, b, &head)?;
                                        b.clone()
                                    }
                                    None => base.clone(),
                                };
                                g.put_file(&o2, &n2, &target, content.as_bytes(), &msg, &branch, sha.as_deref())?;
                                Ok(Some(format!("Committed to {branch}")))
                            });
                        }
                        if w::button(ui, Variant::Default, None, "Cancel changes").clicked() {
                            if self.new_file {
                                cx.go(Route::repo(&o, &n, RepoView::Code { git_ref: Some(git_ref.to_string()), path: self.path.clone() }));
                            } else {
                                self.editing = false;
                                self.buffer = self.bytes.as_ref().and_then(|b| String::from_utf8(b.clone()).ok()).unwrap_or_default();
                            }
                        }
                    });
                });
            });
        });
    }

    fn delete_modal(&mut self, ctx: &egui::Context, cx: &mut Cx, repo: &Repo, git_ref: &str) {
        let pal = crate::theme::pal(ctx);
        let modal = egui::Modal::new(egui::Id::unique("delete-file"))
            .backdrop_color(pal.overlay_shadow)
            .frame(
                Frame::new()
                    .fill(pal.canvas)
                    .stroke(Stroke::new(1.0, pal.border))
                    .corner_radius(CornerRadius::same(12))
                    .inner_margin(Margin::same(16)),
            )
            .show(ctx, |ui| {
                ui.set_width(420.0);
                ui.label(RichText::new(format!("Delete {}?", self.path)).font(semibold(16.0)));
                ui.add_space(8.0);
                w::muted(ui, format!("This commits the deletion directly to {git_ref}."));
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    w::right(ui, |ui| {
                        if w::button(ui, Variant::Danger, None, "Delete file").clicked() {
                            let (g, (o, n), p, b) = (cx.gh.clone(), on(repo), self.path.clone(), git_ref.to_string());
                            let sha = self.file.ready().map(|f| f.sha.clone()).unwrap_or_default();
                            self.save = Load::spawn(ctx, move || {
                                g.delete_file(&o, &n, &p, &format!("Delete {p}"), &b, &sha)?;
                                Ok(None)
                            });
                            self.confirm_delete = false;
                        }
                        if w::button(ui, Variant::Default, None, "Cancel").clicked() {
                            self.confirm_delete = false;
                        }
                    });
                });
            });
        if modal.should_close() {
            self.confirm_delete = false;
        }
    }
}

/// Read-only code with a line number gutter, like the blob view on github.com.
pub fn code_listing(ui: &mut Ui, text: &str, lang: &str) {
    let pal = w::p(ui);
    let theme = egui_extras::syntax_highlighting::CodeTheme::from_style(ui.style());
    // past ~5k lines egui's single galley gets slow, so trim and say so
    let max_lines = 5000;
    let total = text.lines().count();
    let shown: String = if total > max_lines { text.lines().take(max_lines).collect::<Vec<_>>().join("\n") } else { text.to_string() };
    let job = egui_extras::syntax_highlighting::highlight(ui.ctx(), ui.style(), &theme, &shown, lang);
    let nums: String = (1..=shown.lines().count().max(1)).map(|i| format!("{i}\n")).collect();
    egui::ScrollArea::horizontal().id_salt("code-h").show(ui, |ui| {
        Frame::new().inner_margin(Margin::symmetric(0, 8)).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                ui.add_space(8.0);
                ui.add(egui::Label::new(RichText::new(nums.trim_end()).monospace().color(pal.fg_subtle)).selectable(false));
                ui.add(egui::Label::new(job).selectable(true).extend());
            });
        });
    });
    if total > max_lines {
        Frame::new().inner_margin(Margin::same(12)).show(ui, |ui| {
            w::muted(ui, format!("Showing the first {max_lines} of {total} lines. Use Raw to see the whole file."));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::language_breakdown;

    #[test]
    fn tiny_languages_fold_into_other() {
        // PowerNukkitX: java plus a few kilobytes of scripts
        let langs = vec![("Java".to_string(), 17_003_466), ("Python".into(), 5413), ("Dockerfile".into(), 2024), ("Shell".into(), 566)];
        let b = language_breakdown(&langs);
        assert_eq!(b, vec![("Java".to_string(), 99.9), ("Other".to_string(), 0.1)]);
    }

    #[test]
    fn even_split_keeps_everything() {
        let b = language_breakdown(&[("Rust".into(), 50), ("C".into(), 50)]);
        assert_eq!(b, vec![("Rust".to_string(), 50.0), ("C".to_string(), 50.0)]);
    }
}
