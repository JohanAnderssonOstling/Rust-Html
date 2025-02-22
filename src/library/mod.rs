use std::rc::Rc;
use floem::{dyn_view, View, ViewId};
use floem::prelude::{container, create_rw_signal, create_signal, Decorators, dyn_view, RwSignal, SignalGet, SignalUpdate, stack};
use crate::epub_reader::EpubReader;
use crate::library::home::{home_view, library_view, reader_view};

pub mod home;
mod epub;

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum Page {
    Home,
    Library,
    Reader,
}

pub fn page_navigation_view() -> impl View {
    // Create a reactive signal for the current page.
    let (active_page, set_active_page) = create_signal(Page::Home);
    let (epub_path, set_epub_path) = create_signal("".to_string());
    // A dynamic view rebuilds when `active_page` changes.
    
    let content = dyn_view(move || match active_page.get() {
        Page::Home => container(home_view(set_active_page, set_epub_path)),
        Page::Library => container(library_view(set_active_page)),
        Page::Reader => container(EpubReader::new(epub_path.get().as_str())).style(move |s| s.width_full().height_full().flex_grow(1.0)),
    }).style(move |s| s.width_full().height_full().flex_grow(1.0));

    //content.id().clear_focus();


    // Wrap the dynamic content in a container that fills the available space.
    //container(content)
        //.style(|s| s.width_full().height_full())
    content
}