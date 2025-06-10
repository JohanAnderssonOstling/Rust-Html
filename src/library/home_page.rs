use std::fmt::Alignment;
use std::io::BufRead;
use std::time::Duration;

use floem::{IntoView, View};
use floem::action::open_file;
use floem::event::EventPropagation;
use floem::file::{FileDialogOptions, FileInfo};
use floem::prelude::{button, Color, container, create_signal, Decorators, h_stack, label, RwSignal, ScrollExt, SignalGet, SignalUpdate, stack_from_iter, StackExt, v_stack, ViewTuple};
use floem::reactive::WriteSignal;
use floem::style::{AlignContent, AlignItems, CursorStyle, Display, FlexWrap, FontFamily, JustifyContent, Style, TextOverflow};
use floem::views::dyn_stack;
use floem_renderer::text::Weight;
use rayon::iter::IntoParallelRefIterator;
use crate::IO::epub::{get_book_cover, get_epub, Book};
use crate::IO::home::{create_libraries, delete_library, get_libraries, Library};
use crate::library::{Page, Signals};
use crate::library::components::{create_label, create_square_button};
use crate::library::library_page::create_book_cover;

pub fn home_view(signals: Signals) -> impl View {
    let (libraries, set_libraries) = create_signal(get_libraries());
    let stack = dyn_stack(move || libraries.get(),
        move |library| library.clone(),
        move |library| create_library_card(library.path, library.book_paths, signals.clone()))
        
        .style(|s| s
            .flex_row()
            .justify_start()
            .gap(40.0)
            .margin(40)
            .flex_wrap(FlexWrap::Wrap)
            .align_content(Some(AlignContent::Center))
            .height_full()
            //.width_full()
            .flex_grow(1.0)

    );

    let top_bar = create_top_bar(set_libraries);
    let library_stack = v_stack((stack,)).scroll().style(move |s| s.height_full().flex_grow(1.0));
    v_stack((top_bar, library_stack))
}

fn display_files(file: FileInfo) -> String {
    let paths: Vec<&str> = file.path.iter().filter_map(|p| p.to_str()).collect();
    paths.join("\n")
}

fn create_top_bar(set_libraries: WriteSignal<Vec<Library>>) -> impl IntoView {
    let add_button = button(label(move || "+"))
        .on_click(move |s| {
            open_file(FileDialogOptions::new().select_directories().title("Select Folder")
            , move |file_info| {
                    let lib_path = display_files(file_info.unwrap());
                    create_libraries(&lib_path);
                    let lib = Library {path: lib_path, book_paths: Vec::new()};
                    set_libraries.update(move |libraries| {
                        libraries.push(lib);
                    });
                    
                });
            EventPropagation::Continue
        })
        .style(move |s| s
            .background(Color::rgb8(59, 130, 246))
            .color(Color::WHITE)
            .border_radius(8)

            .font_weight(Weight::MEDIUM)
            .hover(|s| s.background(Color::rgb8(37, 99, 235)))
        );

    let settings_button = create_square_button(":".to_string());

    h_stack((add_button, settings_button))
        .style(move |s| s
            .justify_content(Some(JustifyContent::SpaceBetween))
            .align_items(Some(AlignItems::Center))
            .padding_right(40)
            .padding_left(40)
            .font_size(16)
            //.background(Color::WHITE)
            .border_bottom(1)
            .border_color(Color::rgb8(229, 231, 235))
        )
}

fn create_library_card(root_library_path: String, book_paths: Vec<String>, signals: Signals) -> impl IntoView {
    let background  = Color::rgb8(252, 252, 252);
    let box_shadow  = Color::rgba8(0, 0, 0, 15);
    let card_border = Color::rgb8(230, 230, 230);
    let name        = root_library_path.split("/").last().unwrap().to_string();
    let books: Vec<Book> = book_paths.iter().take(10)
        .filter_map(|book_path| get_book_cover(&root_library_path, book_path).ok())
        .collect();
    let c_path = root_library_path.clone();
    let library_path = root_library_path.clone();
    
    let name_label = create_label(name, 22)
        .style(move |s| s.font_weight(Weight::MEDIUM))
        .on_click(move |s| {
            signals.prev_page.set(signals.active_page.get());
            signals.library_path.set(root_library_path.clone());
            signals.root_library_path.set(root_library_path.clone());
            signals.active_page.update(|p| *p = Page::Library);
            EventPropagation::Continue
        })
        .style(move |s| s.justify_center());


    let hamburger_button = create_label(":".to_string(), 24).on_click(move |s| {
        delete_library(&(c_path.clone()));
        EventPropagation::Continue
    });

    let header = h_stack((name_label, hamburger_button)).style(move |s| s.justify_center().margin_top(0).margin_bottom(20));

    let book_list = stack_from_iter(books.into_iter()
        .map(|book_cover| create_book_cover(book_cover.title, book_cover.cover, book_cover.path, signals.clone()) )
    ).style(move |s| s.gap(20))
        .scroll().style(move |s| s);


    v_stack((
        header,
        book_list,
    )).style(move |s| s.background(background)
        .border_radius(8.0)
        .padding(20.0)
        .width(680)
        .height(480)
        .flex_grow(1.0)
        //.width_full()
        .border(1)
        .border_color(card_border)
        .box_shadow_blur(16)

        .box_shadow_color(box_shadow)
        .box_shadow_v_offset(4))
}



fn create_book_item(book: String, library_path: String, signals: Signals) -> impl IntoView {
    let hover_color     = Color::rgb8(248, 250, 252);
    let border_color    = Color::rgb8(226, 232, 240);
    let text_color      = Color::rgb8(64, 64, 64);
    let book_name       = get_epub(&book);
    label(move || format!("{book_name}"))
        .on_click(move |click| {
            signals.epub_path.set(book.clone());
            signals.library_path.set(library_path.clone());
            signals.root_library_path.set(library_path.clone());
            signals.prev_page.set(signals.active_page.get());
            signals.active_page.set(Page::Reader);
            EventPropagation::Continue
        })
            .style(move |s| s
            .padding_bottom(12)
            .padding_top(12)
            .padding_left(8)
            .padding_right(8)
            .border_radius(6)
                .border(1)
                .border_color(Color::rgba8(0,0,0,0))
            .cursor(CursorStyle::Pointer)
            .text_overflow(TextOverflow::Wrap)
            .font_size(15)
            .font_weight(Weight::NORMAL)
            .font_family("Liberation Serif".to_string())
            .hover(|s| s.background(hover_color).border(1).border_color(border_color))
            .color(text_color)

            //.transition_background_color(Duration::from_millis(150)))
        )

}

