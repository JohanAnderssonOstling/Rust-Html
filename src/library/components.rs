use floem::peniko::Color;
use floem::prelude::Decorators;
use floem::views::{Label, label};

const BACKGROUND_COLOR: Color =  Color::WHITE;
const HOVER_COLOR: Color = Color {r: 240, g: 240, b: 240, a: 255};

pub fn create_label(text: String, font_size: i32) -> Label {
    label(move || text.clone()).style(move |s| s 
        .hover(|s| s.background(HOVER_COLOR))
        .text_ellipsis()
        .font_size(font_size)
    )
}

pub fn label_style(label: Label, font_size: i32) -> Label {
    label.style(move |s| s
        .hover(|s| s.background(HOVER_COLOR))
        .text_ellipsis()
        .font_size(font_size))
}