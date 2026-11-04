//! Rivet as a library, so the desktop binary and the Android shared library
//! (the `android` crate) can share one app.

mod app;
mod auth;
mod devtools;
mod github;
mod gitops;
mod icons;
mod platform;
mod route;
mod task;
mod templates;
mod theme;
mod titlebar;
mod views;
mod widgets;

#[cfg(target_os = "android")]
pub use platform::AndroidApp;

fn viewport() -> egui::ViewportBuilder {
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/rivet.png")).ok();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Rivet")
        .with_app_id("dev.rivet.Rivet")
        .with_inner_size([1360.0, 860.0])
        .with_min_inner_size([760.0, 520.0]);
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    titlebar::configure(viewport)
}

fn launch(options: eframe::NativeOptions, input_bridge: platform::InputBridge) -> eframe::Result<()> {
    eframe::run_native("Rivet", options, Box::new(|cc| Ok(Box::new(app::RivetApp::new(cc, input_bridge)))))
}

/// Opens the desktop window and blocks until it closes.
pub fn run() -> eframe::Result<()> {
    launch(eframe::NativeOptions { viewport: viewport(), ..Default::default() }, platform::InputBridge::default())
}

/// Entry point for the Android shared library. Call it from `android_main`.
#[cfg(target_os = "android")]
pub fn run_android(app: platform::AndroidApp) {
    platform::init_android(&app);
    let input_bridge = platform::InputBridge::new(app.clone());
    let options = eframe::NativeOptions { viewport: viewport(), android_app: Some(app), ..Default::default() };
    if let Err(e) = launch(options, input_bridge) {
        eprintln!("rivet: {e}");
    }
}
