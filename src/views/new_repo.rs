//! "Create a new repository" form: owner, name, visibility, README,
//! .gitignore and license templates.

use super::Page;
use crate::app::Cx;
use crate::github::models::{LicenseTemplate, Org, Repo};
use crate::icons::{Icon, icon_sized};
use crate::route::Route;
use crate::task::Load;
use crate::theme::semibold;
use crate::widgets::{self as w, FlashKind, Variant};
use egui::{Frame, Margin, RichText, Ui};
use serde_json::json;

#[derive(Default)]
pub struct NewRepo {
    started: bool,
    owner: Option<String>,
    name: String,
    description: String,
    private: bool,
    readme: bool,
    gitignore: Option<String>,
    license: Option<String>,
    orgs: Load<Vec<Org>>,
    gitignores: Load<Vec<String>>,
    licenses: Load<Vec<LicenseTemplate>>,
    gi_filter: String,
    create: Load<Repo>,
    error: Option<String>,
}

fn name_ok(n: &str) -> bool {
    !n.is_empty() && n.len() <= 100 && n.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) && n != "." && n != ".."
}

impl Page for NewRepo {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) {
        let ctx = ui.ctx().clone();
        if !self.started {
            self.started = true;
            let g = cx.gh.clone();
            self.orgs = Load::spawn(&ctx, move || g.my_orgs());
            let g = cx.gh.clone();
            self.gitignores = Load::spawn(&ctx, move || g.gitignore_templates());
            let g = cx.gh.clone();
            self.licenses = Load::spawn(&ctx, move || g.license_templates());
        }
        match self.create.take() {
            Some(Ok(r)) => {
                cx.ok(format!("Created {}", r.full_name));
                cx.go(Route::repo_home(&r.owner.login, &r.name));
            }
            Some(Err(e)) => self.error = Some(e),
            None => {}
        }
        let pal = w::p(ui);
        let owner = self.owner.clone().unwrap_or_else(|| cx.me.login.clone());

        egui::ScrollArea::vertical().show(ui, |ui| {
            Frame::new().inner_margin(Margin::same(32)).show(ui, |ui| {
                w::page(ui, 680.0, |ui| {
                    w::heading(ui, "Create a new repository", 24.0);
                    ui.add_space(4.0);
                    w::muted(ui, "A repository contains all project files, including the revision history.");
                    ui.add_space(16.0);
                    w::divider(ui);
                    ui.add_space(16.0);
                    if let Some(e) = &self.error {
                        w::flash(ui, FlashKind::Danger, e);
                        ui.add_space(16.0);
                    }
                    w::muted_small(ui, "Required fields are marked with an asterisk (*).");
                    ui.add_space(8.0);
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            w::form_label(ui, "Owner", true);
                            let b = w::button_ex(ui, Variant::Default, None, &owner, Some(Icon::TriangleDown), true);
                            egui::Popup::menu(&b).width(240.0).show(|ui| {
                                if w::menu_item_ex(ui, Some(Icon::Person), &cx.me.login, self.owner.is_none(), false).clicked() {
                                    self.owner = None;
                                }
                                if let Some(orgs) = self.orgs.ready() {
                                    for o in orgs.iter() {
                                        if w::menu_item_ex(
                                            ui,
                                            Some(Icon::Organization),
                                            &o.login,
                                            self.owner.as_deref() == Some(&o.login),
                                            false,
                                        )
                                        .clicked()
                                        {
                                            self.owner = Some(o.login.clone());
                                        }
                                    }
                                }
                            });
                        });
                        ui.vertical(|ui| {
                            ui.add_space(22.0);
                            ui.label(RichText::new("/").size(20.0));
                        });
                        ui.vertical(|ui| {
                            w::form_label(ui, "Repository name", true);
                            w::text_input(ui, &mut self.name, "", 300.0);
                        });
                    });
                    if !self.name.is_empty() && !name_ok(&self.name) {
                        ui.label(RichText::new("Only ASCII letters, digits, '.', '-' and '_' are allowed.").size(12.0).color(pal.danger));
                    } else {
                        w::muted_small(ui, "Great repository names are short and memorable.");
                    }
                    ui.add_space(16.0);
                    w::form_label(ui, "Description (optional)", false);
                    w::text_input(ui, &mut self.description, "", ui.available_width());
                    ui.add_space(16.0);
                    w::divider(ui);
                    ui.add_space(16.0);

                    for (private, ic, title, desc) in [
                        (false, Icon::Repo, "Public", "Anyone on the internet can see this repository. You choose who can commit."),
                        (true, Icon::Lock, "Private", "You choose who can see and commit to this repository."),
                    ] {
                        ui.horizontal_top(|ui| {
                            ui.radio_value(&mut self.private, private, "");
                            icon_sized(ui, ic, 24.0, pal.fg_muted);
                            ui.vertical(|ui| {
                                ui.label(RichText::new(title).font(semibold(14.0)));
                                w::muted_small(ui, desc);
                            });
                        });
                        ui.add_space(8.0);
                    }
                    ui.add_space(8.0);
                    w::divider(ui);
                    ui.add_space(16.0);

                    ui.label(RichText::new("Initialize this repository with:").font(semibold(14.0)));
                    ui.add_space(8.0);
                    ui.checkbox(&mut self.readme, RichText::new("Add a README file").font(semibold(14.0)));
                    w::muted_small(ui, "This is where you can write a long description for your project.");
                    ui.add_space(12.0);

                    ui.label(RichText::new("Add .gitignore").font(semibold(14.0)));
                    let gi_label = format!(".gitignore template: {}", self.gitignore.as_deref().unwrap_or("None"));
                    let b = w::button_ex(ui, Variant::Default, None, &gi_label, Some(Icon::TriangleDown), true);
                    egui::Popup::menu(&b).width(280.0).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                        w::text_input(ui, &mut self.gi_filter, "Filter…", ui.available_width());
                        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                            if w::menu_item_ex(ui, None, "None", self.gitignore.is_none(), false).clicked() {
                                self.gitignore = None;
                                ui.close();
                            }
                            if let Some(list) = self.gitignores.ready() {
                                let f = self.gi_filter.to_lowercase();
                                for t in list.iter().filter(|t| f.is_empty() || t.to_lowercase().contains(&f)) {
                                    if w::menu_item_ex(ui, None, t, self.gitignore.as_deref() == Some(t), false).clicked() {
                                        self.gitignore = Some(t.clone());
                                        ui.close();
                                    }
                                }
                            }
                        });
                    });
                    ui.add_space(12.0);
                    ui.label(RichText::new("Choose a license").font(semibold(14.0)));
                    let lic_name = self
                        .license
                        .as_ref()
                        .and_then(|k| self.licenses.ready().and_then(|l| l.iter().find(|x| &x.key == k).map(|x| x.name.clone())))
                        .unwrap_or_else(|| "None".into());
                    let b = w::button_ex(ui, Variant::Default, None, &format!("License: {lic_name}"), Some(Icon::TriangleDown), true);
                    egui::Popup::menu(&b).width(320.0).show(|ui| {
                        if w::menu_item_ex(ui, None, "None", self.license.is_none(), false).clicked() {
                            self.license = None;
                        }
                        if let Some(list) = self.licenses.ready() {
                            for l in list.iter() {
                                if w::menu_item_ex(ui, Some(Icon::Law), &l.name, self.license.as_deref() == Some(&l.key), false).clicked() {
                                    self.license = Some(l.key.clone());
                                }
                            }
                        }
                    });
                    ui.add_space(16.0);
                    w::divider(ui);
                    ui.add_space(16.0);
                    let vis = if self.private { "private" } else { "public" };
                    w::flash(
                        ui,
                        FlashKind::Info,
                        &format!(
                            "You are creating a {vis} repository in your {} account.",
                            if self.owner.is_some() { "organization" } else { "personal" }
                        ),
                    );
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        w::right(ui, |ui| {
                            let busy = self.create.is_loading();
                            let ok = name_ok(&self.name) && !busy;
                            let label = if busy { "Creating…" } else { "Create repository" };
                            if w::button_ex(ui, Variant::Primary, None, label, None, ok).clicked() {
                                self.error = None;
                                let mut body = json!({
                                    "name": self.name.trim(),
                                    "description": self.description.trim(),
                                    "private": self.private,
                                    "auto_init": self.readme || self.gitignore.is_some() || self.license.is_some(),
                                });
                                if let Some(g) = &self.gitignore {
                                    body["gitignore_template"] = json!(g);
                                }
                                if let Some(l) = &self.license {
                                    body["license_template"] = json!(l);
                                }
                                let (g, org) = (cx.gh.clone(), self.owner.clone());
                                self.create = Load::spawn(&ctx, move || g.create_repo(org.as_deref(), &body));
                            }
                        });
                    });
                });
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::name_ok;

    #[test]
    fn repo_name_rules() {
        assert!(name_ok("rivet"));
        assert!(name_ok("my.repo_name-2"));
        assert!(!name_ok(""));
        assert!(!name_ok("has space"));
        assert!(!name_ok(".."));
    }
}
