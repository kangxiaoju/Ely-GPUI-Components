# GPUI Kit

Ely uses GPUI Fast directly; GPUI Kit is not a product dependency. The independent `compat/kit` workspace demonstrates running Ely beside Longbridge's [GPUI Kit](https://gpui-kit.com) in one app and one window. It pins Kit `=0.9.1` with `gpui-fast` enabled, matching Ely's Fast 0.1.14 backend rather than Kit's default backend.

## Add the dependencies

```toml
[dependencies]
gpui-kit = { version = "=0.9.1", features = ["gpui-fast"] }
ely-gpui-component = { git = "https://github.com/ELY-Open-Source/Ely-GPUI-Components" }
gpui = { package = "gpui-fast", git = "https://github.com/kangxiaoju/Ely-GPUI-Components.git", rev = "a0a941b84dbe0986575b11cb3c9025e177976c4b", default-features = false }
```

With `gpui-fast` enabled, Kit re-exports its Fast API at its root, so this independent app writes `use gpui_kit::*`. Ely does not follow Kit's default GPUI: all participating libraries must resolve the same Fast version and source. If the app also declares `gpui`, `gpui_platform` or `gpui_web`, use the Fast packages and match Ely's pin and source.

The runtime is fixed to `https://github.com/kangxiaoju/Ely-GPUI-Components.git` at revision `a0a941b84dbe0986575b11cb3c9025e177976c4b` on `runtime/gpui-fast-0.1.14`. Registry Kit's Fast dependencies must resolve to that git source too: copy the complete `[patch.crates-io]` table from `compat/kit/Cargo.toml` into your application's workspace root. It maps `gpui-fast`, `gpui-fast-platform`, `gpui-fast-web`, `gpui-fast-macros`, `gpui-fast-linux`, `gpui-fast-macos`, `gpui-fast-windows`, `gpui-fast-wgpu` and `gpui-fast-apple` to that exact git revision. This is a same-package source override, not a pre compatibility shim. Without the root patch, registry Fast and git Fast remain distinct types even at version 0.1.14.

The compatibility gate resolves the fixed Kit version in `compat/kit/Cargo.toml`; it does not search for or upgrade to the newest Kit. This isolated integration does not add Kit's components or theme to an Ely-only product.

## Start both

| Step | Why |
| --- | --- |
| `gpui_kit::application().with_assets(Assets::before(gpui_kit::assets::Assets))` | GPUI takes one asset source. Ely's files load from Ely, every other path from Kit. `ely_gpui_component::init` returns an error if one of Ely's files does not load. |
| `gpui_kit::init(cx)` and `ely_gpui_component::init(cx)?` | In either order. |
| `gpui_kit::open_window(options, cx, build)` | Kit's `Root` hosts Kit's dialogs, sheets and notifications. |
| `FocusScope::new(&handle).root()` around the view, `handle` focused | Ely's Tab, focus return and overlays live in the scope. |

## Focus

Tab and Shift-Tab run through Ely's and Kit's stops in one order. Ely binds them under `ElyFocus`, the key context of its scopes, so Kit's code editor keeps Tab for indenting whichever `init` ran last. A Kit dialog opened from an Ely button keeps Tab inside and hands focus back when it closes; so does an Ely dialog over Kit's fields.

## Two themes

Each library keeps its own theme. A switch turns both:

| Ely | Kit |
| --- | --- |
| `Theme::set_mode(Mode::Dark, cx)` | `Theme::change(ThemeMode::Dark, Some(window), cx)` |

Locales work the same way: Ely reads its `I18n` global, Kit its `gpui_kit::component::set_locale`.

## Inspector

In debug builds Kit's `init` installs Kit's inspector. Call `ely_gpui_component::tooling::install_inspector(cx)` after it to draw Ely's.

## A whole app

[`compat/kit/examples/start.rs`](https://github.com/ELY-Open-Source/Ely-GPUI-Components/blob/main/compat/kit/examples/start.rs) puts Ely's heading and button beside Kit's field and dialog, and one button turns both themes. From a clone, run it with `cargo run --manifest-path compat/kit/Cargo.toml --example start`. The independent crate contains the compatibility tests; migration verification is performed by the integration owner, and passing Fast migration checks are not asserted here.
