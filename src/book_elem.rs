use std::time::Instant;
use floem::kurbo::{Point, Size};
use floem::views::Decorators;
use floem_renderer::text::{Attrs, AttrsList, TextLayout};
use image::DynamicImage;
use roxmltree::Node;
use sha2::{Digest, Sha256};

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
                let elem = &(block.children[index[level]]);
                if index.len() <= level { return elem; }
                elem.get_elem(index, level + 1)
            }
            ElemType::Lines(_) => { self }
        }
    }
}
pub struct Elem             { pub size: Size, pub point: Point, pub elem_type: ElemType }
pub enum ElemType           { Block(BlockElem), Lines(ElemLines) }
pub struct BlockElem        { pub children: Vec<Elem>, pub total_child_count: usize, }
pub struct ElemLines        { height: f64, pub elem_lines: Vec<ElemLine> }
pub struct ElemLine         { pub height: f64, pub inline_elems: Vec<InlineElem> }
pub struct InlineElem       { pub x: f64, pub inline_content: InlineContent }

pub struct InlineItem       { size: Size, inline_content: InlineContent }
pub enum InlineContent      { Text(TextLayout) }
pub struct BookElemFactory  { curr_x: f64, curr_y: f64, }
impl BookElemFactory {
    pub fn new() -> Self{ BookElemFactory {curr_x: 0., curr_y: 0.} }
    pub fn add_line(&mut self, curr_line: ElemLine, mut elem_lines: ElemLines) -> ElemLines{
        self.curr_x         = 0.;
        self.curr_y         += curr_line.height;
        elem_lines.height   += curr_line.height;
        elem_lines.elem_lines.push(curr_line);
        elem_lines
    }
    
    pub fn layout_elem_lines(&mut self, inline_items: Vec<InlineItem>, width: f64) -> Elem{
        let init_point      = Point::new(self.curr_x, self.curr_y);
        let mut elem_lines  = ElemLines {height: 0., elem_lines: Vec::new()};
        let mut curr_line   = ElemLine  {height: 0., inline_elems: Vec::new()};
        for inline_item in inline_items {
            if self.curr_x + inline_item.size.width > 600. {
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
    
    pub fn parse(&mut self, node: Node, font: Attrs) -> Elem {
        if node.tag_name().name().eq("html") {
            for child in node.children() {
                if child.tag_name().name().eq("body") { return self.parse(child, font); }
            }
        }
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
            else if tag_name.eq("") { inline_items.extend(self.parse_text(child, attrs_list.clone())); }
            else                    { inline_items.extend(self.parse_inline(child, attrs_list.clone())); }
        }
        if inline_items.len() != 0 { block_elem.add_child(self.layout_elem_lines(inline_items, 600.)); }
        self.curr_y += bottom;
        let block_height = block_elem.children.iter().fold(0., |acc, elem| acc + elem.size.height);
        Elem {size: Size::new(600., block_height + top + bottom), point: init_point, elem_type: ElemType::Block(block_elem)}
    }

    pub fn parse_inline(&self, node: Node, attrs_list: AttrsList) -> Vec<InlineItem> {
        let mut inline_items: Vec<InlineItem> = Vec::new();
        for child in node.children() {
            if child.tag_name().name().eq("") { inline_items.extend(self.parse_text(child, attrs_list.clone())); }
            else { inline_items.extend(self.parse_inline(child, attrs_list.clone())); }
        }
        inline_items
    }

    pub fn parse_text(&self, node: Node, attrs_list: AttrsList) -> Vec<InlineItem> {
        let mut inline_items: Vec<InlineItem> = Vec::new();
        if node.text().unwrap().eq("\n") {return Vec::new()}
        for word in node.text().unwrap().split(" ") {
            let mut text_layout = TextLayout::new();
            text_layout.set_text(format!{"{word} "}.as_str(), attrs_list.clone());
            let size = text_layout.size();
            inline_items.push(InlineItem {size, inline_content: InlineContent::Text(text_layout)});
        }
        inline_items
    }
    
    /*pub fn img(image: DynamicImage, col_width: f32) -> BookElem{
        let aspect_ratio = image.width() as f64 / image.height() as f64;
        let mut s = Sha256::new();
        s.update(image.as_bytes());
        let hash = s.finalize().to_vec();
        println!("Loading image");
        BookElem::Img(image, aspect_ratio, hash)
    }*/
}