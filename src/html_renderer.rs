use std::collections::HashMap;
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
use floem_renderer::text::{Attrs, LineHeightValue, TextLayout, Weight};
use roxmltree::Document;
use rustc_data_structures::fx::FxHashMap;
use sha2::Digest;

use crate::book_elem::{BookElemFactory, Elem, ElemLine, ElemLines, ElemType, GlyphCache, InlineContent};
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

    current_url: String,
    pages: HashMap<String, Elem>,

    start_index: Vec<usize>,
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
}

impl HtmlRenderer {

    pub fn new(document: &Document, cache: GlyphCache, pages: HashMap<String, Elem>, current_url: String) -> Self{
        let mut base_font = Attrs::new().font_size(20.).line_height(LineHeightValue::Normal(1.2));
        //base_font = base_font.font_size(base_font.font_size * 1.5);
        //let mut cache: HashMap<char, TextLayout> = HashMap::new();
        let mut book_factory = BookElemFactory::new(cache);
        let now = Instant::now();
        let root_elem = book_factory.parse(document.root_element(), base_font);
        //let mut elem_lines  = ElemLines {height: 0., elem_lines: Vec::new()};
        //let root_elem = Elem {size: Size::new(0., elem_lines.height), point: Point::new(0.,0.), elem_type: ElemType::Lines(elem_lines)};

        let glyph_cache = book_factory.cache;
        println!("Elapsed {}", now.elapsed().as_millis());
        let mut html_renderer = HtmlRenderer {id: ViewId::new(), start_index: Vec::new(), end_index: Vec::new(),
            col_gap: 0., col_width: 600., col_count: 0.
            , size: Size::default(), start_offset_y: 0., end_offset_y: 0.,
            render_forward: true, scale: 1.0, orig_col_width: 600., root_elem, glyph_cache, pages, current_url
        };
        html_renderer = html_renderer.keyboard_navigable();
        html_renderer
    }
    

    pub fn next(&mut self) {
        if self.end_index.len() != 0 && self.end_index[0] == 1 {return;}
        self.start_index        = self.end_index.clone();
        self.render_forward = true;
    }

    pub fn prev(&mut self ) {
        if self.start_index.len() == 0 {return}
        self.end_index          = self.start_index.clone();
        self.render_forward = false;
    }

    pub fn goto_last(&mut self) {
        let root_elem = self.pages.get(&self.current_url).unwrap();

        let mut last_index = root_elem.get_last_index();
        let last_elem = root_elem.get_elem(&last_index, 0);
        self.end_index = last_index;
        self.start_offset_y     = last_elem.point.y + last_elem.size.height;
        let content_height      = self.size.height * self.col_count;

        self.start_offset_y     -= content_height as f64;
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
        let mut line_point = Point::new(elem.point.x, elem.point.y + line_offset_y);
        (render_state, line_point) = self.resolve_point(line_point, line.height, render_state);
        for elem in line.inline_elems.iter() {
            let elem_point = Point::new(line_point.x + elem.x, line_point.y);
            cx.set_scale(self.scale);
            if render {
                match &elem.inline_content {
                    InlineContent::Text(text) => {
                        for char in text {
                            let glyph = self.glyph_cache.get(char.char).unwrap();
                            let glyph_point = Point::new(elem_point.x + char.x, elem_point.y);
                            cx.draw_text(glyph, glyph_point)
                        }
                    }
                }
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
                let mut last_index = index.clone();
                for child in block.children.iter().take(elem_index + 1).rev() {
                    //render_state = self.paint_child(cx, child, render_state);
                    //if render_state.terminate           { return (render_state, elem.point.y, index); }
                    last_index = index.clone();

                    (render_state, new_offset_y, index) = self.paint_backward(cx, child, render_state, level + 1, index);
                    if render_state.terminate           {return (render_state, new_offset_y, index); }
                    if index[level] != 0    { index[level] -= 1; }
                    //else {index.pop();}
                }
                index.pop();
            }
            ElemType::Lines(lines) => {
                let mut line_offset_y = elem.size.height;
                let mut dummy_state = render_state.clone();
                for line in lines.elem_lines.iter().rev() {
                    dummy_state  = self.paint_line(cx, &elem, &line, line_offset_y, dummy_state , false);
                    if dummy_state .terminate       { return (dummy_state , elem.point.y, index);}
                    line_offset_y -= line.height;
                }
                line_offset_y = elem.size.height;
                for line in lines.elem_lines.iter().rev() {
                    render_state = self.paint_line(cx, &elem, &line, line_offset_y, render_state, true);
                    if render_state.terminate {return (render_state, elem.point.y, index)}
                    line_offset_y -= line.height;
                }
            }
        }
        (render_state, new_offset_y, index)
    }

    fn paint_recursive(&self, cx: &mut PaintCx, elem: &Elem, mut render_state: RenderState, level: usize, mut index: Vec<usize>) -> (RenderState, f64, Vec<usize>){
        let mut new_offset_y = 0.;
        match &elem.elem_type {
            ElemType::Block(block) => {
                if index.len() <= level { index.push(0);}
                for child in block.children.iter().skip(index[level]) {
                    //render_state = self.paint_child(cx, child, render_state);
                    //if render_state.terminate           { return (render_state, elem.point.y, index); }
                    (render_state, new_offset_y, index) = self.paint_recursive(cx, child, render_state, level + 1, index);
                    if render_state.terminate           { return (render_state, new_offset_y, index); }
                    index[level] += 1;
                }
                if index.len() != 1 { index.pop(); }
            }
            ElemType::Lines(lines) => {
                let mut line_offset_y = 0.;
                let mut dummy_state = render_state.clone();
                for line in lines.elem_lines.iter() {
                    dummy_state  = self.paint_line(cx, &elem, &line, line_offset_y, dummy_state , false);
                    if dummy_state .terminate       { return (dummy_state , elem.point.y, index);}
                    line_offset_y += line.height;
                }
                line_offset_y = 0.;
                for line in lines.elem_lines.iter() {
                    render_state  = self.paint_line(cx, &elem, &line, line_offset_y, render_state , true);
                    if render_state .terminate       { return (render_state , elem.point.y, index);}
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
                    Key::Named(NamedKey::ArrowUp)       => {self.goto_last()}
                    _ => ()
                }
            }
            _ => ()
        }
        cx.app_state_mut().request_paint(self.id());
        EventPropagation::Continue
    }
    fn paint(&mut self, cx: &mut PaintCx) {
        let root_elem = self.pages.get(&self.current_url).unwrap();
        let size                = cx.size();
        self.col_count          = (size.width / self.col_width).floor();
        self.size               = Size::new(size.width, size.height);
        self.col_gap            = (size.width - self.col_count * self.col_width) / (self.col_count + 1.);
        let mut render_state    = RenderState {x: 0., y: 0., col_index: 0., terminate: false};
        if self.render_forward {
            if self.start_index.len() != 0 {
                let first_elem = root_elem.get_elem(&self.start_index, 0);
                self.start_offset_y = first_elem.point.y;
            }
            let now = Instant::now();
            (_, self.end_offset_y, self.end_index) = self.paint_recursive(cx, root_elem, render_state, 0, self.start_index.clone());
            println!("End index: {:#?}", self.end_index);
        }
        else {
            let last_elem = root_elem.get_elem(&self.end_index, 0);
            let content_height      = self.size.height * self.col_count;
            self.start_offset_y     = last_elem.point.y + last_elem.size.height + 0.0;
            self.start_offset_y     -= content_height as f64;
            if (self.start_offset_y < 0.) {
                self.start_index = Vec::new();
                self.start_offset_y = 0.;
                self.render_forward = true;
                (_, self.end_offset_y, self.end_index) = self.paint_recursive(cx, root_elem, render_state, 0, self.start_index.clone());
                return;
            }
            (render_state, _, self.start_index) = self.paint_backward(cx, root_elem, render_state, 0, self.end_index.clone());

           // println!("Start index: {:#?}", self.start_index);
            //self.render_forward = true;
        }
    }
    
}