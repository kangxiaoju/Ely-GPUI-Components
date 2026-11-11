# Ely GPUI Component

A component library for [GPUI](https://www.gpui.rs), the Rust UI framework behind Zed.
Every component ships in light and dark. The palette is warm and quiet. Color carries meaning, not decoration.

Every component runs live at https://elygpui.com, compiled to WebAssembly.

Status: early. `TASKS.md` tracks every component, chapter by chapter. To help, see `CONTRIBUTING.md`.

## Use

```toml
[dependencies]
ely-gpui-component = { git = "https://github.com/kangxiaoju/Ely-GPUI-Components.git", branch = "feat/gpui-fast-migration" }
gpui = { package = "gpui-fast", git = "https://github.com/kangxiaoju/Ely-GPUI-Components.git", rev = "40427c396cc92b6946940e0e498612a8b680ce30", default-features = false }
gpui_platform = { package = "gpui-fast-platform", git = "https://github.com/kangxiaoju/Ely-GPUI-Components.git", rev = "40427c396cc92b6946940e0e498612a8b680ce30", default-features = false, features = ["font-kit", "wayland", "x11"] }
```

The component migration branch and the runtime branch are distinct. For deployment, replace the component's `branch` with `rev` set to a full component commit from `feat/gpui-fast-migration`; keep the runtime dependencies at the exact revision shown above. Do not substitute the upstream component main branch, which uses a different backend.

```rust
use ely_gpui_component::{Assets, theme::{Mode, Theme}};
use gpui::App;

fn main() {
    gpui_platform::application().with_assets(Assets).run(|cx: &mut App| {
        ely_gpui_component::init(cx).expect("Ely failed to start");
        Theme::set_mode(Mode::Dark, cx);
    });
}
```

`init` registers the fonts and the theme. It returns an error if the application's asset source cannot load one of Ely's files.

Ely uses GPUI Fast directly, without GPUI Kit or another component/theme layer. The workspace keeps the Rust dependency names `gpui`, `gpui_platform` and `gpui_web`, backed by the Fast 0.1.14 packages from `https://github.com/kangxiaoju/Ely-GPUI-Components.git` at revision `40427c396cc92b6946940e0e498612a8b680ce30` (the `runtime/gpui-fast-0.1.14` branch). Applications must use this exact backend source and revision; mixing registry Fast with git Fast produces incompatible GPUI types.

## Native HTML email composition

`ely_documents::email` (also `ely_gpui_component::documents::email`) provides `HtmlEmailEditor` and the serializable `EmailDocument`. The document stores literal Unicode text, normalized `EmailRun` style intervals and stable block IDs, not Markdown or editable HTML source. Call `EmailDocument::from_plain` for literal text or `from_html` for untrusted HTML; export the same model with `to_html` and `to_plain`.

Create an `Entity<HtmlEmailEditor>` with `HtmlEmailEditor::new(document, window, cx)`. Render the entity for the body alone, or wrap it with `EmailEditorView::new(id, &editor)` for the selection-aware `EmailToolbar`. Call Ely's `init` before use to register its key bindings. The editor owns one focus handle and one GPUI `EntityInputHandler` across paragraphs, inline objects and table cells; the toolbar does not create another body input. `MailComposer` remains the separate envelope, attachment and send UI.

### Supported content and safety

- Paragraphs, headings 1–6, quotes, bulleted/numbered/task items, preformatted code, hard breaks and horizontal rules.
- Bold, italic, underline, strike, inline code, safe font-family names, 8–72 pt sizes, explicit RGB text colors, alignment and indentation. Links accept HTTP, HTTPS and `mailto` only.
- Atomic block and inline images, including rich table-cell content. `EmailText::images` associates image metadata with U+FFFC object tokens at UTF-8 byte offsets. Tables support row/column insertion and deletion and horizontal merge/unmerge; row spans are not preserved.
- HTML5 import normalizes the supported subset. Scripts, stylesheets, embedded documents, forms, event attributes, hidden content and unsafe URLs are discarded; unsupported CSS and browser layout are not retained. This is a mail-body authoring component, not a browser, general DOM editor or lossless editor for arbitrary marketing HTML/CSS.
- Images accept `cid:`, `asset:`, HTTP(S) and base64 PNG/JPEG/GIF/WebP data references, not SVG or file URLs. Import and rendering never fetch remote resources implicitly; unresolved images need host-provided assets.

HTML and plain-text imports are limited to 2 MiB. The validated document budget also limits text and image/style-string metadata to 2 MiB, with at most 4,096 blocks and 65,536 style runs. HTML nesting is limited to 32 levels; tables to 64 rows × 64 columns (4,096 cells). Invalid or oversized editor operations return `EmailError` without replacing existing content. Undo/redo stores affected block splices and directed selections, retaining at most 100 complete operations within a 32 MiB accounting budget; oldest complete operations are evicted. Contiguous typing can coalesce, while paste, formatting and committed IME composition use separate transactions.

### Host integration

Subscribe to `EmailEditorEvent` for `Changed` and actionable `Problem` notifications. Use `snapshot()` when saving or exporting committed content: it excludes active IME previews. `committed_revision`, `committed_is_empty`, `committed_is_blank` and `committed_estimated_bytes` expose cached committed-state information; the byte estimate excludes MIME markup and attachments.

GPUI's default clipboard path supports plain text and Ely's HTML metadata for internal copies; it is not an external `text/html` adapter. Enable `set_host_clipboard(true)` to handle `PasteRequested` and `CopyRequested` in your application. Read external HTML/plain-text flavors, import the chosen representation, and call `complete_paste(request, document, cx)`. Write both HTML and plain-text flavors on copy. For a cut, delete the original selection only after copying succeeds and only if the document and selection still match the request context.

Handle `InsertImageRequested` by picking, validating and decoding the image outside the editor, storing its attachment/CID or asset reference, then calling `complete_image`. Supply resolved display assets with `set_images`; the host owns image I/O, decoding limits, attachment lifetime and CID-to-MIME assembly. Paste/image completions reject stale request IDs or changed document revisions/selections rather than overwrite newer edits. Use `set_problem` to present host failures. Neither MIME delivery nor operating-system rich clipboard access is supplied by this component.

The [mail gallery source](crates/ely-gallery/src/pages/mail/write.rs) pairs the body editor with `MailComposer`; the standalone source is [`crates/ely-gallery/examples/html_email.rs`](crates/ely-gallery/examples/html_email.rs). End-to-end runtime acceptance is not recorded here; macOS behavior is unverified.

## With GPUI Kit

The independent `compat/kit` workspace pins Longbridge's [GPUI Kit](https://gpui-kit.com) to `=0.9.1` and enables its `gpui-fast` feature, whose published dependency targets Fast 0.1.14. Kit is not part of Ely's product dependency graph. Pass `Assets::before(gpui_kit::assets::Assets)` as the one asset source, call both `init`s, open the window with `gpui_kit::open_window`, and wrap the view in Ely's root `FocusScope`. The [guide](https://elygpui.com/docs/gpui-kit) covers themes, focus and the inspector; `compat/kit` holds the independent app and compatibility tests. Migration compatibility is pending the integration owner's checks, not asserted here.

The independent workspace's `[patch.crates-io]` maps all nine Fast packages to the same runtime git revision as Ely. Copy that patch table from `compat/kit/Cargo.toml` into your application's workspace root when using registry Kit beside Ely; dependency manifests cannot impose patches on their consumers. This unifies Fast's source identity, not a `gpui-pre` shim.

## On Omarchy

`desktop::follow_omarchy(dir, cx)` dresses every component in Omarchy's current theme and follows each switch; `desktop::omarchy_current_dir()` finds the folder, and `stop_omarchy` gives Ely's palettes back. The [theme guide](https://elygpui.com/docs/theme) says how the colors map.

## Gallery

```sh
cargo run -p ely-gallery
cargo run -p ely-gallery -- --page buttons
cargo run -p ely-gallery -- --capture shots   # macOS: PNG of every page, light and dark
ELY_GALLERY_ASSETS=https://example.com/gallery/assets/ scripts/web.sh dist   # for the browser
```

In a browser, `?page=buttons&story=icon-button&theme=dark` draws one section alone; `crates/ely-gallery/stories.json` lists them all.

The web gallery uses `gpui-fast-web` 0.1.14 from the same exact runtime git revision, with default features disabled, as does `gpui-fast-platform`. Its explicit `WebPlatform` constructor keeps multithreading disabled and browser font fallback limited to emoji and CJK; Hebrew remains bundled. No shared-memory deployment requirement is introduced.

## Website

```sh
cd frontend
pnpm install
ELY_GALLERY_ASSETS=https://<site>/gallery/assets/ pnpm gallery   # the wasm gallery, into public/gallery
pnpm dev        # or: pnpm build && pnpm preview; both fetch the showcase at showcase.lock's commit
ELY_SHOWCASE=../../Ely-GPUI-Showcases pnpm dev   # the showcase from a local checkout
node scripts/og.mjs   # with pnpm preview up: public/og.png from the hero
node scripts/favicon.mjs   # public/favicon.svg, apple-touch-icon.png and ../logo.png from the mark
pnpm deploy     # Cloudflare, through wrangler
```

## How it is built

Ely is built by AI coding agents. A person sets each goal and decides what ships; agents write the code, its tests and these docs, and other agents review them.

- Work runs as task items. One agent writes an item and another model reviews it, round after round, up to five; every round is in [`tasks/reviews.md`](tasks/reviews.md). Some items end on a FAIL row there: their last fixes went in after the cap, unreviewed. These reviews are models reading models; no person has signed a crate yet.
- No crate is stable until the owner signs its public API and, past `ely-core`, an outside user has given feedback. [`tasks/stability.md`](tasks/stability.md) keeps the ledger; today no crate is stable.
- [`scripts/check.sh`](scripts/check.sh) gates every change: formatting, lints, tests, the docs build, the web lint, the panic list and the house rules. CI runs it on Linux, Windows and macOS, beside checks on the public API, semver, licenses, the minimum Rust and pixels.
- Each `expect`, `panic!` and `unreachable!` in library code is listed with its reason in [`scripts/panics.tsv`](scripts/panics.tsv); [`tasks/panics.md`](tasks/panics.md) records how each chapter moved to failing visibly instead.
- The rules the agents follow are in [`AGENTS.md`](AGENTS.md) and each crate's own; decisions are in [`docs/adr/`](docs/adr/README.md).

## Build notes

- gpui compiles its Metal shaders at runtime here (`runtime_shaders`, on by default in `ely-gpui-component`), so a full Xcode install is not required. `ely-core` alone turns on no gpui backend; the app's own gpui_platform does. gpui_platform needs `font-kit`, or macOS draws no text, and `wayland` and `x11`, or Linux opens no window.
- Fast's platform defaults are disabled explicitly. Native applications still enable `font-kit`, `wayland` and `x11`; the root workspace supplies all three for the gallery and examples. The core library's optional-platform boundary and `runtime_shaders` behavior are unchanged.
- Migration verification is owned by the integrating parent: regenerate the workspace lockfile, check the native workspace and gallery, check the wasm gallery, and run the independent Kit compatibility suite. These checks have not been run as part of this source migration.
- CI runs `scripts/check.sh` and builds the gallery on GitHub-hosted Linux, Windows and macOS. The capture tool shoots pages on all three; only macOS plays the scripted states.

## License

MIT. Lucide icons: ISC. Inter, JetBrains Mono, IBM Plex Sans and the gallery's Noto Sans Hebrew: SIL Open Font License 1.1.
The gallery photos in `crates/ely-gallery/assets` were generated for this project with gpt-image-2.
The native email editor draws architectural inspiration from [Aviary](https://github.com/Catvert/aviary) (Apache-2.0) and [gpui-flowtext](https://github.com/realartists-gitmo/gpui-flowtext) (MIT). These are design references, not copied source or runtime dependencies.
