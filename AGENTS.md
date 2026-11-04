# AGENTS.md

Guidance for AI agents and contributors working on Rivet, a native GitHub desktop client written in Rust with egui.

## Goal

Rivet should look and behave like github.com. When a screen differs from GitHub, GitHub is right. Check the real page (or a screenshot of it) before changing layout, labels, order of sections or colors, and don't invent a different visual identity.

Never ship fake functionality. If something has no API or isn't built yet, leave it out or say clearly that it isn't available. No buttons that do nothing, no placeholder data, no embedded web views.

## Commands

```sh
cargo build
cargo test
cargo clippy
cargo fmt
```

Run `cargo build` and `cargo test` before calling any change done. Keep the build free of warnings.

The Android app is built with Gradle, which runs cargo for the Android targets (needs the SDK, NDK 28.2.13676358 and `rustup target add aarch64-linux-android x86_64-linux-android`):

```sh
cd android && ./gradlew assembleDebug -Privet.abis=x86_64
```

To look at a screen without clicking through the app, debug builds read these variables (see `src/devtools.rs`):

```sh
RIVET_DEV_TOKEN=$(gh auth token) RIVET_DEV_ROUTE=owner/name RIVET_DEV_THEME=dark RIVET_DEV_SHOT=shot.png cargo run
```

`RIVET_DEV_ROUTE` also takes github.com URLs, `local:<path>`, `dashboard`, `feed`, `notifications`, `settings`, `local` and `new`. `RIVET_DEV_SCROLL=<points>` scrolls before the screenshot. Check UI changes this way in both themes.

## Layout

| Path | What lives there |
| --- | --- |
| `src/lib.rs` | Window setup and the entry points for desktop and Android |
| `src/main.rs` | Desktop binary, just calls `rivet::run` |
| `src/platform.rs` | Desktop and Android differences: file dialogs, local git availability, Android startup, soft keyboard bridge |
| `src/app.rs` | Session, global header, navigation history, command palette, toasts, preferences |
| `src/route.rs` | Every screen is a `Route`; github.com URLs map to routes here |
| `src/theme.rs` | Primer color tokens (light and dark), fonts, egui style |
| `src/widgets.rs` | Primer components: buttons, labels, pills, UnderlineNav, inputs, flash, blankslate |
| `src/icons.rs` | Octicons enum, generated from `assets/octicons/*.svg` |
| `src/titlebar.rs` | Custom window chrome, dragging and edge resizing |
| `src/task.rs` | `Load<T>`, the background job pattern used everywhere |
| `src/github/` | REST and GraphQL client, models, and `web.rs` for the few profile bits only the web page has |
| `src/gitops.rs` | Local git through libgit2 |
| `src/templates.rs` | README, community file, issue and PR templates |
| `src/views/` | Screens. `repo/` holds everything under a repository |
| `android/` | Android shared library crate and the Gradle project around it |
| `.github/` | CI and release workflows, issue and PR templates |

## Conventions

* Network and git work never runs on the UI thread. Start it with `Load::spawn`, poll it each frame, render with `widgets::show_load` or a match on the state.
* Colors come from the palette (`widgets::p(ui)` or `theme::pal(ctx)`). Never hardcode a color in a view.
* Reuse the components in `widgets.rs` and `views/common.rs` before writing new drawing code. New GitHub style components go into `widgets.rs`.
* Icons: copy the 16px octicon SVG into `assets/octicons/` with `fill="#ffffff"` on the root element, then regenerate the enum in `src/icons.rs` (variant names are the kebab case file names in PascalCase).
* Pages implement `views::Page` (or `RepoSub` under a repository) and get a `Cx` for navigation, toasts, preferences and the API client.
* Anything that needs a native file dialog, opens local files, or works on a local clone has to check `platform::NATIVE_FILES` or `platform::LOCAL_GIT` and stay hidden when it's off, since Android has neither.
* Add new user preferences to `Prefs` in `app.rs` with a default that keeps existing behavior.
* Watch for egui layout traps: `allocate_ui` inside a horizontal parent lays children out horizontally (use `allocate_ui_with_layout` with `top_down`), a `Frame` takes its parent's layout direction, and anything that sizes itself from `available_width` inside a resizable panel or a two axis scroll area must account for margins and strokes or it grows every frame.
* Only parse github.com HTML (`github/web.rs`) when there is no API at all, and fail soft when the markup changes.
* Tokens live in the OS keychain only. Never log them, write them to disk or put them in screenshots or test fixtures.

## Writing style

* Code, comments, docs and commit messages are in English.
* Comments only where they add something. Write them like a developer thinking out loud, short and casual, and explain non obvious assumptions or workarounds.
* Comments and doc blocks describe the current behavior, never the history of a change.
* Don't use dashes as punctuation in prose.

## Git

* Commit titles follow Conventional Commits without a scope, all lowercase: `fix: handle empty chunk palette`. Title only, no body.
* One purpose per commit. No unrelated formatting changes.
* Never commit or push unless asked, and never force push.
