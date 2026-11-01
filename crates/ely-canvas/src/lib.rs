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
//! the same font bytes with their SVG exporter. UI fonts are never replaced.
//! Native and export outlines share `sketch_outline` and `sketch_seed`; a second trace
//! uses `seed ^ 0x9e3779b97f4a7c15`, width × 0.65 and opacity × 0.45.
//! `LinkAnimation::glow` adds a theme-hue particle with a 35% inter-pass gap.
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
mod nodes;
mod paint;
mod palette;
mod panel;
mod plane;
mod press;
mod shape;
mod sketch;
mod style;
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
pub use links::{Link, LinkAnimation, FlowMode, FlowDirection, CanvasScene};
pub use marks::{SelectionBox, SnapIndicator, TransformHandles};
pub use mindmap::Topic;
pub use mindmap_view::MindMap;
pub use minimap::MiniMap;
pub use nodes::NodeGraph;
pub use paint::{at, finish, outline};
pub use palette::{BrushSettings, ToolPalette};
pub use panel::{OnEdit, caption, color_well, editing};
pub use plane::InfiniteCanvas;
pub use shape::{Artboard, CanvasFont, Corner, Guide, Shape, ShapeKind, ShapeAppearance};
pub use sketch::{sketch_outline, sketch_seed};
pub use style::{Border, BorderEditor, ColorPanel, Dash, Shadow, ShadowEditor};
pub use tools::ToolLayer;
pub use typography::{TypeStyle, TypographyPanel};
pub use view::{Frame, Viewport};
pub use whiteboard::{Board, Whiteboard};
pub use zoom::{ZoomControls, ZoomIndicator};
