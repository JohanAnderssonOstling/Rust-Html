use std::io::BufRead;

use floem::{IntoView, View};
use floem::event::EventPropagation;
use floem::prelude::{Color, container, Decorators, label, RwSignal, ScrollExt, SignalGet, SignalUpdate, stack_from_iter, StackExt, v_stack, ViewTuple};
use floem::reactive::WriteSignal;
use floem::style::{AlignContent, CursorStyle, Display, FlexWrap, Style};

use crate::IO::epub::get_epub;
use crate::IO::home::get_libraries;
use crate::library::Page;


pub fn home_view(set_active_page: RwSignal<Page>, set_prev_page: WriteSignal<Page>, set_epub_path: WriteSignal<String>, set_library_path: WriteSignal<String>) -> impl View {
    let libraries = get_libraries();
    let stack = stack_from_iter(libraries.into_iter()
        .map(|library| create_view(library.path, library.book_paths, set_active_page, set_prev_page, set_epub_path, set_library_path)))
        .style(|s| s.flex_row().flex_wrap(FlexWrap::Wrap)
            .justify_start()
            .gap(20.0)
            .margin(20)
            .align_content(Some(AlignContent::FlexStart))
            //.justify_content(Some(JustifyContent::Center))
            .display(Display::Flex));
    let centered_container = container(stack)
        .style(|s| s.justify_center()).scroll();
    centered_container.into_view()
}

fn create_view(path: String, books: Vec<String>, set_active_page: RwSignal<Page>, set_prev_page: WriteSignal<Page>, set_epub_path: WriteSignal<String>, set_library_path: WriteSignal<String>) -> impl IntoView {
    let background  = Color::WHITE;
    let hover_color = Color::parse("#f0f0f0").unwrap();
    let box_shadow  = Color::rgba8(0, 0, 0, 25);
    let name        = path.split("/").last().unwrap().to_string();
    let header_style = Style::new()
        .font_size(24)
        .margin_bottom(10)
        .hover(|s| s.background(hover_color));
    
    let name_label = label(move || format!("📚 {}", name.clone())).on_click(move |s| {
        set_prev_page.set(set_active_page.get());
        set_library_path.set(path.clone());
        set_active_page.update(|p| *p = Page::Library);
        EventPropagation::Continue
    }).style(move |s| {header_style.clone()});
    
    let book_list = stack_from_iter(books.into_iter()
        .map(|book| create_book_item(book, set_active_page, set_prev_page, set_epub_path)))
        .style(move |s| s.flex_col());

    container(v_stack((
        name_label,
        book_list.into_view(),
    )).style(move |s| s.background(background)
        .border_radius(8.0)
        .padding(15.0)
        .width(660).height(330)
        .box_shadow_blur(2).box_shadow_color(box_shadow).box_shadow_spread(1)))
}

fn create_book_item(book: String, set_active_page: RwSignal<Page>, set_prev_page: WriteSignal<Page>, set_epub_path: WriteSignal<String>) -> impl IntoView {
    let hover_color     = Color::parse("#f0f0f0").unwrap();
    let border_color    = Color::parse("#dddddd").unwrap();
    let book_name       = get_epub(&book);
    label(move || format!("📖 {book_name}"))
        .style(move |s| s.padding(5.0)
            .border_bottom(1)
            .border_color(border_color)
            .cursor(CursorStyle::Pointer)
            .text_ellipsis()
            .font_size(16)
            .hover(|s| s.background(hover_color)))
        .on_click(move |click| {
            set_epub_path.set(book.clone());
            set_prev_page.set(set_active_page.get());
            set_active_page.set(Page::Reader);
            EventPropagation::Continue
        })
}

