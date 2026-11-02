//! Native structured HTML mail composition, without Markdown source editing.

mod model;
mod edit;
mod history;
mod html;
mod export;
mod editor;
mod input;
mod actions;
mod prefix;
mod layout;
mod element;
mod view;
mod toolbar;
mod dialogs;
#[cfg(all(test, feature = "test-support"))]
mod native_tests;
#[cfg(all(test, feature = "test-support"))]
mod boundary_history_tests;
#[cfg(all(test, feature = "test-support"))]
mod layout_tests;

pub use model::*;
pub use history::{EmailCheckpoint, EmailHistory};
pub use editor::{EmailEditorEvent, HtmlEmailEditor};
pub use view::EmailEditorView;
pub use toolbar::EmailToolbar;
pub(crate) use actions::bind_keys;
#[cfg(all(test, feature = "test-support"))]
mod viewport_tests;
