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
use roxmltree::{Document, Node, NodeId};
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
pub fn get_size(elem: &Elem, usage: &mut MemUsage) {
    match &elem.elem_type {
        ElemType::Block(block) => {
            for child in &block.children {
                usage.elem_size += std::mem::size_of::<Elem>();
                get_size(child, usage);
            }
        }
        ElemType::Lines(lines) => {
            for line in &lines.elem_lines {
                usage.line_size += std::mem::size_of::<ElemLine>();
                for inline in &line.inline_elems {
                    usage.inline_size += std::mem::size_of::<InlineElem>();
                    match &inline.inline_content { 
                        InlineContent::Text(glyphs) => {
                            usage.char_size += size_of_vec(glyphs)
                        }
                        _ => ()
                    }
                }
            }
        }
    }
}
pub struct MemUsage {
    pub elem_size: usize,
    pub line_size: usize,
    pub inline_size: usize, 
    pub char_size: usize,
}
pub fn size_of_vec<T>(vec: &Vec<T>) -> usize {
    std::mem::size_of::<Vec<T>>() + vec.capacity() * std::mem::size_of::<T>()
}
pub type ImagePromise = Arc<RwLock<Option<(Image, Vec<u8>)>>>;
pub struct HTMLPage { pub root: Elem, pub locations: FxHashMap<String, Vec<usize>>}
pub struct Elem             { pub size: Size, pub point: Point, pub elem_type: ElemType }
pub enum ElemType           { Block(BlockElem), Lines(ElemLines) }
pub struct BlockElem        { pub children: Vec<Elem>, pub total_child_count: usize, }
pub struct ElemLines        { pub height: f64, pub elem_lines: Vec<ElemLine> }
#[derive(Clone)]
pub struct ElemLine         { pub height: f64, pub inline_elems: Vec<InlineElem> }
#[derive(Clone)]
pub struct InlineElem       { pub x: f64, pub inline_content: InlineContent }
#[derive(Clone)]
pub struct InlineItem       { pub size: Size, pub inline_content: InlineContent }
#[derive(Clone)]
pub enum InlineContent      { Text(Vec<CharGlyph>), Image(ImageElem), Link((Vec<CharGlyph>, String)) }
#[derive(Clone)]
pub struct CharGlyph        { pub char: u16, pub x: f32}
#[derive(Clone)]
pub struct ImageElem { pub width: u16, pub height: u16, pub image_promise: ImagePromise}

#[derive(Clone, Copy)]
pub struct ListContext {
    pub list_type: ListType,
    pub item_number: usize,
    pub depth: usize,
    pub indent_width: f64,
}

#[derive(Clone, Copy)]
pub enum ListType {
    Unordered(BulletStyle),
    Ordered(NumberStyle),
    None,
}

#[derive(Clone, Copy)]
pub enum BulletStyle { Disc, Circle, Square }

#[derive(Clone, Copy)]  
pub enum NumberStyle { Decimal, LowerAlpha, UpperAlpha, LowerRoman, UpperRoman }
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
#[derive(Clone)]
pub struct ParseState {
    pub x: f64,
    pub width: f64,
    pub font_weight: u16,
    pub text_align: TextAlign,
    pub text_style: floem_renderer::text::Style,
    pub root_font_size: f32,
    pub list_context: ListContext,
    pub prefix: &'static str,
    pub ancestors: Vec<NodeId>,
}

impl BookElemFactory {
    pub fn new(cache: GlyphCache, images: HashMap<String, ImageElem>, font: &Attrs) -> Self {
        BookElemFactory { curr_x: 0., curr_y: 0., cache, images, base_path: String::new(), locations: FxHashMap::default(), root_font_size: font.font_size, style_time: 0 }
    }

    pub fn parse_root(&mut self, node: Node, font: Attrs, file_path: String, style_sheets: &Vec<StyleSheet>, document: &Document) -> HTMLPage {
        self.curr_x = 0.;
        self.curr_y = 0.;
        self.base_path = file_path;


        for child in node.children() {
            if child.tag_name().name().eq("body") {
                let parse_state = ParseState {
                    x: 0.,
                    width: 600.,
                    font_weight: 400,
                    text_align: TextAlign::Left,
                    root_font_size: font.font_size,
                    text_style: floem_renderer::text::Style::Normal,
                    prefix: "",
                    list_context: ListContext {
                        list_type: ListType::None,
                        item_number: 0,
                        depth: 0,
                        indent_width: 0.0
                    },
                    ancestors: Vec::new(),
                };
                let block = self.parse(child, font, style_sheets, parse_state, vec![0], document);
                let block_type = BlockElem { children: vec![block], total_child_count: 1 };
                let root = Elem { size: Size::default(), point: Point::default(), elem_type: ElemType::Block(block_type) };
                return HTMLPage { root, locations: self.locations.clone() }
            }
        }
        let elem_lines = ElemLines { height: 0., elem_lines: Vec::new() };
        let root = Elem { size: Size::default(), point: Point::default(), elem_type: ElemType::Lines(elem_lines) };
        return HTMLPage { root, locations: FxHashMap::default() }
    }

    pub fn parse(&mut self, node: Node, mut font: Attrs, style_sheets: &Vec<StyleSheet>, mut parse_state: ParseState, mut index: Vec<usize>, document: &Document) -> Elem {
        let mut block_elem      = BlockElem { children: Vec::new(), total_child_count: 0 };
        let mut inline_items    = Vec::new();
        let     init_point      = Point::new(self.curr_x, self.curr_y);
        let now = Instant::now();

        let (margins, mut parse_state) = resolve_style(style_sheets, &node, &mut font, parse_state, document);
        parse_state.ancestors.push(node.id());

        self.style_time += (Instant::now() - now).as_nanos();
        parse_state.width   -= margins.left + margins.right;
        parse_state.x       += margins.left / 2.;
        self.curr_x         = parse_state.x;
        self.curr_y         += margins.top;
        index.push(0);
        if let Some(id) = node.attribute("id") {
            self.locations.insert(id.to_string(), index.clone());
        }

        for child in node.children() {
            let tag_name = child.tag_name().name();

            if BLOCK_ELEMENTS.contains(&tag_name) {
                self.flush_inline_items(&mut block_elem, font, &mut inline_items, &parse_state, &mut index);

                block_elem.add_child(match tag_name {
                    "ul" | "ol" => self.parse_list(child, font, style_sheets, parse_state.clone(), index.clone(), document),
                    "li" => {
                        let mut li_state = parse_state.clone();
                        li_state.prefix = "-\t";
                        self.parse(child, font, style_sheets, li_state, index.clone(), document)
                        //self.parse_list_item(child, font, style_sheets, parse_state, index.clone())
                    },
                    "pre"   => self.parse_pre(child, font, style_sheets, parse_state.clone(), index.clone(), document),
                    "table" => self.parse_table(child, font, style_sheets, parse_state.clone(), index.clone(), document),
                    _       => self.parse(child, font, style_sheets, parse_state.clone(), index.clone(), document)
                });
                *index.last_mut().unwrap() += 1;
            }
            else {
                self.process_inline_element(&mut block_elem, child, style_sheets, font, &parse_state, &mut index, &mut inline_items, document);
            }
        }
        self.flush_inline_items(&mut block_elem, font, &mut inline_items, &parse_state, &mut index);
        self.curr_y += margins.bottom;

        let block_height = block_elem.children.iter().fold(0., |acc, elem| acc + elem.size.height);
        Elem { size: Size::new(600., block_height + margins.top + margins.bottom), point: init_point, elem_type: ElemType::Block(block_elem) }
    }

    fn flush_inline_items(&mut self, block_elem: &mut BlockElem, font: Attrs, inline_items: &mut Vec<InlineItem>, parse_state: &ParseState, index: &mut Vec<usize>) {
        if !inline_items.is_empty() {
            if parse_state.prefix != "" {
                inline_items.insert(0, self.parse_text(parse_state.prefix, font, parse_state.clone(), None).first().unwrap().clone())
            }
            block_elem.add_child(layout_elem_lines(self, std::mem::take(inline_items), parse_state));
            *index.last_mut().unwrap() += 1;
        }
    }

    fn process_inline_element(&mut self, block_elem: &mut BlockElem, child: Node, style_sheets: &Vec<StyleSheet>, font: Attrs, parse_state: &ParseState, index: &mut Vec<usize>, inline_items: &mut Vec<InlineItem>, document: &Document) {
        let tag_name = child.tag_name().name();
        match tag_name {
            "" => inline_items.extend(self.parse_text(child.text().unwrap_or_default(), font, parse_state.clone(), None)),
            "img" => inline_items.push(self.parse_img(child, style_sheets, font, index, parse_state.clone(), document)),
            "br" => { self.flush_inline_items(block_elem, font, inline_items, parse_state, index)},
            "a" => {
                let href = child.attribute("href");
                inline_items.extend(self.parse_inline(child, style_sheets, font, parse_state.clone(), href, index, document).0);
            },
            _ => inline_items.extend(self.parse_inline(child, style_sheets, font, parse_state.clone(), None, index, document).0),
        }
    }

    pub fn parse_inline(&mut self, node: Node, style_sheets: &Vec<StyleSheet>, mut font: Attrs, mut parse_state: ParseState, href: Option<&str>, index: &Vec<usize>, document: &Document) -> (Vec<InlineItem>, TextAlign) {
        let mut inline_items: Vec<InlineItem> = Vec::new();
        let now = Instant::now();
        let (_, mut parse_state) = resolve_style(style_sheets, &node, &mut font, parse_state, document);
        parse_state.ancestors.push(node.id());

        self.style_time += (Instant::now() - now).as_nanos();
        let mut final_text_align = parse_state.text_align;
        if let Some(id) = node.attribute("id") {
            self.locations.insert(id.to_string(), index.clone());
        }
        for child in node.children() {
            if child.tag_name().name().eq("") { 
                inline_items.extend(self.parse_text(child.text().unwrap_or_default(), font, parse_state.clone(), href)); 
            } 
            else if child.has_tag_name("a")  {
                if let Some(href) = child.attribute("href") { 
                    inline_items.extend(self.parse_inline(child, style_sheets, font, parse_state.clone(), Some(href), index, document).0) 
                }
                else {
                    let (inline, text_align) = self.parse_inline(child, style_sheets, font, parse_state.clone(), href, index, document);
                    final_text_align = text_align;
                    inline_items.extend(inline);
                }
            } else { 
                let (inline, text_align) = self.parse_inline(child, style_sheets, font, parse_state.clone(), href, index, document);
                final_text_align = text_align;
                inline_items.extend(inline); 
            }
        }
        (inline_items, final_text_align)
    }

    pub fn parse_img(&mut self, node: Node, style_sheets: &Vec<StyleSheet>, mut font: Attrs, index: &Vec<usize>, mut parse_state: ParseState, document: &Document) -> InlineItem {
        if let Some(id) = node.attribute("id") {
            self.locations.insert(id.to_string(), index.clone());
        }
        parse_state.ancestors.push(node.id());
        let (_, mut parse_state) = resolve_style(style_sheets, &node, &mut font, parse_state, document);
        let relative_path   = node.attribute("src").unwrap();
        let image_path      = resolve_path(&self.base_path, relative_path);
        let image           = self.images.get(&image_path).unwrap();
        let size            = Size::new(image.width as f64, image.height as f64);
        InlineItem {size, inline_content: InlineContent::Image(image.clone())}
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

    fn parse_list(&mut self, node: Node, font: Attrs, style_sheets: &Vec<StyleSheet>, mut parse_state: ParseState, mut index: Vec<usize>, document: &Document) -> Elem {
        let list_type = match node.tag_name().name() {
            "ul" => ListType::Unordered(BulletStyle::Disc),
            "ol" => ListType::Ordered(NumberStyle::Decimal),
            _ => unreachable!(),
        };
        let init_point = Point::new(self.curr_x, self.curr_y);

        let mut list_context = parse_state.list_context;
        list_context.list_type = list_type;
        list_context.indent_width = 20.0 * list_context.depth as f64;
        list_context.depth += 1;
        list_context.item_number = 0;

        parse_state.list_context = list_context;
        parse_state.width -= list_context.indent_width;
        parse_state.x += list_context.indent_width;
        self.curr_x = parse_state.x;
        self.parse(node, font, style_sheets, parse_state, index, document)
            /*


        let mut block_elem = BlockElem { children: Vec::new(), total_child_count: 0 };

        for child in node.children() {
            parse_state.list_context.item_number += 1;
            block_elem.add_child(self.parse_list_item(child, font, style_sheets, parse_state, index.clone()));
            *index.last_mut().unwrap() += 1;
        }
        let block_height = block_elem.children.iter().fold(0., |acc, elem| acc + elem.size.height);
        Elem { size: Size::new(parse_state.width, block_height), point: init_point, elem_type: ElemType::Block(block_elem) }
*/
    }
    
    fn parse_list_item(&mut self, node: Node, font: Attrs, style_sheets: &Vec<StyleSheet>, mut parse_state: ParseState, index: Vec<usize>, document: &Document) -> Elem {
        let mut list_context = parse_state.list_context;

        let marker = self.generate_list_marker(list_context, font, parse_state.clone());
        
        let mut block_elem = BlockElem { children: Vec::new(), total_child_count: 0 };
        let marker_elem = layout_elem_lines(self, vec![marker.clone()], &parse_state);
        block_elem.add_child(marker_elem);
        
        //parse_state.width -= marker.size.width;
        parse_state.x += marker.size.width;
        //self.curr_x += marker.size.width;
        self.curr_y -= marker.size.height;

        let mut content = self.parse(node, font, style_sheets, parse_state.clone(), index, document);

        block_elem.add_child(content);

        let total_height = block_elem.children.iter().fold(0., |acc, elem| acc + elem.size.height);

        parse_state.width += marker.size.width;
        parse_state.x -= marker.size.width;
        Elem { 
            size: Size::new(parse_state.width + marker.size.width, total_height), 
            point: Point::new(self.curr_x, self.curr_y),
            elem_type: ElemType::Block(block_elem) 
        }


    }
    
    fn generate_list_marker(&mut self, context: ListContext, font: Attrs, parse_state: ParseState) -> InlineItem {
        let marker_text = match context.list_type {
            ListType::Unordered(BulletStyle::Disc) => "• ".to_string(),
            ListType::Unordered(BulletStyle::Circle) => "◦ ".to_string(),
            ListType::Unordered(BulletStyle::Square) => "▪ ".to_string(),
            ListType::Ordered(NumberStyle::Decimal) => format!("{}. ", context.item_number),
            ListType::Ordered(NumberStyle::LowerAlpha) => format!("{}. ", self.to_alpha_lower(context.item_number)),
            ListType::Ordered(NumberStyle::UpperAlpha) => format!("{}. ", self.to_alpha_upper(context.item_number)),
            ListType::Ordered(NumberStyle::LowerRoman) => format!("{}. ", self.to_roman_lower(context.item_number)),
            ListType::Ordered(NumberStyle::UpperRoman) => format!("{}. ", self.to_roman_upper(context.item_number)),
            ListType::None => " ".to_string(),
        };
        
        let mut inline_items = self.parse_text(&marker_text, font, parse_state, None);
        inline_items.into_iter().next().unwrap_or_else(|| {
            InlineItem { 
                size: Size::new(0.0, 0.0), 
                inline_content: InlineContent::Text(Vec::new()) 
            }
        })
    }
    
    fn to_alpha_lower(&self, num: usize) -> char {
        if num == 0 || num > 26 { 'a' } else { (b'a' + (num - 1) as u8) as char }
    }
    
    fn to_alpha_upper(&self, num: usize) -> char {
        if num == 0 || num > 26 { 'A' } else { (b'A' + (num - 1) as u8) as char }
    }
    
    fn to_roman_lower(&self, num: usize) -> String {
        self.to_roman(num).to_lowercase()
    }
    
    fn to_roman_upper(&self, num: usize) -> String {
        self.to_roman(num)
    }
    
    fn to_roman(&self, mut num: usize) -> String {
        let vals = [1000, 900, 500, 400, 100, 90, 50, 40, 10, 9, 5, 4, 1];
        let numerals = ["M", "CM", "D", "CD", "C", "XC", "L", "XL", "X", "IX", "V", "IV", "I"];
        let mut result = String::new();
        
        for (i, &val) in vals.iter().enumerate() {
            while num >= val {
                result.push_str(numerals[i]);
                num -= val;
            }
        }
        result
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
    use std::collections::HashMap;
    use std::fs;
    use floem::peniko::Color;
    use floem_renderer::text::{Attrs, FamilyOwned, LineHeightValue};
    use roxmltree::Document;
    use crate::book_elem::{BookElemFactory, InlineContent};
    use crate::glyph_interner::GlyphCache;

    #[test]
    fn test_mem_size() {
        for i in 0..100 {
            let path = "/home/johan/RustroverProjects/Rust-Html/test_files/A Concise History of Switzerland/text/part0012.html";
            let html = fs::read_to_string(path).unwrap();
            let document = Document::parse(&html).unwrap();
            let cache = GlyphCache::new();
            let font_family = "Liberation Serif".to_string();
            let f = &[FamilyOwned::Name(font_family)];
            let base_font = Attrs::new()
                .font_size(20.)
                .family(f)
                .line_height(LineHeightValue::Normal(1.5))
                .color(Color::rgb8(43, 43, 43))
                ;
            let mut book_factory = BookElemFactory::new(cache, HashMap::new(), &base_font);
            //let root = book_factory.parse_root(document.root(), base_font, "/".to_string(), &Vec::new());
        }

    }
}