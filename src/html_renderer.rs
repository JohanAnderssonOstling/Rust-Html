use std::collections::HashMap;
use std::ops::Deref;

use floem::{View, ViewId};
use floem::context::{EventCx, PaintCx};
use floem::event::{Event, EventPropagation};
use floem::keyboard::{Key, NamedKey};
use floem::kurbo::{Point, Rect, Size};
use floem::pointer::PointerInputEvent;
use floem::prelude::{Color, RwSignal, SignalUpdate};
use floem::reactive::{ReadSignal, SignalGet, SignalRead, WriteSignal};
use floem::views::Decorators;
use floem_renderer::{Img, Renderer};
use lightningcss::traits::Op;
use sha2::Digest;
use uuid::Version::Random;
use crate::book_elem::{HTMLPage, Elem, ElemLine, ElemType, InlineContent};
use crate::glyph_interner::GlyphCache;

#[derive(Clone)]
struct RenderState {
    x: f64,
    y: f64,
    col_index: f32,
    terminate: bool,
}

pub struct HtmlRenderer {
    id: ViewId,

    read_current_url: RwSignal<String>,
    pages: HashMap<String, HTMLPage>,

    start_index: RwSignal<Vec<usize>>,
    end_index: Vec<usize>,

    size: Size,
    col_width: f64,
    orig_col_width: f64,
    col_count: f64,
    col_gap: f64,
    scale: f64,

    glyph_cache : GlyphCache,

    start_offset_y: f64,
    end_offset_y: f64,

    render_forward: bool,
    get_go_on: ReadSignal<bool>,
    at_ends: WriteSignal<i8>,

    click_location: Option<Point>,
}

impl HtmlRenderer {

    pub fn new(start_index: RwSignal<Vec<usize>>, glyph_cache: GlyphCache, pages: HashMap<String, HTMLPage>, read_current_url: RwSignal<String>, at_ends: WriteSignal<i8>, get_go_on: ReadSignal<bool>) -> Self{
        let mut html_renderer = HtmlRenderer {
            id: ViewId::new(), start_index, end_index: Vec::new(),
            col_gap: 0., col_count: 0., col_width: 600., orig_col_width: 600., 
            size: Size::default(), start_offset_y: 0., end_offset_y: 0.,
            render_forward: true, scale: 1.0, 
            glyph_cache, pages,
            read_current_url, get_go_on, at_ends,
            click_location: None,
        };
        html_renderer = html_renderer.keyboard_navigable();
        html_renderer
    }
    
    pub fn next(&mut self) {
        self.render_forward = true;
        if self.end_index.len() != 0 && self.end_index[0] == 1 {
            self.at_ends.set(1);
            if !self.get_go_on.get() {return}
            self.start_index.set(Vec::new());
            self.start_offset_y = 0.;
            self.end_index = Vec::new();
            return;
        }
        self.at_ends.set(0);
        self.start_index.set(self.end_index.clone());
    }

    pub fn prev(&mut self ) {
        self.render_forward = false;
        if self.start_index.get().len() == 0 {
            self.at_ends.set(-1);
            if !self.get_go_on.get() {return}
            self.goto_last();
            return
        }
        self.at_ends.set(0);
        self.end_index      = self.start_index.get_untracked();
    }

    pub fn goto(&self, link: &String) {
        if link.contains("www") || link.contains("http") {
            open::that(link);
            return;
        }
        println!("Clicked link: {link}");
        let current_url = self.read_current_url.get_untracked();
        let mut paths: Vec<&str> = current_url.split("/").collect();
        paths.pop().unwrap();
        let path = paths.join("/");
        let parts: Vec<&str> = link.split("#").collect();
        let new_url = format!("{path}/{}", parts[0]);
        let new_index = parts[1].to_string();
        let document = &self.pages.get(&new_url).unwrap();
        self.read_current_url.set(new_url);
        match document.locations.get(&new_index) {
            None => {self.start_index.set(Vec::new())}
            Some(new_index) => {self.start_index.set(new_index.clone())}
        }
    }

    pub fn goto_last(&mut self) {
        let current_url     = self.read_current_url.get();
        let root_elem       = &self.pages.get(&current_url).unwrap().root;
        let last_index      = root_elem.get_last_index();
        self.end_index      = last_index;
        self.render_forward = false;
    }

    fn resolve_point(&self, point: Point, elem_height: f64, mut render_state: RenderState) -> (RenderState, Point) {
        let mut y = point.y + render_state.y - self.start_offset_y;
        let mut col_index = (y / (self.size.height / self.scale) ).floor();
        y = y - col_index * (self.size.height / self.scale);
        if y + elem_height > self.size.height {
            render_state.col_index += 1.0;
            if self.render_forward {
                col_index += 1.0;
                render_state.y += self.size.height - y;
                y = 0.;
            }
            else {
                render_state.y -= y - self.size.height + elem_height;
                y = self.size.height as f64 - elem_height;
            }
        }
        let x = (self.col_gap / self.scale + col_index * (self.col_width / self.scale + self.col_gap / self.scale)) as f64 + point.x;
        if self.render_forward && x + 1.0  >= self.size.width { render_state.terminate = true; }
        if !self.render_forward && x < 0.               {render_state.terminate = true; };
        (render_state, Point::new(x, y))
    }

    fn paint_line(&self, cx: &mut PaintCx, elem: &Elem, line: &ElemLine, line_offset_y: f64, mut render_state: RenderState, render: bool) -> RenderState {
        let mut line_point          = Point::new(elem.point.x, elem.point.y + line_offset_y);
        (render_state, line_point)  = self.resolve_point(line_point, line.height, render_state);
        if !render {return render_state}
        for elem in line.inline_elems.iter() {
            let elem_point = Point::new(line_point.x + elem.x, line_point.y);
            match &elem.inline_content {
                InlineContent::Text(text) => {
                    for char_glyph in text {
                        let glyph = self.glyph_cache.get(char_glyph.char);
                        cx.draw_text(glyph, Point::new(elem_point.x + char_glyph.x as f64, elem_point.y))
                    }
                }
                InlineContent::Link((text, link)) => {
                    for char_glyph in text {
                        let glyph = self.glyph_cache.get(char_glyph.char);
                        let x = elem_point.x + char_glyph.x as f64;
                        cx.draw_text(glyph, Point::new(x, elem_point.y));
                        let descent = glyph.lines().first().unwrap().layout_opt().as_ref().unwrap().first().as_ref().unwrap().max_descent as f64;
                        let y = elem_point.y + glyph.size().height - descent;
                        let x0 = x;
                        let x1 = x0 + glyph.size().width;
                        let rect = Rect::new(x0 - 1., y, x1 + 1., y + 2.0);
                        cx.fill(&rect, Color::DARK_GREEN, 0.);
                        if let Some(location) = self.click_location {
                            if x <= location.x && location.x <= x1 && elem_point.y <= location.y && location.y <= elem_point.y + glyph.size().height {
                                self.goto(link);
                            }
                        }
                    }
                }
                InlineContent::Image(image_elem) => {
                    let image_promise = image_elem.image_promise.read().unwrap();
                    match image_promise.deref() {
                        None => {}
                        Some(image) => {
                            let rect = Rect::new(line_point.x, line_point.y, line_point.x + image_elem.width as f64, line_point.y + image_elem.height as f64);
                            let img = Img {img: image.0.clone(), hash: &image.1};
                            cx.draw_img(img, rect);
                           // println!("Rendered image: {}", line_point.x);
                        }
                    }
                }
            }
        }
        render_state
    }

    fn paint_backward(&self, cx: &mut PaintCx, elem: &Elem, mut render_state: RenderState, level: usize, mut index: Vec<usize>) -> (RenderState, Vec<usize>){
        match &elem.elem_type {
            ElemType::Block(block) => {
                if block.children.len() == 0    { return (render_state, index) }
                if index.len() <= level         { index.push(block.children.len() - 1); }
                for child in block.children.iter().take(index[level] + 1).rev() {
                    (render_state, index) = self.paint_backward(cx, child, render_state, level + 1, index);
                    if render_state.terminate       {return (render_state, index); }
                    if index[level] != 0            { index[level] -= 1; }
                }
                index.pop();
            }
            ElemType::Lines(lines) => {
                let mut line_offset_y   = elem.size.height;
                let mut dummy_state     = render_state.clone();
                for line in lines.elem_lines.iter().rev() {
                    line_offset_y -= line.height;
                    dummy_state  = self.paint_line(cx, &elem, &line, line_offset_y, dummy_state , false);
                    if dummy_state.terminate       { return (dummy_state, index);}
                }
                line_offset_y = elem.size.height;
                for line in lines.elem_lines.iter().rev() {
                    line_offset_y -= line.height;
                    render_state = self.paint_line(cx, &elem, &line, line_offset_y, render_state, true);
                    if render_state.terminate {return (render_state, index)}
                }
            }
        }
        (render_state, index)
    }

    fn paint_recursive(&self, cx: &mut PaintCx, elem: &Elem, mut render_state: RenderState, level: usize, mut index: Vec<usize>) -> (RenderState, Vec<usize>){
        match &elem.elem_type {
            ElemType::Block(block) => {
                if index.len() <= level { index.push(0);}
                for child in block.children.iter().skip(index[level]) {
                    (render_state, index) = self.paint_recursive(cx, child, render_state, level + 1, index);
                    if render_state.terminate           { return (render_state, index); }
                    index[level] += 1;
                }
                if index.len() != 1 { index.pop(); }
            }
            ElemType::Lines(lines) => {
                let mut line_offset_y = 0.;
                let mut dummy_state = render_state.clone();
                for line in lines.elem_lines.iter() {
                    dummy_state  = self.paint_line(cx, &elem, &line, line_offset_y, dummy_state , false);
                    if dummy_state .terminate       { return (dummy_state, index);}
                    line_offset_y += line.height;
                }
                line_offset_y = 0.;
                for line in lines.elem_lines.iter() {
                    render_state  = self.paint_line(cx, &elem, &line, line_offset_y, render_state , true);
                    if render_state .terminate       { return (render_state, index);}
                    line_offset_y += line.height;
                }
            }
        }
        (render_state, index)
    }
}

impl View for HtmlRenderer {
    fn id(&self) -> ViewId { self.id }
    fn event_before_children(&mut self, cx: &mut EventCx, event: &Event) -> EventPropagation {
        match &event {
            Event::KeyDown(event) => {
                match event.key.logical_key {
                    Key::Named(NamedKey::ArrowRight)    => {self.next()},
                    Key::Named(NamedKey::ArrowLeft)     => {self.prev()}
                    Key::Named(NamedKey::ArrowUp)       => {self.goto_last()}
                    Key::Named(NamedKey::F11)           => {self.id.inspect()}
                    _ => ()
                }
            }
            Event::PointerDown(event) => {
                self.click_location = Some(event.pos);
            }


            _ => ()
        }
        cx.app_state_mut().request_paint(self.id());
        EventPropagation::Continue
    }



    fn paint(&mut self, cx: &mut PaintCx) {
        let root_elem           = &self.pages.get(&self.read_current_url.get()).unwrap().root;
        self.size               = self.id.get_size().unwrap();
        self.col_count          = (self.size.width / self.col_width).floor();
        self.col_gap            = (self.size.width - self.col_count * self.col_width) / (self.col_count + 1.);
        let render_state        = RenderState {x: 0., y: 0., col_index: 0., terminate: false};
        let mut start_index     = self.start_index.get_untracked();
        if self.render_forward {
            if start_index.len() != 0 {
                let first_elem      = root_elem.get_elem(&start_index, 0);
                self.start_offset_y = first_elem.point.y;
            }
            (_, self.end_index) = self.paint_recursive(cx, root_elem, render_state, 0, start_index);
        }
        else {
            let last_elem       = root_elem.get_elem(&self.end_index, 0);
            let content_size    = self.size.height * self.col_count;
            self.start_offset_y = last_elem.point.y + last_elem.size.height - content_size + 20.;

            if self.start_offset_y < 0. {
                self.start_index.set( Vec::new());
                self.start_offset_y = 0.;
                self.render_forward = true;
                (_, self.end_index) = self.paint_recursive(cx, root_elem, render_state, 0, start_index);
                return;
            }
            (_, start_index) = self.paint_backward(cx, root_elem, render_state, 0, self.end_index.clone());
            self.start_index.set(start_index);
        }
        self.click_location = None;
    }
    
}