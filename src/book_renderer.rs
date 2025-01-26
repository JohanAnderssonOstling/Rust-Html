use std::cmp::{max, min};
use std::time::Instant;
use floem::{EventPropagation, Renderer};
use floem::context::{EventCx, PaintCx};
use floem::event::Event;
use floem::id::Id;
use floem::keyboard::{Key, KeyEvent, NamedKey};
use floem::kurbo::{Point, Rect};
use floem::peniko::Color;
use floem::taffy::geometry::Size;
use floem::view::{View, ViewData};
use floem::views::Decorators;
use sha2::Digest;

use crate::book_elem::{BlockElem, BookElemFactory, Elem, ElemType, InlineContent};
use crate::epub::{EpubBook};

pub struct BookRenderer {
    id: Id,
    data: ViewData,
    epub_book: EpubBook,
    root_elem: Elem,
    elem_factory: BookElemFactory,

    section_index: usize,
    start_index: Vec<usize>,
    end_index: Vec<usize>,
    indentation: usize,

    size: Size<f32>,
    col_width: f32,
    col_count: f32,
    col_gap: f32,

    start_offset_y: f64,
    end_offset_y: f64,

    render_forward: bool,

    offset_y: f64,
}

#[derive(Clone)]
struct RenderState {
    x: f64,
    y: f64,
    col_index: f32,
    indendation: usize,
}

pub fn book_renderer(path: &str) -> BookRenderer {
    let epub_book = EpubBook::new(path);
    let id = Id::next();
    let mut elem_factory = BookElemFactory::new();
    let document = roxmltree::Document::parse(&epub_book.contents[15]).unwrap();
    let epub_book = EpubBook::new(path);
    let now = Instant::now();
    let mut book_renderer = BookRenderer {id, data: ViewData::new(id), epub_book,
        section_index: 15, start_index: Vec::new(), end_index: Vec::new(),
        root_elem: elem_factory.parse(document.root_element()), col_gap: 0., col_width: 600., col_count: 0.,
        elem_factory, size: Size::default(), offset_y: 0., start_offset_y: 0., end_offset_y: 0.,
        indentation: 0, render_forward: true,
    };
    let elapsed = now.elapsed();
    println!("Elapsed: {}", elapsed.as_millis());
    //book_renderer.load_section(15);
    //prints(&book_renderer.root_elem, 0);
    book_renderer = book_renderer.keyboard_navigatable();
    book_renderer
}

fn prints(elem: &Elem, indentation: usize) {
    match &elem.elem_type {
        ElemType::Block(block) => {
            println!("{} Block", " ".repeat(indentation * 4));
            for child in block.children.iter() {
                prints(child, indentation + 1);
            }
        }
        ElemType::Lines(lines) => {
            for line in lines.elem_lines.iter() {
                let text = line.inline_elems.iter()
                    .filter_map(|inline_elem| {
                        if let InlineContent::Text(text_layout) = &inline_elem.inline_content {
                            Some(&text_layout.lines)
                        }
                        else { None }
                    })
                    .flatten()
                    .map(|line| line.text().to_string())
                    .collect::<Vec<String>>()
                    .join("");
                println!("{} Line: {}", " ".repeat(indentation * 4), indentation);
            }
        }
    }
}

impl BookRenderer {
    
    fn load_section(&mut self, section_index: usize) {
        let document = roxmltree::Document::parse(&self.epub_book.contents[section_index]);
        let f = roxmltree::Document::parse(&self.epub_book.contents[section_index]);
        let elem = self.elem_factory.parse(f.unwrap().root_element());
        //Self::prints(elem, 0);
    }

    fn handle_key_down(&mut self, cx: &mut EventCx, event: &KeyEvent){
        match event.key.logical_key {
            Key::Named(NamedKey::ArrowRight)    => {self.next()},
            Key::Named(NamedKey::ArrowLeft)     => {self.prev()}
            _ => ()
        }
        cx.app_state_mut().request_paint(self.id());
    }

    fn next(&mut self) {
        self.start_index = self.end_index.clone();
        self.offset_y = self.end_offset_y;
    }

    fn prev(&mut self ) {
        self.end_index = self.start_index.clone();
        let content_height = self.size.height * self.col_count;
        self.offset_y -= content_height as f64;
        if (self.offset_y < 0.) {
            self.start_index = Vec::new();
            self.offset_y = 0.;
            return;
        }
        self.render_forward = false;
        //self.resolve_index(&self.root_elem.clone(), 0);
    }

    fn resolve_index(&mut self, elem: &Elem, indentation: usize) {
        match &elem.elem_type {
            ElemType::Block(block) => {
                for child in block.children.iter() {
                    if child.point.y + child.size.height < self.offset_y {
                        self.start_index[indentation] += 1;
                    }
                    else if child.point.y > self.offset_y {
                        self.start_index[indentation] += 1;
                        break;
                    }
                    else {
                        self.resolve_index(child, indentation + 1);
                    }
                }
            }
            ElemType::Lines(lines) => {

            }
        }
    }

    fn resolve_point(&self, point: Point, elem_height: f64, mut render_state: RenderState) -> (RenderState, Point) {
        let mut y = point.y + render_state.y - self.offset_y;
        let mut col_index = (y / self.size.height as f64).floor();
        y = y - col_index * self.size.height as f64;
        if y + elem_height > self.size.height as f64 {
            render_state.col_index += 1.0;

            if self.render_forward {
                col_index += 1.0;
                render_state.y += self.size.height as f64 - y;
                y = 0.;
            }
            else {
                y = self.size.height as f64 - elem_height;
                render_state.y -= self.size.height as f64 - y;
            }
        }
        let x = (self.col_gap + col_index as f32 * (self.col_width + self.col_gap)) as f64 + point.x;
        (render_state, Point::new(x, y))
    }

    fn paint_backward(&self, cx: &mut PaintCx, elem: &Elem, mut render_state: RenderState, indendation: usize, mut start_index: Vec<usize>) -> (RenderState, f64, Vec<usize>){
        match &elem.elem_type {
            ElemType::Block(block) => {
                let mut elem_index = 0;
                if self.end_index.len() > indendation {
                    elem_index = self.end_index[indendation];
                }
                if start_index.len() <= indendation {
                    start_index.push(block.children.len() - 1);
                }
                for child in block.children.iter().take(elem_index + 1).rev() {
                    match &child.elem_type {
                        ElemType::Block(block) => {
                        }
                        ElemType::Lines(lines) => {
                            let mut lines_point = Point::default();
                            let mut state = render_state.clone();
                            (state, lines_point) = self.resolve_point(elem.point, elem.size.height, state);
                            if state.col_index >= self.col_count { return (state, elem.point.y, start_index); }
                        }
                    }
                    (render_state, _, start_index) = self.paint_backward(cx, child ,render_state, indendation + 1, start_index);
                    if render_state.col_index >= self.col_count {
                        return (render_state, 0., start_index);
                    }
                    println!("Index g: {}", start_index[indendation]);
                    if start_index[indendation] != 0 {
                        start_index[indendation] -= 1;
                    }
                }
                start_index.pop();
            }
            ElemType::Lines(lines) => {
                let mut line_offset_y = elem.size.height;
                for line in lines.elem_lines.iter().rev() {
                    let mut line_point = Point::new(elem.point.x, elem.point.y + line_offset_y);
                    (render_state, line_point) = self.resolve_point(line_point, line.height, render_state);
                    for elem in line.inline_elems.iter() {
                        let elem_point = Point::new(line_point.x + elem.x, line_point.y);
                        match &elem.inline_content {
                            InlineContent::Text(text) => {
                                cx.draw_text(&text, elem_point);
                            }
                        }
                    }
                    line_offset_y -= line.height;
                }
            }
        }
        (render_state, 0., start_index)
    }

    fn paint_recursive(&self, cx: &mut PaintCx, elem: &Elem, mut render_state: RenderState, indendation: usize, mut end_index: Vec<usize>) -> (RenderState, f64, Vec<usize>){

        let mut new_offset_y = 0.;
        match &elem.elem_type {
            ElemType::Block(block) => {
                let mut elem_index = 0;
                if self.start_index.len() > indendation {
                    elem_index = self.start_index[indendation];
                }
                if end_index.len() <= indendation {
                    end_index.push(0);
                }
                for child in block.children.iter().skip(elem_index) {
                    match &child.elem_type {
                        ElemType::Block(_) => {}
                        ElemType::Lines(_) => {
                            let mut lines_point = Point::default();
                            let mut state = render_state.clone();
                            (state, lines_point) = self.resolve_point(elem.point, elem.size.height, state);
                            if state.col_index >= self.col_count { return (state, elem.point.y, end_index); }
                        }
                    }
                    (render_state, new_offset_y, end_index) = self.paint_recursive(cx, child, render_state, indendation + 1, end_index);
                    if render_state.col_index >= self.col_count {
                        return (render_state, new_offset_y, end_index);
                    }
                    end_index[indendation] += 1;

                }
                end_index.pop();
            }
            ElemType::Lines(lines) => {
                let mut line_offset_y = 0.;
                for line in lines.elem_lines.iter() {
                    let mut line_point = Point::new(elem.point.x, elem.point.y + line_offset_y);
                    (render_state, line_point) = self.resolve_point(line_point, line.height, render_state);
                    for elem in line.inline_elems.iter() {
                        let elem_point = Point::new(line_point.x + elem.x, line_point.y);
                        match &elem.inline_content {
                            InlineContent::Text(text) => {
                                cx.draw_text(&text, elem_point);
                            }
                        }
                    }
                    line_offset_y += line.height;
                }
            }
            _ => {}
        }
        (render_state, new_offset_y, end_index)
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
        let size                = cx.get_layout(self.id()).unwrap().size;
        self.end_index          = self.start_index.clone();
        self.col_count          = (size.width / self.col_width).floor();
        self.size               = size;
        self.col_gap            = (size.width - self.col_count * self.col_width) / (self.col_count + 1.);
        let render_state        = RenderState {x: 0., y: 0., col_index: 0., indendation: 0};
        let now = Instant::now();
        if self.render_forward {
            println!("Back: {:#?}", self.start_index);
            (_, self.end_offset_y, self.end_index) = self.paint_recursive(cx, &self.root_elem, render_state, 0, self.end_index.clone());
        }
        else {
            println!("Index: {:#?}", self.end_index);

            (_, _, self.start_index) = self.paint_backward(cx, &self.root_elem, render_state, 0, self.start_index.clone());
            self.render_forward = true;
        }
        println!("Paint {}", now.elapsed().as_millis());
    }
    
}