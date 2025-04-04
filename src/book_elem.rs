use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::vec;

use floem::kurbo::{Point, Size};
use floem::peniko::Image;
use floem_renderer::text::{Attrs, AttrsList, TextLayout};
use floem_renderer::usvg::filter::CompositeOperator::In;
use lightningcss::printer::PrinterOptions;
use lightningcss::properties::font::{AbsoluteFontWeight, FontSize, FontWeight};
use lightningcss::rules::CssRule;
use lightningcss::stylesheet::{ParserOptions, StyleSheet};
use lightningcss::traits::ToCss;
use lightningcss::values::ident::Ident;
use lightningcss::values::length::{LengthPercentage, LengthPercentageOrAuto};
use roxmltree::Node;
use rustc_data_structures::fx::FxHashMap;
use sha2::Digest;

use crate::glyph_interner::GlyphCache;
use crate::style::{apply_style_sheet, CSSValue, S};

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
}
pub type ImagePromise = Arc<RwLock<Option<(Image, Vec<u8>)>>>;
pub struct HTMLPage { pub root: Elem, pub locations: FxHashMap<String, Vec<usize>>}
pub struct Elem             { pub size: Size, pub point: Point, pub elem_type: ElemType }
pub enum ElemType           { Block(BlockElem), Lines(ElemLines) }
pub struct BlockElem        { pub children: Vec<Elem>, pub total_child_count: usize, }
pub struct ElemLines        { pub height: f64, pub elem_lines: Vec<ElemLine> }
pub struct ElemLine         { pub height: f64, pub inline_elems: Vec<InlineElem> }
pub struct InlineElem       { pub x: f64, pub inline_content: InlineContent }

pub struct InlineItem       { size: Size, inline_content: InlineContent }
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
}
#[derive(Clone, Copy)]
pub struct ParseState {
    pub x: f64,
    pub width: f64,
    pub font_weight: u16,
}

impl BookElemFactory {
    pub fn new(cache: GlyphCache, images: HashMap<String, ImageElem>) -> Self{

        BookElemFactory {curr_x: 0., curr_y: 0., cache, images, base_path: String::new(), locations: FxHashMap::default()}
    }
    pub fn add_line(&mut self, curr_line: ElemLine, mut elem_lines: ElemLines, parse_state: ParseState) -> ElemLines{
        self.curr_x         = parse_state.x;
        self.curr_y         += curr_line.height;
        elem_lines.height   += curr_line.height;
        elem_lines.elem_lines.push(curr_line);
        elem_lines
    }
    
    pub fn layout_elem_lines(&mut self, mut inline_items: Vec<InlineItem>, parse_state: ParseState) -> Elem{
        let init_point      = Point::new(self.curr_x, self.curr_y);
        let mut elem_lines  = ElemLines {height: 0., elem_lines: Vec::new()};
        let mut curr_line   = ElemLine  {height: 0., inline_elems: Vec::new()};
        for inline_item in inline_items {
            if inline_item.size.width > parse_state.x + parse_state.width {
                elem_lines          = self.add_line(curr_line, elem_lines, parse_state);
                let mut new_line    = ElemLine {height: inline_item.size.height, inline_elems: Vec::new()};
                let inline_elem     = InlineElem {x: 0., inline_content: inline_item.inline_content};
                new_line.inline_elems.push(inline_elem);
                elem_lines          = self.add_line(new_line, elem_lines, parse_state);
                curr_line           = ElemLine {height: 0., inline_elems: Vec::new()};
                continue
            }
            else if self.curr_x + inline_item.size.width > parse_state.width {
                elem_lines          = self.add_line(curr_line, elem_lines, parse_state);
                curr_line           = ElemLine {height: 0., inline_elems: Vec::new()};
            }
            curr_line.height    = f64::max(curr_line.height, inline_item.size.height);
            let inline_elem     = InlineElem {x: self.curr_x, inline_content: inline_item.inline_content};
            self.curr_x         += inline_item.size.width;
            curr_line.inline_elems.push(inline_elem);
        }
        elem_lines = self.add_line(curr_line, elem_lines, parse_state);
        Elem {size: Size::new(parse_state.width, elem_lines.height), point: init_point, elem_type: ElemType::Lines(elem_lines)}
    }

    pub fn parse_root<'a, 'c>(&mut self, node: Node, font: Attrs, file_path: String, style_sheets: & Vec<StyleSheet>) -> HTMLPage {
        self.curr_x = 0.;
        self.curr_y = 0.;
        self.base_path = file_path;
        let parse_state = ParseState {x: 0., width: 600., font_weight: 400,};

        for child in node.children() {
                if child.tag_name().name().eq("body") {
                    let block = self.parse(child, font, style_sheets, parse_state, vec![0]);
                    let block_type = BlockElem{children: vec![block], total_child_count: 1};
                    let root = Elem {size: Size::default(), point: Point::default(), elem_type: ElemType::Block(block_type)};
                    return HTMLPage {root, locations: self.locations.clone()}
                }
            }
        let elem_lines  = ElemLines {height: 0., elem_lines: Vec::new()};
        let root = Elem {size: Size::default(), point: Point::default(), elem_type: ElemType::Lines(elem_lines)};
        return HTMLPage {root, locations: FxHashMap::default()}
    }
    
    pub fn parse<'a, 'c>(&mut self, node: Node, mut font: Attrs, style_sheets: & Vec<StyleSheet>, mut parse_state: ParseState, mut index: Vec<usize>) -> Elem {

        let mut block_elem = BlockElem {children: Vec::new(), total_child_count: 0};
        let mut inline_items: Vec<InlineItem> = Vec::new();
        let init_point = Point::new(self.curr_x, self.curr_y);
        let mut left = 0.;
        let mut right = 0.;
        let mut top = 0.;
        let mut bottom = 0.;
        let mut style = S::new(node.tag_name().name());
        
        for style_sheet in style_sheets {
            apply_style_sheet(style_sheet, &node, &mut style);
        }
        if let Some(font_size) = &style.font_size {
            let resolved_font_size = resolve_font_size(font_size, parse_state.width, (font.font_size as f64)).round();
            if resolved_font_size != 0. {font = font.font_size(resolved_font_size as f32)}
            //println!("New font size: {}", font.font_size);
        }
        let font_size = font.font_size as f64;
        for (key, value) in style.properties.iter() {
            match value {
                CSSValue::Length(value) => {
                    match key.as_str() {
                        "margin-top"        => top      += resolve_length(value, parse_state.width, font_size),
                        "margin-right"      => right    += resolve_length(value, parse_state.width, font_size),
                        "margin-bottom"     => bottom   += resolve_length(value, parse_state.width, font_size),
                        "margin-left"       => left     += resolve_length(value, parse_state.width, font_size),
                        "padding-top"       => top      += resolve_length(value, parse_state.width, font_size),
                        "padding-right"     => right    += resolve_length(value, parse_state.width, font_size),
                        "padding-bottom"    => bottom   += resolve_length(value, parse_state.width, font_size),
                        "padding-left"      => left     += resolve_length(value, parse_state.width, font_size),
                        _ => (println!("Unresolved key: {key}"))
                    }
                }
                CSSValue::FontWeight(value) => {parse_state.font_weight = resolve_font_weight(value);}
                CSSValue::TextAlign(value) => ()
            }

        }
        //println!("Font size: {}", font.font_size);
        parse_state.width -= left + right;
        parse_state.x += left / 2.;
        //println!("X: {}\t{}", parse_state.x, node.tag_name().name());
        self.curr_x = parse_state.x;
        self.curr_y += top;

        index.push(0);
        if let Some(id) = node.attribute("id") {
            self.locations.insert(id.to_string(), index.clone());
        }
        for child in node.children() {
            let tag_name = child.tag_name().name();
            
            if BLOCK_ELEMENTS.contains(&tag_name) {
                if inline_items.len() != 0 {
                    block_elem.add_child(self.layout_elem_lines(inline_items, parse_state));
                    *index.last_mut().unwrap() += 1;
                    inline_items = Vec::new();
                }
                block_elem.add_child(self.parse(child, font, style_sheets, parse_state, index.clone()));
                *index.last_mut().unwrap() += 1;
            }
            else if tag_name.eq("")     { inline_items.extend(self.parse_text(child, font, parse_state, None)); }
            else if tag_name.eq("img")  { inline_items.push(self.parse_img(child, &index));}
            else if tag_name.eq("br") {
                block_elem.add_child(self.layout_elem_lines(inline_items, parse_state));
                inline_items = Vec::new();
                *index.last_mut().unwrap() += 1;
            }
            else if tag_name.eq("a")  {
                if let Some(href) = child.attribute("href") {inline_items.extend(self.parse_inline(child, font, parse_state, Some(href), &index))}
                else {inline_items.extend(self.parse_inline(child, font, parse_state, None, &index))}
            }
            else                        { inline_items.extend(self.parse_inline(child, font, parse_state, None, &index)); }
           // println!("Tag name: {}", tag_name);
        }
        if inline_items.len() != 0 {
            block_elem.add_child(self.layout_elem_lines(inline_items, parse_state));
            *index.last_mut().unwrap() += 1;
        }
        self.curr_y += bottom;
        let block_height = block_elem.children.iter().fold(0., |acc, elem| acc + elem.size.height);
        Elem {size: Size::new(600., block_height + top + bottom), point: init_point, elem_type: ElemType::Block(block_elem)}
    }

    pub fn parse_inline(&mut self, node: Node, font: Attrs, parse_state: ParseState, href: Option<&str>, index: &Vec<usize>) -> Vec<InlineItem> {
        let mut inline_items: Vec<InlineItem> = Vec::new();
        if let Some(id) = node.attribute("id") {
            println!("Inserting id: {id}");
            self.locations.insert(id.to_string(), index.clone());
        }
        for child in node.children() {
            if child.tag_name().name().eq("") { inline_items.extend(self.parse_text(child, font, parse_state, href)); }
            else if child.has_tag_name("a")  {
                if let Some(href) = child.attribute("href") {inline_items.extend(self.parse_inline(child, font, parse_state, Some(href), index))}
            }
            else { inline_items.extend(self.parse_inline(child, font, parse_state, href, index)); }
        }
        inline_items
    }

    pub fn parse_text(&mut self, node: Node, font: Attrs, parse_state: ParseState, href: Option<&str>) -> Vec<InlineItem> {
        let mut inline_items: Vec<InlineItem> = Vec::new();
        if node.text().is_none() {return Vec::new()}
        if node.text().unwrap().eq("\n") {return Vec::new()}
        for word in node.text().unwrap().split_whitespace() {
            let mut char_x  = 0.;
            let mut word_height: f64 = 0.;
            
            let chars = word.chars();
            let mut char_glyphs: Vec<CharGlyph> = Vec::with_capacity(word.len());
            for char in chars {
                let (text_layout, index) = self.cache.get_or_insert(char, font, parse_state);
                char_glyphs.push(CharGlyph {char: index, x: char_x});
                char_x += text_layout.size().width as f32;
                word_height = word_height.max(text_layout.size().height);
            }
            let (text_layout, index) = self.cache.get_or_insert(' ', font, parse_state);
            char_glyphs.push(CharGlyph{char: index, x: char_x});
            char_x += text_layout.size().width as f32;
            let size = Size::new(char_x as f64, word_height as f64);
            match href {
                None => inline_items.push(InlineItem {size, inline_content: InlineContent::Text(char_glyphs)}),
                Some(href) => inline_items.push(InlineItem {size, inline_content: InlineContent::Link((char_glyphs, href.to_string()))})
            }
        }
        inline_items
    }
    
    pub fn parse_img(&mut self, node: Node, index: &Vec<usize>) -> InlineItem {
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
fn resolve_font_size(value: &FontSize, width: f64, font_size: f64) -> f64 {
    match value {
        FontSize::Length(length) => {
            resolve_length_percentage(&length, width, font_size)
        }
        FontSize::Absolute(absolute) => {0.}
        FontSize::Relative(relative) => {0.}
    }
}
fn resolve_font_weight(font_weight: &FontWeight) -> u16{
    match font_weight {
        FontWeight::Absolute(absolute_value) => {
            match absolute_value {
                AbsoluteFontWeight::Weight(weight) => *weight as u16,
                AbsoluteFontWeight::Normal => 400,
                AbsoluteFontWeight::Bold => 700,
            }
        }
        FontWeight::Bolder => {400}
        FontWeight::Lighter => {400}
    }

}

fn resolve_length_percentage(length: &LengthPercentage, width: f64, font_size: f64) -> f64{
    match length {
        LengthPercentage::Dimension(dim) => {
            let (value, unit) = dim.to_unit_value();
            return match unit {
                "px" => value as f64,
                "em" => value as f64 * font_size,
                "pt" => dim.to_px().unwrap() as f64,
                _ => {
                    println!("Unsupported unit: {unit}");
                    0.
                }
            };
        }
        LengthPercentage::Percentage(percentage) => {width * (percentage.0 as f64)}
        LengthPercentage::Calc(_) => {0.}
    }
}

fn resolve_length(value: &LengthPercentageOrAuto, width: f64, font_size: f64) -> f64 {
    match value {
        LengthPercentageOrAuto::Auto => {0.}
        LengthPercentageOrAuto::LengthPercentage(length) => {
            resolve_length_percentage(length, width, font_size)
        }
    }
}

    
    /*pub fn img(image: DynamicImage, col_width: f32) -> BookElem{
        let aspect_ratio = image.width() as f64 / image.height() as f64;
        let mut s = Sha256::new();
        s.update(image.as_bytes());
        let hash = s.finalize().to_vec();
        println!("Loading image");
        BookElem::Img(image, aspect_ratio, hash)
    }*/
