use floem::kurbo::{Point, Size};
use floem_renderer::text::Attrs;
use lightningcss::properties::text::TextAlign;
use lightningcss::stylesheet::StyleSheet;
use roxmltree::{Document, Node};
use crate::book_elem::{BookElemFactory, CharGlyph, Elem, ElemLine, ElemLines, ElemType, InlineContent, InlineElem, ParseState};
use crate::layout::layout_elem_lines;
const CELL_PAD_X: f64 = 10.0;   // px on the left *and* right
const CELL_PAD_Y: f64 = 2.0;   // px on the top *and* bottom
const ROW_GAP   : f64 = 6.0;   // empty space *between* rows
impl BookElemFactory {
    pub fn parse_table(
        &mut self,
        node: Node,
        font: Attrs,
        style_sheets: &Vec<StyleSheet>,
        mut parse_state: ParseState,
        mut index: Vec<usize>,
        document: &Document,
    ) -> Elem {
        use crate::book_elem::InlineItem;

        let init_point = Point::new(self.curr_x, self.curr_y);

        struct TableCell {
            lines: ElemLines,
            width: f64,
            height: f64,
        }

        struct ParsedTableCell {
            inline_items: Vec<InlineItem>,
            text_align: TextAlign
        }
        

        let mut parsed_table_rows: Vec<Vec<ParsedTableCell>> = Vec::new();

        let mut table_rows: Vec<Vec<TableCell>> = Vec::new();
        let mut max_cols = 0;
        //parse_state.width = 200. - CELL_PAD_X;
        for row_node in node.children().filter(|n| n.has_tag_name("tr")) {
            let col_count = row_node.children().filter(|n| n.has_tag_name("td") || n.has_tag_name("th")).count();
            max_cols = max_cols.max(col_count);
        }
        let mut min_widths: Vec<f64> = vec![0.0; max_cols];
        let mut max_widths: Vec<f64> = vec![0.0; max_cols];
        for row_node in node.children().filter(|n| n.has_tag_name("tr")) {
            let mut parsed_row_cells: Vec<ParsedTableCell> = Vec::new();
            for (idx, cell_node) in row_node.children().filter(|n| n.has_tag_name("td") || n.has_tag_name("th")).enumerate() {
                let (inline_items, text_align) = self.parse_inline(cell_node, style_sheets, font, parse_state.clone(), None, &index, document);
                let mut min_width: f64 = 0.;
                let mut max_width = 0.;
                for inline_item in inline_items.iter() {
                    let inline_width = inline_item.size.width;
                    min_width = min_width.max(inline_width + CELL_PAD_X);
                    max_width += inline_width;
                }
                min_widths[idx] = min_widths[idx].max(min_width);
                max_widths[idx] = max_widths[idx].max(max_width);
                let parsed_table_cell = ParsedTableCell {inline_items, text_align};
                parsed_row_cells.push(parsed_table_cell);
            }
            parsed_table_rows.push(parsed_row_cells);
        }

        //parse_state.width = (parse_state.width / max_cols as f64);
        //let col_widths = resolve_auto_widths(&min_widths, &max_widths, parse_state.width);
        let max_width = parse_state.width - CELL_PAD_X * (max_cols - 1) as f64;
        let col_widths = resolve_auto_widths(&min_widths, &max_widths, max_width);
        for row in parsed_table_rows {
            let mut col_idx = 0;
            let mut row_cells: Vec<TableCell> = Vec::new();

            for cell in row {
                parse_state.width = *col_widths.get(col_idx).unwrap();
                parse_state.text_align = cell.text_align;
                let elem = layout_elem_lines(self, cell.inline_items, &parse_state);
                self.curr_y -= elem.size.height;
                match elem.elem_type {
                    ElemType::Lines(lines) => {
                        row_cells.push(TableCell {
                            width: elem.size.width,
                            height: elem.size.height,
                            lines,
                        });
                    }
                    _ => panic!("Expected table cell to yield ElemType::Lines"),
                }
                col_idx += 1;
            }
            table_rows.push(row_cells);

        }

        // Step 2: Compute natural widths and row heights
        //let mut col_widths: Vec<f64> = vec![0.0; max_cols];
        let mut row_heights: Vec<f64> = Vec::with_capacity(table_rows.len());

        // Step 4: Lay out rows
        let mut lines: Vec<ElemLine> = Vec::new();
        let mut y_cursor               = 0.0;
        let mut tallest_row_idx: usize = 0;
        let mut tallest_row_height     = 0.0;

        let mut total_height = 0.;
        for (row_idx, row) in table_rows.iter().enumerate() {
            // ───────────── 1. how many display‑lines does this row span? ───────────
            let max_lines = row
                .iter()
                .map(|cell| cell.lines.elem_lines.len())
                .max()                     // longest column decides
                .unwrap_or(0);             // row might be empty

            let mut row_height = 0.0;      // total (visual) height of this row so far

            // ───────────── 2. build every logical line in this row ─────────────────
            for line_idx in 0..max_lines {
                let mut x_cursor      = 0.0;
                let mut inline_elems  = Vec::<InlineElem>::new();

                let mut line_height: f64   = 0.0;
                for (col_idx, cell) in row.iter().enumerate() {

                    if let Some(elem_line) = cell.lines.elem_lines.get(line_idx) {
                        line_height = line_height.max(elem_line.height);

                        // Copy inline elements and shift them horizontally
                        for mut inline in elem_line.inline_elems.clone() {
                            inline.x += x_cursor;
                            inline_elems.push(inline);
                        }
                    }

                    x_cursor += col_widths[col_idx] + CELL_PAD_X;
                }

                row_height += line_height;     // accumulate row’s height
                lines.push(ElemLine { height: line_height, inline_elems });
            }
            total_height += row_height;
        }

        let total_width = col_widths.iter().sum::<f64>();
        //let total_height = row_heights.iter().sum::<f64>();
        self.curr_y += total_height;

        Elem {
            size: Size::new(total_width, total_height),
            point: init_point,
            elem_type: ElemType::Lines(ElemLines {
                height: 10.,
                elem_lines: lines,
            }),
        }
    }
}

fn resolve_auto_widths(mins: &[f64], maxs: &[f64], total: f64) -> Vec<f64> {
    assert_eq!(mins.len(), maxs.len());

    let mut widths: Vec<f64> = mins.to_vec();
    let mut slack:  Vec<f64> = maxs.iter()
        .zip(mins)
        .map(|(max, min)| (max - min).max(0.0))
        .collect();

    let mut remaining = total - mins.iter().sum::<f64>();
    if remaining <= 0.0 {
        return widths;                      // already over‑constrained
    }

    // Expand columns while we still have space and at least one can grow
    loop {
        // Sum the slack *only* of columns that still have room
        let total_slack: f64 = slack.iter().sum();
        if total_slack == 0.0 || remaining == 0.0 {
            break;                          // everyone’s at max or no space left
        }

        let mut progress = 0.0;             // track how much we actually assigned

        for i in 0..widths.len() {
            if slack[i] == 0.0 { continue; } // column already maxed out

            // Give this column its proportional share of the leftovers
            let share = remaining * (slack[i] / total_slack);

            let grow  = share.min(slack[i]); // but never exceed its own slack
            widths[i] += grow;
            slack[i]  -= grow;
            progress  += grow;
        }

        // Numerical safety: if we didn’t move forward, quit
        if progress == 0.0 { break; }
        remaining -= progress;
    }

    widths
}
