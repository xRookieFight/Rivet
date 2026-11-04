# Contributing to Rivet

Thanks for wanting to help. Rivet is a native GitHub client, so most changes are about making a screen look and behave like github.com.

## Ground rules

* **GitHub is the reference.** When a screen differs from github.com, GitHub is right. Check the real page before changing layout, labels, section order or colors.
* **No fake functionality.** If something has no API or isn't built yet, leave it out or say clearly that it isn't available. No buttons that do nothing, no placeholder data, no embedded web views.
* **Tokens stay in the OS keychain.** Never log them, write them to disk, or put them in screenshots or test fixtures.

[AGENTS.md](AGENTS.md) has the full list of conventions, the code layout and the egui pitfalls we keep running into. It's written for AI agents, but it's the best map of the codebase for humans too.

## Getting started

You need a recent stable Rust toolchain. On Linux you also need the Wayland or X11 libraries and a Secret Service provider (GNOME Keyring, KWallet).

```sh
git clone https://github.com/xRookieFight/Rivet.git
cd Rivet
cargo run
```

Before opening a pull request, run:

```sh
cargo fmt
cargo build
cargo test
cargo clippy
```

The build should stay free of warnings.

### Looking at a screen without clicking around

Debug builds can sign in, open a route and save a screenshot on their own, which is the quickest way to check a UI change in both themes:

```sh
RIVET_DEV_TOKEN=$(gh auth token) RIVET_DEV_ROUTE=owner/name RIVET_DEV_THEME=dark RIVET_DEV_SHOT=shot.png cargo run
```

See the Development section of the [README](README.md#development) for every variable.

### Android

The Android app lives in `android/`: a small Rust crate that builds the shared library, plus a Gradle project around it. You need the Android SDK, NDK 28.2.13676358, JDK 17 or newer, and the Rust targets:

```sh
rustup target add aarch64-linux-android x86_64-linux-android
cd android
./gradlew assembleDebug                      # both ABIs
./gradlew assembleDebug -Privet.abis=x86_64  # just the emulator ABI, faster
```

Gradle runs cargo for you. Local git isn't available on Android, and native file dialogs aren't either, so code behind `platform::LOCAL_GIT` and `platform::NATIVE_FILES` has to keep working when those are off.

## Pull requests

* Keep each pull request to one purpose. Unrelated formatting changes make review and blame harder.
* For UI changes, attach screenshots in light and dark themes, ideally next to the same page on github.com.
* Commit titles follow [Conventional Commits](https://www.conventionalcommits.org/) without a scope, all lowercase, title only: `fix: handle empty chunk palette`.
* Comments explain why, not what. Describe the current behavior, never the history of a change.

## Reporting bugs and ideas

Use the [issue templates](https://github.com/xRookieFight/Rivet/issues/new/choose). Security problems go through [private reporting](SECURITY.md) instead.

## License

By contributing, you agree that your contributions are dual licensed under the [MIT](LICENSE-MIT) and [Apache 2.0](LICENSE-APACHE) licenses, without any additional terms or conditions.
