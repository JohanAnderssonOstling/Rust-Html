use floem::kurbo::{Point, Size};
use lightningcss::properties::text::TextAlign;
use crate::book_elem::{BookElemFactory, Elem, ElemLine, ElemLines, ElemType, InlineContent, InlineElem, InlineItem, ParseState};

pub fn add_line(parser: &mut BookElemFactory, mut curr_line: ElemLine, mut elem_lines: ElemLines, parse_state: ParseState) -> ElemLines{
    let line_width = parser.curr_x - parse_state.x;
    match parse_state.text_align {
        TextAlign::Start | TextAlign::Left=> {}
        TextAlign::Right | TextAlign::End => {
            let offset = parse_state.width - line_width;
            for inline_elem in &mut curr_line.inline_elems {
                inline_elem.x += offset;
            }
        }
        TextAlign::Center => {
            let offset = (parse_state.width - line_width) / 2.0;
            for inline_elem in &mut curr_line.inline_elems {
                inline_elem.x += offset;
            }
        }
        TextAlign::Justify => {
            let count = curr_line.inline_elems.len();
            if count > 1 && line_width / parse_state.width > 0.80 {
                let extra_space = parse_state.width - line_width;
                let gap_count = count - 1;
                let gap = extra_space / gap_count as f64;
                for (i, inline_elem) in curr_line.inline_elems.iter_mut().enumerate() {
                    inline_elem.x += gap * i as f64;
                    match &mut inline_elem.inline_content {
                        InlineContent::Text(glyphs) | InlineContent::Link((glyphs, _)) => {
                            for glyph in glyphs {
                                glyph.x = glyph.x // Add scaling here
                            }
                        }
                        InlineContent::Image(_) => {}
                        InlineContent::Link(_) => {}
                    }
                }
            }
        }
        TextAlign::MatchParent => {}
        TextAlign::JustifyAll => {}
    }
    parser.curr_x         = parse_state.x;
    parser.curr_y         += curr_line.height;
    elem_lines.height   += curr_line.height;
    elem_lines.elem_lines.push(curr_line);
    elem_lines
}

pub fn layout_elem_lines(parser: &mut BookElemFactory, mut inline_items: Vec<InlineItem>, parse_state: ParseState) -> Elem{
    let init_point      = Point::new(parser.curr_x, parser.curr_y);
    let mut elem_lines  = ElemLines {height: 0., elem_lines: Vec::new()};
    let mut curr_line   = ElemLine  {height: 0., inline_elems: Vec::new()};
    for mut inline_item in inline_items {
        if inline_item.size.width > parse_state.x + parse_state.width {
            elem_lines          = add_line(parser, curr_line, elem_lines, parse_state);
            if let InlineContent::Image(image) = &mut inline_item.inline_content {
                let scale_factor = inline_item.size.width / (parse_state.x + parse_state.width);
                image.width = (image.width as f64 / scale_factor) as u32;
                image.height = (image.height as f64 / scale_factor) as u32;
                inline_item.size.width = inline_item.size.width / scale_factor;
                inline_item.size.height = inline_item.size.height / scale_factor;
            }
            let mut new_line    = ElemLine {height: inline_item.size.height, inline_elems: Vec::new()};

            let inline_elem     = InlineElem {x: 0., inline_content: inline_item.inline_content};
            new_line.inline_elems.push(inline_elem);
            elem_lines          = add_line(parser, new_line, elem_lines, parse_state);
            curr_line           = ElemLine {height: 0., inline_elems: Vec::new()};
            continue
        }
        else if parser.curr_x + inline_item.size.width > parse_state.width {
            elem_lines          = add_line(parser, curr_line, elem_lines, parse_state);
            curr_line           = ElemLine {height: 0., inline_elems: Vec::new()};
        }
        curr_line.height    = f64::max(curr_line.height, inline_item.size.height);
        let inline_elem     = InlineElem {x: parser.curr_x, inline_content: inline_item.inline_content};
        parser.curr_x         += inline_item.size.width;
        curr_line.inline_elems.push(inline_elem);
    }
    elem_lines = add_line(parser, curr_line, elem_lines, parse_state);
    Elem {size: Size::new(parse_state.width, elem_lines.height), point: init_point, elem_type: ElemType::Lines(elem_lines)}
}