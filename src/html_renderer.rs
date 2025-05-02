use std::collections::HashMap;
use std::ops::Deref;
use std::time;
use std::time::Instant;
use floem::{Clipboard, View, ViewId};
use floem::context::{EventCx, PaintCx};
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

#[derive(Clone)]
struct RenderState {
    x: f64,
    y: f64,
    col_index: f32,
    line_index: isize,
    terminate: bool,
    selected_text: String,
    first_line_rendered: bool,
    selection: Option<Selection>
}

#[derive(Clone)]
struct Selection {
    start_col: usize,
    end_col: usize,
    start_selection: Point,
    end_selection: Point,
}

impl Selection {
    pub fn new(start_col: usize, end_col: usize, start_selection: Point, end_selection: Point) -> Self {
        Self {start_col, end_col, start_selection, end_selection}
    }
}

pub struct HtmlRenderer {
    id: ViewId,

    read_current_url: RwSignal<String>,
    pages: HashMap<String, HTMLPage>,

    start_index: RwSignal<Vec<usize>>,
    end_index: Vec<usize>,

    start_elem_index: RwSignal<usize>,
    end_elem_index: RwSignal<usize>,

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

    line_reader_assist_x_index: isize,
    line_reader_assist_y_index: isize,
    click_location: Option<Point>,
    press_location: Option<Point>,

    move_location: Point,

    copy: bool,
    selection_active: bool,
    drag_in_progress: bool,
    key_press: bool,

}

impl HtmlRenderer {

    pub fn new(start_index: RwSignal<Vec<usize>>, glyph_cache: GlyphCache, pages: HashMap<String, HTMLPage>, read_current_url: RwSignal<String>, at_ends: WriteSignal<i8>, get_go_on: ReadSignal<bool>) -> Self{
        let mut html_renderer = HtmlRenderer {
            id: ViewId::new(), start_index, end_index: Vec::new(),
            start_elem_index: RwSignal::new(0), end_elem_index: RwSignal::new(0),
            col_gap: 0., col_count: 0., col_width: 600., orig_col_width: 600., 
            size: Size::default(), start_offset_y: 0., end_offset_y: 0.,
            render_forward: true, scale: 1.0, 
            glyph_cache, pages,
            read_current_url, get_go_on, at_ends,
            line_reader_assist_x_index: -1, line_reader_assist_y_index: -1,
            click_location: None, press_location: None, move_location: Point::default(),
            copy: false, selection_active: false, drag_in_progress: false, key_press: false,
        };
        html_renderer = html_renderer.keyboard_navigable();
        html_renderer
    }

    fn get_col_index(&self, x: f64) -> usize {
        ((x) / (self.col_width + self.col_gap)).floor() as usize
    }

    fn get_selection(&self) -> Option<Selection> {
        if !self.selection_active {return None}
        let press_x         = self.press_location.unwrap().x;
        let press_y         = self.press_location.unwrap().y;
        let press_col_index = self.get_col_index(press_x);
        let move_x          = self.move_location.x;
        let move_y          = self.move_location.y;
        let move_col_index  = self.get_col_index(move_x);
        if press_col_index < move_col_index {return Some(Selection::new(press_col_index, move_col_index, self.press_location.unwrap(), self.move_location))}
        if press_col_index > move_col_index {return Some(Selection::new(move_col_index, press_col_index, self.move_location, self.press_location.unwrap()))}
        if press_y         < move_y         {return Some(Selection::new(press_col_index, move_col_index, self.press_location.unwrap(), self.move_location))}
        if press_y         > move_y         {return Some(Selection::new(move_col_index, press_col_index, self.move_location, self.press_location.unwrap()))}
        return Some(Selection::new(move_col_index, press_col_index, self.move_location, self.press_location.unwrap()))
    }

    fn hit(&self, render_state: &RenderState, gx0: f64, gy0: f64, gx1: f64, gy1: f64,) -> bool{
        let glyph_col_index = self.get_col_index(gx0);
        let selection = render_state.selection.as_ref().unwrap();
        // Middle column
        if selection.start_col < glyph_col_index && glyph_col_index < selection.end_col {
            return true;
        }

        let is_first_line = gy0 < selection.start_selection.y && gy1 > selection.start_selection.y;
        let is_last_line = gy0 < selection.end_selection.y && gy1 > selection.end_selection.y;
        let is_first_x = gx1 >= selection.start_selection.x;
        let is_last_x = gx1 <= selection.end_selection.x;
        // Single column
        if selection.start_col == glyph_col_index && selection.end_col == glyph_col_index {
            if is_first_line && is_last_line {
                let x_min = selection.start_selection.x.min(selection.end_selection.x);
                let x_max = selection.start_selection.x.max(selection.end_selection.x);
                if gx1 > x_min && gx1 < x_max {return true}
            }
            else if is_first_line {
                if is_first_x {return true};
            }
            else if is_last_line {
                if is_last_x {return true}
            }
            else if gy0 > selection.start_selection.y && gy1 < selection.end_selection.y {
                return true;
            }
            
            return false;
        }

        // First column
        if selection.start_col == glyph_col_index {
            if is_first_line {
                if is_first_x {return true}
            }
            else if gy0 > selection.start_selection.y {
                return true;
            }
        }
        //End column
        if selection.end_col == glyph_col_index {
            if is_last_line {
                if is_last_x {return true}
            }
            else if gy0 < selection.end_selection.y {
                return true
            }
        }

        false
    }
    
    pub fn next(&mut self) {
        self.render_forward = true;
        if self.end_index.len() != 0 && self.end_index[0] == 1 {
            self.at_ends.set(1);
            if !self.get_go_on.get() {return}
            self.start_index.set(Vec::new());
            self.start_offset_y = 0.;
            self.start_elem_index.set(0);
            self.end_elem_index.set(0);
            self.end_index = Vec::new();
            return;
        }
        self.at_ends.set(0);
        self.start_index.set(self.end_index.clone());
        self.start_elem_index.set(self.end_elem_index.get_untracked());
    }

    pub fn prev(&mut self ) {
        self.render_forward = false;
        if self.start_index.get().len() == 0 {
            self.at_ends.set(-1);
            if !self.get_go_on.get() {return}
            self.start_elem_index.set(0);
            self.end_elem_index.set(0);
            self.goto_last();
            return
        }
        self.at_ends.set(0);
        self.end_index      = self.start_index.get_untracked();
        self.end_elem_index.set(self.start_elem_index.get_untracked());
    }

    pub fn goto(&self, link: &String) {
        if link.contains("www") || link.contains("http") {
            open::that(link).unwrap();
            return;
        }
        println!("Clicked link: {link}");
        let parts: Vec<&str> = link.split("#").collect();
        let mut new_url = parts[0].to_string();
        let current_url = self.read_current_url.get_untracked();
        let mut paths: Vec<&str> = current_url.split("/").collect();
        if paths.len() > 1 {
            println!("Len: {}", paths.len());
            paths.pop().unwrap();
            let path = paths.join("/");
            new_url = format!("{path}/{new_url}")
        }
        println!("Current url: {current_url}\t New url: {new_url}");

        let document = &self.pages.get(&new_url).unwrap();
        self.read_current_url.set(new_url);
        self.start_elem_index.set(0);
        self.end_elem_index.set(0);
        if parts.len() == 1 {
            self.start_index.set(Vec::new());
            return;
        }
        let new_index = parts[1].to_string();
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
        let x = (self.col_gap + col_index * (self.col_width+ self.col_gap)) as f64 + point.x;
        if self.render_forward && x + 1.0  >= self.size.width { render_state.terminate = true; }
        if !self.render_forward && x < 0.               {render_state.terminate = true; };
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
        if !render {return render_state}

            for elem in line.inline_elems.iter() {
                let elem_point = Point::new(line_point.x + elem.x, line_point.y);
                let mut elem_width = 0.;
                match &elem.inline_content {
                    InlineContent::Text(text) => {
                        for char_glyph in text {
                            let glyph = self.glyph_cache.get(char_glyph.char);
                            elem_width = char_glyph.x;
                            if self.selection_active {//&& //let Some(location) = self.press_location {
                                let gx0 = elem_point.x + char_glyph.x as f64;
                                let gy0 = elem_point.y;
                                let gx1 = gx0 + glyph.size().width;
                                let gy1 = gy0 + glyph.size().height;
                                //if hit4(self.press_location.unwrap(), self.move_location, gx0, gy0, gx1, gy1, self.col_count, self.col_width, self.col_gap)
                                if self.hit(&render_state, gx0, gy0, gx1, gy1)
                                {
                                    let rect = Rect::new(gx0, gy0, gx1, gy1);
                                    let text = glyph.lines().first().unwrap().text();
                                    render_state.selected_text.push_str(text);
                                    cx.fill(&rect, Color::LIGHT_BLUE, 0.);
                                }
                            }
                            cx.draw_text(glyph, Point::new(elem_point.x + char_glyph.x as f64, elem_point.y))

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
                            let x1 = x0 + glyph.size().width;
                            let rect = Rect::new(x0 - 1., y, x1 + 1., y + 2.0);
                            cx.fill(&rect, Color::DARK_GREEN, 0.);
                            if let Some(location) = self.click_location {
                                if x <= location.x && location.x <= x1 && elem_point.y <= location.y
                                    && location.y <= elem_point.y + glyph.size().height {
                                    self.goto(link);

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
                                let rect = Rect::new(line_point.x, line_point.y, line_point.x + image_elem.width as f64, line_point.y + image_elem.height as f64);
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
                    if self.end_elem_index.get() == 0 || self.end_elem_index.get() > current_elem_index - line.inline_elems.len() {render_state.first_line_rendered = true;}
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
                    if self.start_elem_index.get() < current_elem_index + line.inline_elems.len() {render_state.first_line_rendered = true}
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
                self.click_location = Some(event.pos);
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
                self.press_location = Some(event.pos);
            }
            Event::PointerMove(event) => {
                if self.key_press {
                    self.move_location = event.pos;
                    self.drag_in_progress = true;
                    self.selection_active = true;
                    cx.app_state_mut().request_paint(self.id);
                }
            }

            _ => ()
        }
        EventPropagation::Continue
    }
    fn paint(&mut self, cx: &mut PaintCx) {
        let now = Instant::now();
        let root_elem           = &self.pages.get(&self.read_current_url.get()).unwrap().root;
        self.size               = self.id.get_size().unwrap();
        self.col_count          = (self.size.width / self.col_width).floor();
        self.col_gap            = (self.size.width - self.col_count * self.col_width) / (self.col_count + 1.);
        let mut render_state        = RenderState {x: 0., y: 0., col_index: 0., terminate: false, line_index: 0, selected_text: String::new(), first_line_rendered: false, selection: self.get_selection()};
        let mut start_index     = self.start_index.get();
        let mut start_elem_index = self.start_elem_index.get_untracked();
        let mut end_elem_index = self.end_elem_index.get_untracked();
        if self.render_forward {
            //if start_index.len() != 0 {
                let first_elem      = root_elem.get_elem(&start_index, 0);
                self.start_offset_y = first_elem.get_y(self.start_elem_index.get_untracked());
            //}
            (render_state, self.end_index, end_elem_index) = self.paint_recursive(cx, root_elem, render_state, 0, start_index);
            self.end_elem_index.set(end_elem_index);
        }
        else {
            let last_elem       = root_elem.get_elem(&self.end_index, 0);
            let content_size    = self.size.height * self.col_count;
            self.start_offset_y = last_elem.get_y(self.end_elem_index.get_untracked()) - content_size + 1.;

            if self.start_offset_y < 0. {
                self.start_index.set( Vec::new());
                start_index = Vec::new();
                self.start_offset_y = 0.;
                self.start_elem_index.set(0);
                self.end_elem_index.set(0);
                self.render_forward = true;
                (render_state, self.end_index, end_elem_index) = self.paint_recursive(cx, root_elem, render_state, 0, start_index);
                self.end_elem_index.set(end_elem_index);
                return;
            }
            (render_state, start_index, start_elem_index) = self.paint_backward(cx, root_elem, render_state, 0, self.end_index.clone());
            self.start_elem_index.set(start_elem_index);
            self.start_index.set(start_index);
        }
        if self.copy {
            println!("Clipboard: {}", render_state.selected_text);
            self.copy = false;
        }
        self.click_location = None;
        //println!("Render time: {}", now.elapsed().as_micros())

    }

}
fn hit(drag_start: Point, drag_end: Point, cg: &CharGlyph, g: &TextLayout, line_origin: Point) -> bool{
    let (sx0, sx1) = (drag_start.x.min(drag_end.x), drag_start.x.max(drag_end.x));
    let (sy0, sy1) = (drag_start.y.min(drag_end.y), drag_start.y.max(drag_end.y));
    let downward = drag_start.y < drag_end.y;
    let x0 = line_origin.x + cg.x as f64;
    let y0 = line_origin.y;
    let x1 = x0 + g.size().width;
    let y1 = y0 + g.size().height;

    let is_first  = y0 <= drag_start.y && drag_start.y <= y1;
    let is_last   = y0 <= drag_end.y   && drag_end.y   <= y1;
    let is_middle = sy0 < y0 && y1 < sy1;

    let hit = if is_middle {
        true                          // full line
    } else if is_first && is_last {
        x1 >= sx0 && x0 <= sx1        // single-line drag
    } else if is_first {
        if downward { x1 >= sx0 }     // rest of first line
        else        { x0 <= sx1 }     // upward rest of first
    } else if is_last {
        if downward { x0 <= sx1 }     // start→end on last line
        else        { x1 >= sx0 }     // upward rest of last
    } else {
        false
    };
    hit
}

fn hit2(start: Point, end: Point, gx0: f64, gy0: f64, gx1: f64, gy1: f64) -> bool{
    let start_y = start.y;
    let start_x = start.x;
    let end_y   = end.y;
    let end_x   = end.x;

    // canonical vertical bounds of the drag
    let y0 = start_y.min(end_y);
    let y1 = start_y.max(end_y);

    let dragging_down = start_y < end_y;

    // Which “zone” is this line in?
    let is_start_line  = (start_y >= gy0 && start_y <= gy1);
    let is_end_line    = (end_y   >= gy0 && end_y   <= gy1);
    let is_middle_line = (y0 < gy0 && y1 > gy1);

    // Decide per‑glyph whether to fill
    let mut hit = false;

    if is_middle_line {
        // selection spans fully above & below → full line
        hit = true;
    }
    else if is_start_line && !is_end_line {
        // the very first line of the drag
        if dragging_down {
            // downward: select from start_x → line end
            if gx1 >= start_x {
                hit = true;
            }
        } else {
            // upward: select from line start → start_x
            if gx0 <= start_x {
                hit = true;
            }
        }
    }
    else if is_end_line && !is_start_line {
        // the very last line of the drag
        if dragging_down {
            // downward: select from line start → end_x
            if gx0 <= end_x {
                hit = true;
            }
        } else {
            // upward: select from end_x → line end
            if gx1 >= end_x {
                hit = true;
            }
        }
    }
    else if is_start_line && is_end_line {
        // single-line drag: between start_x and end_x
        let x_min = start_x.min(end_x);
        let x_max = start_x.max(end_x);
        if gx1 >= x_min && gx0 <= x_max {
            hit = true;
        }
    }
    hit
}

fn hit3(
    start: Point,
    end: Point,
    gx0: f64,
    gy0: f64,
    gx1: f64,
    gy1: f64,
    cols: f64,
    col_w: f64,
    col_gap: f64,
) -> bool {
    // 1) normalize the drag rect
    let sx0 = start.x.min(end.x);
    let sx1 = start.x.max(end.x);
    let sy0 = start.y.min(end.y);
    let sy1 = start.y.max(end.y);
    let down = start.y < end.y;

    // 2) figure out which column start, end and this glyph live in
    let stride = col_w + col_gap;
    let last_col = (cols as i32) - 1;
    let col_of = |x: f64| {
        ((x / stride).floor() as i32)
            .clamp(0, last_col) as usize
    };
    let c_start = col_of(start.x);
    let c_end   = col_of(end.x);
    let c_glyph = col_of(gx0);

    // 3) if glyph’s column isn’t between start‑col and end‑col, it’s out
    let c_min = c_start.min(c_end);
    let c_max = c_start.max(c_end);
    if c_glyph < c_min || c_glyph > c_max {
        return false;
    }

    // 4) convert everything to “local X” within this column
    let origin_x = (c_glyph as f64) * stride;
    let lsx0 = sx0 - origin_x;
    let lsx1 = sx1 - origin_x;
    let lgx0 = gx0 - origin_x;
    let lgx1 = gx1 - origin_x;

    // 5) vertical zones (global Y!)
    let is_first  = (start.y >= gy0 && start.y <= gy1);
    let is_last   = (end.y   >= gy0 && end.y   <= gy1);
    let is_middle = (sy0 < gy0 && gy1 < sy1);

    // 6) same first/middle/last logic, but using lsx0/lsx1 & lgx0/lgx1
    if is_middle {
        return true;
    }
    if is_first && is_last {
        // single‐line drag in this column
        return lgx1 >= lsx0 && lgx0 <= lsx1;
    }
    if is_first {
        // first line in this column
        if down { return lgx1 >= lsx0 }    // from start→end‑of‑line
        else    { return lgx0 <= lsx1 }    // upward: start→line‑start
    }
    if is_last {
        // last line in this column
        if down { return lgx0 <= lsx1 }    // from line‑start→end
        else    { return lgx1 >= lsx0 }    // upward: line‑end→end
    }
    false
}

fn hit4(
    start: Point,
    end: Point,
    gx0: f64,
    gy0: f64,
    gx1: f64,
    gy1: f64,
    cols: f64,
    col_width: f64,
    col_gap: f64,
) -> bool {
    // 1) normalize selection coords
    let sx = start.x.min(end.x);
    let ex = start.x.max(end.x);
    let sy = start.y.min(end.y);
    let ey = start.y.max(end.y);
    let (start, end) = if start.y != end.y {
        if start.y > end.y {
            (end, start)
        } else {
            (start, end)
        }
    } else {
        // y is equal, so check x
        if start.x > end.x {
            (end, start)
        } else {
            (start, end)
        }
    };

    let sx = start.x;
    let ex = end.x;
    let sy = start.y;
    let ey = end.y;
    let downward = start.y < end.y;

    // 2) figure column indices
    let stride = col_width + col_gap;
    let last_col = (cols as isize) - 1;
    let col_of = |x: f64| {
        let idx = ((x - col_gap) / stride).floor() as isize;
        idx.clamp(0, last_col) as usize
    };
    let c_start = col_of(start.x);
    let c_end   = col_of(end.x);
    let c_glyph = col_of(gx0);

    // 3) single‐column case
    if c_start == c_end {
        if c_start != c_glyph {return false}
        let is_start_line = (start.y >= gy0 && start.y <= gy1);
        let is_end_line   = (end.y   >= gy0 && end.y   <= gy1);
        let is_middle     = (sy < gy0 && gy1 < ey);

        if is_middle {
            return true;
        }
        if is_start_line && is_end_line {
            return gx1 >= sx && gx0 <= ex;
        }
        if is_start_line {
            return if downward { gx1 >= sx } else { gx0 <= sx };
        }
        if is_end_line {
            return if downward { gx0 <= ex } else { gx1 >= ex };
        }
        return false;
    }

    // 4) multi‐column case
    if c_glyph == c_start {
        // --- Start column ---
        let is_on_start_line = start.y >= gy0 && start.y <= gy1;
        if is_on_start_line {
            // rest of this line
            return if downward { gx1 >= sx } else { gx0 <= sx };
        }
        // full subsequent (or prior) lines
        return if downward { gy0 >  sy } else { gy1 <  sy };
    }

    // --- Middle columns: everything! ---
    let c_min = c_start.min(c_end);
    let c_max = c_start.max(c_end);
    if c_glyph > c_min && c_glyph < c_max {
        return true;
    }

    if c_glyph == c_end {
        // --- End column ---
        let is_on_end_line = end.y >= gy0 && end.y <= gy1;
        if is_on_end_line {
            // up to end-X on that line
            return if downward { gx0 <= ex } else { gx1 >= ex };
        }
        // full lines before (or after) the end line
        return if downward { gy1 <  ey } else { gy0 >  ey };
    }

    // out of the spanned columns
    false
}

fn hit5(
    start: Point,
    end:   Point,
    gx0:   f64,  // grid origin X
    gy0:   f64,  // grid origin Y (top)
    gx1:   f64,  // grid end   X (not actually used below)
    gy1:   f64,  // grid end   Y (bottom)
    cols:      f64,
    col_width: f64,
    col_gap:   f64,
) -> bool {
    // 1) normalize your drag rectangle
    let min_x = start.x.min(end.x);
    let max_x = start.x.max(end.x);
    let min_y = start.y.min(end.y);
    let max_y = start.y.max(end.y);

    // 2) fast‑reject if completely above or below the grid
    if max_y <= gy0 || min_y >= gy1 {
        return false;
    }

    // 3) figure out which column indices your drag spans
    let span     = col_width + col_gap;
    let mut first = ((min_x - gx0) / span).floor() as isize;
    let mut last  = ((max_x - gx0) / span).floor() as isize;
    let max_idx   = (cols as isize) - 1;

    // clamp into valid [0..cols-1]
    first = first.max(0).min(max_idx);
    last  = last .max(0).min(max_idx);

    // 4) test each candidate column for real overlap
    for ci in first..=last {
        let x0 = gx0 + (ci as f64) * span;
        let x1 = x0 + col_width;

        // if your drag rectangle overlaps this column’s box, you hit
        if max_x > x0 && min_x < x1 {
            return true;
        }
    }

    // nothing hit
    false
}
fn hit6(
    start: Point,
    end: Point,
    gx0: f64,
    gy0: f64,
    gx1: f64,
    gy1: f64,
    cols: f64,
    col_width: f64,
    col_gap: f64,
) -> bool {
    // 1) normalise selection coords
    let (sx, ex) = (start.x.min(end.x), start.x.max(end.x));
    let (sy, ey) = (start.y.min(end.y), start.y.max(end.y));
    let downward = start.y < end.y;

    // 2) column indices
    let stride   = col_width + col_gap;
    let last_col = cols as isize - 1;
    let col_of = |x: f64| ((x + col_gap / stride).floor() as isize).clamp(0, last_col) as usize;

    let c_start = col_of(start.x);
    let c_end   = col_of(end.x);
    let c_glyph = col_of(gx0);

    // 3) single‑column selection
    if c_start == c_end {
        let is_start_line = (start.y >= gy0 && start.y <= gy1);
        let is_end_line   = (end.y   >= gy0 && end.y   <= gy1);
        let is_middle     = (sy < gy0 && gy1 < ey);

        return if is_middle {
            true
        } else if is_start_line && is_end_line {
            gx1 >= sx && gx0 <= ex
        } else if is_start_line {
            if downward { gx1 >= sx } else { gx0 <= sx }
        } else if is_end_line {
            if downward { gx0 <= ex } else { gx1 >= ex }
        } else {
            false
        };
    }

    // 4) multi‑column selection
    // ── Start column ────────────────────────────────────────────
    if c_glyph == c_start {
        let is_on_start_line = start.y >= gy0 && start.y <= gy1;
        if is_on_start_line {
            return if downward { gx1 >= sx } else { gx0 <= sx };
        }
        // ↓ FIX: compare against start.y, not sy
        return if downward { gy0 >  start.y } else { gy1 <  start.y };
    }

    // ── Middle columns – always fully selected ──────────────────
    let (c_min, c_max) = (c_start.min(c_end), c_start.max(c_end));
    if c_glyph > c_min && c_glyph < c_max {
        return true;
    }

    // ── End column ──────────────────────────────────────────────
    if c_glyph == c_end {
        let is_on_end_line = end.y >= gy0 && end.y <= gy1;
        if is_on_end_line {
            return if downward { gx0 <= ex } else { gx1 >= ex };
        }
        // ↓ FIX: compare against end.y, not ey
        return if downward { gy1 <  end.y } else { gy0 >  end.y };
    }

    false
}

fn hit7(
    start: Point,
    end: Point,
    gx0: f64,
    gy0: f64,
    gx1: f64,
    gy1: f64,
    cols: f64,
    col_width: f64,
    col_gap: f64,
) -> bool {

    false
}


/*
/*let top = elem_point.y;
                                let bottom = top + glyph.size().height;
                                let x0 = elem_point.x + char_glyph.x as f64;
                                let x1 = x0 + glyph.size().width;
                                if ((location.y < top && bottom < self.move_location.y)
                                    || (location.y > bottom && top > self.move_location.y))

                                    {
                                    let rect = Rect::new(x0, top, x1, bottom);
                                    cx.fill(&rect, Color::LIGHT_BLUE, 0.);
                                }*/
                                /*let top    = elem_point.y;
                                let bottom = top + glyph.size().height;
                                let x0     = elem_point.x + char_glyph.x as f64;
                                let x1     = x0 + glyph.size().width;

                                // canonicalize selection bounds
                                let y0 = location.y.min(self.move_location.y);
                                let y1 = location.y.max(self.move_location.y);
                                let x0_sel = location.x.min(self.move_location.x);
                                let x1_sel = location.x.max(self.move_location.x);

                                // 1) fully inside vertically
                                let inside_vert = top >= y0 && bottom <= y1;

                                // 2) touching top edge at same Y and matching selection start‑X or end‑X
                                let on_top_edge = (top == y0) && (x0 == x0_sel || x1 == x0_sel);

                                // 3) touching bottom edge at same Y and matching selection start‑X or end‑X
                                let on_bot_edge = (bottom == y1) && (x0 == x1_sel || x1 == x1_sel);

                                if inside_vert || on_top_edge || on_bot_edge {
                                    let rect = Rect::new(x0, top, x1, bottom);
                                    cx.fill(&rect, Color::LIGHT_BLUE, 0.);
                                }*/
                                let sel_y0 = location.y.min(self.move_location.y);
                                let sel_y1 = location.y.max(self.move_location.y);
                                let sel_x0 = location.x.min(self.move_location.x);
                                let sel_x1 = location.x.max(self.move_location.x);
                                let gx0 = elem_point.x + char_glyph.x as f64;
                                let gy0 = elem_point.y;
                                let gx1 = gx0 + glyph.size().width;
                                let gy1 = gy0 + glyph.size().height;

                                // Inclusive overlap test: “touching” still counts
                                let is_first = gy0 <= sel_y0 && sel_y0 <= gy1;
                                let is_last = gy0 <= sel_y1 && sel_y1 < gy1;
                                let single_line = is_first && is_last && gx1 >= sel_x0 && gx0 <= sel_x1;
                                let first_line = is_first && !is_last && gx0 >= sel_x0;
                                let middle_line = gy0 >= sel_y0 && gy1 <= sel_y1;
                                let other = gx1  >= sel_x0
                                    && gx0 <= sel_x1
                                    && gy1 >= sel_y0
                                    && gy0 <= sel_y1;
                                //if middle_line || single_line || first_line
 */