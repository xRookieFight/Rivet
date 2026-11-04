//! The shared library GameActivity loads. All the real work lives in the
//! `rivet` crate; this only exists because the desktop build shouldn't pay
//! for a cdylib.

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: rivet::AndroidApp) {
    rivet::run_android(app);
}
