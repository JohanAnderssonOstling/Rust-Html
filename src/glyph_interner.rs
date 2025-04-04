use floem_renderer::text::{Attrs, AttrsList, TextLayout, Weight};
use rustc_data_structures::fx::FxHashMap;
use crate::book_elem::ParseState;

pub struct GlyphCache {
    table: FxHashMap<(char, u8, u16), u16>,
    reverse: Vec<TextLayout>
}

impl GlyphCache {
    pub fn new() -> Self {
        Self {table: FxHashMap::default(), reverse: Vec::new()}
    }
    pub fn get_or_insert(&mut self, char: char, mut font: Attrs, parse_state: ParseState) -> (&TextLayout, u16) {
        let font_size = font.font_size as u8;
        font = font.raw_weight(parse_state.font_weight);
        let index = self.table.entry((char, font_size, parse_state.font_weight)).or_insert_with(|| {
            let index = self.reverse.len() as u16;
            let mut layout = TextLayout::new();

            layout.set_text(&char.to_string(), AttrsList::new(font));
            self.reverse.push(layout);
            index
        });
        (self.reverse.get(*index as usize).unwrap(), *index)
    }
    pub fn get(&self, index: u16) -> &TextLayout{
        self.reverse.get(index as usize).unwrap()
    }
}