use ely_gpui_component::{
    documents::email::{EmailDocument, EmailEditorView, HtmlEmailEditor},
    theme::{ActiveTheme, Mode, Theme},
    Assets,
};
use gpui::{App, AppContext, Bounds, Context, Entity, Focusable, IntoElement, ParentElement, Render, Styled, TitlebarOptions, Window, WindowBounds, WindowOptions, div, px, size};

struct Compose { editor: Entity<HtmlEmailEditor> }
impl Render for Compose {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p_4().bg(cx.theme().colors.bg).text_color(cx.theme().colors.fg)
            .child(EmailEditorView::new("html-mail-example", &self.editor))
    }
}
fn main() {
    gpui_platform::application().with_assets(Assets).run(|cx: &mut App| {
        ely_gpui_component::init(cx).expect("Ely initialization");
        Theme::set_mode_now(if std::env::var("EMAIL_THEME").as_deref() == Ok("dark") { Mode::Dark } else { Mode::Light }, cx);
        let html = "<p>Hello <strong>team</strong>, <span style='font-size:28pt'>HTML</span> email.</p><p>中文输入、emoji 🦀、العربية and Latin.</p><ul><li>First item</li><li>Second item</li></ul><table><tr><td>A</td><td>B</td></tr><tr><td colspan='2'>Merged summary</td></tr></table>";
        let document = EmailDocument::from_html(html).expect("example HTML");
        cx.open_window(WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(900.), px(700.)), cx))),
            titlebar: Some(TitlebarOptions { title: Some("Ely HTML Email Editor".into()), ..Default::default() }),
            ..Default::default()
        }, |window, cx| {
            let editor = cx.new(|cx| HtmlEmailEditor::new(document, window, cx));
            window.focus(&editor.focus_handle(cx), cx);
            cx.new(|_| Compose { editor })
        }).expect("example window");
        cx.activate(true);
    });
}
