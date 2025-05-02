use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use std::vec;

use floem::kurbo::{Point, Size};
use floem::peniko::Image;
use floem_renderer::text::Attrs;
use lightningcss::properties::text::TextAlign;
use lightningcss::stylesheet::StyleSheet;
use regex::Regex;
use roxmltree::Node;
use rustc_data_structures::fx::FxHashMap;
use sha2::Digest;

use crate::glyph_interner::GlyphCache;
use crate::layout::layout_elem_lines;
use crate::style::resolve_style;

static BLOCK_ELEMENTS: [&str; 37] = [
    "html", "body", "article", "section", "nav", "aside",
    "h1", "h2", "h3", "h4", "h5", "h6", "hgroup", "header",
    "footer", "address", "p", "hr", "pre", "blockquote",
    "ol", "ul", "menu", "li", "dl", "dt", "dd", "figure",
    "figcaption", "main", "div", "table", "form", "fieldset",
    "legend", "details", "summary"
];

impl BlockElem {
    pub fn add_child (&mut self, elem: Elem) {
        match &elem.elem_type {
            ElemType::Block(block) => { self.total_child_count += block.total_child_count; }
            ElemType::Lines(_) => {self.total_child_count += 1}
        }
        self.children.push(elem);
    }
}

impl Elem {
    pub fn get_elem (&self, index: &Vec<usize>, level: usize) -> &Elem{
        return match &self.elem_type {
            ElemType::Block(block) => {
                if index.len() == level {return self}
                let curr_index = index[level];
                if curr_index >= block.children.len() {return self}
                let elem = &(block.children[curr_index]);
                //if index.len() <= level { return elem; }
                elem.get_elem(index, level + 1)
            }
            ElemType::Lines(_) => { self }
        }
    }
    pub fn get_last_index (&self) -> Vec<usize> {
        let mut indexes = Vec::new();
        match &self.elem_type {
            ElemType::Block(block) => {
                if block.children.len() == 0 {return Vec::new()}
                let index = block.children.len() - 1;
                indexes.push(index);
                let elem = &block.children[index];
                indexes.append(&mut elem.get_last_index());
                indexes
            }
            ElemType::Lines(_) => {
                Vec::new()
            }
        }
    }
    pub fn get_y(&self, elem_index: usize) -> f64 {
        let mut y = self.point.y;
        let mut current_elem_index = 0;
        match &self.elem_type {
            ElemType::Block(_) => y,
            ElemType::Lines(lines) => {
                for line in &lines.elem_lines {
                    if elem_index < current_elem_index + line.inline_elems.len() {
                        break
                    }
                    current_elem_index += line.inline_elems.len();
                    y += line.height;
                }
                y
            }
        }
    }
}
pub type ImagePromise = Arc<RwLock<Option<(Image, Vec<u8>)>>>;
pub struct HTMLPage { pub root: Elem, pub locations: FxHashMap<String, Vec<usize>>}
pub struct Elem             { pub size: Size, pub point: Point, pub elem_type: ElemType }
pub enum ElemType           { Block(BlockElem), Lines(ElemLines) }
pub struct BlockElem        { pub children: Vec<Elem>, pub total_child_count: usize, }
pub struct ElemLines        { pub height: f64, pub elem_lines: Vec<ElemLine> }
pub struct ElemLine         { pub height: f64, pub inline_elems: Vec<InlineElem> }
pub struct InlineElem       { pub x: f64, pub inline_content: InlineContent }

pub struct InlineItem       { pub size: Size, pub inline_content: InlineContent }
pub enum InlineContent      { Text(Vec<CharGlyph>), Image(ImageElem), Link((Vec<CharGlyph>, String)) }
pub struct CharGlyph        { pub char: u16, pub x: f32}
#[derive(Clone)]
pub struct ImageElem { pub width: u32, pub height: u32, pub image_promise: ImagePromise}
pub struct BookElemFactory  { 
    pub curr_x: f64, 
    pub curr_y: f64,
    base_path: String,
    pub cache: GlyphCache,
    pub images: HashMap<String, ImageElem>,
    pub locations: FxHashMap<String, Vec<usize>>,
    pub root_font_size: f32,
    pub style_time: u128,
}
#[derive(Clone, Copy)]
pub struct ParseState {
    pub x: f64,
    pub width: f64,
    pub font_weight: u16,
    pub text_align: TextAlign,
    pub text_style: floem_renderer::text::Style,
    pub root_font_size: f32,
}

impl BookElemFactory {
    pub fn new(cache: GlyphCache, images: HashMap<String, ImageElem>, font: &Attrs) -> Self {
        BookElemFactory { curr_x: 0., curr_y: 0., cache, images, base_path: String::new(), locations: FxHashMap::default(), root_font_size: font.font_size, style_time: 0 }
    }

    pub fn parse_root(&mut self, node: Node, font: Attrs, file_path: String, style_sheets: &Vec<StyleSheet>) -> HTMLPage {
        self.curr_x = 0.;
        self.curr_y = 0.;
        self.base_path = file_path;
        let parse_state = ParseState { x: 0., width: 600., font_weight: 400, text_align: TextAlign::Left, root_font_size: font.font_size, text_style: floem_renderer::text::Style::Normal };

        for child in node.children() {
            if child.tag_name().name().eq("body") {
                let block = self.parse(child, font, style_sheets, parse_state, vec![0]);
                let block_type = BlockElem { children: vec![block], total_child_count: 1 };
                let root = Elem { size: Size::default(), point: Point::default(), elem_type: ElemType::Block(block_type) };
                return HTMLPage { root, locations: self.locations.clone() }
            }
        }
        let elem_lines = ElemLines { height: 0., elem_lines: Vec::new() };
        let root = Elem { size: Size::default(), point: Point::default(), elem_type: ElemType::Lines(elem_lines) };
        return HTMLPage { root, locations: FxHashMap::default() }
    }

    pub fn parse(&mut self, node: Node, mut font: Attrs, style_sheets: &Vec<StyleSheet>, mut parse_state: ParseState, mut index: Vec<usize>) -> Elem {
        let mut block_elem = BlockElem { children: Vec::new(), total_child_count: 0 };
        let mut inline_items: Vec<InlineItem> = Vec::new();
        let init_point = Point::new(self.curr_x, self.curr_y);
        let now = Instant::now();
        let (margins, mut parse_state) = resolve_style(style_sheets, &node, &mut font, parse_state);
        self.style_time += (Instant::now() - now).as_nanos();
        parse_state.width -= margins.left + margins.right;
        parse_state.x += margins.left / 2.;
        self.curr_x = parse_state.x;
        self.curr_y += margins.top;
        index.push(0);
        if let Some(id) = node.attribute("id") {
            self.locations.insert(id.to_string(), index.clone());
        }
        if node.tag_name().name().eq("li") { inline_items.extend(self.parse_text("- ", font, parse_state, None)); }
        for child in node.children() {
            let tag_name = child.tag_name().name();

            if BLOCK_ELEMENTS.contains(&tag_name) {
                
                if inline_items.len() != 0 {
                    block_elem.add_child(layout_elem_lines(self, inline_items, parse_state));
                    *index.last_mut().unwrap() += 1;
                    inline_items = Vec::new();
                }
                
                if tag_name.eq("pre") { block_elem.add_child(self.parse_pre(child, font, style_sheets, parse_state, index.clone())); } 
                else { block_elem.add_child(self.parse(child, font, style_sheets, parse_state, index.clone())); }

                *index.last_mut().unwrap() += 1;
            } 
            else if tag_name.eq("") { inline_items.extend(self.parse_text(child.text().unwrap_or_default(), font, parse_state, None)); } 
            else if tag_name.eq("img") { inline_items.push(self.parse_img(child, &index, parse_state)); } 
            else if tag_name.eq("br") {
                block_elem.add_child(layout_elem_lines(self, inline_items, parse_state));
                inline_items = Vec::new();
                *index.last_mut().unwrap() += 1;
            } else if tag_name.eq("a") {
                if let Some(href) = child.attribute("href") { inline_items.extend(self.parse_inline(child, style_sheets, font, parse_state, Some(href), &index)) } else { inline_items.extend(self.parse_inline(child, style_sheets, font, parse_state, None, &index)) }
            } else { inline_items.extend(self.parse_inline(child, style_sheets, font, parse_state, None, &index)); }
            // println!("Tag name: {}", tag_name);
        }
        if inline_items.len() != 0 {
            block_elem.add_child(layout_elem_lines(self, inline_items, parse_state));
            *index.last_mut().unwrap() += 1;
        }
        self.curr_y += margins.bottom;
        let block_height = block_elem.children.iter().fold(0., |acc, elem| acc + elem.size.height);
        Elem { size: Size::new(600., block_height + margins.top + margins.bottom), point: init_point, elem_type: ElemType::Block(block_elem) }
    }

    pub fn parse_inline(&mut self, node: Node, style_sheets: &Vec<StyleSheet>, mut font: Attrs, parse_state: ParseState, href: Option<&str>, index: &Vec<usize>) -> Vec<InlineItem> {
        let mut inline_items: Vec<InlineItem> = Vec::new();
        let now = Instant::now();
        let (_, parse_state) = resolve_style(style_sheets, &node, &mut font, parse_state);
        self.style_time += (Instant::now() - now).as_nanos();
        if let Some(id) = node.attribute("id") {
            self.locations.insert(id.to_string(), index.clone());
        }
        for child in node.children() {
            if child.tag_name().name().eq("") { inline_items.extend(self.parse_text(child.text().unwrap_or_default(), font, parse_state, href)); } else if child.has_tag_name("a") {
                if let Some(href) = child.attribute("href") { inline_items.extend(self.parse_inline(child, style_sheets, font, parse_state, Some(href), index)) }
            } else { inline_items.extend(self.parse_inline(child, style_sheets, font, parse_state, href, index)); }
        }
        inline_items
    }

    pub fn parse_pre(
        &mut self,
        node: Node,
        mut font: Attrs,
        style_sheets: &Vec<StyleSheet>,
        parse_state: ParseState,
        index: Vec<usize>,
    ) -> Elem {
        let init_point = Point::new(self.curr_x, self.curr_y);
        let mut lines: Vec<ElemLine> = Vec::new();
        let mut current_line: Vec<InlineElem> = Vec::new();
        let mut x = 0.0;
        let mut max_height = 0.0;

        let now = Instant::now();
        let (_, parse_state) = resolve_style(style_sheets, &node, &mut font, parse_state);
        self.style_time += (Instant::now() - now).as_nanos();

        fn recurse_pre<'a>(
            factory: &mut BookElemFactory,
            node: Node<'a, 'a>,
            font: Attrs,
            style_sheets: &Vec<StyleSheet>,
            parse_state: ParseState,
            x: &mut f64,
            max_height: &mut f64,
            current_line: &mut Vec<InlineElem>,
            lines: &mut Vec<ElemLine>,
        ) {
            if let Some(text) = node.text() {
                for line in text.split_inclusive('\n') {
                    for ch in line.chars() {
                        if ch == '\n' {
                            lines.push(ElemLine {
                                height: *max_height,
                                inline_elems: std::mem::take(current_line),
                            });
                            *x = 0.0;
                            *max_height = 0.0;
                            continue;
                        }
                        let (text_layout, index) = factory.cache.get_or_insert(ch, font, &parse_state);
                        *max_height = max_height.max(text_layout.size().height);
                        current_line.push(InlineElem {
                            x: *x,
                            inline_content: InlineContent::Text(vec![CharGlyph { char: index, x: 0. }]),
                        });
                        *x += text_layout.size().width;
                    }
                }
            } else {
                for child in node.children() {
                    if child.is_element() {
                        recurse_pre(factory, child, font, style_sheets, parse_state, x, max_height, current_line, lines);
                    }
                }
            }
        }

        recurse_pre(
            self,
            node,
            font,
            style_sheets,
            parse_state,
            &mut x,
            &mut max_height,
            &mut current_line,
            &mut lines,
        );

        // Add final line if needed
        if !current_line.is_empty() {
            lines.push(ElemLine {
                height: max_height,
                inline_elems: current_line,
            });
        }

        let total_height = lines.iter().map(|l| l.height).sum::<f64>();
        self.curr_y += total_height;

        Elem {
            size: Size::new(parse_state.width, total_height),
            point: init_point,
            elem_type: ElemType::Lines(ElemLines { height: total_height, elem_lines: lines }),
        }
    }

    pub fn parse_text(&mut self, text: &str, font: Attrs, parse_state: ParseState, href: Option<&str>) -> Vec<InlineItem> {
        let mut inline_items: Vec<InlineItem> = Vec::new();
        //if node.text().is_none() { return Vec::new(); }
        if text.eq("") {return Vec::new()}
        if text.eq("\n") { return Vec::new(); }

        // Instead of split_whitespace, we manually segment the text.
        // Each segment is a tuple where the first element is the segment string
        // and the second element is a boolean indicating if it's composed of whitespace.
        let mut segments: Vec<String> = Vec::new();
        let mut current_segment = String::new();
        let mut current_is_space: Option<bool> = None;
        let mut only_whitespace = true;
        for ch in text.chars() {
            let is_space = ch.is_whitespace();
            if !is_space {only_whitespace = false}
            match current_is_space {
                Some(flag) if flag == is_space => {
                    current_segment.push(ch);
                }
                Some(flag) => {
                    segments.push((current_segment.clone()));
                    current_segment.clear();
                    current_segment.push(ch);
                    current_is_space = Some(is_space);
                }
                None => {
                    current_is_space = Some(is_space);
                    current_segment.push(ch);
                }
            }
        }
        if only_whitespace {return Vec::new()}
        if !current_segment.is_empty() {
            segments.push((current_segment));
        }

        // Process each segment individually. Each segment, whether it's a word or a sequence of spaces,
        // will be converted to an InlineItem with the exact glyph layout.
        for segment in segments {
            let mut char_x = 0.;
            let mut segment_height: f64 = 0.;
            let mut char_glyphs = Vec::with_capacity(segment.len());
            for ch in segment.chars() {
                let (text_layout, index) = self.cache.get_or_insert(ch, font, &parse_state);
                char_glyphs.push(CharGlyph { char: index, x: char_x });
                char_x += text_layout.size().width as f32;
                segment_height = segment_height.max(text_layout.size().height);
            }
            let size = Size::new(char_x as f64, segment_height as f64);
            match href {
                None => inline_items.push(InlineItem { size, inline_content: InlineContent::Text(char_glyphs) }),
                Some(href) => inline_items.push(InlineItem { size, inline_content: InlineContent::Link((char_glyphs, href.to_string())) })
            }
        }

        inline_items
    }
    pub fn parse_text5(&mut self, node: Node, font: Attrs, parse_state: ParseState, href: Option<&str>) -> Vec<InlineItem> {
        let mut inline_items: Vec<InlineItem> = Vec::new();

        // Check for no text or just a newline.
        if node.text().is_none() {
            return Vec::new();
        }
        let text = node.text().unwrap();
        if text == "\n" {
            return Vec::new();
        }

        let mut tokens: Vec<String> = Vec::new();
        let mut start_index = 0;

        // Handle leading whitespace as its own token, if present.
        if let Some(first_char) = text.chars().next() {
            if first_char.is_whitespace() {
                let mut whitespace_end = 0;
                for (i, ch) in text.char_indices() {
                    if !ch.is_whitespace() {
                        break;
                    }
                    whitespace_end = i + ch.len_utf8();
                }
                tokens.push(text[0..whitespace_end].to_string());
                start_index = whitespace_end;
            }
        }

        // Use a regex to capture a word and any trailing whitespace together.
        // This pattern matches one or more non-space characters (\S+)
        // followed by zero or more whitespace characters ([\s]*).
        let re = Regex::new(r"\S+[\s]*").unwrap();
        for cap in re.find_iter(&text[start_index..]) {
            tokens.push(cap.as_str().to_string());
        }

        // Process each token to build InlineItems.
        for token in tokens {
            let mut char_x = 0.0;
            let mut token_height: f64 = 0.0;
            let mut char_glyphs = Vec::with_capacity(token.len());
            for ch in token.chars() {
                let (text_layout, index) = self.cache.get_or_insert(ch, font, &parse_state);
                char_glyphs.push(CharGlyph { char: index, x: char_x });
                char_x += text_layout.size().width as f32;
                token_height = token_height.max(text_layout.size().height);
            }
            let size = Size::new(char_x as f64, token_height as f64);
            match href {
                None => inline_items.push(InlineItem { size, inline_content: InlineContent::Text(char_glyphs) }),
                Some(href) => inline_items.push(InlineItem { size, inline_content: InlineContent::Link((char_glyphs, href.to_string())) })
            }
        }

        inline_items
    }

    pub fn parse_text2(&mut self, node: Node, font: Attrs, parse_state: ParseState, href: Option<&str>) -> Vec<InlineItem> {
        let mut inline_items: Vec<InlineItem> = Vec::new();

        if node.text().is_none() { return Vec::new() }
        let text = node.text().unwrap();
        if text.eq("\n") { return Vec::new() }
        if text.eq(" ") {
            let (text_layout, index) = self.cache.get_or_insert(' ', font, &parse_state);
            let mut char_glyphs = Vec::with_capacity(1);
            char_glyphs.push(CharGlyph { char: index, x: 0. });
            let size = text_layout.size();
            match href {
                None => inline_items.push(InlineItem { size, inline_content: InlineContent::Text(char_glyphs) }),
                Some(href) => inline_items.push(InlineItem { size, inline_content: InlineContent::Link((char_glyphs, href.to_string())) })
            }
            return inline_items;
        }
        let has_trailing_space = node.text().unwrap().ends_with(" ");
        let mut word_iter = text.split_whitespace().peekable();
        while let Some(word) = word_iter.next() {
            let mut char_x = 0.;
            let mut word_height: f64 = 0.;
            let chars = word.chars();
            let mut char_glyphs: Vec<CharGlyph> = Vec::with_capacity(word.len());
            for char in chars {
                let (text_layout, index) = self.cache.get_or_insert(char, font, &parse_state);
                char_glyphs.push(CharGlyph { char: index, x: char_x });
                char_x += text_layout.size().width as f32;
                word_height = word_height.max(text_layout.size().height);
            }
            if word_iter.peek().is_some() || has_trailing_space {
                let (text_layout, index) = self.cache.get_or_insert(' ', font, &parse_state);
                char_glyphs.push(CharGlyph { char: index, x: char_x });
                char_x += text_layout.size().width as f32;
            }
            let size = Size::new(char_x as f64, word_height as f64);
            match href {
                None => inline_items.push(InlineItem { size, inline_content: InlineContent::Text(char_glyphs) }),
                Some(href) => inline_items.push(InlineItem { size, inline_content: InlineContent::Link((char_glyphs, href.to_string())) })
            }
        }

        inline_items
    }

    pub fn parse_img(&mut self, node: Node, index: &Vec<usize>, parse_state: ParseState) -> InlineItem {
        if let Some(id) = node.attribute("id") {
            self.locations.insert(id.to_string(), index.clone());
        }
        let relative_path   = node.attribute("src").unwrap();
        let image_path      = resolve_path(&self.base_path, relative_path);
        let image           = self.images.get(&image_path).unwrap();
        let size            = Size::new(image.width as f64, image.height as f64);
        InlineItem {size, inline_content: InlineContent::Image(image.clone())}
    }
}

fn resolve_path(html_path: &str, relative_path: &str) -> String {
    let html_dir = Path::new(html_path).parent().unwrap_or_else(|| Path::new(""));
    let joined = html_dir.join(relative_path);
    let mut normalized_path = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::ParentDir => { normalized_path.pop(); } // Move up a directory
            Component::CurDir => { /* Ignore "." */ }
            _ => normalized_path.push(component),
        }
    }
    normalized_path.to_str().unwrap().to_string()
}

mod tests {
    use crate::book_elem::InlineContent;

    #[test]
    fn test_mem_size() {
        let size = std::mem::size_of::<InlineContent>();
        println!("Size: {size}")
    }
}