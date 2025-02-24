use std::path::Display;
use floem::{IntoView, View};
use floem::event::EventPropagation;
use floem::peniko::Color;
use floem::prelude::{button, Decorators, h_stack, label, RwSignal, ScrollExt, SignalGet, SignalUpdate, v_stack};
use floem::reactive::{ReadSignal, WriteSignal};
use floem::style::{CursorStyle, FlexWrap};
use floem::views::{dyn_view, img, stack, stack_from_iter};
use image::DynamicImage;
use rayon::*;
use rayon::iter::ParallelIterator;
use rayon::prelude::IntoParallelRefIterator;
use crate::IO::epub::{Book, get_book_cover};
use crate::IO::library::get_library;
use crate::library::{Page, Signals};

pub fn library_view(signals: Signals) -> impl View{
    let root_library_path = signals.root_library_path.get_untracked();
    //signals.root_library_path.set((root_library_path.clone()));
    let back_button = button(label(move || {"Back"}))
        .on_click(move |_| {
            println!("{root_library_path}");
            let library_path = signals.library_path.get();
            if library_path.eq(&root_library_path) {
                signals.active_page.set(Page::Home);
                return EventPropagation::Continue
            }
            let mut path: Vec<&str> = library_path.split("/").collect();
            path.pop().unwrap();
            let prev_path = path.join("/");
            signals.library_path.set(prev_path);
            EventPropagation::Continue
        });
    let top_panel = h_stack((back_button, )).style(move |s| s.height(20).border_bottom(1));

    let main_view = dyn_view(move ||
        dir_view(&signals.library_path.get(), signals.clone())
    );
    v_stack((top_panel, main_view)).style(move |s| s.flex_grow(1.0)).scroll()
}


pub fn dir_view(library_path: &str, signals: Signals) -> impl View{
    let (book_paths, dirs) = get_library(library_path);
    let books: Vec<Book> = book_paths.par_iter()
        .map(|book_path| get_book_cover(book_path))
        .collect();

    let book_stack = stack_from_iter(books.into_iter().zip(book_paths)
        .map(|book_cover| create_book_cover(book_cover.0.title, book_cover.0.cover, book_cover.1, signals.clone()) )
    ).style(move |s| s.gap(20).flex_row().flex_wrap(FlexWrap::Wrap));

    let dir_stack = stack_from_iter(dirs.into_iter()
        .map(|dir| create_dir_cover(dir, signals.library_path)))
        .style(move |s| s.gap(20).flex_row().flex_wrap(FlexWrap::Wrap));

    v_stack((book_stack, dir_stack)).style(move |s| s.margin(20))
        .into_view()
}

fn create_book_cover(title: String, cover: Option<Vec<u8>>, path: String, signals: Signals) -> impl View {
    let title_label = label(move || title.clone()).style(|s| s
        .width(300).font_size(16).text_ellipsis());
    let cover_image = img(move || cover.clone().unwrap())
        .on_click(move |s| {
            signals.prev_page.set(Page::Library);
            signals.epub_path.set(path.clone());
            signals.active_page.set(Page::Reader);
            EventPropagation::Continue
        })
        .style(move |s| s.width(300).height(500)
            .cursor(CursorStyle::Pointer)
            .border_radius(15));
    v_stack((cover_image, title_label)).into_view()
}

fn create_dir_cover(dir: String, set_library_path: RwSignal<String>) -> impl View{
    let background = Color::WHITE;
    let box_shadow  = Color::rgba8(0, 0, 0, 25);
    let name = dir.split("/").last().unwrap().to_string();
    let name_label = label(move || format!("📁 {}", name.clone()))
        .on_click(move |s| {
            set_library_path.set(dir.clone());
            EventPropagation::Continue
        })
        .style(move |s| s
        .border_radius(8.0)
        .font_size(16)
        .padding(15)
        .background(background)
        .box_shadow_blur(2).box_shadow_color(box_shadow).box_shadow_spread(1));
    name_label.into_view()
}