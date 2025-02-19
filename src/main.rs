use floem::{IntoView, View, ViewId};
use floem::views::{button, container, Container, Decorators, label};
use rbook::{Ebook, Epub};
use crate::epub_reader::EpubReader;

mod html_renderer;
mod epub_reader;
mod book_elem;
mod css;
mod layout;

fn app_view() -> impl View {
    let mut epub_renderer = EpubReader::new("/home/johan/Hem/Downloads/A Concise History of Switzerland.epub");
    epub_renderer = epub_renderer.style(move |style| {style.width_full()});
    let mut main_container = container(epub_renderer);
    main_container = main_container.style(move |style| {style.width_full()});
    main_container.into_view()
}

fn main() {
    floem::launch(app_view);
}
