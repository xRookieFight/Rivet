//! Debug-build hooks for checking screens without clicking through them.
//!
//! RIVET_DEV_TOKEN   sign in with this token for the session only (nothing hits the keychain)
//! RIVET_DEV_ROUTE   start on a github.com url, "owner/name", local:<path>, or dashboard / notifications / settings / local
//! RIVET_DEV_THEME   light or dark
//! RIVET_DEV_SHOT    save the window to this PNG once things settle, then quit
//! RIVET_DEV_SCROLL  scroll the page under the window center down by this many points first

use crate::route::Route;
use egui::Context;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct Dev {
    pub token: Option<String>,
    pub route: Option<Route>,
    pub theme: Option<crate::theme::ThemePref>,
    shot: Option<String>,
    scroll: f32,
    scrolled: bool,
    started: Instant,
    requested: bool,
    done: Arc<Mutex<bool>>,
}

impl Dev {
    pub fn from_env() -> Option<Self> {
        if !cfg!(debug_assertions) {
            return None;
        }
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        let token = var("RIVET_DEV_TOKEN");
        let shot = var("RIVET_DEV_SHOT");
        let route = var("RIVET_DEV_ROUTE").and_then(|r| match r.as_str() {
            "dashboard" => Some(Route::Dashboard),
            "feed" => Some(Route::Feed),
            "notifications" => Some(Route::Notifications),
            "settings" => Some(Route::Settings),
            "local" => Some(Route::Local(None)),
            "new" => Some(Route::NewRepo),
            other if other.starts_with("local:") => Some(Route::Local(Some(other.trim_start_matches("local:").into()))),
            other => Route::from_github_url(other).or_else(|| Route::from_slug(other)),
        });
        let theme =
            var("RIVET_DEV_THEME").map(|t| if t == "dark" { crate::theme::ThemePref::Dark } else { crate::theme::ThemePref::Light });
        if token.is_none() && shot.is_none() && route.is_none() && theme.is_none() {
            return None;
        }
        let scroll = var("RIVET_DEV_SCROLL").and_then(|v| v.parse().ok()).unwrap_or(0.0);
        Some(Self {
            token,
            route,
            theme,
            shot,
            scroll,
            scrolled: false,
            started: Instant::now(),
            requested: false,
            done: Arc::new(Mutex::new(false)),
        })
    }

    /// Feeds a fake wheel scroll once pages had time to load, so screenshots can
    /// show content below the fold.
    pub fn input(&mut self, raw: &mut egui::RawInput) {
        if self.scrolled || self.scroll == 0.0 || self.started.elapsed() < Duration::from_secs(5) {
            return;
        }
        self.scrolled = true;
        let center = raw.screen_rect.map(|r| r.center()).unwrap_or(egui::pos2(600.0, 400.0));
        raw.events.push(egui::Event::PointerMoved(center));
        raw.events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -self.scroll),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
            modifiers: egui::Modifiers::NONE,
        });
    }

    /// Called every frame; takes the screenshot after a fixed delay so network
    /// requests and images have had time to land.
    pub fn tick(&mut self, ctx: &Context) {
        let Some(path) = self.shot.clone() else { return };
        ctx.request_repaint_after(Duration::from_millis(200));
        if *self.done.lock().unwrap() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.requested || self.started.elapsed() < Duration::from_secs(8) {
            return;
        }
        self.requested = true;
        let done = self.done.clone();
        let ctx2 = ctx.clone();
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |img| {
            let [w, h] = img.size;
            let mut buf = Vec::with_capacity(w * h * 4);
            for p in &img.pixels {
                buf.extend_from_slice(&p.to_srgba_unmultiplied());
            }
            match image::RgbaImage::from_raw(w as u32, h as u32, buf).map(|i| i.save(&path)) {
                Some(Ok(())) => eprintln!("rivet: saved screenshot to {path}"),
                Some(Err(e)) => eprintln!("rivet: screenshot failed: {e}"),
                None => eprintln!("rivet: screenshot had a bad size"),
            }
            *done.lock().unwrap() = true;
            ctx2.request_repaint();
        })));
    }
}
