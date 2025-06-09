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
use crate::IO::epub::get_epub;
use crate::IO::home::{create_libraries, delete_library, get_libraries, Library};
use crate::library::{Page, Signals};
use crate::library::components::create_label;

pub fn home_view(signals: Signals) -> impl View {
    let (libraries, set_libraries) = create_signal(get_libraries());
    let stack = dyn_stack(move || libraries.get(),
    move |library| library.clone(),
    move |library| create_view(library.path, library.book_paths, signals.clone()) )
        
        .style(|s| s
            .flex_row()
            .flex_wrap(FlexWrap::Wrap)
            .justify_start()
            .gap(40.0)
            .margin(40)
            .align_content(Some(AlignContent::Center))
            .height_full()
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
    let add_button = button(label(move || "Add Library"))
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
            .padding_horiz(16)
            .padding_vert(10)
            .font_weight(Weight::MEDIUM)
            .hover(|s| s.background(Color::rgb8(37, 99, 235)))
        );

    let settings_button = button(label(move || "Settings"))
        .style(move |s| s
            .background(Color::rgb8(243, 244, 246))
            .color(Color::rgb8(55, 65, 81))
            .border_radius(8)
            .padding_horiz(16)
            .padding_vert(10)
            .font_weight(Weight::MEDIUM)
            .hover(|s| s.background(Color::rgb8(229, 231, 235)))
        );

    h_stack((add_button, settings_button))
        .style(move |s| s
            .justify_content(Some(JustifyContent::SpaceBetween))
            .align_items(Some(AlignItems::Center))
            .padding(20)
            .background(Color::WHITE)
            .border_bottom(1)
            .border_color(Color::rgb8(229, 231, 235))
        )
}

fn create_view(path: String, books: Vec<String>, signals: Signals) -> impl IntoView {
    let background  = Color::rgb8(252, 252, 252);
    let box_shadow  = Color::rgba8(0, 0, 0, 15);
    let card_border = Color::rgb8(230, 230, 230);
    let name        = path.split("/").last().unwrap().to_string();

    let c_path = path.clone();
    let library_path = path.clone();
    
    let name_label = create_label(name, 22)
        .style(move |s| s.font_weight(Weight::MEDIUM))
        .on_click(move |s| {
            signals.prev_page.set(signals.active_page.get());
            signals.library_path.set(path.clone());
            signals.root_library_path.set(path.clone());
            signals.active_page.update(|p| *p = Page::Library);
            EventPropagation::Continue
        })
        .style(move |s| s.justify_center());


    let hamburger_button = create_label(":".to_string(), 24).on_click(move |s| {
        delete_library(&(c_path.clone()));
        EventPropagation::Continue
    });

    let header = h_stack((name_label, hamburger_button)).style(move |s| s.justify_center().margin_top(0).margin_bottom(20));

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
        .border_radius(12.0)
        .padding(20.0)
        .width(660).height(360)
        .border(1)
        .border_color(card_border)
        .box_shadow_blur(16)
        .box_shadow_color(box_shadow)
        .box_shadow_spread(0)
        .box_shadow_h_offset(0)
        .box_shadow_v_offset(4)))
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

