//! App preferences: theme, accounts, clone location and shortcuts.

use super::Page;
use crate::app::Cx;
use crate::auth;
use crate::icons::{Icon, icon};
use crate::theme::{self, ThemePref, semibold};
use crate::widgets::{self as w, Variant};
use egui::{CornerRadius, Frame, Margin, RichText, Stroke, Ui};

#[derive(Default)]
pub struct SettingsPage {
    section: usize,
}

const SECTIONS: [(Icon, &str); 4] = [
    (Icon::Paintbrush, "Preferences"),
    (Icon::People, "Accounts"),
    (Icon::FileDirectory, "Repositories"),
    (Icon::CommandPalette, "Keyboard shortcuts"),
];

impl Page for SettingsPage {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        let pal = w::p(ui);
        egui::ScrollArea::vertical().show(ui, |ui| {
            Frame::new().inner_margin(Margin::same(32)).show(ui, |ui| {
                w::page(ui, 1012.0, |ui| {
                    ui.horizontal(|ui| {
                        w::avatar(ui, &cx.me.avatar_url, 48.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(cx.me.name.clone().unwrap_or_else(|| cx.me.login.clone())).font(semibold(20.0)));
                            w::muted(ui, "Rivet settings");
                        });
                    });
                    ui.add_space(24.0);
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(240.0);
                            for (i, (ic, label)) in SECTIONS.iter().enumerate() {
                                // clone location and recent clones mean nothing without local git
                                if i == 2 && !crate::platform::LOCAL_GIT {
                                    continue;
                                }
                                let r = w::menu_item(ui, Some(*ic), label);
                                if i == self.section {
                                    let bar = egui::Rect::from_min_size(r.rect.left_top() + egui::vec2(-8.0, 6.0), egui::vec2(4.0, 20.0));
                                    ui.painter().rect_filled(bar, CornerRadius::same(2), pal.accent_emphasis);
                                }
                                if r.clicked() {
                                    self.section = i;
                                }
                            }
                        });
                        ui.add_space(24.0);
                        ui.vertical(|ui| match self.section {
                            0 => appearance(ui, cx),
                            1 => accounts(ui, cx),
                            2 => repositories(ui, cx),
                            _ => shortcuts(ui),
                        });
                    });
                });
            });
        });
    }
}

fn section_title(ui: &mut Ui, t: &str) {
    ui.label(RichText::new(t).size(24.0));
    ui.add_space(8.0);
    w::divider(ui);
    ui.add_space(16.0);
}

fn appearance(ui: &mut Ui, cx: &mut Cx) {
    let pal = w::p(ui);
    section_title(ui, "Theme preferences");
    w::muted(ui, "Choose how Rivet looks to you. Select a single theme, or sync with your system.");
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        for (t, label, preview) in [
            (ThemePref::System, "Sync with system", None),
            (ThemePref::Light, "Light default", Some(&theme::LIGHT)),
            (ThemePref::Dark, "Dark default", Some(&theme::DARK)),
        ] {
            let selected = cx.prefs.theme == t;
            let r = Frame::new()
                .stroke(Stroke::new(if selected { 2.0 } else { 1.0 }, if selected { pal.accent_emphasis } else { pal.border }))
                .corner_radius(CornerRadius::same(6))
                .show(ui, |ui| {
                    ui.set_width(200.0);
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(200.0, 100.0), egui::Sense::hover());
                    let p = ui.painter();
                    let draw = |p: &egui::Painter, r: egui::Rect, pl: &theme::Palette| {
                        p.rect_filled(r, CornerRadius::same(4), pl.canvas);
                        let header = egui::Rect::from_min_size(r.min, egui::vec2(r.width(), 18.0));
                        p.rect_filled(header, CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 }, pl.header_bg);
                        for i in 0..3 {
                            let y = r.top() + 30.0 + i as f32 * 16.0;
                            p.rect_filled(
                                egui::Rect::from_min_size(egui::pos2(r.left() + 12.0, y), egui::vec2(r.width() * 0.6, 8.0)),
                                CornerRadius::same(4),
                                pl.neutral_muted,
                            );
                        }
                        p.rect_filled(
                            egui::Rect::from_min_size(egui::pos2(r.right() - 50.0, r.bottom() - 22.0), egui::vec2(40.0, 14.0)),
                            CornerRadius::same(4),
                            pl.success_emphasis,
                        );
                    };
                    let inner = rect.shrink(8.0);
                    match preview {
                        Some(pl) => draw(p, inner, pl),
                        None => {
                            let (l, r) = inner.split_left_right_at_fraction(0.5);
                            draw(p, l, &theme::LIGHT);
                            draw(p, r, &theme::DARK);
                        }
                    }
                    w::divider(ui);
                    Frame::new().inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let mut sel = selected;
                            ui.radio_value(&mut sel, true, "");
                            ui.label(RichText::new(label).font(semibold(14.0)));
                        });
                    });
                })
                .response;
            if ui.interact(r.rect, ui.scope_id().with(("theme", label)), egui::Sense::click()).clicked() {
                cx.prefs.theme = t;
                theme::apply_pref(ui.ctx(), t);
            }
        }
    });
}

fn accounts(ui: &mut Ui, cx: &mut Cx) {
    let pal = w::p(ui);
    section_title(ui, "Accounts");
    w::muted(ui, "Tokens are stored in your operating system keychain. Switch accounts from the avatar menu in the header.");
    ui.add_space(16.0);
    let mut remove = None;
    w::bordered(ui, |ui| {
        for acc in cx.prefs.accounts.clone() {
            w::list_row(ui, |ui| {
                ui.horizontal(|ui| {
                    w::avatar(ui, &acc.avatar_url, 32.0);
                    ui.label(RichText::new(&acc.login).font(semibold(14.0)));
                    if acc.login == cx.me.login {
                        w::outline_label(ui, "Active", Some(pal.success));
                    }
                    w::right(ui, |ui| {
                        if acc.login != cx.me.login && w::button(ui, Variant::Danger, None, "Remove").clicked() {
                            remove = Some(acc.login.clone());
                        }
                    });
                });
            });
            w::divider(ui);
        }
    });
    if let Some(login) = remove {
        match auth::forget_token(&login) {
            Ok(()) => {
                cx.prefs.accounts.retain(|a| a.login != login);
                cx.ok(format!("Removed {login}"));
            }
            Err(e) => cx.err(format!("Could not remove the token: {e}")),
        }
    }
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        icon(ui, Icon::ShieldCheck, pal.fg_muted);
        w::muted_small(ui, format!("Requested scopes: {}", auth::SCOPES));
    });
}

fn repositories(ui: &mut Ui, cx: &mut Cx) {
    section_title(ui, "Repositories");
    w::form_label(ui, "Default clone location", false);
    ui.horizontal(|ui| {
        let current = cx.prefs.clone_dir.clone().or_else(default_clone_dir).map(|p| p.display().to_string()).unwrap_or_default();
        ui.label(RichText::new(current).monospace());
        if crate::platform::NATIVE_FILES
            && w::button(ui, Variant::Default, Some(Icon::FileDirectory), "Choose…").clicked()
            && let Some(dir) = crate::platform::pick_folder("Default clone location", None)
        {
            cx.prefs.clone_dir = Some(dir);
        }
    });
    ui.add_space(16.0);
    w::form_label(ui, "Recent local repositories", false);
    if cx.prefs.recent_local.is_empty() {
        w::muted(ui, "None yet.");
    } else if w::button(ui, Variant::Default, Some(Icon::Trash), "Clear list").clicked() {
        cx.prefs.recent_local.clear();
    }
}

pub fn default_clone_dir() -> Option<std::path::PathBuf> {
    directories::UserDirs::new().map(|d| d.home_dir().join("Rivet"))
}

fn shortcuts(ui: &mut Ui) {
    section_title(ui, "Keyboard shortcuts");
    let rows = [
        ("Ctrl/⌘ K", "Open the command palette"),
        ("Ctrl/⌘ Shift P", "Open the command palette"),
        ("/", "Focus the search bar"),
        ("Alt ←", "Go back"),
        ("Alt →", "Go forward"),
        ("Ctrl/⌘ R, F5", "Reload the current page"),
        ("g d", "Go to the dashboard"),
        ("g n", "Go to notifications"),
        ("g i", "Issues (repository or yours)"),
        ("g p", "Pull requests (repository or yours)"),
        ("g c", "Repository code"),
        ("g a", "Repository actions"),
    ];
    w::bordered(ui, |ui| {
        for (k, d) in rows {
            w::list_row(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(d);
                    w::right(ui, |ui| super::dashboard::kbd(ui, k));
                });
            });
            w::divider(ui);
        }
    });
}
