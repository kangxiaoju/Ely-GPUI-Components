//! Canvas, chapter 32: an endless plane, its shapes and tools, design panels, whiteboards, flows, node graphs and mind maps.
//!
//! For retained editors, keep an `Arc<CanvasScene>` per content revision and pass it to
//! `InfiniteCanvas::scene`; `ToolLayer::shared_shapes` shares its shape array. Connector
//! routes and rotated image resources are prepared once. `link_animations` paints on an
//! isolated view, so animation does not rebuild the editor or scene. Set
//! `animations_active(false)` on hidden tabs; `reduced_motion` and the theme preference
//! suppress frame scheduling. `Link` itself remains source-compatible.
//!
//! `ShapeAppearance` uses theme hue slots, canvas-unit widths/font sizes and optional
//! fills. Shape rotation and `ToolLayer::on_rotate` use radians. Diamond, embedded Image
//! and frame-relative Polyline extend the same production painter and hit-testing path.
//! Tool previews use `appearance` and preserve committed rotation/style during transforms.
//!
//! `ShapeKind::words(cx)` localizes the type without rewriting user names. Use
//! `InspectorPanel::defaults` to edit a new-shape appearance with no selection.
//! `CanvasFont::Handwritten` uses the host-registered `Xiaolai` family; hosts register
//! the same font bytes with their SVG exporter and `register_canvas_text_outliner`.
//! Outlined SVGs are cached by source text, font and geometry, independent of viewport.
//! Native and export outlines share `sketch_outline` and `sketch_seed`; a second trace
//! uses `seed ^ 0x9e3779b97f4a7c15`, width × 0.65 and opacity × 0.45.
//! `LinkAnimation::glow` adds a theme-hue particle with a 35% inter-pass gap.
//!
//! `ShapeKind::RoundedRect` and `Tool::RoundedRect` share `rounded_rectangle_outline`
//! with exporters: clockwise, unclosed canvas points, radius capped at 12 units.
//! Inspector Standard/Hand-drawn geometry changes roughness only (0 / 1.2), retaining
//! fonts and colors; choosing Hand-drawn again preserves an existing roughness.
//! Eraser strokes accumulate unique unlocked targets and call `ToolLayer::on_delete`
//! once after release, so owners can confirm one batch without interrupting the drag.
//! `NodeEmphasis` overlays viewport-visible `NodeEmphasisPath` outlines in view pixels.
//! Hosts refresh paths on layout/pan/zoom, clip the canvas, and hide the layer explicitly;
//! only running paths animate, below 30fps, with theme hues and reduced-motion support.
//!
//! Architecture nodes use `ShapeKind::Symbol` and `symbol_paths` for shared native/SVG
//! geometry. `CanvasScene::with_link_presentations` binds prepared world-space routes
//! and independent stroke/head styles, including self-loops. `hit_link` uses those
//! same routes. `RouteEditor` drags stored waypoints and commits once on release.
//! `CanvasScene::with_link_fonts` sets connector label families independently by key;
//! omitted entries retain the original inherited UI font.
#![deny(missing_docs)]

use ely_core::{buttons, feedback, forms, i18n, layout, lists, overlays, primitives, shell, theme};

mod align;
mod assets;
mod autolayout;
mod constraints;
mod edit;
mod export;
mod flow;
mod gesture;
mod graph;
mod history;
mod inspector;
mod layers;
mod links;
mod link_surface;
mod marks;
mod mindmap;
mod mindmap_view;
mod minimap;
mod node_emphasis;
mod nodes;
mod paint;
mod palette;
mod panel;
mod plane;
mod press;
mod rounded;
mod shape;
mod sketch;
mod style;
mod symbol;
mod route_editor;
mod text_outline;
mod tools;
mod typography;
mod view;
mod whiteboard;
mod wiring;
mod writing;
mod zoom;

#[cfg(all(test, feature = "test-support"))]
mod tests;

pub use align::{Align, AlignmentToolbar, aligned};
pub use assets::AssetPanel;
pub use autolayout::{AutoLayout, AutoLayoutControls, Flow, Place};
pub use constraints::{Constraints, ConstraintsEditor, Pin};
pub use edit::{Handle, Tool};
pub use export::{ExportFormat, ExportPanel, ExportSetting, export_files};
pub use flow::{FlowChart, Step, StepKind};
pub use gesture::Brush;
pub use graph::{Edge, Node, Port};
pub use history::HistoryPanel;
pub use inspector::InspectorPanel;
pub use layers::LayerPanel;
pub use links::{Link, LinkPresentation, LinkAnimation, FlowMode, FlowDirection, CanvasScene};
pub use marks::{SelectionBox, SnapIndicator, TransformHandles};
pub use mindmap::Topic;
pub use mindmap_view::MindMap;
pub use minimap::MiniMap;
pub use node_emphasis::{NodeEmphasis, NodeEmphasisPath};
pub use nodes::NodeGraph;
pub use paint::{at, finish, outline};
pub use palette::{BrushSettings, ToolPalette};
pub use panel::{OnEdit, caption, color_well, editing};
pub use plane::InfiniteCanvas;
pub use shape::{Artboard, CanvasFont, Corner, Guide, Shape, ShapeKind, ShapeAppearance};
pub use rounded::rounded_rectangle_outline;
pub use sketch::{sketch_outline, sketch_seed};
pub use symbol::{SymbolKind, symbol_paths, symbol_label_frame};
pub use route_editor::RouteEditor;
pub use style::{Border, BorderEditor, ColorPanel, Dash, Shadow, ShadowEditor};
pub use tools::ToolLayer;
pub use text_outline::register_canvas_text_outliner;
pub use typography::{TypeStyle, TypographyPanel};
pub use view::{Frame, Viewport};
pub use whiteboard::{Board, Whiteboard};
pub use zoom::{ZoomControls, ZoomIndicator};
