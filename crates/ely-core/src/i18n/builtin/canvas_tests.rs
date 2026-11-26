use super::{TABLES, builtin};

#[test]
fn canvas_editor_labels_resolve_in_every_locale() {
    for (locale, _) in TABLES {
        for key in [
            "canvas.tool.rounded_rect",
            "canvas.geometry",
            "canvas.geometry.standard",
            "canvas.geometry.handdrawn",
            "canvas.tool.diamond",
            "canvas.tool.eraser",
            "canvas.appearance",
            "canvas.rotation",
            "canvas.stroke_width",
            "canvas.opacity",
            "canvas.roughness",
            "canvas.font_size",
            "canvas.fill",
            "canvas.read_only",
            "canvas.solid",
            "canvas.dashed",
            "canvas.dotted",
            "canvas.defaults",
            "canvas.empty",
            "canvas.selected_count",
            "canvas.empty_hint",
            "canvas.frame",
            "canvas.stroke",
            "canvas.visible",
            "canvas.locked",
            "canvas.font",
            "canvas.font.default",
            "canvas.font.handwritten",
            "canvas.font.monospace",
            "canvas.zoom_in",
            "canvas.zoom_out",
            "canvas.zoom_fit",
            "canvas.kind.rect",
            "canvas.kind.rounded_rect",
            "canvas.kind.diamond",
            "canvas.kind.image",
            "canvas.kind.polyline",
            "canvas.kind.ellipse",
            "canvas.kind.polygon",
            "canvas.kind.line",
            "canvas.kind.arrow",
            "canvas.kind.text",
            "canvas.kind.path",
            "canvas.kind.note",
            "canvas.hue.0",
            "canvas.hue.1",
            "canvas.hue.2",
            "canvas.hue.3",
            "canvas.hue.4",
            "canvas.hue.5",
            "canvas.hue.6",
            "canvas.hue.7",
            "canvas.x",
            "canvas.y",
            "canvas.width",
            "canvas.height",
        ] {
            let label = builtin(locale, key);
            assert!(!label.trim().is_empty(), "{locale}: {key}");
        }
        assert!(builtin(locale, "canvas.selected_count").contains("{count}"));
    }
}
