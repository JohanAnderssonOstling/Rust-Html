use std::mem::MaybeUninit;
use floem_renderer::text::{AttrsList, TextLayout};
use rustc_data_structures::fx::FxHashMap;

pub struct GlyphCache {
    small_cache: [MaybeUninit<TextLayout>; 100],
    initialized: [bool; 100],
    large_cache: FxHashMap<char, TextLayout>
}

impl GlyphCache {
    pub fn new() -> Self {
        Self {
            small_cache: unsafe { MaybeUninit::uninit().assume_init() }, // No initialization cost
            initialized: [false; 100],
            large_cache: FxHashMap::default(),
        }
    }
    pub fn get_or_insert(&mut self, char: char, attrs_list: &AttrsList) -> &TextLayout {
        let char_index = char as u32;
        if char_index >= 32 && char_index <= 126 {
            let index = (char_index - 32) as usize;
            if self.initialized[index] {
                return unsafe { self.small_cache[index].assume_init_ref() };
            }
            let mut new_layout = TextLayout::new();
            new_layout.set_text(&char.to_string(), attrs_list.clone());

            self.small_cache[index] = MaybeUninit::new(new_layout);
            self.initialized[index] = true;

            return unsafe { self.small_cache[index].assume_init_ref() };
        }
        self.large_cache.entry(char).or_insert_with(|| {
            let mut layout = TextLayout::new();
            layout.set_text(&char.to_string(), attrs_list.clone());
            layout
        })
    }

    pub fn get(&self, char: char) -> Option<&TextLayout>{
        let char_index = char as u32;
        if char_index >= 32 && char_index <= 126 {
            let index = (char_index - 32) as usize;
            if self.initialized[index] {
                return unsafe { Some(self.small_cache[index].assume_init_ref()) };
            }
            return None;
        }
        self.large_cache.get(&char)
    }
}