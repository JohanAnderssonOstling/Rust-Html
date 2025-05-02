use floem_renderer::text::{Attrs, AttrsList, Style, TextLayout, Weight};
use rustc_data_structures::fx::FxHashMap;
use crate::book_elem::ParseState;

pub struct GlyphCache {
    table: FxHashMap<(char, u8, u16, floem_renderer::text::Style), u16>,
    reverse: Vec<TextLayout>
}

impl GlyphCache {
    pub fn new() -> Self {
        Self {table: FxHashMap::default(), reverse: Vec::with_capacity(100)}
    }
    pub fn get_or_insert(&mut self, char: char, mut font: Attrs, parse_state: &ParseState) -> (&TextLayout, u16) {
        let font_size = font.font_size as u8;
        font = font.raw_weight(parse_state.font_weight);
        font = font.style(parse_state.text_style);
        
        
        let index = self.table.entry((char, font_size, parse_state.font_weight, parse_state.text_style)).or_insert_with(|| {
            let index = self.reverse.len() as u16;
            let mut layout = TextLayout::new();
            let mut buf = [0u8; 4];
            let s = char.encode_utf8(&mut buf);
            layout.set_text(s, AttrsList::new(font));
            self.reverse.push(layout);
            index
        });
        (self.reverse.get(*index as usize).unwrap(), *index)
    }
    pub fn get(&self, index: u16) -> &TextLayout{
        self.reverse.get(index as usize).unwrap()
    }
}