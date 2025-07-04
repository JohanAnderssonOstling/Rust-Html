use floem::peniko::Color;
use floem::prelude::{button, Button, Decorators};
use floem::style::{AlignContent, StyleValue};
use floem::views::{Label, label};
use floem_renderer::text::Weight;

const BACKGROUND_COLOR: Color =  Color::WHITE;
const HOVER_COLOR: Color = Color {r: 240, g: 240, b: 240, a: 255};
const Text_COLOR: Color = Color {r: 43, g: 43, b: 43, a: 255};

pub fn create_label(text: String, font_size: i32) -> Label {
    label(move || text.clone()).style(move |s| s 
        .hover(|s| s.background(HOVER_COLOR))
        .text_ellipsis()
        .font_size(font_size)
        .font_family("Liberation Serif".to_string())
        .font_weight(Weight::LIGHT)
        .color(Text_COLOR)
    )
}

pub fn label_style(label: Label, font_size: i32) -> Label {
    label.style(move |s| s
        .hover(|s| s.background(HOVER_COLOR))
        .text_ellipsis()
        .font_size(font_size))
}

pub fn create_square_button(text: String) -> Button {
    button(label(move || text.clone())).style(move |s| s
        .hover(|s| s.background(HOVER_COLOR))
        .text_ellipsis()
        .font_size(20)
        .width(36)
        .height(36)
        .border_radius(8)
        .justify_center()
        .align_content(AlignContent::Center)
    )
}