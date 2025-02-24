use std::rc::Rc;
use floem::{dyn_view, View, ViewId};
use floem::prelude::{container, create_rw_signal, create_signal, Decorators, dyn_view, RwSignal, SignalGet, SignalUpdate, stack};
use crate::epub_reader::create_epub_reader;
use crate::library::home_page::{home_view};
use crate::library::library_page::library_view;

pub mod home_page;
mod library_page;

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub enum Page { Home, Library, Reader, }
#[derive(Clone, Copy)]
pub struct Signals {
    pub active_page: RwSignal<Page>,
    pub prev_page: RwSignal<Page>,
    pub epub_path: RwSignal<String>,
    pub library_path: RwSignal<String>,
    pub root_library_path: RwSignal<String>,
}
pub fn page_navigation_view() -> impl View {
    /*let (active_page) = create_rw_signal(Page::Home);
    let (prev_page, set_prev_page)      = create_signal(Page::Home);
    let (epub_path, set_epub_path) = create_signal("".to_string());
    let (library_path, set_library_path) = create_signal("".to_string());*/
    
    let signals = Signals {
        active_page     : create_rw_signal(Page::Home),
        prev_page           : create_rw_signal(Page::Home),
        epub_path           : create_rw_signal("".to_string()),
        library_path        : create_rw_signal("".to_string()),
        root_library_path   : create_rw_signal("".to_string()),
    };

    let content = dyn_view(move ||
        match signals.active_page.get() {
        Page::Home      => container(home_view(signals.clone())),
        Page::Library   => container(library_view(signals.clone())),
        Page::Reader    => container(create_epub_reader(signals.epub_path.get().as_str(), signals.root_library_path.get().as_str(), signals.prev_page.get(), signals.clone()))
            .style(move |s| s.flex_grow(1.0)),
    }).style(move |s| s.width_full().height_full().flex_grow(1.0));

    content
}