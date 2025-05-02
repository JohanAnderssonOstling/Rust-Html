use std::fmt::Alignment;
use std::io::BufRead;

use floem::{IntoView, View};
use floem::action::open_file;
use floem::event::EventPropagation;
use floem::file::{FileDialogOptions, FileInfo};
use floem::prelude::{button, Color, container, create_signal, Decorators, h_stack, label, RwSignal, ScrollExt, SignalGet, SignalUpdate, stack_from_iter, StackExt, v_stack, ViewTuple};
use floem::reactive::WriteSignal;
use floem::style::{AlignContent, CursorStyle, Display, FlexWrap, Style};
use floem::views::dyn_stack;
use lightningcss::properties::Property::AlignItems;
use lightningcss::properties::PropertyId::JustifyContent;
use crate::IO::epub::get_epub;
use crate::IO::home::{create_libraries, delete_library, get_libraries, Library};
use crate::library::{Page, Signals};


pub fn home_view(signals: Signals) -> impl View {
    let (libraries, set_libraries) = create_signal(get_libraries());
    let stack = dyn_stack(move || libraries.get(),
    move |library| library.clone(),
    move |library| create_view(library.path, library.book_paths, signals.clone()) )
        
        .style(|s| s
            .flex_row()
            .flex_wrap(FlexWrap::Wrap)
            .justify_start()
            .gap(20.0)
            .margin(20)
            .align_content(Some(AlignContent::Center))
            
            
                        //.justify_content(Some(JustifyContent::Center))
            //.display(Display::Flex)
            .height_full()
            .flex_grow(1.0)

    );

    let add_button = button(label(move || "Add library"))
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
        });
    let library_stack = v_stack((stack,)).scroll().style(move |s| s.height_full().flex_grow(1.0));
    v_stack((add_button, library_stack))
}

fn display_files(file: FileInfo) -> String {
    let paths: Vec<&str> = file.path.iter().filter_map(|p| p.to_str()).collect();
    paths.join("\n")
}

fn create_view(path: String, books: Vec<String>, signals: Signals) -> impl IntoView {
    let background  = Color::WHITE;
    let hover_color = Color::parse("#f0f0f0").unwrap();
    let box_shadow  = Color::rgba8(0, 0, 0, 25);
    let name        = path.split("/").last().unwrap().to_string();
    let header_style = Style::new()
        .font_size(24)
        .margin_bottom(10)
        .text_ellipsis()
        .hover(|s| s.background(hover_color));

    let c_path = path.clone();
    let library_path = path.clone();

    let c_header_style = header_style.clone();
    let name_label = label(move || format!("📚 {}", name.clone())).on_click(move |s| {
        signals.prev_page.set(signals.active_page.get());
        signals.library_path.set(path.clone());
        signals.root_library_path.set(path.clone());
        signals.active_page.update(|p| *p = Page::Library);
        EventPropagation::Continue
    }).style(move |s| {header_style.clone().justify_center()});


    let hamburger_button = label(move || ":").on_click(move |s| {
        delete_library(&(c_path.clone()));
        EventPropagation::Continue
    }).style(move |s| {c_header_style.clone()});

    let header = h_stack((name_label, hamburger_button)).style(move |s| s.justify_center());

    let book_list = container(stack_from_iter(books.into_iter().take(8)
        .map(|book| create_book_item(book, library_path.clone(), signals.clone())))
        .style(move |s| s
            .flex_col()
            .width(630)
            .height(250)
            .flex_grow(0.)
        ));

    container(v_stack((
        header,
        book_list.into_view(),
    )).style(move |s| s.background(background)
        .border_radius(8.0)
        .padding(15.0)
        .width(660).height(360)
        .box_shadow_blur(2).box_shadow_color(box_shadow).box_shadow_spread(1)
        .border_color(Color::BLACK)
        .border(2)
        .box_shadow_blur(8).box_shadow_color(box_shadow).box_shadow_spread(0)
        .box_shadow_h_offset(6)

        .box_shadow_v_offset(12)))
}



fn create_book_item(book: String, library_path: String, signals: Signals) -> impl IntoView {
    let hover_color     = Color::parse("#f0f0f0").unwrap();
    let border_color    = Color::parse("#dddddd").unwrap();
    let book_name       = get_epub(&book);
    label(move || format!("📖 {book_name}"))
        .style(move |s| s//.padding(15.0)
            .padding_bottom(10)
            .padding_top(10)
            .border_bottom(1)
            .border_color(border_color)
            .cursor(CursorStyle::Pointer)
            .text_ellipsis()
            .font_size(16)
            .hover(|s| s.background(hover_color)))
        .on_click(move |click| {
            signals.epub_path.set(book.clone());
            signals.library_path.set(library_path.clone());
            signals.root_library_path.set(library_path.clone());
            signals.prev_page.set(signals.active_page.get());
            signals.active_page.set(Page::Reader);
            EventPropagation::Continue
        })
}

