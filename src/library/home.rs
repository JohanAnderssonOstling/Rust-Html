use std::fmt::format;
use std::fs;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Duration;
use floem::prelude::{button, Color, container, Decorators, dyn_stack, h_stack, label, RwSignal, ScrollExt, SignalGet, SignalUpdate, stack, stack_from_iter, StackExt, v_stack, ViewTuple};
use floem::{IntoView, View};
use floem::style::{AlignContent, AlignItems, CursorStyle, Display, FlexWrap, FontWeight, JustifyContent, Transition};
use walkdir::WalkDir;
use csv::Reader;
use floem::event::EventPropagation;
use floem::reactive::WriteSignal;
use floem_renderer::text::Weight;
use crate::library::epub::get_epub;
use crate::library::Page;

pub fn library_view(set_active_page: WriteSignal<Page>) -> impl View {
    //create_book_item("Hello".to_string())
    //home_view(set_active_page)
    println!("Creating library");
    label(move || {"Hello"})
        .on_click(move |s| {
            set_active_page.set(Page::Home);
            EventPropagation::Continue
        }).into_view()
    

}

pub fn reader_view(set_active_page: WriteSignal<Page>) -> impl IntoView {
    //create_view("Library", vec!["Book".to_string()], set_active_page.clone())
    //create_book_item("Hello".to_string())
    label(move || {"Hello"})

}

pub fn home_view(set_active_page: WriteSignal<Page>, set_epub_path: WriteSignal<String>) -> impl View {
    let libraries = get_libraries();
    let stack = stack_from_iter(libraries.into_iter()
        .map(|library| create_view(library.path.split("/").last().unwrap(), library.book_paths, set_active_page, set_epub_path)))
        .style(|s| s.flex_row().flex_wrap(FlexWrap::Wrap)
            .justify_start()
            .gap(20.0)
            .margin(20)
            .align_content(Some(AlignContent::FlexStart))
            //.justify_content(Some(JustifyContent::Center))
            .display(Display::Flex));
    let centered_container = container(stack)
        .style(|s| s.height_full().justify_center());
    centered_container.into_view()
}

fn create_view(text: &str, books: Vec<String>, set_active_page: WriteSignal<Page>, set_epub_path: WriteSignal<String>) -> impl IntoView {
    let background = Color::WHITE;
    let hover_color = Color::parse("#f0f0f0").unwrap();
    let box_shadow = Color::rgba8(0, 0, 0, 25);
    let text = text.to_string();
    let name_label = label(move || format!("📁 {}", text.clone())).on_click(move |s| {
        println!("Change library");
        set_active_page.update(|p| *p = Page::Library);
        EventPropagation::Continue
    })
        .style(move |s| s.font_size(24)
            .font_weight(Weight::NORMAL)
            .margin_bottom(10.0)
            .hover(|s| s.background(hover_color)));
    let book_list = stack_from_iter(books.into_iter()
        .map(|book| create_book_item(book, set_active_page, set_epub_path)))
        .style(move |s| s.flex_col());
    
    container(v_stack((
        name_label,
        book_list.into_view(),
    )).style(move |s| s.background(background).border(0).border_radius(8.0).padding(15.0)
            .width(660).height(330).box_shadow_blur(2).box_shadow_color(box_shadow).box_shadow_spread(1)))
}

fn create_book_item(book: String, set_active_page: WriteSignal<Page>, set_epub_path: WriteSignal<String>) -> impl IntoView {
    let hover_color = Color::parse("#f0f0f0").unwrap();
    let border_color = Color::parse("#dddddd").unwrap();
    let book_name = get_epub(&book);
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
            set_active_page.set(Page::Reader);
            EventPropagation::Continue
        })
}

const CSV_PATH: &str = "/home/johan/.local/share/bookrium/home.csv";

pub struct Library { path: String, book_paths: Vec<String> }

pub fn get_libraries() -> Vec<Library> {
    Reader::from_path(CSV_PATH).unwrap().deserialize()
        .map    (|res: Result<String, _>| res.unwrap())
        .filter (|path| Path::new(path).exists())
        .map    (|path| Library {path: path.clone(), book_paths: get_last_read_books(&path)})
        .collect()
}

pub fn get_last_read_books(library_path: &str) -> Vec<String>{
    let lib_path        = format!("{library_path}/.bookrium");
    let last_read_path  = format!("{lib_path}/last_read.txt");
    fs::create_dir_all(&lib_path).unwrap_or_default();
    let file = OpenOptions::new().read(true).write(true).create(true).open(last_read_path).unwrap();
    BufReader::new(file).lines()
        .map        (|hash| format!("{lib_path}/book_paths/{}.txt", hash.unwrap()))
        .filter     (|path| Path::new(path).exists())
        .filter_map (|path| get_book(path, &library_path)).take(10)
        .collect()
}

fn get_book(path: String, library_path: &str) -> Option<String> {
    let book_path       = fs::read_to_string(&path).unwrap();
    if Path::new(&book_path).exists() {return Some(book_path) }
    let book_name       = book_path.split("/").last().unwrap();
    for entry in WalkDir::new(library_path).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() && path.file_name().unwrap() == book_name {
            return Some(path.display().to_string());
        }
    }
    None
}