# Changelog

What changes for an app built on Ely, newest first. An entry names the crates it touches. Versions follow semver: below 1.0, a breaking change raises the minor version, and CI's `semver` job fails one that does not.

## Unreleased
- `ely-canvas`: public `NodeEmphasis` / `NodeEmphasisPath` provide isolated, theme-hue node outlines in view pixels. Running paths move along their geometry below 30fps; hidden/static paths and reduced motion stop the timer. The canvas gallery demonstrates viewport-filtered running and static outlines without changing its retained scene.
- `ely-core`: `Tag::hue`, `outlined`, `selected` and `on_click` support theme-colored label chips and keyboard-accessible filter buttons with Ely focus rings; removing a tag does not activate its filter.
- `ely-core`: `TextInput::clear_sensitive` zeroizes owned text and composition/undo/redo snapshots and resets selection/layout. Masked fields retain sensitive-drop handling after reveal; OS, renderer and caller copies are outside this guarantee.
- `ely-core`, `ely-documents`, `ely-git`: merge official upstream `e32d7f2` while retaining Yu's context-menu dismissal, selectable Markdown, spinner pacing, native mail, canvas, tree and theme extensions; dependencies remain on `gpui-pre` 0.3.10.
- `ely-editor`: typewriter scrolling centers the caret in the full editor viewport, independent of reserved scrollbar padding; horizontal scrolling and end-of-document room remain intact.
- `ely-documents`: literal ampersand runs skip empty entity-name lookups while preserving HTML decoding rules and linear scanning.
- `ely-core`: `Spinner::max_fps` optionally bounds decorative redraws while preserving cycle duration, styles, and reduced-motion behavior. Existing spinners keep display-rate updates.
- `ely-core`: `RadioCard::thumbnail` places a preview above its name, with native selected/focus outlines and the radio mark beside the name. Existing card layouts are unchanged.

- `ely-core`: `ContextMenu::on_close` notifies its owner after dismissal, allowing numbered manual requests to be cleared before a host is mounted again.

- `ely-core`: Shell code colors a `#` right after the `)` that closes `$(`, `$((`, `<(` or `>(` as part of the word, as Bash reads it; only a bare subshell's `)` ends a word before a comment. A `case` pattern's `)` inside `$(` no longer counts as the `)` that closes it.
- `ely-core`: a menu's icon buttons carry no selected state, so Windows offers their Invoke pattern to a screen reader; in a `SearchableMenu`, the arrows lend the panel focus so a reader hears the marked row or button, and typing or Backspace gives the filter focus back.
- `ely-core`: adds the Korean `ko-KR` locale and Korean component messages. The gallery's i18n page holds one catalog per listed locale, Spanish and Korean too; its table is sized to `LOCALES`, so a new locale without one does not build.
- `ely-documents`: a wheel over `MarkdownDocument`'s margins beside a narrowed text column, or over its bar's track, scrolls the text as far as one over the text, once; in the source view a line wheel keeps the code's own line height at any text size, and a sideways wheel (Shift and the wheel) scrolls the code down there as over the code. Split is unchanged.
- `ely-editor` 0.2.0: `Metrics` holds `wheel`, one wheel line as the code's list scrolls it, so a struct literal of `Metrics` names it.
- `ely-core`: `TreeDrag` is public, with `key` and `label`, so a drop past a `Tree`'s rows names the dragged node.
- `ely-core`: `SegmentedControl::on_context` hears a right press on a segment, with its value and the pointer, as `Tabs::on_context` does; the press stops on the segment, so the focus stays where it was.
- `ely-core`: adds the Vietnamese `vi-VN` locale and Vietnamese component messages; the gallery's i18n page holds a Vietnamese catalog.
- `ely-core`: adds the Spanish `es-ES` locale and Spanish component messages.
- `ely-documents`: in `MarkdownDocument`'s visual view, F6 and Shift+F6 move focus to the nearest stop outside the document, so a keyboard user leaves a long document in one key; they had stepped block to block as Tab does. With nothing outside, focus stays where it was.
- `ely-core`: a `Banner`'s action carries the debug selector `banner-action {label}`, so a test can press it.
- `ely-core`: each box of a `CheckboxGroup` carries the debug selector `check {value}`, as `ToggleGroup`'s buttons carry `toggle {value}`, so a test can press one box by its value.
- `ely-core`: a `Scrollbar` over a `ListState` reads the list's attainable end from its measured tail, so a held drag turned back from the bottom moves at once, and returning from the end keeps the thumb's size and range; an unmeasured gap had shrunk the range to the items measured.
- `ely-documents`: on macOS, a formula or diagram letter only the system's Last Resort font draws counts as having no font, so `formula_svg` and `diagram_svg` refuse it instead of writing Last Resort's placeholder box.
- `ely-documents` (within the unreleased 0.2.0): add `email::{HtmlEmailEditor, EmailEditorView, EmailToolbar, EmailDocument}` for native, structured HTML mail bodies, with one GPUI input handler, shared selection/history across rich paragraphs, inline images and table cells, sanitized HTML/plain-text codecs, and bounded import/model/history budgets. Host events delegate rich clipboard and image/CID work; the component does not provide MIME sending, implicit remote image loading or arbitrary HTML/CSS fidelity. The mail gallery composes it with `MailComposer`. These APIs are additive; the existing 0.2.0 bump remains for the table-constructor change below. Runtime acceptance is not asserted; macOS is unverified.
- `ely-documents`: group contiguous native typing into one undo step while isolating paste, formatting and IME commits; capture both paragraphs for boundary Backspace/Delete so undo restores exact blocks and the original caret.
- `ely-documents`: navigate cached offscreen paragraph lines and reveal the edited/selected caret through Ely's scroll primitive, with a physical-pixel gutter for snapped offsets; manual scrolling is unchanged when selection does not move.

- `ely-core`: `Avatar::muted` keeps a subtle name-colored background with normal theme text. `Tree::on_open` reports expansion for host state and lazy loading; `row_content` composes file-type icons and host editing fields inside native tree interaction.
- `ely-core`: `ListItem::compact` uses smaller theme typography and spacing for dense navigation without changing other rows.
- `ely-core`: `Avatar::solid` renders a name's full theme-chart background with contrasting theme text, for app and project letter marks; existing subtle avatars stay unchanged.
- `ely-core`: top-level raised overlays namespace their descendants by owner, preventing sibling accessible labels and keyed state from sharing identities.
- `ely-documents`, `ely-core`: pending Markdown pictures show a localized loading label instead of their potentially large data address; preview-only images no longer copy addresses for unused point handlers.
- `ely-core`: `Dialog::height` keeps asynchronous loading and ready content in the same viewport-capped modal frame.
- `ely-core`: standard and fullscreen dialogs wrap long titles and descriptions without squeezing or hiding Close. Standard dialog widths, including custom widths, stay inside narrow viewports.
- `ely-project`: `KanbanColumn::footer` puts controls after the cards without adding move slots, and `total` shows the full count for paged columns. `KanbanBoard::track_scroll` shares its horizontal handle with the host; the board keeps overflow inside its available width. The gallery demonstrates paged columns.
- `ely-core`: `DatePicker::iso` displays selected dates as YYYY-MM-DD. Standard dialogs keep intrinsic body height when long composed forms overflow, so the body scrolls without compressing hidden controls; the title and actions remain fixed.
- `ely-mail`: `MailboxList::section_action` places an independent action beside a section title while preserving folded unread counts and row selection.
- `ely-documents` 0.2.0: `MarkdownBlock::Table` adds `spans` (breaking table constructors and exhaustive matches): head first, positive values span columns and zero hides covered cells; Markdown uses an empty matrix for one column per cell. Rendered tables fill their container and merged explanations use their full column span.
- `ely-canvas` 0.3.0, `ely-core`: add `ShapeKind::RoundedRect` / `Tool::RoundedRect` (breaking exhaustive enum matches), localized rounded-rectangle labels and shared `rounded_rectangle_outline` geometry for native/export rendering. Radius is `min(12, min(width, height) * 0.25)` canvas units; rounded hit testing and rotated connector bindings follow the corners. Inspector Standard/Hand-drawn segments change only roughness (0 / 1.2), preserving fonts, colors and an already-hand-drawn roughness. Only the canvas crate receives a semver bump; other components keep their versions.
- `ely-canvas`, `ely-core`: localize canvas inspectors, type names, layers and zoom; separate stroke and background fill; reuse inspector appearance controls for new-shape defaults. `ShapeAppearance.font` selects standard, host-bundled Xiaolai handwriting or monospace without changing UI fonts. Shared seeded `sketch_outline` geometry produces bowed double-traced edges in native and export renderers. `LinkAnimation.glow` adds an intermittent route-length particle on the isolated visible-link surface; pause, hidden tabs and reduced motion retain phase without continuous frames.
- `ely-documents`: a `TemplatePicker` as narrow as a `Dialog` gives its list at most half, so the preview's name and Use button both show; narrower still, the name is cut with an ellipsis, never the button. The button had been squeezed over the name.
- `ely-documents`: in `MarkdownDocument`'s visual view, a drag or Shift+Up/Down past the open block opens the blocks it crosses as one field, selected across them; copy, cut, delete and typing take the whole selection, written back as one undo step. A drag past the view's top or bottom scrolls it.
- `ely-documents`: bold, italic, strikethrough, underline and inline code over a selection that crosses blocks apply in each paragraph, heading or cell it touches; when all are styled, all lose it. Link and picture still refuse with `EditRefusal::Across`.
- `ely-core`: `TextInput::select_from(anchor, head)` selects in either direction, the caret at `head`.
- All crates: Rust 1.98 at least, the floor gpui 0.3.10 builds on, since its macOS crate calls `Option::map_or_default`; 1.95 had been declared, and failed on macOS.
- `ely-core`: a `FormField` names the text field it frames for screen readers, its label as the name and its error or description as the description, unless the field's owner named it.
- `ely-core`: a `Dialog` without `focus_first` focuses its first control a frame after it opens; `Dialog`, `ActionSheet`, `Lightbox`, `Spotlight` and `Tour` name the element that takes their focus. `FocusScope::node` names a scope.

## 2026-10-26: `ely-core` 0.2.0, every other crate 0.1.1

The first release on crates.io.

- 31 library crates: `ely-core` (chapters 1–14, theme, motion, i18n), one crate per vertical chapter from `ely-charts` to `ely-rendering`, `ely-desktop` (tray, Dock badge, Omarchy), and `ely-gpui-component`, which re-exports them all under their chapter names.
- gpui, gpui_platform and gpui_web from `gpui-pre` `=0.3.10`, the snapshot GPUI Kit 0.8.1 pins, so Ely and Kit share a window.
- `ely-core` 0.2.0: gpui_platform moves behind the `platform` feature, off by default, so a Button-only app links no gpui backend Ely chose; `runtime_shaders` turns it on, and `ely-gpui-component` keeps both on. `init` fails on a platform with no text system, naming the features that give one: `font-kit` on macOS, `wayland` and `x11` on Linux.
- Every public item documented, each component with an example that compiles and runs; `api/` holds each crate's public API.
- Every focus stop has a role and a name for screen readers; `gallery --audit` walks them on Windows.
- Messages in English, Chinese, German, French, Japanese and Hebrew; the last four machine translated, awaiting native review.
- Icons link by use and fonts by feature (`fonts`, on by default); fonts embed with `include_bytes!`, and `Combobox::languages` reads Ely's own ISO 639-1 table, so neither `rust-embed` nor `isolang` is a dependency.
