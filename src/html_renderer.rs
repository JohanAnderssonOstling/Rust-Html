use std::ops::Deref;
use std::rc::Rc;
use std::time::Instant;
use floem::{View, ViewId};
use floem::context::{EventCx, PaintCx};
use floem::event::{Event, EventPropagation};
use floem::keyboard::{Key, NamedKey};
use floem::kurbo::{Point, Size};
use floem::prelude::{create_rw_signal, SignalUpdate};
use floem::reactive::{create_effect, RwSignal, SignalGet, SignalRead};
use floem::views::Decorators;
use floem_renderer::Renderer;
use floem_renderer::text::{Attrs, LineHeightValue};
use roxmltree::Document;
use sha2::Digest;

use crate::book_elem::{BookElemFactory, Elem, ElemLine, ElemType, InlineContent};
#[derive(Clone)]
struct RenderState {
    x: f64,
    y: f64,
    col_index: f32,
    terminate: bool,
}

pub struct HtmlRenderer {
    id: ViewId,
    root_elem: Elem,

    start_index: Vec<usize>,
    end_index: Vec<usize>,

    size: Size,
    col_width: f64,
    orig_col_width: f64,
    col_count: f64,
    col_gap: f64,
    scale: f64,

    start_offset_y: f64,
    end_offset_y: f64,

    render_forward: bool,
}


impl HtmlRenderer {

    pub fn new(document: &Document) -> Self{
        let base_font = Attrs::new().font_size(20.).line_height(LineHeightValue::Normal(1.2));
        let mut book_factory = BookElemFactory::new();
        let now = Instant::now();
        let root_elem = book_factory.parse(document.root_element(), base_font);
        println!("Elapsed {}", now.elapsed().as_millis());
        let mut html_renderer = HtmlRenderer {id: ViewId::new(), start_index: Vec::new(), end_index: Vec::new(),
            col_gap: 0., col_width: 600., col_count: 0.
            , size: Size::default(), start_offset_y: 0., end_offset_y: 0.,
            render_forward: true, scale: 1.0, orig_col_width: 600., root_elem
        };
        html_renderer = html_renderer.keyboard_navigable();
        //html_renderer.id.request_focus();
        html_renderer
    }
    

    pub fn next(&mut self) {
        println!("Next");
        if self.end_index.len() != 0 && self.end_index[0] == 1 {return;}
        self.start_index        = self.end_index.clone();
        self.start_offset_y     = self.end_offset_y;
    }

    pub fn prev(&mut self ) {
        if self.start_index.len() == 0 {return}
        self.end_index          = self.start_index.clone();
        let content_height      = self.size.height * self.col_count;
        let last_elem           = self.root_elem.get_elem(&self.end_index, 0);
        self.end_offset_y       = self.start_offset_y;
        self.start_offset_y     = last_elem.point.y + last_elem.size.height + 10.;
        self.start_offset_y     -= content_height as f64;
        if (self.start_offset_y < 0.) {
            self.start_index = Vec::new();
            self.start_offset_y = 0.;
            return;
        }
        self.render_forward = false;
    }

    fn resolve_point(&self, point: Point, elem_height: f64, mut render_state: RenderState) -> (RenderState, Point) {
        let mut y = point.y + render_state.y - self.start_offset_y;
        let mut col_index = (y / (self.size.height / self.scale) ).floor();
        y = y - col_index * (self.size.height / self.scale);
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
        let x = (self.col_gap / self.scale + col_index * (self.col_width / self.scale + self.col_gap / self.scale)) as f64 + point.x;
        if x >= self.size.width || x < 0.{ render_state.terminate = true; }
        (render_state, Point::new(x, y))
    }

    fn paint_line(&self, cx: &mut PaintCx, elem: &Elem, line: &ElemLine, line_offset_y: f64, mut render_state: RenderState) -> RenderState {
        let mut line_point = Point::new(elem.point.x, elem.point.y + line_offset_y);
        (render_state, line_point) = self.resolve_point(line_point, line.height, render_state);
        for elem in line.inline_elems.iter() {
            let elem_point = Point::new(line_point.x + elem.x, line_point.y);
            cx.set_scale(self.scale);
            match &elem.inline_content {
                InlineContent::Text(text) => { cx.draw_text(&text, elem_point); }
            }
            cx.set_scale(1.0);
        }
        render_state
    }

    fn paint_backward(&self, cx: &mut PaintCx, elem: &Elem, mut render_state: RenderState, level: usize, mut index: Vec<usize>) -> (RenderState, f64, Vec<usize>){
        let mut new_offset_y = 0.;
        match &elem.elem_type {
            ElemType::Block(block) => {
                let mut elem_index = 0;
                if self.end_index.len() > level { elem_index = self.end_index[level]; }
                if index.len() <= level { index.push(block.children.len() - 1); }
                for child in block.children.iter().take(elem_index + 1).rev() {
                    render_state = self.paint_child(cx, child, render_state);
                    if render_state.terminate           { return (render_state, elem.point.y, index); }
                    (render_state, new_offset_y, index) = self.paint_backward(cx, child, render_state, level + 1, index);
                    if render_state.terminate           { return (render_state, new_offset_y, index); }
                    if index[level] != 0    { index[level] -= 1; }
                }
                index.pop();
            }
            ElemType::Lines(lines) => {
                let mut line_offset_y = elem.size.height;
                for line in lines.elem_lines.iter().rev() {
                    render_state = self.paint_line(cx, &elem, &line, line_offset_y, render_state);
                    line_offset_y -= line.height;
                }
            }
        }
        (render_state, new_offset_y, index)
    }

    fn paint_child (&self, cx: &mut PaintCx, elem: &Elem, mut render_state: RenderState) -> RenderState{
        match &elem.elem_type {
            ElemType::Block(block) => {}
            ElemType::Lines(lines) => {
                let mut point = Point::new(elem.point.x, elem.point.y + elem.size.height);
                (_, point) = self.resolve_point(elem.point, elem.size.height, render_state.clone());
                if self.size.width <= point.x || 0. > point.x { render_state.terminate = true; }
            }
        }
        render_state
    }

    fn paint_recursive(&self, cx: &mut PaintCx, elem: &Elem, mut render_state: RenderState, level: usize, mut index: Vec<usize>) -> (RenderState, f64, Vec<usize>){
        let mut new_offset_y = 0.;
        match &elem.elem_type {
            ElemType::Block(block) => {
                if index.len() <= level { index.push(0);}
                for child in block.children.iter().skip(index[level]) {
                    render_state = self.paint_child(cx, child, render_state);
                    if render_state.terminate           { return (render_state, elem.point.y, index); }
                    (render_state, new_offset_y, index) = self.paint_recursive(cx, child, render_state, level + 1, index);
                    if render_state.terminate           { return (render_state, new_offset_y, index); }
                    index[level] += 1;
                }
                if index.len() != 1 { index.pop(); }
            }
            ElemType::Lines(lines) => {
                let mut line_offset_y = 0.;
                for line in lines.elem_lines.iter() {
                    render_state = self.paint_line(cx, &elem, &line, line_offset_y, render_state);
                    line_offset_y += line.height;
                }
            }
        }
        (render_state, new_offset_y, index)
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
                    _ => ()
                }
            }
            _ => ()
        }
        cx.app_state_mut().request_paint(self.id());
        EventPropagation::Continue
    }
    fn paint(&mut self, cx: &mut PaintCx) {
        //if self.root_elem.is_none() {return}
        let size                = cx.size();
        self.col_count          = (size.width / self.col_width).floor();
        self.size               = Size::new(size.width, size.height);
        self.col_gap            = (size.width - self.col_count * self.col_width) / (self.col_count + 1.);
        let render_state        = RenderState {x: 0., y: 0., col_index: 0., terminate: false};
        if self.render_forward {
            (_, self.end_offset_y, self.end_index) = self.paint_recursive(cx, &self.root_elem, render_state, 0, self.start_index.clone());
        }
        else {
            (_, self.start_offset_y, self.start_index) = self.paint_backward(cx, &self.root_elem, render_state, 0, self.start_index.clone());
            self.render_forward = true;
        }
    }
    
}