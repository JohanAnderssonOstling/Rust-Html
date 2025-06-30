
use floem::{IntoView, View};
use floem::views::Decorators;
use rbook::Ebook;

use crate::library::page_navigation_view;


mod epub_reader;
mod book_elem;
mod glyph_cache;
mod library;
pub mod IO;
mod css;
mod style;
mod glyph_interner;
mod layout;
mod parser;
mod toc;
mod table_parser;
mod pre_parser;
mod renderer;
mod arena;
mod book_elem_arena;

fn app_view() -> impl View {
    /*let mut epub_renderer = EpubReader::new("/home/johan/Hem/Downloads/A Concise History of Switzerland.epub");
    epub_renderer = epub_renderer.style(move |style| {style.width_full()});
    epub_renderer.into_view()*/
    page_navigation_view()
}

fn main() {
    floem::launch(app_view);
}
