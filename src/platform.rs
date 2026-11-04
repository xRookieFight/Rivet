//! The bits that differ between desktop and Android: native file dialogs,
//! handing local files to other apps, and Android's startup wiring.

use std::path::{Path, PathBuf};

// same type winit re-exports as platform::android::activity
#[cfg(target_os = "android")]
pub use android_activity::AndroidApp;

/// Whether we can show native file pickers and open local files in other apps.
/// Android has neither without a Java side, so views hide those buttons there.
pub const NATIVE_FILES: bool = cfg!(not(target_os = "android"));

/// Whether local repositories work. The android build has no OpenSSL, so its
/// libgit2 can't talk https and cloning, fetching and pushing would all fail.
pub const LOCAL_GIT: bool = cfg!(not(target_os = "android"));

#[cfg(not(target_os = "android"))]
pub fn pick_folder(title: &str, start: Option<&Path>) -> Option<PathBuf> {
    let mut d = rfd::FileDialog::new().set_title(title);
    if let Some(start) = start {
        d = d.set_directory(start);
    }
    d.pick_folder()
}

#[cfg(not(target_os = "android"))]
pub fn pick_files(title: &str) -> Option<Vec<PathBuf>> {
    rfd::FileDialog::new().set_title(title).pick_files()
}

#[cfg(not(target_os = "android"))]
pub fn save_file(name: &str) -> Option<PathBuf> {
    rfd::FileDialog::new().set_file_name(name).save_file()
}

/// Opens a file or folder with whatever the OS would use for it.
#[cfg(not(target_os = "android"))]
pub fn open_path(path: &Path) {
    let _ = open::that(path);
}

#[cfg(target_os = "android")]
pub fn pick_folder(_title: &str, _start: Option<&Path>) -> Option<PathBuf> {
    None
}

#[cfg(target_os = "android")]
pub fn pick_files(_title: &str) -> Option<Vec<PathBuf>> {
    None
}

#[cfg(target_os = "android")]
pub fn save_file(_name: &str) -> Option<PathBuf> {
    None
}

#[cfg(target_os = "android")]
pub fn open_path(_path: &Path) {}

/// Wires up the things desktop gets for free: a home dir, the keystore backed
/// token store and the TLS verifier. Runs before eframe starts any threads.
#[cfg(target_os = "android")]
pub fn init_android(app: &AndroidApp) {
    if let Some(dir) = app.internal_data_path() {
        // yeah, android has no HOME. pointing it at the app's private dir makes
        // eframe's storage, `directories` and libgit2's config lookup just work
        unsafe { std::env::set_var("HOME", dir) };
    }
    match android_native_keyring_store::Store::new() {
        Ok(store) => keyring_core::set_default_store(store),
        Err(e) => eprintln!("rivet: no keystore, tokens can't be saved: {e}"),
    }
    if let Err(e) = init_tls() {
        eprintln!("rivet: tls verifier init failed: {e}");
    }
}

#[cfg(target_os = "android")]
fn init_tls() -> Result<(), jni::errors::Error> {
    // hmm, android-activity already filled ndk-context with the vm and the activity
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) };
    vm.attach_current_thread(|env| {
        let activity = unsafe { jni::objects::JObject::from_raw(env, ctx.context().cast()) };
        rustls_platform_verifier::android::init_with_env(env, activity)
    })
}

/// Soft keyboard bridge for Android. GameActivity hands typed text to its own
/// edit buffer instead of sending key events, and winit 0.30 drops those, so we
/// drain the buffer into egui text events ourselves. A no-op on desktop.
#[derive(Default)]
pub struct InputBridge {
    #[cfg(target_os = "android")]
    app: Option<AndroidApp>,
}

impl InputBridge {
    #[cfg(target_os = "android")]
    pub fn new(app: AndroidApp) -> Self {
        use android_activity::input::{ImeOptions, InputType, TextInputAction};
        // hmm, with suggestions on the IME keeps a composing word in the buffer
        // and we'd fight it every frame. visible password turns that off on gboard
        app.set_ime_editor_info(
            InputType::TYPE_CLASS_TEXT | InputType::TYPE_TEXT_FLAG_NO_SUGGESTIONS | InputType::TYPE_TEXT_VARIATION_VISIBLE_PASSWORD,
            TextInputAction::Done,
            ImeOptions::empty(),
        );
        Self { app: Some(app) }
    }

    #[cfg_attr(not(target_os = "android"), allow(unused_variables))]
    pub fn feed(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        #[cfg(target_os = "android")]
        if let Some(app) = &self.app {
            if ctx.egui_wants_keyboard_input() {
                // winit doesn't wake us for the buffer changing, so poll while a field has focus
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
            let state = app.text_input_state();
            if !state.text.is_empty() {
                raw.events.push(egui::Event::Text(state.text));
                app.set_text_input_state(Default::default());
            }
        }
    }
}
