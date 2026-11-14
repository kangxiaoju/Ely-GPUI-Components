# AGENTS.md: ely-canvas

Rules for `ely-canvas`, on top of the root `AGENTS.md` and `crates/ely-core/AGENTS.md`.

## Rules

- A ScrubInput shows its number in about 140px beside a short label. In a narrower column a pair hides it, so a long label takes a row of its own (`canvas::TypographyPanel`).
- Design panels edit a whole value and hand the owner a changed copy through `canvas::panel::editing`; the owner's handler is an argument of `new`. A swatch that opens a picker is `canvas::panel::color_well`.
- Architecture silhouettes and details live in `symbol_paths`; native paint, hit geometry and exporters share them. Closed paths repeat their first point. Container hit regions are border/title only so their contents remain selectable.
- Prepared connector routes belong to `CanvasScene`, not animation frames. `LinkPresentation` owns static dash/color/width/heads; flow direction never changes them. `RouteEditor` previews locally and commits on release.
