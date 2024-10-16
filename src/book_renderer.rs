use floem::{EventPropagation, Renderer};
use floem::context::{EventCx, PaintCx};
use floem::event::Event;
use floem::id::Id;
use floem::keyboard::{Key, KeyEvent, NamedKey};
use floem::kurbo::{Point, Rect};
use floem::view::{View, ViewData};
use floem::views::Decorators;
use sha2::Digest;

use crate::book_elem::{BookElem, BookElemFactory};
use crate::epub::{EpubBook};

pub struct BookRenderer {
    data: ViewData,
    epub_book: EpubBook,
    elems: Vec<BookElem>,
    elem_factory: BookElemFactory,

    section_index: usize,
    content_start_index: usize,
    content_end_index: usize,

    col_width: f32,
    col_count: f32,
}

pub fn book_renderer(path: &str) -> BookRenderer {
    let epub_book = EpubBook::new(path);
    let id = Id::next();
    let elem_factory = BookElemFactory::default();
    let mut book_renderer = BookRenderer {data: ViewData::new(id), epub_book,
        section_index: 8, content_start_index: 0, content_end_index: 0, 
        elems: Vec::new(), col_width: 600., col_count: 0.,
        elem_factory
    };
    book_renderer.load_section(8);
    book_renderer = book_renderer.keyboard_navigatable();
    book_renderer
}

impl BookRenderer {

    pub fn next (&mut self) {
        if self.content_end_index >= self.elems.len() {
            if self.section_index == self.epub_book.contents.len() - 1 {return;}
            self.section_index += 1;
            self.content_start_index = 0;
            self.load_section(self.section_index);
            return;
        }
        self.content_start_index = self.content_end_index;
    }

    pub fn prev(&mut self, cx: &mut EventCx) {
        if self.content_start_index == 0 {
            if self.section_index == 0 {return;}
            self.section_index -= 1;
            self.load_section(self.section_index);
            self.content_start_index = self.elems.len();
        }
        self.content_end_index = self.content_start_index;
        let height = cx.get_layout(self.id()).unwrap().size.height;
        let mut height_left = height;
        let mut col_left = self.col_count;
        for x in (0..self.content_end_index).rev() {
            let elem_height = self.elems[x].get_height(self.col_width as f64);
            if elem_height > height_left as f64 {
                if col_left == 1. {return;}
                height_left = height;
                col_left -= 1.;
            }
            height_left = height_left - elem_height as f32;
            self.content_start_index -= 1;
        }
    }

    fn load_section(&mut self, section_index: usize) {
        let document = roxmltree::Document::parse(&self.epub_book.contents[section_index]);
        self.elems = self.elem_factory.layout(document.unwrap());
    }

    fn handle_key_down(&mut self, cx: &mut EventCx, event: &KeyEvent){
        match event.key.logical_key {
            Key::Named(NamedKey::ArrowRight)    => {self.next()},
            Key::Named(NamedKey::ArrowLeft)     => {self.prev(cx)}
            _ => ()
        }
        cx.app_state_mut().request_paint(self.id());
    }
}

impl View for BookRenderer {
    fn view_data(&self) -> &ViewData                { &self.data }
    fn view_data_mut(&mut self) -> &mut ViewData    {&mut self.data}

    fn event(&mut self, cx: &mut EventCx, id_path: Option<&[Id]>, event: Event, ) -> EventPropagation {
        match &event {
            Event::KeyDown(event) => self.handle_key_down(cx, event),
            _ => ()
        }
        EventPropagation::Continue
    }

    fn paint(&mut self, cx: &mut PaintCx) {
        let layout              = cx.get_layout(self.id()).unwrap();

        self.content_end_index  = self.content_start_index;
        let width:f32           = layout.size.width;
        let renderer_height     = layout.size.height;

        self.col_count          = (width / self.col_width).floor();
        let col_gap             = (width - self.col_count * self.col_width) / (self.col_count + 1.);
        let mut col_index       = 0.;
        let mut curr_height     = 0.;

        for elem in self.elems.iter().skip(self.content_start_index) {
            if curr_height + elem.get_height(self.col_width as f64) > renderer_height as f64 {
                if col_index == self.col_count - 1. {return;}
                col_index += 1.;
                curr_height = 0.;
            }
            let x = (col_gap + col_index * (self.col_width + col_gap)) as f64 ;
            
            match elem {
                BookElem::Text(text_layout, block_style) => {
                    //println!("Top margin: {}", block_style.margins.0);
                    curr_height += block_style.margins.0;
                    cx.draw_text(&text_layout, Point::new(x, curr_height));
                    curr_height += block_style.margins.2;
                }
                BookElem::Img(img, size, hash) => {
                    println!("Drawing image:");
                    let image_height = self.col_width as f64 / size;
                    let image = floem_renderer::Img{img, data: img.as_bytes(), hash};

                    cx.draw_img(image, Rect::new(x, curr_height, x + self.col_width as f64, curr_height + image_height))
                }
                _ => {}
            }
            curr_height += elem.get_height(self.col_width as f64);
            self.content_end_index += 1;
        }
    }
}