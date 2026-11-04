//! Building blocks shared by several screens: markdown, timeline comments,
//! comment composer, diffs, list rows, pickers and pagination.

use crate::app::Cx;
use crate::github::models::{ChangedFile, Issue, Label, Reactions, Repo, User};
use crate::icons::{Icon, icon};
use crate::route::Route;
use crate::theme::semibold;
use crate::widgets::{self as w, Variant};
use chrono::{DateTime, Utc};
use egui::{Color32, CornerRadius, FontId, Frame, Margin, RichText, Sense, Stroke, Ui, Vec2};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

/// GitHub Flavored Markdown. `base` resolves relative images/links (raw url of
/// the repo at a ref), so READMEs render like on github.com.
pub fn markdown(ui: &mut Ui, cache: &mut CommonMarkCache, text: &str, base: Option<&str>) {
    let mut v = CommonMarkViewer::new().max_image_width(Some(ui.available_width() as usize));
    if let Some(b) = base {
        v = v.default_implicit_uri_scheme(b);
    }
    let text = expand_gh_shortcuts(text);
    ui.scope(|ui| {
        ui.style_mut().spacing.item_spacing.y = 8.0;
        v.show(ui, cache, &text);
    });
}

/// GitHub turns bare `#123` and `@user` into links; commonmark doesn't, so
/// rewrite the obvious cases. Code spans and fences are left alone.
fn expand_gh_shortcuts(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_fence = false;
    for line in text.split_inclusive('\n') {
        if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
            in_fence = !in_fence;
            out.push_str(line);
            continue;
        }
        if in_fence {
            out.push_str(line);
            continue;
        }
        let mut in_code = false;
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '`' {
                in_code = !in_code;
            }
            let prev_ok = i == 0 || chars[i - 1].is_whitespace() || chars[i - 1] == '(';
            if !in_code && c == '@' && prev_ok {
                let mut j = i + 1;
                while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '-') {
                    j += 1;
                }
                if j > i + 1 {
                    let user: String = chars[i + 1..j].iter().collect();
                    out.push_str(&format!("[@{user}](https://github.com/{user})"));
                    i = j;
                    continue;
                }
            }
            out.push(c);
            i += 1;
        }
    }
    out
}

/// Raw content base for a repo at a ref, used for relative README images.
pub fn raw_base(owner: &str, name: &str, git_ref: &str, dir: &str) -> String {
    let dir = if dir.is_empty() { String::new() } else { format!("{dir}/") };
    format!("https://raw.githubusercontent.com/{owner}/{name}/{git_ref}/{dir}")
}

/// Small grey "Owner" / "Member" / "Contributor" badge in comment headers.
fn association_badge(ui: &mut Ui, assoc: &str) {
    let text = match assoc {
        "OWNER" => "Owner",
        "MEMBER" => "Member",
        "COLLABORATOR" => "Collaborator",
        "CONTRIBUTOR" => "Contributor",
        "FIRST_TIME_CONTRIBUTOR" => "First-time contributor",
        _ => return,
    };
    w::outline_label(ui, text, None);
}

pub struct CommentAction {
    pub edit: bool,
    pub delete: bool,
    pub react: Option<&'static str>,
    pub quote: bool,
}

/// A timeline comment: avatar on the left, bordered box with header and body.
#[allow(clippy::too_many_arguments)]
pub fn comment_card(
    ui: &mut Ui,
    cache: &mut CommonMarkCache,
    user: &User,
    when: Option<&DateTime<Utc>>,
    body: &str,
    assoc: &str,
    reactions: Option<&Reactions>,
    highlight: bool,
    can_edit: bool,
    cx: &mut Cx,
) -> CommentAction {
    let pal = w::p(ui);
    let mut act = CommentAction { edit: false, delete: false, react: None, quote: false };
    ui.horizontal_top(|ui| {
        if w::avatar(ui, &user.avatar_url, 40.0).clicked() {
            cx.go(Route::User(user.login.clone()));
        }
        ui.add_space(8.0);
        let border = if highlight { pal.accent.gamma_multiply(0.6) } else { pal.border };
        ui.vertical(|ui| {
            Frame::new().stroke(Stroke::new(1.0, border)).corner_radius(CornerRadius::same(6)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                Frame::new()
                    .fill(if highlight { pal.accent_subtle } else { pal.canvas_subtle })
                    .corner_radius(CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 })
                    .inner_margin(Margin::symmetric(16, 8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            if w::title_link(ui, &user.login, 14.0).clicked() {
                                cx.go(Route::User(user.login.clone()));
                            }
                            if let Some(t) = when {
                                w::muted(ui, format!("commented {}", w::rel_time(t)));
                            }
                            w::right(ui, |ui| {
                                let more = w::icon_button_invisible(ui, Icon::KebabHorizontal, "Show options");
                                egui::Popup::menu(&more).align(egui::RectAlign::BOTTOM_END).show(|ui| {
                                    if w::menu_item(ui, Some(Icon::Copy), "Copy text").clicked() {
                                        ui.ctx().copy_text(body.to_string());
                                    }
                                    if w::menu_item(ui, Some(Icon::Comment), "Quote reply").clicked() {
                                        act.quote = true;
                                    }
                                    if can_edit {
                                        if w::menu_item(ui, Some(Icon::Pencil), "Edit").clicked() {
                                            act.edit = true;
                                        }
                                        if w::menu_item_ex(ui, Some(Icon::Trash), "Delete", false, true).clicked() {
                                            act.delete = true;
                                        }
                                    }
                                });
                                association_badge(ui, assoc);
                            });
                        });
                    });
                w::divider(ui);
                Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if body.trim().is_empty() {
                        ui.label(RichText::new("No description provided.").italics().color(pal.fg_muted));
                    } else {
                        markdown(ui, cache, body, None);
                    }
                    ui.add_space(4.0);
                    if let Some(r) = reaction_bar(ui, reactions) {
                        act.react = Some(r);
                    }
                });
            })
        });
    });
    act
}

const REACTIONS: [(&str, &str); 8] =
    [("+1", "👍"), ("-1", "👎"), ("laugh", "😄"), ("hooray", "🎉"), ("confused", "😕"), ("heart", "❤"), ("rocket", "🚀"), ("eyes", "👀")];

/// Existing reaction pills plus the smiley picker. Returns a reaction to add.
pub fn reaction_bar(ui: &mut Ui, r: Option<&Reactions>) -> Option<&'static str> {
    let pal = w::p(ui);
    let mut picked = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let smile = w::button(ui, Variant::Default, Some(Icon::Smiley), "").on_hover_text("Add reaction");
        egui::Popup::menu(&smile).show(|ui| {
            ui.horizontal(|ui| {
                for (key, emoji) in REACTIONS {
                    if ui.add(egui::Button::new(RichText::new(emoji).size(16.0)).frame(false)).on_hover_text(key).clicked() {
                        picked = Some(key);
                        ui.close();
                    }
                }
            });
        });
        if let Some(r) = r {
            let counts = [r.plus1, r.minus1, r.laugh, r.hooray, r.confused, r.heart, r.rocket, r.eyes];
            for ((key, emoji), n) in REACTIONS.iter().zip(counts) {
                if n == 0 {
                    continue;
                }
                let resp = Frame::new()
                    .fill(pal.canvas)
                    .stroke(Stroke::new(1.0, pal.border))
                    .corner_radius(CornerRadius::same(255))
                    .inner_margin(Margin::symmetric(8, 2))
                    .show(ui, |ui| ui.label(RichText::new(format!("{emoji} {n}")).size(12.0)))
                    .response;
                if ui.interact(resp.rect, ui.scope_id().with(("react", *key)), Sense::click()).clicked() {
                    picked = Some(*key);
                }
            }
        }
    });
    picked
}

/// Write / Preview comment box with a submit row, like the one under issues.
#[derive(Default)]
pub struct Composer {
    pub text: String,
    preview: bool,
    cache: CommonMarkCache,
}

impl Composer {
    pub fn with_text(text: String) -> Self {
        Self { text, ..Default::default() }
    }

    /// Draws the editor. The caller adds buttons in `actions`, which sits right-aligned below.
    pub fn ui(&mut self, ui: &mut Ui, hint: &str, rows: usize, actions: impl FnOnce(&mut Ui, &mut String)) {
        let pal = w::p(ui);
        Frame::new().stroke(Stroke::new(1.0, pal.border)).corner_radius(CornerRadius::same(6)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            Frame::new()
                .fill(pal.canvas_subtle)
                .corner_radius(CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 })
                .inner_margin(Margin { left: 8, right: 8, top: 8, bottom: 0 })
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        for (i, label) in ["Write", "Preview"].into_iter().enumerate() {
                            let sel = (i == 1) == self.preview;
                            let resp = Frame::new()
                                .fill(if sel { pal.canvas } else { Color32::TRANSPARENT })
                                .stroke(Stroke::new(1.0, if sel { pal.border } else { Color32::TRANSPARENT }))
                                .corner_radius(CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 })
                                .inner_margin(Margin::symmetric(16, 8))
                                .show(ui, |ui| ui.label(RichText::new(label).color(if sel { pal.fg } else { pal.fg_muted })))
                                .response;
                            if ui.interact(resp.rect, ui.scope_id().with(("composer-tab", i)), Sense::click()).clicked() {
                                self.preview = i == 1;
                            }
                        }
                        w::right(ui, |ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            toolbar(ui, &mut self.text);
                        });
                    });
                });
            Frame::new().inner_margin(Margin::same(8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                if self.preview {
                    ui.set_min_height(rows as f32 * 18.0);
                    if self.text.trim().is_empty() {
                        w::muted(ui, "Nothing to preview");
                    } else {
                        markdown(ui, &mut self.cache, &self.text, None);
                    }
                } else {
                    w::text_area(ui, &mut self.text, hint, rows, false);
                    ui.horizontal(|ui| {
                        icon(ui, Icon::Note, pal.fg_muted);
                        w::muted_small(ui, "Markdown is supported");
                    });
                }
            });
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            w::right(ui, |ui| actions(ui, &mut self.text));
        });
    }
}

/// Markdown formatting buttons. They append syntax at the end since egui's
/// TextEdit doesn't expose the selection here without extra plumbing.
fn toolbar(ui: &mut Ui, text: &mut String) {
    let items: [(&str, &str, &str); 7] = [
        ("H", "Heading", "### "),
        ("B", "Bold", "**bold**"),
        ("I", "Italic", "_italic_"),
        ("<>", "Code", "`code`"),
        ("🔗", "Link", "[text](url)"),
        ("•", "Bulleted list", "\n- "),
        ("☑", "Task list", "\n- [ ] "),
    ];
    let pal = w::p(ui);
    for (label, tip, snippet) in items.iter().rev() {
        if ui
            .add(egui::Button::new(RichText::new(*label).color(pal.fg_muted)).frame(false).min_size(Vec2::new(24.0, 24.0)))
            .on_hover_text(*tip)
            .clicked()
        {
            text.push_str(snippet);
        }
    }
}

/// Unified diff with old/new line numbers, GitHub colors.
pub fn diff_view(ui: &mut Ui, patch: &str) {
    let pal = w::p(ui);
    let font = FontId::monospace(12.0);
    let row_h = 20.0;
    let (mut old, mut new) = (0u64, 0u64);
    let lines: Vec<&str> = patch.lines().collect();
    // wide enough for the longest line so horizontal scroll areas can reach it,
    // never less than the box itself
    let longest = lines.iter().max_by_key(|l| l.chars().count()).copied().unwrap_or("");
    let text_w = ui.painter().layout_no_wrap(longest.replace('\t', "    "), font.clone(), pal.fg).size().x;
    let width = ui.available_width().max(text_w + 100.0 + 24.0);
    for line in lines {
        let (bg, num_bg, o, n) = if line.starts_with("@@") {
            if let Some((a, b)) = parse_hunk(line) {
                old = a;
                new = b;
            }
            (pal.diff_hunk_bg, pal.diff_hunk_bg, String::new(), String::new())
        } else if let Some(_rest) = line.strip_prefix('+') {
            new += 1;
            (pal.diff_add_bg, pal.diff_add_num, String::new(), (new - 1).to_string())
        } else if line.starts_with('-') {
            old += 1;
            (pal.diff_del_bg, pal.diff_del_num, (old - 1).to_string(), String::new())
        } else if line.starts_with('\\') {
            (pal.canvas, pal.canvas, String::new(), String::new())
        } else {
            old += 1;
            new += 1;
            (pal.canvas, pal.canvas, (old - 1).to_string(), (new - 1).to_string())
        };
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, row_h), Sense::hover());
        if !ui.is_rect_visible(rect) {
            continue;
        }
        let num_w = 50.0;
        let p = ui.painter();
        p.rect_filled(rect, 0.0, bg);
        let nums = egui::Rect::from_min_size(rect.min, Vec2::new(num_w * 2.0, row_h));
        p.rect_filled(nums, 0.0, num_bg);
        let fg_num = pal.fg_subtle;
        p.text(egui::pos2(rect.left() + num_w - 8.0, rect.center().y), egui::Align2::RIGHT_CENTER, o, font.clone(), fg_num);
        p.text(egui::pos2(rect.left() + num_w * 2.0 - 8.0, rect.center().y), egui::Align2::RIGHT_CENTER, n, font.clone(), fg_num);
        let color = if line.starts_with("@@") { pal.fg_muted } else { pal.fg };
        p.text(
            egui::pos2(rect.left() + num_w * 2.0 + 10.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            line.replace('\t', "    "),
            font.clone(),
            color,
        );
    }
}

fn parse_hunk(line: &str) -> Option<(u64, u64)> {
    // @@ -12,7 +12,9 @@ context
    let mut parts = line.split_whitespace().skip(1);
    let o = parts.next()?.trim_start_matches('-').split(',').next()?.parse().ok()?;
    let n = parts.next()?.trim_start_matches('+').split(',').next()?.parse().ok()?;
    Some((o, n))
}

/// The bordered box for one file in a diff, collapsible like on github.com.
pub fn file_diff_box(ui: &mut Ui, f: &ChangedFile, open: &mut bool, extra: impl FnOnce(&mut Ui)) {
    let pal = w::p(ui);
    ui.push_id(&f.filename, |ui| {
        w::bordered(ui, |ui| {
            Frame::new()
                .fill(pal.canvas_subtle)
                .corner_radius(CornerRadius { nw: 6, ne: 6, sw: if *open { 0 } else { 6 }, se: if *open { 0 } else { 6 } })
                .inner_margin(Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let ic = if *open { Icon::ChevronDown } else { Icon::ChevronRight };
                        if w::icon_button_invisible(ui, ic, "Toggle diff").clicked() {
                            *open = !*open;
                        }
                        diffstat_blocks(ui, f.additions, f.deletions);
                        let name = match &f.previous_filename {
                            Some(prev) if f.status == "renamed" => format!("{prev} → {}", f.filename),
                            _ => f.filename.clone(),
                        };
                        ui.label(RichText::new(name).monospace().size(12.0).strong());
                        if w::icon_button_invisible(ui, Icon::Copy, "Copy path").clicked() {
                            ui.ctx().copy_text(f.filename.clone());
                        }
                        w::right(ui, extra);
                    });
                });
            if *open {
                w::divider(ui);
                match &f.patch {
                    Some(p) => {
                        egui::ScrollArea::horizontal().id_salt("diff-h").show(ui, |ui| diff_view(ui, p));
                    }
                    None => {
                        Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                            w::muted(ui, "Binary file or diff too large to display.");
                        });
                    }
                }
            }
        });
    });
    ui.add_space(16.0);
}

/// "+12 −3" with the five little squares GitHub shows.
pub fn diffstat_blocks(ui: &mut Ui, add: u64, del: u64) {
    // inside right-aligned rows everything is laid out back to front
    if ui.layout().prefer_right_to_left() {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| diffstat_blocks(ui, add, del));
        return;
    }
    let pal = w::p(ui);
    ui.spacing_mut().item_spacing.x = 4.0;
    ui.label(RichText::new(format!("+{add}")).size(12.0).font(semibold(12.0)).color(pal.success));
    ui.label(RichText::new(format!("−{del}")).size(12.0).font(semibold(12.0)).color(pal.danger));
    let total = (add + del).max(1) as f32;
    let green = ((add as f32 / total) * 5.0).round() as usize;
    let red = if add + del == 0 { 0 } else { (((del as f32) / total) * 5.0).round() as usize };
    let (rect, _) = ui.allocate_exact_size(Vec2::new(5.0 * 10.0, 10.0), Sense::hover());
    for i in 0..5 {
        let c = if i < green {
            pal.success_emphasis
        } else if i < green + red {
            pal.danger_emphasis
        } else {
            pal.neutral_muted
        };
        let r = egui::Rect::from_min_size(rect.min + Vec2::new(i as f32 * 10.0, 1.0), Vec2::splat(8.0));
        ui.painter().rect_filled(r, CornerRadius::same(2), c);
    }
}

/// One issue / PR row the way the issues index renders it.
pub fn issue_row(ui: &mut Ui, it: &Issue, show_repo: bool, cx: &mut Cx) {
    let pal = w::p(ui);
    let resp = w::list_row(ui, |ui| {
        ui.horizontal_top(|ui| {
            let (ic, color) = it.item_state().list_icon(pal, it.is_pr());
            ui.add_space(0.0);
            icon(ui, ic, color);
            ui.vertical(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if show_repo {
                        w::muted(ui, it.repo_full_name());
                    }
                    let t = w::title_link(ui, &it.title, 16.0);
                    if t.clicked() {
                        open_issue(cx, it);
                    }
                    for l in &it.labels {
                        w::issue_label(ui, &l.name, &l.color);
                    }
                });
                let mut meta = format!("#{} ", it.number);
                if let Some(t) = &it.created_at {
                    if it.state == "open" {
                        meta.push_str(&format!("opened {} by {}", w::rel_time(t), it.user.login));
                    } else if let Some(c) = &it.closed_at {
                        meta.push_str(&format!("by {} was closed {}", it.user.login, w::rel_time(c)));
                    }
                }
                ui.horizontal(|ui| {
                    w::muted_small(ui, meta);
                    if let Some(m) = &it.milestone {
                        w::meta(ui, Icon::Milestone, RichText::new(&m.title).size(12.0).color(pal.fg_muted));
                    }
                });
            });
            w::right(ui, |ui| {
                if it.comments > 0 {
                    w::meta(ui, Icon::Comment, RichText::new(it.comments.to_string()).size(12.0).color(pal.fg_muted));
                }
                for a in it.assignees.iter().take(3) {
                    w::avatar(ui, &a.avatar_url, 20.0).on_hover_text(&a.login);
                }
            });
        });
    });
    if resp.clicked() {
        open_issue(cx, it);
    }
    w::divider(ui);
}

fn open_issue(cx: &mut Cx, it: &Issue) {
    let slug = it.repo_full_name();
    if let Some((o, n)) = slug.split_once('/') {
        let view = if it.is_pr() { crate::route::RepoView::Pull(it.number) } else { crate::route::RepoView::Issue(it.number) };
        cx.go(Route::repo(o, n, view));
    }
}

/// A repository row like on profile / search pages.
pub fn repo_row(ui: &mut Ui, r: &Repo, cx: &mut Cx) {
    let pal = w::p(ui);
    let resp = w::list_row(ui, |ui| {
        ui.horizontal(|ui| {
            if w::link_strong(ui, &r.full_name, 16.0).clicked() {
                cx.go(Route::repo_home(&r.owner.login, &r.name));
            }
            w::outline_label(ui, if r.private { "Private" } else { "Public" }, None);
            if r.archived {
                w::outline_label(ui, "Archived", Some(pal.attention));
            }
        });
        if let Some(d) = &r.description {
            ui.add(egui::Label::new(RichText::new(d).color(pal.fg_muted)).wrap());
        }
        if !r.topics.is_empty() {
            ui.horizontal_wrapped(|ui| {
                for t in r.topics.iter().take(8) {
                    topic(ui, t);
                }
            });
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            if let Some(l) = &r.language {
                w::language_dot(ui, l);
            }
            if r.stargazers_count > 0 {
                w::meta(ui, Icon::Star, RichText::new(w::format_count(r.stargazers_count)).size(12.0).color(pal.fg_muted));
            }
            if r.forks_count > 0 {
                w::meta(ui, Icon::RepoForked, RichText::new(w::format_count(r.forks_count)).size(12.0).color(pal.fg_muted));
            }
            if let Some(t) = &r.pushed_at.or(r.updated_at) {
                w::muted_small(ui, format!("Updated {}", w::rel_time(t)));
            }
        });
    });
    if resp.clicked() {
        cx.go(Route::repo_home(&r.owner.login, &r.name));
    }
    w::divider(ui);
}

pub fn topic(ui: &mut Ui, t: &str) {
    let pal = w::p(ui);
    w::pill(
        ui,
        w::Pill {
            text: t,
            font: semibold(12.0),
            fg: pal.accent,
            fill: pal.accent_subtle,
            stroke: Color32::TRANSPARENT,
            pad_x: 10.0,
            height: 22.0,
            radius: 255,
        },
    );
}

/// Previous / Next pager, GitHub style.
pub fn pager(ui: &mut Ui, page: &mut u32, has_next: bool) -> bool {
    let mut changed = false;
    ui.add_space(16.0);
    ui.vertical_centered(|ui| {
        ui.horizontal(|ui| {
            let total_w = 220.0;
            ui.add_space((ui.available_width() - total_w).max(0.0) / 2.0);
            if w::button_ex(ui, Variant::Invisible, Some(Icon::ChevronLeft), "Previous", None, *page > 1).clicked() {
                *page -= 1;
                changed = true;
            }
            ui.label(RichText::new(page.to_string()).font(semibold(14.0)));
            if w::button_ex(ui, Variant::Invisible, None, "Next", Some(Icon::ChevronRight), has_next).clicked() {
                *page += 1;
                changed = true;
            }
        });
    });
    changed
}

/// Sidebar section in issue/PR pages: heading with a gear that opens a picker.
pub fn sidebar_section(ui: &mut Ui, title: &str, editable: bool, add_body: impl FnOnce(&mut Ui), picker: impl FnOnce(&mut Ui)) {
    let pal = w::p(ui);
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).font(semibold(12.0)).color(pal.fg_muted));
        if editable {
            w::right(ui, |ui| {
                let g = w::icon_button_invisible(ui, Icon::Gear, &format!("Edit {}", title.to_lowercase()));
                egui::Popup::menu(&g)
                    .width(280.0)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .align(egui::RectAlign::BOTTOM_END)
                    .show(picker);
            });
        }
    });
    add_body(ui);
    ui.add_space(8.0);
    w::divider(ui);
    ui.add_space(8.0);
}

/// Multi-select filterable list for labels in a popup. Returns true when changed.
pub fn label_picker(ui: &mut Ui, all: &[Label], selected: &mut Vec<String>, filter: &mut String) -> bool {
    let mut changed = false;
    ui.label(RichText::new("Apply labels").font(semibold(12.0)));
    w::text_input(ui, filter, "Filter labels", ui.available_width());
    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        for l in all.iter().filter(|l| filter.is_empty() || l.name.to_lowercase().contains(&filter.to_lowercase())) {
            let on = selected.contains(&l.name);
            let resp = ui
                .horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                    if on {
                        Icon::Check.image(16.0, w::p(ui).fg).paint_at(ui, r);
                    }
                    let (dot, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                    ui.painter().circle_filled(dot.center(), 7.0, w::parse_hex(&l.color).unwrap_or(Color32::GRAY));
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&l.name).font(semibold(14.0)));
                        if let Some(d) = l.description.as_ref().filter(|d| !d.is_empty()) {
                            w::muted_small(ui, d);
                        }
                    });
                })
                .response;
            if ui.interact(resp.rect, ui.scope_id().with(("lbl", &l.name)), Sense::click()).clicked() {
                if on {
                    selected.retain(|x| x != &l.name);
                } else {
                    selected.push(l.name.clone());
                }
                changed = true;
            }
        }
    });
    changed
}

pub fn user_picker(ui: &mut Ui, title: &str, all: &[User], selected: &mut Vec<String>, filter: &mut String, max: usize) -> bool {
    let mut changed = false;
    ui.label(RichText::new(title).font(semibold(12.0)));
    w::text_input(ui, filter, "Type or choose a user", ui.available_width());
    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        for u in all.iter().filter(|u| filter.is_empty() || u.login.to_lowercase().contains(&filter.to_lowercase())) {
            let on = selected.contains(&u.login);
            let resp = ui
                .horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                    if on {
                        Icon::Check.image(16.0, w::p(ui).fg).paint_at(ui, r);
                    }
                    w::avatar(ui, &u.avatar_url, 20.0);
                    ui.label(RichText::new(&u.login).font(semibold(14.0)));
                })
                .response;
            if ui.interact(resp.rect, ui.scope_id().with(("usr", &u.login)), Sense::click()).clicked() {
                if on {
                    selected.retain(|x| x != &u.login);
                } else if selected.len() < max {
                    selected.push(u.login.clone());
                }
                changed = true;
            }
        }
    });
    changed
}

/// Status icon for checks and workflow runs.
pub fn run_status_icon(ui: &mut Ui, status: &str, conclusion: Option<&str>) -> egui::Response {
    let pal = w::p(ui);
    let (ic, color) = run_status(pal, status, conclusion);
    icon(ui, ic, color)
}

pub fn run_status(pal: &crate::theme::Palette, status: &str, conclusion: Option<&str>) -> (Icon, Color32) {
    match (status, conclusion) {
        ("completed", Some("success")) => (Icon::CheckCircleFill, pal.success),
        ("completed", Some("failure")) | ("completed", Some("timed_out")) | ("completed", Some("startup_failure")) => {
            (Icon::XCircleFill, pal.danger)
        }
        ("completed", Some("cancelled")) => (Icon::Stop, pal.fg_muted),
        ("completed", Some("skipped")) | ("completed", Some("neutral")) => (Icon::Skip, pal.fg_muted),
        ("completed", Some("action_required")) => (Icon::Alert, pal.attention),
        ("in_progress", _) => (Icon::DotFill, pal.attention),
        ("queued", _) | ("waiting", _) | ("pending", _) | ("requested", _) => (Icon::Clock, pal.attention),
        _ => (Icon::Circle, pal.fg_muted),
    }
}

/// Repository-scoped filter bar, "is:open is:issue …" text and a Filters dropdown.
pub fn filter_bar(ui: &mut Ui, q: &mut String, presets: &[(&str, &str)]) -> bool {
    let mut submit = false;
    ui.horizontal(|ui| {
        let filters = w::button(ui, Variant::Default, None, "Filters");
        egui::Popup::menu(&filters).width(260.0).show(|ui| {
            for (label, value) in presets {
                if w::menu_item(ui, None, label).clicked() {
                    *q = value.to_string();
                    submit = true;
                }
            }
        });
        let r = w::text_input_ex(ui, q, "Search all issues", (ui.available_width() - 8.0).max(200.0), Some(Icon::Search));
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            submit = true;
        }
    });
    submit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mentions_become_links_outside_code() {
        let s = expand_gh_shortcuts("hi @octocat and `@notme`\n```\n@fenced\n```\n");
        assert!(s.contains("[@octocat](https://github.com/octocat)"));
        assert!(s.contains("`@notme`"));
        assert!(s.contains("\n@fenced\n"));
    }

    #[test]
    fn email_is_not_a_mention() {
        assert_eq!(expand_gh_shortcuts("mail me@x.com"), "mail me@x.com");
    }

    #[test]
    fn hunk_header() {
        assert_eq!(parse_hunk("@@ -12,7 +14,9 @@ fn x()"), Some((12, 14)));
        assert_eq!(parse_hunk("@@ -1 +1 @@"), Some((1, 1)));
    }
}
