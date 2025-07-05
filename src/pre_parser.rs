use std::time::Instant;
use floem::kurbo::{Point, Size};
use floem_renderer::text::Attrs;
use lightningcss::stylesheet::StyleSheet;
use roxmltree::{Document, Node};
use scraper::ElementRef;
use crate::book_elem::{BookElemFactory, CharGlyph, Elem, ElemLine, ElemLines, ElemType, InlineContent, InlineElem, ParseState};
use crate::styling::style::{resolve_style, resolve_style_scraper};

impl BookElemFactory {
    pub fn parse_pre(
        &mut self,
        node: Node,
        mut font: Attrs,
        style_sheets: &Vec<StyleSheet>,
        parse_state: ParseState,
        index: Vec<usize>,
        document: &Document,
    ) -> Elem {
        let init_point = Point::new(self.curr_x, self.curr_y);
        let mut lines: Vec<ElemLine> = Vec::new();
        let mut current_line: Vec<InlineElem> = Vec::new();
        let mut x = 0.0;
        let mut max_height = 0.0;

        let now = Instant::now();
        let (_, parse_state) = resolve_style(style_sheets, &node, &mut font, parse_state, document);
        self.style_time += (Instant::now() - now).as_nanos();

        fn recurse_pre<'a>(
            factory: &mut BookElemFactory,
            node: Node<'a, 'a>,
            font: Attrs,
            style_sheets: &Vec<StyleSheet>,
            parse_state: &ParseState,
            x: &mut f64,
            max_height: &mut f64,
            current_line: &mut Vec<InlineElem>,
            lines: &mut Vec<ElemLine>,
        ) {
            if let Some(text) = node.text() {
                for line in text.split_inclusive('\n') {
                    for ch in line.chars() {
                        if ch == '\n' {
                            lines.push(ElemLine {
                                height: *max_height,
                                inline_elems: std::mem::take(current_line),
                            });
                            *x = 0.0;
                            *max_height = 0.0;
                            continue;
                        }
                        let (text_layout, index) = factory.cache.get_or_insert(ch, font, parse_state);
                        *max_height = max_height.max(text_layout.size().height);
                        current_line.push(InlineElem {
                            x: *x,
                            inline_content: InlineContent::Text(vec![CharGlyph { char: index, x: 0. }]),
                        });
                        *x += text_layout.size().width;
                    }
                }
            } else {
                for child in node.children() {
                    if child.is_element() {
                        //println!("pre: {}", child.tag_name().name());
                        recurse_pre(factory, child, font, style_sheets, parse_state, x, max_height, current_line, lines);
                    }
                }
            }
        }

        recurse_pre(
            self,
            node,
            font,
            style_sheets,
            &parse_state,
            &mut x,
            &mut max_height,
            &mut current_line,
            &mut lines,
        );

        // Add final line if needed
        if !current_line.is_empty() {
            lines.push(ElemLine {
                height: max_height,
                inline_elems: current_line,
            });
        }

        let total_height = lines.iter().map(|l| l.height).sum::<f64>();
        self.curr_y += total_height;

        Elem {
            size: Size::new(parse_state.width, total_height),
            point: init_point,
            elem_type: ElemType::Lines(ElemLines { height: total_height, elem_lines: lines }),
        }
    }

    pub fn parse_pre_scraper(
        &mut self,
        elem_ref: ElementRef,
        mut font: Attrs,
        style_sheets: &Vec<StyleSheet>,
        parse_state: ParseState,
        index: Vec<usize>,
    ) -> Elem {
        let init_point = Point::new(self.curr_x, self.curr_y);
        let mut lines: Vec<ElemLine> = Vec::new();
        let mut current_line: Vec<InlineElem> = Vec::new();
        let mut x = 0.0;
        let mut max_height = 0.0;

        let now = Instant::now();
        let (_, parse_state) = resolve_style_scraper(style_sheets, &elem_ref, &mut font, parse_state);
        self.style_time += (Instant::now() - now).as_nanos();

        fn recurse_pre_scraper(
            factory: &mut BookElemFactory,
            elem_ref: ElementRef,
            font: Attrs,
            style_sheets: &Vec<StyleSheet>,
            parse_state: &ParseState,
            x: &mut f64,
            max_height: &mut f64,
            current_line: &mut Vec<InlineElem>,
            lines: &mut Vec<ElemLine>,
        ) {
            // Process text nodes within this element
            for node in elem_ref.children() {
                if let Some(text_node) = node.value().as_text() {
                    let text = text_node;
                    for line in text.split_inclusive('\n') {
                        for ch in line.chars() {
                            if ch == '\n' {
                                lines.push(ElemLine {
                                    height: *max_height,
                                    inline_elems: std::mem::take(current_line),
                                });
                                *x = 0.0;
                                *max_height = 0.0;
                                continue;
                            }
                            let (text_layout, index) = factory.cache.get_or_insert(ch, font, parse_state);
                            *max_height = max_height.max(text_layout.size().height);
                            current_line.push(InlineElem {
                                x: *x,
                                inline_content: InlineContent::Text(vec![CharGlyph { char: index, x: 0. }]),
                            });
                            *x += text_layout.size().width;
                        }
                    }
                } else if let Some(child_elem) = ElementRef::wrap(node) {
                    //println!("pre: {}", child_elem.value().name());
                    recurse_pre_scraper(factory, child_elem, font, style_sheets, parse_state, x, max_height, current_line, lines);
                }
            }
        }

        recurse_pre_scraper(
            self,
            elem_ref,
            font,
            style_sheets,
            &parse_state,
            &mut x,
            &mut max_height,
            &mut current_line,
            &mut lines,
        );

        // Add final line if needed
        if !current_line.is_empty() {
            lines.push(ElemLine {
                height: max_height,
                inline_elems: current_line,
            });
        }

        let total_height = lines.iter().map(|l| l.height).sum::<f64>();
        self.curr_y += total_height;

        Elem {
            size: Size::new(parse_state.width, total_height),
            point: init_point,
            elem_type: ElemType::Lines(ElemLines { height: total_height, elem_lines: lines }),
        }
    }
}