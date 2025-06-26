use std::collections::HashMap;
use std::ops::Deref;
use std::time;
use std::time::Instant;
use floem::{Clipboard, View, ViewId};
use floem::context::{ComputeLayoutCx, EventCx, PaintCx};
use floem::event::{Event, EventPropagation};
use floem::keyboard::{Key, Modifiers, NamedKey};
use floem::kurbo::{Point, Rect, Size};
use floem::prelude::{Color, RwSignal, SignalUpdate};
use floem::reactive::{ReadSignal, SignalGet, SignalRead, WriteSignal};
use floem::style::{Cursor, CursorStyle};
use floem::views::Decorators;
use floem_renderer::{Img, Renderer};
use floem_renderer::text::TextLayout;
use sha2::Digest;

use crate::book_elem::{CharGlyph, Elem, ElemLine, ElemType, HTMLPage, InlineContent};
use crate::glyph_interner::GlyphCache;
use crate::renderer::navigation::{Navigator, NavigationState};

#[derive(Clone)]
pub(crate) struct RenderState {
    x: f64,
    y: f64,
    col_index: f32,
    line_index: isize,
    terminate: bool,
    selected_text: String,
    first_line_rendered: bool,
    pub(crate) selection: Option<Selection>,
    clicked_link: Option<String>,
}

#[derive(Clone)]
pub struct Selection {
    pub(crate) start_col: usize,
    pub(crate) end_col: usize,
    pub(crate) start_selection: Point,
    pub(crate) end_selection: Point,
}

impl Selection {
    pub fn new(start_col: usize, end_col: usize, start_selection: Point, end_selection: Point) -> Self {
        Self {start_col, end_col, start_selection, end_selection}
    }
}

pub struct HtmlRenderer {
    id: ViewId,

    navigator: Navigator,
    nav_state: NavigationState,

    size: Size,
    point: Point,
    col_width: f64,
    orig_col_width: f64,
    col_count: f64,
    col_gap: f64,
    scale: f64,

    glyph_cache : GlyphCache,

    end_offset_y: f64,

    line_reader_assist_x_index: isize,
    line_reader_assist_y_index: isize,
    click_location: Option<Point>,
    pub(crate) press_location: Option<Point>,

    pub(crate) move_location: Point,

    copy: bool,
    pub(crate) selection_active: bool,
    drag_in_progress: bool,
    key_press: bool,
    clicked_link: Option<String>,

}

impl HtmlRenderer {

    pub fn new(start_index: RwSignal<Vec<usize>>, glyph_cache: GlyphCache, pages: HashMap<String, HTMLPage>, read_current_url: RwSignal<String>, at_ends: WriteSignal<i8>, get_go_on: ReadSignal<bool>) -> Self{
        let navigator = Navigator::new(
            pages,
            read_current_url,
            start_index,
            RwSignal::new(0),
            RwSignal::new(0),
            at_ends,
            get_go_on,
        );
        let nav_state = NavigationState::default();
        
        let mut html_renderer = HtmlRenderer {
            id: ViewId::new(),
            navigator,
            nav_state,
            col_gap: 0., col_count: 0., col_width: 600., orig_col_width: 600., 
            size: Size::default(), point: Point::default(), end_offset_y: 0.,
            scale: 1.0, 
            glyph_cache,
            line_reader_assist_x_index: -1, line_reader_assist_y_index: -1,
            click_location: None, press_location: None, move_location: Point::default(),
            copy: false, selection_active: false, drag_in_progress: false, key_press: false,
            clicked_link: None,
        };
        html_renderer = html_renderer.keyboard_navigable();
        html_renderer
    }

    pub(crate) fn get_col_index(&self, x: f64) -> usize {
        ((x) / (self.col_width + self.col_gap)).floor() as usize
    }
    
    pub fn next(&mut self) {
        self.nav_state = self.navigator.next(&self.nav_state);
    }

    pub fn prev(&mut self) {
        self.nav_state = self.navigator.prev(&self.nav_state);
    }

    pub fn goto(&mut self, link: &str) {
        self.nav_state = self.navigator.goto(link);
    }

    pub fn goto_last(&mut self) {
        self.nav_state = self.navigator.goto_last();
    }

    fn resolve_point(&self, point: Point, elem_height: f64, mut render_state: RenderState) -> (RenderState, Point) {
        let mut y = point.y + render_state.y - self.nav_state.start_offset_y;
        let mut col_index = (y / self.size.height ).floor();
        y = y - col_index * self.size.height;
        if y + elem_height > self.size.height {
            render_state.col_index += 1.0;
            if self.nav_state.render_forward {
                col_index += 1.0;
                render_state.y += self.size.height - y;
                y = 0.;
            }
            else {
                render_state.y -= y - self.size.height + elem_height;
                y = self.size.height as f64 - elem_height;
            }
        }
        let x = (self.col_gap + col_index * (self.col_width+ self.col_gap)) as f64 + point.x;
        if self.nav_state.render_forward && x + 1.0  >= self.size.width { render_state.terminate = true; }
        if !self.nav_state.render_forward && x < 0.               {render_state.terminate = true; };
        (render_state, Point::new(x, y))
    }



    fn paint_line(&self, cx: &mut PaintCx, elem: &Elem, line: &ElemLine, line_offset_y: f64, mut render_state: RenderState, render: bool) -> RenderState {
        let mut reader_assist_y = false;
        let mut x_index = 0;
        if self.line_reader_assist_y_index == render_state.line_index {
            reader_assist_y = true;
            render_state.y += 20.
        }
        let mut line_point          = Point::new(elem.point.x, elem.point.y + line_offset_y);
        (render_state, line_point)  = self.resolve_point(line_point, line.height, render_state);
        if render_state.terminate {return render_state}
        if !render {return render_state}

            for elem in line.inline_elems.iter() {
                let elem_point = Point::new(line_point.x + elem.x, line_point.y);
                let mut elem_width = 0.;
                match &elem.inline_content {
                    InlineContent::Text(text) => {
                        for char_glyph in text {
                            let glyph = self.glyph_cache.get(char_glyph.char);
                            let ascent = glyph.lines().first().unwrap().layout_opt().as_ref().unwrap().first().as_ref().unwrap().max_ascent as f64;
                            let descent = glyph.lines().first().unwrap().layout_opt().as_ref().unwrap().first().as_ref().unwrap().max_descent as f64;

                            elem_width = char_glyph.x;
                            let gx0 = elem_point.x + char_glyph.x as f64;
                            let gy0 = elem_point.y;
                            let gx1 = gx0 + glyph.size().width + 1.0;
                            let gy1 = gy0 + line.height + 1.0;
                            if self.selection_active && self.hit(&render_state, gx0, gy0, gx1, gy1){
                                let rect = Rect::new(gx0, gy0, gx1, gy1);
                                let text = glyph.lines().first().unwrap().text();
                                render_state.selected_text.push_str(text);
                                cx.fill(&rect, Color::LIGHT_BLUE, 0.);
                            }

                            //cx.draw_text(glyph, Point::new(elem_point.x + char_glyph.x as f64, elem_point.y + line.height - glyph.size().height))
                            cx.draw_text(glyph, Point::new(elem_point.x + char_glyph.x as f64, elem_point.y + line.height / 1.6 - ascent - descent));
                        }
                    }
                    InlineContent::Link((text, link)) => {
                        for char_glyph in text {
                            let glyph = self.glyph_cache.get(char_glyph.char);
                            elem_width = char_glyph.x;
                            let x = elem_point.x + char_glyph.x as f64;
                            cx.draw_text(glyph, Point::new(x, elem_point.y));
                            let descent = glyph.lines().first().unwrap().layout_opt().as_ref().unwrap().first().as_ref().unwrap().max_descent as f64;
                            let y = elem_point.y + glyph.size().height - descent;
                            let x0 = x;
                            let x1 = x0 + glyph.size().width + 1.0;
                            let rect = Rect::new(x0 - 1., y, x1 + 1., y + 2.0);
                            cx.fill(&rect, Color::DARK_GREEN, 0.);
                            if let Some(location) = self.click_location {
                                if x <= location.x && location.x <= x1 && elem_point.y <= location.y
                                    && location.y <= elem_point.y + glyph.size().height {
                                    render_state.clicked_link = Some(link.clone());
                                }
                                self.id.get_combined_style().cursor(CursorStyle::Pointer) ;
                                self.id.request_style();
                            }
                        }
                    }
                    InlineContent::Image(image_elem) => {
                        let image_promise = image_elem.image_promise.read().unwrap();
                        match image_promise.deref() {
                            None => {println!("Found no image")}
                            Some(image) => {

                                let rect = Rect::new(line_point.x + elem.x, line_point.y, line_point.x + elem.x + image_elem.width as f64, line_point.y + image_elem.height as f64);
                                let img = Img {img: image.0.clone(), hash: &image.1};
                                cx.draw_img(img, rect);
                               // println!("Rendered image: {}", line_point.x);
                            }
                        }
                    }
                }
                if reader_assist_y && self.line_reader_assist_x_index == x_index {
                    let rect = Rect::new(elem_point.x, line_point.y, elem_point.x + elem_width as f64, line_point.y + 2.0);
                    cx.fill(&rect, Color::BLACK, 0.);
                }
                if reader_assist_y {println!("{}, {}", self.line_reader_assist_x_index, x_index);}
                if elem_width == 0. {x_index -= 1}
                x_index += 1;
        }
        //println!("{}, {}", self.line_reader_assist_y_index, render_state.line_index);
        if self.line_reader_assist_y_index == render_state.line_index {
            let rect = Rect::new(line_point.x, line_point.y + line.height, line_point.x + self.col_width, line_point.y + line.height + 2.0);
            cx.fill(&rect, Color::BLACK, 0.);

            render_state.y += 20.;
        }
        render_state.line_index += 1;
        render_state
    }

    fn paint_backward(&self, cx: &mut PaintCx, elem: &Elem, mut render_state: RenderState, level: usize, mut index: Vec<usize>) -> (RenderState, Vec<usize>, usize){
        let mut elem_index = 0;
        match &elem.elem_type {
            ElemType::Block(block) => {
                if block.children.len() == 0    { return (render_state, index, elem_index) }
                if index.len() <= level         { index.push(block.children.len() - 1); }
                for child in block.children.iter().take(index[level] + 1).rev() {
                    (render_state, index, elem_index) = self.paint_backward(cx, child, render_state, level + 1, index);
                    if render_state.terminate       {return (render_state, index, elem_index); }
                    if index[level] != 0            { index[level] -= 1; }
                }
                index.pop();
            }
            ElemType::Lines(lines) => {
                let mut line_offset_y   = elem.size.height;
                /*let mut dummy_state     = render_state.clone();
                for line in lines.elem_lines.iter().rev() {
                    line_offset_y -= line.height;
                    dummy_state  = self.paint_line(cx, &elem, &line, line_offset_y, dummy_state , false);
                    if dummy_state.terminate       { return (dummy_state, index); }
                }*/
                line_offset_y = elem.size.height;
                let mut current_elem_index: usize = lines.elem_lines.iter().map(|s| s.inline_elems.len()).sum();
                for line in lines.elem_lines.iter().rev() {
                    line_offset_y -= line.height;
                    if self.nav_state.end_elem_index == 0 || self.nav_state.end_elem_index > current_elem_index - line.inline_elems.len() {render_state.first_line_rendered = true;}
                    if render_state.first_line_rendered {

                        render_state = self.paint_line(cx, &elem, &line, line_offset_y, render_state, true);
                        if render_state.terminate {return (render_state, index, current_elem_index)}
                    }
                    current_elem_index -= line.inline_elems.len();
                    
                }
            }
        }
        render_state.first_line_rendered = true;
        (render_state, index, elem_index)
    }

    fn paint_recursive(&self, cx: &mut PaintCx, elem: &Elem, mut render_state: RenderState, level: usize, mut index: Vec<usize>) -> (RenderState, Vec<usize>, usize){
        let mut elem_index = 0;
        match &elem.elem_type {
            ElemType::Block(block) => {
                if index.len() <= level { index.push(0);}
                for child in block.children.iter().skip(index[level]) {
                    (render_state, index, elem_index) = self.paint_recursive(cx, child, render_state, level + 1, index);
                    if render_state.terminate           { return (render_state, index, elem_index); }
                    match child.elem_type {
                        ElemType::Block(_) => {index[level] += 1;}
                        ElemType::Lines(_) => {index[level] += 1;}
                    }

                }
                if index.len() != 1 { index.pop(); }
            }
            ElemType::Lines(lines) => {
                let mut line_offset_y = 0.;
                let mut dummy_state = render_state.clone();
                /*for line in lines.elem_lines.iter() {
                    dummy_state  = self.paint_line(cx, &elem, &line, line_offset_y, dummy_state , false);
                    if dummy_state .terminate       { return (dummy_state, index);}
                    line_offset_y += line.height;
                }*/
                line_offset_y = 0.;
                let mut current_elem_index = 0;
                for line in lines.elem_lines.iter() {
                    if self.nav_state.start_elem_index < current_elem_index + line.inline_elems.len() {render_state.first_line_rendered = true}
                    if render_state.first_line_rendered  {
                        render_state  = self.paint_line(cx, &elem, &line, line_offset_y, render_state , true);
                        if render_state .terminate       { return (render_state, index, current_elem_index);}
                    }
                    current_elem_index += line.inline_elems.len();
                    line_offset_y += line.height;
                }
            }
        }
        render_state.first_line_rendered = true;
        (render_state, index, elem_index)
    }

}

impl View for HtmlRenderer {
    fn id(&self) -> ViewId { self.id }
    fn event_before_children(&mut self, cx: &mut EventCx, event: &Event) -> EventPropagation {
        match &event {
            Event::KeyDown(event) => {

                if event.modifiers.control(){
                    match &event.key.logical_key {
                        Key::Named(NamedKey::ArrowRight) => self.line_reader_assist_x_index += 1,
                        Key::Named(NamedKey::ArrowLeft)  => self.line_reader_assist_x_index -= 1,
                        Key::Character(str) => {
                            if str.eq("c") {
                                self.copy = true;
                            }
                            if str.eq("+") || str.eq("-") {
                                let orig_scale = self.scale;
                                let orig_move_x = self.move_location.x * self.scale;
                                let orig_move_y = self.move_location.y * self.scale;
                                let mut orig_press_x = 0.;
                                let mut orig_press_y = 0.;
                                if let Some(mut loc) = self.press_location  {
                                    orig_press_x = loc.x * self.scale;
                                    orig_press_y = loc.y * self.scale;
                                }
                                if str.eq("+") {
                                    self.scale += 0.1;
                                }
                                if str.eq("-") {
                                    self.scale -= 0.1;
                                }
                                let scale_change = orig_scale - self.scale;
                                if let Some(mut loc) = self.press_location  {
                                   loc.x = orig_press_x / self.scale;
                                    loc.y = orig_press_y / self.scale;
                                }
                                self.move_location.x = orig_move_x / self.scale;
                                self.move_location.y = orig_move_y / self.scale;
                            }

                        }
                        _ => ()
                    }
                }
                else {
                    match event.key.logical_key {
                        Key::Named(NamedKey::ArrowRight)    => {self.next()},
                        Key::Named(NamedKey::ArrowLeft)     => {self.prev()}
                        Key::Named(NamedKey::ArrowUp)       => {self.line_reader_assist_y_index -= 1}
                        Key::Named(NamedKey::ArrowDown)     => {self.line_reader_assist_y_index += 1}
                        Key::Named(NamedKey::F11)           => {self.id.inspect()}
                        _ => ()
                    }
                }
                cx.app_state_mut().request_paint(self.id());
            }
            Event::PointerWheel(event) => {
                if event.delta.y > 0.       {self.next()}
                else if event.delta.y < 0.  {self.prev()}
                cx.app_state_mut().request_paint(self.id());

            }
            Event::PointerUp(event) => {
                self.click_location = Some(Point::new(event.pos.x / self.scale, event.pos.y / self.scale));
                self.drag_in_progress = false;
                self.key_press = false;
                //self.press_location = None;
                cx.app_state_mut().request_paint(self.id());
            }
            Event::PointerDown(event) => {
                if self.selection_active {
                    self.selection_active = false;
                }
                self.key_press = true;
                self.press_location = Some(Point::new(event.pos.x / self.scale, event.pos.y / self.scale));
            }
            Event::PointerMove(event) => {
                if self.key_press {
                    self.move_location = Point::new(event.pos.x / self.scale, event.pos.y / self.scale);
                    self.drag_in_progress = true;
                    self.selection_active = true;
                    cx.app_state_mut().request_paint(self.id);
                }
            }

            _ => ()
        }
        EventPropagation::Continue
    }
    fn compute_layout(&mut self, cx: &mut ComputeLayoutCx) -> Option<Rect> {
        self.point = cx.window_origin();
        None
    }
    fn paint(&mut self, cx: &mut PaintCx) {
        let now = Instant::now();
        let current_url = self.navigator.get_current_url();
        let root_elem = &self.navigator.get_pages().get(&current_url).unwrap().root;
        self.size = self.id.get_size().unwrap();
        self.size.width /= self.scale;
        self.size.height /= self.scale;
        self.col_count = (self.size.width / self.col_width).floor();
        self.col_gap = (self.size.width - self.col_count * self.col_width) / (self.col_count + 1.);
        
        let mut render_state = RenderState {
            x: 0., y: 0., col_index: 0., terminate: false, line_index: 0, 
            selected_text: String::new(), first_line_rendered: false, 
            selection: self.get_selection(), clicked_link: None,
        };
        
        let mut start_index = self.nav_state.start_index.clone();
        let mut start_elem_index = self.nav_state.start_elem_index;
        let mut end_elem_index = self.nav_state.end_elem_index;
        
        let scaling_offset_x = self.point.x / self.scale - self.point.x;
        let scaling_offset_y = self.point.y / self.scale - self.point.y;
        cx.set_scale(self.scale);
        cx.offset((scaling_offset_x, scaling_offset_y));
        
        if self.nav_state.render_forward {
            let first_elem = root_elem.get_elem(&start_index, 0);
            self.nav_state.start_offset_y = first_elem.get_y(start_elem_index);
            (render_state, self.nav_state.end_index, end_elem_index) = self.paint_recursive(cx, root_elem, render_state, 0, start_index);
            self.nav_state.end_elem_index = end_elem_index;
        }
        else {
            let last_elem = root_elem.get_elem(&self.nav_state.end_index, 0);
            let content_size = self.size.height * self.col_count;
            self.nav_state.start_offset_y = last_elem.get_y(end_elem_index) - content_size + 1.;

            if self.nav_state.start_offset_y < 0. {
                self.nav_state.start_index = Vec::new();
                start_index = Vec::new();
                self.nav_state.start_offset_y = 0.;
                self.nav_state.start_elem_index = 0;
                self.nav_state.end_elem_index = 0;
                self.nav_state.render_forward = true;
                (render_state, self.nav_state.end_index, end_elem_index) = self.paint_recursive(cx, root_elem, render_state, 0, start_index);
                self.nav_state.end_elem_index = end_elem_index;
            }
            else {
                (render_state, start_index, start_elem_index) = self.paint_backward(cx, root_elem, render_state, 0, self.nav_state.end_index.clone());
                self.nav_state.start_elem_index = start_elem_index;
                self.nav_state.start_index = start_index;
            }
        }
        
        if self.copy {
            println!("Clipboard: {}", render_state.selected_text);
            Clipboard::set_contents(render_state.selected_text).unwrap();
            self.copy = false;
        }
        
        if let Some(link) = render_state.clicked_link {
            self.nav_state = self.navigator.goto(&link);
        }
        
        cx.set_scale(1.0);
        self.click_location = None;
    }

}
