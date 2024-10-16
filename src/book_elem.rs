use std::ops::Range;

use floem::cosmic_text::{Attrs, AttrsList, TextLayout, Weight};
use floem::kurbo::Size;
use floem::peniko;
use floem::peniko::Color;
use floem::style::{FontWeight, MarginBottom};
use floem::view::View;
use floem::views::Decorators;
use image::DynamicImage;
use roxmltree::{Document, Node};
use sha2::{Digest, Sha256};

pub enum BookElem {
    Text((TextLayout), BlockStyle),
    Img(DynamicImage, f64, Vec<u8>),
    Block(Vec<BookElem>, BlockStyle),
}

pub enum Measurement {
    em(f32),
    px(f32),
}

#[derive(Copy, Clone)]
pub struct BlockStyle {
    pub margins : (f64, f64, f64, f64)
}

pub struct TextStyle {
    font_size   : Measurement,
    font_weight : FontWeight,
    font_color  : Color,
}

#[derive(Copy, Clone)]
pub struct Style {
    block_style : BlockStyle,
    text_style  : Attrs<'static>
}

impl Default for Style {
    fn default() -> Self {
        let margins = (0., 0., 0., 0.);
        let block_style = BlockStyle {margins};
        let text_style = Attrs::new().font_size(20.);
        Style {block_style, text_style}
    }
}

pub struct BookElemFactory {
    text_style  : Attrs<'static>,
    header_style: Attrs<'static>,
    small_style : Attrs<'static>,
    link_style  : Attrs<'static>,
    
}

impl BookElemFactory {
    pub fn new(text_style: Attrs<'static>, header_style: Attrs<'static>, small_style: Attrs<'static>, link_style: Attrs<'static>) -> Self{
        BookElemFactory {text_style, header_style, small_style, link_style}
    }
}

impl Default for BookElemFactory {
    fn default() -> Self {

        let text_style      = Attrs::new().font_size(20.).monospaced(true);
        let header_style    = Attrs::new().weight(Weight::BOLD).font_size(30.);
        let small_style     = Attrs::new().font_size(17.);
        let link_style      = Attrs::new().font_size(19.).color(Color::BLUE);
        Self {text_style, header_style, small_style, link_style}
    }
}

struct PElem {
    default_style: Attrs<'static>,
    attrs_list: AttrsList,
    text_result: String,
    text_length: usize,
    book_elems: Vec<BookElem>,
    style: Style,
}

impl PElem {
    pub fn new(default_style: Attrs<'static>) -> Self{
        let attrs_list = AttrsList::new(default_style);
        let style = Style::default();
        Self {default_style, attrs_list, text_result: String::new(), text_length: 0, book_elems: Vec::new(), style}
    }
    fn add_styled_text (mut self, text: &str, attrs: Attrs) -> Self{
        let new_text_length = self.text_length + text.chars().count();
        let range = Range {start: self.text_length, end: new_text_length};
        self.attrs_list.add_span(range, attrs);
        self.text_result.push_str(text);
        self.text_length = new_text_length;
        self
    }
    
    fn add_plain_text (mut self, text: &str) -> Self {
        self.text_length += text.chars().count();
        self.text_result.push_str(text);
        self
    }
    
    //Empties the text added by add_plain_text and add_styled_text
    fn empty_text(mut self, style: &Style) -> Self {
        let mut text_layout = TextLayout::new();
        text_layout.set_text(&self.text_result, self.attrs_list);
        text_layout.set_size(600., f32::MAX);
        self.book_elems.push(BookElem::Text(text_layout, self.style.block_style));
        self.attrs_list = AttrsList::new(self.default_style);
        self.text_result = String::new();
        self.text_length = 0;
        self
    }
}

impl BookElemFactory {
    pub fn layout(&self, document: Document) -> Vec<BookElem>{
        let mut pelem = PElem::new(self.text_style);
        pelem = self.parse_node(document.root_element(), pelem, Style::default());
        pelem = pelem.empty_text(&Style::default());
        pelem.book_elems
    }

    fn parse_inline(&self, node: Node, mut pelem: PElem, style: Attrs) -> PElem{
        //println!("Inline Parsing: {}\t{}l", node.tag_name().name(), node.text().unwrap_or_default());
        if node.text().is_some() && node.tag_name().name().eq("") {pelem = pelem.add_styled_text(node.text().unwrap(), style);}
        for child in node.children() {pelem = self.parse_inline(child, pelem, style);}
        pelem
    }

    fn parse_node(&self, node: Node, mut pelem: PElem, mut style: Style) -> PElem {
        //println!("Parsing: {}", node.tag_name().name());
        match node.tag_name().name() {
            "div"   => {pelem = pelem.empty_text(&style); pelem.style.block_style.margins.0 = 0.;}
            "p"     => {pelem = pelem.empty_text(&style); pelem.style.block_style.margins.0 = 20.;}
            "h1"    => {pelem = pelem.empty_text(&style); return self.parse_inline(node, pelem, self.header_style)}
            "h2"    => {pelem = pelem.empty_text(&style); return self.parse_inline(node, pelem, self.header_style)}
            "h3"    => {pelem = pelem.empty_text(&style); return self.parse_inline(node, pelem, self.header_style)}
            "dl"    => {pelem = pelem.empty_text(&style);}
            "a"     => {return self.parse_inline(node, pelem, self.link_style)}
            "small" => {return self.parse_inline(node, pelem, self.small_style)}
            "head"  => {return pelem;}
            "dt"    => {}
            "dd"    => {}
            "span"  => {}
            "img"   => {}
            _ => {
                pelem = pelem.add_plain_text(node.text().unwrap_or_default())}
        };
        for child in node.children() {
            println!("{}", style.block_style.margins.0);
            pelem = self.parse_node(child, pelem, style.clone());
        }
        pelem
    }
    

    pub fn img(image: DynamicImage, col_width: f32) -> BookElem{
        let aspect_ratio = image.width() as f64 / image.height() as f64;
        let mut s = Sha256::new();
        s.update(image.as_bytes());
        let hash = s.finalize().to_vec();
        println!("Loading image");
        BookElem::Img(image, aspect_ratio, hash)
    }
    
    fn create_text_layout(text: &str, attrs_list: AttrsList, col_width: f32) -> TextLayout {
        let mut text_layout = TextLayout::new();
        text_layout.set_size(col_width, f32::MAX);
        text_layout.set_text(&text, attrs_list);
        text_layout
    }
}

impl BookElem {
    pub fn size(&self) -> Size {
        match self {
            BookElem::Text(text_layout, block_style) => {text_layout.size()}
            BookElem::Img(img, _, _) => {Size::new(img.width()as f64, img.height() as f64)}
            _ => {Size::new(0., 0.)}
        }
    }

    pub fn get_height(&self, col_width: f64) -> f64{
        match self {
            BookElem::Text(text_layout, block_style) => {text_layout.size().height}
            BookElem::Img(_, aspect, _) => {col_width / aspect}
            _ => {0.}
        }
    }
}
