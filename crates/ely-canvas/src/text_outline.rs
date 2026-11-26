use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};
use gpui::{App, Global};

type Outliner = Rc<dyn Fn(&str) -> Result<Vec<u8>, String>>;
struct CanvasTextOutliner {
    render: Outliner,
    cache: RefCell<HashMap<String, Arc<[u8]>>>,
}
impl Global for CanvasTextOutliner {}

/// Registers a host SVG text-to-path converter using the same bundled fonts as export.
/// GPUI's SVG rasterizer does not share fonts registered through its text system.
/// The converter must return SVG bytes with text converted to paths; errors are logged.
/// Results are cached in a bounded cache, cleared whenever the converter is replaced.
pub fn register_canvas_text_outliner(cx: &mut App, handler: impl Fn(&str) -> Result<Vec<u8>, String> + 'static) {
    cx.set_global(CanvasTextOutliner { render: Rc::new(handler), cache: RefCell::new(HashMap::new()) });
}

pub(crate) fn outlined(svg: &str, cx: &App) -> Arc<[u8]> {
    if !cx.has_global::<CanvasTextOutliner>() { return Arc::from(svg.as_bytes()); }
    let outliner=cx.global::<CanvasTextOutliner>();
    if let Some(bytes)=outliner.cache.borrow().get(svg) { return bytes.clone(); }
    match (outliner.render)(svg) {
        Ok(bytes) => {
            let bytes:Arc<[u8]>=Arc::from(bytes);
            let mut cache=outliner.cache.borrow_mut();
            if cache.len()>=256 { cache.clear(); }
            cache.insert(svg.to_owned(),bytes.clone());
            bytes
        }
        Err(error) => {
            log::error!("canvas: bundled-font text outlining failed: {error}");
            Arc::from([])
        }
    }
}
