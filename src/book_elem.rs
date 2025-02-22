use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::vec;

use floem::kurbo::{Point, Size};
use floem::peniko::Image;
use floem_renderer::text::{Attrs, AttrsList};
use roxmltree::Node;
use sha2::Digest;

use crate::glyph_cache::GlyphCache;

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
pub struct Elem             { pub size: Size, pub point: Point, pub elem_type: ElemType }
pub enum ElemType           { Block(BlockElem), Lines(ElemLines) }
pub struct BlockElem        { pub children: Vec<Elem>, pub total_child_count: usize, }
pub struct ElemLines        { pub height: f64, pub elem_lines: Vec<ElemLine> }
pub struct ElemLine         { pub height: f64, pub inline_elems: Vec<InlineElem> }
pub struct InlineElem       { pub x: f64, pub inline_content: InlineContent }

pub struct InlineItem       { size: Size, inline_content: InlineContent }
pub enum InlineContent      { Text(Vec<CharGlyph>), Image(ImageElem) }
pub struct CharGlyph        { pub char: char, pub x: f64}
#[derive(Clone)]
pub struct ImageElem { pub width: u32, pub height: u32, pub image_promise: ImagePromise}
pub struct BookElemFactory  { 
    pub curr_x: f64, 
    pub curr_y: f64,
    base_path: String,
    pub cache: GlyphCache,
    pub images: HashMap<String, ImageElem>
}
impl BookElemFactory {
    pub fn new(cache: GlyphCache, images: HashMap<String, ImageElem>) -> Self{
        BookElemFactory {curr_x: 0., curr_y: 0.,cache, images, base_path: String::new()}
    }
    pub fn add_line(&mut self, curr_line: ElemLine, mut elem_lines: ElemLines) -> ElemLines{
        self.curr_x         = 0.;
        self.curr_y         += curr_line.height;
        elem_lines.height   += curr_line.height;
        elem_lines.elem_lines.push(curr_line);
        elem_lines
    }
    
    pub fn layout_elem_lines(&mut self, mut inline_items: Vec<InlineItem>, width: f64) -> Elem{
        let init_point      = Point::new(self.curr_x, self.curr_y);
        let mut elem_lines  = ElemLines {height: 0., elem_lines: Vec::new()};
        let mut curr_line   = ElemLine  {height: 0., inline_elems: Vec::new()};
        for inline_item in inline_items {
            if inline_item.size.width > 600. {
                elem_lines          = self.add_line(curr_line, elem_lines);
                let new_line        = ElemLine {height: inline_item.size.height, inline_elems: Vec::new()};
                elem_lines          = self.add_line(new_line, elem_lines);
                curr_line           = ElemLine {height: 0., inline_elems: Vec::new()};
                continue
            }
            else if self.curr_x + inline_item.size.width > 600. {
                elem_lines          = self.add_line(curr_line, elem_lines);
                curr_line           = ElemLine {height: 0., inline_elems: Vec::new()};
            }


            curr_line.height    = f64::max(curr_line.height, inline_item.size.height);
            let inline_elem     = InlineElem {x: self.curr_x, inline_content: inline_item.inline_content};
            self.curr_x         += inline_item.size.width;
            curr_line.inline_elems.push(inline_elem);
        }
        elem_lines = self.add_line(curr_line, elem_lines);
        Elem {size: Size::new(width, elem_lines.height), point: init_point, elem_type: ElemType::Lines(elem_lines)}
    }

    pub fn parse_root(&mut self, node: Node, font: Attrs, file_path: String) -> Elem {
        self.curr_x = 0.;
        self.curr_y = 0.;
        self.base_path = file_path;
            for child in node.children() {
                if child.tag_name().name().eq("body") {
                    let block = self.parse(child, font);
                    let block_type = BlockElem{children: vec![block], total_child_count: 1};
                    return Elem {size: Size::default(), point: Point::default(), elem_type: ElemType::Block(block_type)}
                }
            }
        let elem_lines  = ElemLines {height: 0., elem_lines: Vec::new()};
        Elem {size: Size::default(), point: Point::default(), elem_type: ElemType::Lines(elem_lines)}
    }
    
    pub fn parse(&mut self, node: Node, font: Attrs) -> Elem {

        let mut block_elem = BlockElem {children: Vec::new(), total_child_count: 0};
        let mut inline_items: Vec<InlineItem> = Vec::new();
        let attrs_list = AttrsList::new(font);
        let init_point = Point::new(self.curr_x, self.curr_y);
        let mut top = 0.;
        let mut bottom = 0.;
        if node.tag_name().name().eq("p") {
            top = font.font_size as f64;
            bottom = font.font_size as f64;
        }
        self.curr_y += top;
        for child in node.children() {
            let tag_name = child.tag_name().name();
            if BLOCK_ELEMENTS.contains(&tag_name) {
                if inline_items.len() != 0 {
                    block_elem.add_child(self.layout_elem_lines(inline_items, 600.));
                    inline_items = Vec::new();
                }
                block_elem.add_child(self.parse(child, font));
            }
            else if tag_name.eq("")     { inline_items.extend(self.parse_text(child, attrs_list.clone())); }
            //else if tag_name.eq("img")  { inline_items.push(self.parse_img(child));}
            else if tag_name.eq("br") {
                block_elem.add_child(self.layout_elem_lines(inline_items, 600.));
                inline_items = Vec::new();
            }
            else                        { inline_items.extend(self.parse_inline(child, attrs_list.clone())); }

        }
        if inline_items.len() != 0 { block_elem.add_child(self.layout_elem_lines(inline_items, 600.)); }
        self.curr_y += bottom;
        let block_height = block_elem.children.iter().fold(0., |acc, elem| acc + elem.size.height);
        Elem {size: Size::new(600., block_height + top + bottom), point: init_point, elem_type: ElemType::Block(block_elem)}
    }

    pub fn parse_inline(&mut self, node: Node, attrs_list: AttrsList) -> Vec<InlineItem> {
        let mut inline_items: Vec<InlineItem> = Vec::new();
        for child in node.children() {
            if child.tag_name().name().eq("") { inline_items.extend(self.parse_text(child, attrs_list.clone())); }
            else { inline_items.extend(self.parse_inline(child, attrs_list.clone())); }
        }
        inline_items
    }

    pub fn parse_text(&mut self, node: Node, attrs_list: AttrsList) -> Vec<InlineItem> {
        let mut inline_items: Vec<InlineItem> = Vec::new();
        if node.text().unwrap().eq("\n") {return Vec::new()}
        for word in node.text().unwrap().split_whitespace() {
            let mut char_x = 0.;
            let word_height = 24.;
            let chars = word.chars();
            let mut char_glyphs: Vec<CharGlyph> = Vec::with_capacity(word.len());

            for char in chars {
                //println!("{}", char as u32);
                char_glyphs.push(CharGlyph {char, x: char_x});
                let text_layout = self.cache.get_or_insert(char, &attrs_list);
                char_x += text_layout.size().width;
            }
            let word_width = char_x + 10.;
            let size = Size::new(word_width, word_height);
            inline_items.push(InlineItem {size, inline_content: InlineContent::Text(char_glyphs)});
        }
        inline_items
    }
    
    pub fn parse_img(&self, node: Node) -> InlineItem {
        let relative_path = node.attribute("src").unwrap();
        let image_path = resolve_path(&self.base_path, relative_path);
        println!("IMAGEPATH: {relative_path} \n\n {image_path}");
        let image = self.images.get(&image_path).unwrap();
        let size = Size::new(image.width as f64, image.height as f64);
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

    
    /*pub fn img(image: DynamicImage, col_width: f32) -> BookElem{
        let aspect_ratio = image.width() as f64 / image.height() as f64;
        let mut s = Sha256::new();
        s.update(image.as_bytes());
        let hash = s.finalize().to_vec();
        println!("Loading image");
        BookElem::Img(image, aspect_ratio, hash)
    }*/
