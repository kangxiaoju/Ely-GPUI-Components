# Installation

## Requirements

- Rust 1.98 or later, the version Ely's `Cargo.toml` names.
- macOS, Windows or Linux. The crate's tests run on all three; on Linux, GPUI needs the `wayland` and `x11` features below.
- No full Xcode: GPUI compiles its Metal shaders at run time.

## Add the dependencies

Depend on Ely by git and use its direct GPUI Fast backend. Keep the dependency names `gpui` and `gpui_platform`, but select the Fast packages at the same version and source as Ely; GPUI Kit is not required:

```toml
[dependencies]
ely-gpui-component = { git = "https://github.com/ELY-Open-Source/Ely-GPUI-Components" }
gpui = { package = "gpui-fast", git = "https://github.com/kangxiaoju/Ely-GPUI-Components.git", rev = "a0a941b84dbe0986575b11cb3c9025e177976c4b", default-features = false }
gpui_platform = { package = "gpui-fast-platform", git = "https://github.com/kangxiaoju/Ely-GPUI-Components.git", rev = "a0a941b84dbe0986575b11cb3c9025e177976c4b", default-features = false, features = ["font-kit", "wayland", "x11"] }
```

These are Fast 0.1.14 packages from the fixed runtime revision on `runtime/gpui-fast-0.1.14`. Do not replace these git dependencies with registry packages of the same version: Cargo treats the two sources as distinct types.

| Line | Why |
| --- | --- |
| `ely-gpui-component` | The library. `Cargo.lock` records the commit; add `rev = "<commit>"` to hold it in `Cargo.toml` as well. |
| `gpui` | The same Fast version and source as Ely's, under the name `gpui`. A different backend or source builds a second GPUI, whose types do not match the ones Ely takes. |
| `gpui_platform` | Opens windows on each platform. Without `font-kit`, macOS falls back to GPUI's no-op text system and draws no text. Without `wayland` and `x11`, Linux runs headless and opens no window; building them needs `libxkbcommon-x11-dev` and `libfontconfig-dev`. |

The browser gallery uses `gpui_web = { package = "gpui-fast-web", git = "https://github.com/kangxiaoju/Ely-GPUI-Components.git", rev = "a0a941b84dbe0986575b11cb3c9025e177976c4b", default-features = false }` on wasm targets. Keep both web and platform default features disabled: the gallery explicitly creates a single-threaded `WebPlatform`, retaining its browser emoji/CJK fallback and bundled Hebrew font without requiring shared memory.

## Shaders

`ely-gpui-component`'s default feature, `runtime_shaders`, turns on the same feature in `gpui_platform`, so GPUI compiles its Metal shaders at run time. Disabling Ely's default features without enabling `runtime_shaders` makes the build run `xcrun metal` instead, which needs Xcode's Metal compiler. Disabling defaults on the Fast dependencies above does not disable Ely's `runtime_shaders`. `ely-core` alone turns on no `gpui_platform` feature: its `platform` feature, off by default, turns on `font-kit`, `wayland` and `x11`, or the app sets them in its own `gpui_platform` line, as above.

## Check the install

Copy the [first window](/docs/#a-first-window) into `src/main.rs` and run it:

```sh
cargo run
```

A window opens with a counter and a Press button. From a clone of the repository, `cargo run -p ely-gallery` opens every chapter.

## When it fails

| Symptom | Cause and fix |
| --- | --- |
| `init` error: `ely: the asset source lacks N of Ely's files, first <path>: <reason>` | The app's source cannot load every file Ely bundles. Pass `Assets`, or `Assets::before(yours)` when the app has a source of its own. |
| `init` error: `ely: embedded fonts failed to register` | GPUI's text system refused Ely's fonts. File a bug with the log. |
| `init` error: `ely: the platform has no text system` | `gpui_platform` lacks `font-kit` on macOS, or `wayland` and `x11` on Linux. `ely-core`'s `platform` feature turns on all three. |
| No window opens on Linux, and the log says nothing failed | `gpui_platform` lacks the `wayland` and `x11` features. |
| `mismatched types`, with the note ``there are multiple different versions of crate `gpui` in the dependency graph`` | The app selected another backend, version or source. Match Ely's Fast 0.1.14 packages from `https://github.com/kangxiaoju/Ely-GPUI-Components.git` at `a0a941b84dbe0986575b11cb3c9025e177976c4b` across core, platform and web; do not mix registry Fast, git Fast or `gpui-pre`. |
| `metal shader compilation failed` | Default features are off and Xcode's Metal compiler is missing. Keep `runtime_shaders`. |
