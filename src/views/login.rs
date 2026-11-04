//! "Sign in to GitHub" screen: OAuth device flow, personal access token, or the
//! token the GitHub CLI already holds.

use crate::auth::{self, Account, DeviceCode};
use crate::github::Client;
use crate::github::models::User;
use crate::icons::Icon;
use crate::task::Load;
use crate::theme::semibold;
use crate::widgets::{self as w, FlashKind, Variant};
use egui::{CornerRadius, Frame, Margin, RichText, Stroke, Ui};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub enum LoginOutcome {
    SignedIn(User, String),
    UseSaved(Account),
}

#[derive(Default)]
pub struct LoginPage {
    token: String,
    verify: Load<(User, String)>,
    device: Load<DeviceCode>,
    device_token: Load<String>,
    cancel: Option<Arc<AtomicBool>>,
    error: Option<String>,
}

fn verify(ctx: &egui::Context, token: String) -> Load<(User, String)> {
    Load::spawn(ctx, move || {
        let user = Client::new(token.clone()).me()?;
        Ok((user, token))
    })
}

impl LoginPage {
    pub fn ui(&mut self, ui: &mut Ui, saved: &[Account]) -> Option<LoginOutcome> {
        let pal = w::p(ui);
        let ctx = ui.ctx().clone();
        let mut out = None;

        match self.verify.take() {
            Some(Ok((u, t))) => return Some(LoginOutcome::SignedIn(u, t)),
            Some(Err(e)) => self.error = Some(e),
            None => {}
        }
        // device flow step 1 done: start polling in the background
        if let Load::Ready(code) = self.device.poll()
            && self.device_token.is_idle()
            && self.cancel.is_none()
        {
            let code = code.clone();
            let cancel = Arc::new(AtomicBool::new(false));
            self.cancel = Some(cancel.clone());
            let id = auth::client_id();
            // on a phone the browser covers the app, so the code has to be seen first
            if !cfg!(target_os = "android") {
                ctx.open_url(egui::OpenUrl::new_tab(&code.verification_uri));
            }
            self.device_token = Load::spawn(&ctx, move || auth::poll_device_token(&id, &code, &cancel));
        }
        if let Load::Failed(e) = self.device.poll() {
            self.error = Some(e.clone());
            self.device = Load::Idle;
        }
        match self.device_token.take() {
            Some(Ok(tok)) => {
                self.device = Load::Idle;
                self.cancel = None;
                self.verify = verify(&ctx, tok);
            }
            Some(Err(e)) => {
                self.device = Load::Idle;
                self.cancel = None;
                if e != "cancelled" {
                    self.error = Some(e);
                }
            }
            None => {}
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.add(Icon::MarkGithub.image(48.0, pal.fg));
                ui.add_space(16.0);
                ui.label(RichText::new("Sign in to GitHub").size(24.0).color(pal.fg));
                ui.add_space(4.0);
                w::muted(ui, "Rivet talks to GitHub with your account. Tokens are kept in your OS keychain.");
                ui.add_space(16.0);

                let width = 340.0;
                ui.allocate_ui_with_layout(egui::vec2(width, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_width(width);
                    if let Some(e) = &self.error {
                        w::flash(ui, FlashKind::Danger, e);
                        ui.add_space(16.0);
                    }

                    let busy = self.verify.is_loading();
                    let device_busy = !self.device.is_idle() || self.device_token.is_loading();
                    card(ui, |ui| {
                        if device_busy {
                            self.device_ui(ui);
                            return;
                        }
                        ui.vertical_centered_justified(|ui| {
                            if w::button(ui, Variant::Primary, Some(Icon::MarkGithub), "Sign in with GitHub").clicked() {
                                self.error = None;
                                let id = auth::client_id();
                                self.device = Load::spawn(&ctx, move || auth::request_device_code(&id));
                            }
                        });
                        ui.add_space(12.0);
                        w::divider(ui);
                        ui.add_space(12.0);
                        w::form_label(ui, "Or use a personal access token", false);
                        let r = w::text_input(ui, &mut self.token, "ghp_… or github_pat_…", ui.available_width());
                        let submit = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            w::muted_small(ui, "Needs scopes:");
                            ui.label(RichText::new(auth::SCOPES).monospace().size(11.0).color(pal.fg_muted));
                        });
                        if w::link(ui, "Create a token on github.com").clicked() {
                            ctx.open_url(egui::OpenUrl::new_tab(format!(
                                "https://github.com/settings/tokens/new?description=Rivet&scopes={}",
                                auth::SCOPES.replace(' ', ",")
                            )));
                        }
                        ui.add_space(12.0);
                        ui.vertical_centered_justified(|ui| {
                            let label = if busy { "Signing in…" } else { "Sign in with token" };
                            let can = !busy && !self.token.trim().is_empty();
                            if (w::button_ex(ui, Variant::Default, None, label, None, can).clicked() || submit && can) && can {
                                self.error = None;
                                self.verify = verify(&ctx, self.token.trim().to_string());
                            }
                        });
                    });

                    // no gh cli to borrow a token from on a phone
                    if !cfg!(target_os = "android") {
                        ui.add_space(16.0);
                        card(ui, |ui| {
                            ui.horizontal(|ui| {
                                w::muted(ui, "Already using the GitHub CLI?");
                                if w::link(ui, "Use gh credentials").clicked() {
                                    self.error = None;
                                    match auth::token_from_gh_cli() {
                                        Ok(t) => self.verify = verify(&ctx, t),
                                        Err(e) => self.error = Some(format!("{e:#}")),
                                    }
                                }
                            });
                        });
                    }

                    if !saved.is_empty() {
                        ui.add_space(16.0);
                        card(ui, |ui| {
                            ui.label(RichText::new("Saved accounts").font(semibold(14.0)));
                            ui.add_space(4.0);
                            for acc in saved {
                                ui.horizontal(|ui| {
                                    w::avatar(ui, &acc.avatar_url, 20.0);
                                    if w::link(ui, &acc.login).clicked() {
                                        out = Some(LoginOutcome::UseSaved(acc.clone()));
                                    }
                                });
                            }
                        });
                    }
                });
            });
        });
        out
    }

    fn device_ui(&mut self, ui: &mut Ui) {
        let pal = w::p(ui);
        ui.vertical_centered(|ui| match self.device.poll() {
            Load::Ready(code) => {
                let code = code.clone();
                ui.label(RichText::new("Enter this code on GitHub").font(semibold(16.0)));
                ui.add_space(8.0);
                Frame::new()
                    .fill(pal.canvas_subtle)
                    .stroke(Stroke::new(1.0, pal.border))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(Margin::symmetric(16, 8))
                    .show(ui, |ui| ui.label(RichText::new(&code.user_code).monospace().size(28.0)));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    // no clipboard on android, the code has to be typed over by hand
                    if !cfg!(target_os = "android") && w::button(ui, Variant::Default, Some(Icon::Copy), "Copy code").clicked() {
                        ui.ctx().copy_text(code.user_code.clone());
                    }
                    if w::button(ui, Variant::Default, Some(Icon::LinkExternal), "Open github.com").clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(&code.verification_uri));
                    }
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(14.0));
                    w::muted_small(ui, "Waiting for you to authorize Rivet…");
                });
                ui.add_space(8.0);
                if w::button(ui, Variant::Invisible, None, "Cancel").clicked() {
                    if let Some(c) = &self.cancel {
                        c.store(true, Ordering::Relaxed);
                    }
                    self.device = Load::Idle;
                }
            }
            _ => w::spinner(ui),
        });
    }
}

fn card(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let pal = w::p(ui);
    Frame::new()
        .fill(pal.canvas)
        .stroke(Stroke::new(1.0, pal.border_muted))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        });
}
