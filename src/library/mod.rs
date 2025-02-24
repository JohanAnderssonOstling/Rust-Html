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

pub fn page_navigation_view() -> impl View {
    let (active_page) = create_rw_signal(Page::Home);
    let (prev_page, set_prev_page)      = create_signal(Page::Home);
    let (epub_path, set_epub_path) = create_signal("".to_string());
    let (library_path, set_library_path) = create_signal("".to_string());
    
    let content = dyn_view(move || 
        match active_page.get() {
        Page::Home      => container(home_view(active_page, set_prev_page, set_epub_path, set_library_path)),
        Page::Library   => container(library_view(library_path, active_page, set_prev_page, set_epub_path, set_library_path)),
        Page::Reader    => container(create_epub_reader(epub_path.get().as_str(), active_page, prev_page.get())).style(move |s| s.flex_grow(1.0)),
    }).style(move |s| s.width_full().height_full().flex_grow(1.0));
    
    content
}