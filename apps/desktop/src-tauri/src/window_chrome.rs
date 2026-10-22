//! GTK owns window dragging, resizing, focus and accessible decoration buttons.
#[cfg(target_os = "linux")]
pub(crate) fn install(window: &tauri::WebviewWindow) -> Result<(), Box<dyn std::error::Error>> {
    use gtk::prelude::*;
    let native = window.gtk_window()?;
    let header = gtk::HeaderBar::new();
    header.set_title(Some(&window.title()?));
    header.set_has_subtitle(false);
    header.set_show_close_button(true);
    header.set_decoration_layout(Some(":minimize,maximize,close"));
    header.style_context().add_class("openusage-titlebar");
    let provider = gtk::CssProvider::new();
    provider.load_from_data(b".openusage-titlebar { min-height: 38px; padding: 0 8px; background-image: none; background-color: @theme_base_color; box-shadow: none; border-bottom: 1px solid alpha(@theme_fg_color, 0.12); } .openusage-titlebar .title { font-size: 13px; font-weight: 500; }")?;
    if let Some(screen) = gtk::prelude::GtkWindowExt::screen(&native) {
        gtk::StyleContext::add_provider_for_screen(
            &screen,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    native.set_titlebar(Some(&header));
    header.show_all();
    Ok(())
}
