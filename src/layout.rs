use floem::kurbo::Point;
use crate::book_elem::{Elem, ElemLine, ElemLines, ElemType};

pub struct Layout {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub column_width: f64,
    pub colum_height: f64,
}

impl Layout {
    pub fn layout_elem(&mut self, elem: &mut Elem){

        match &mut elem.elem_type {
            ElemType::Block(block) =>{
                elem.size.height = 0.;

                self.y += 20.;
                //self.width -= 40.;
                //
                // self.x += 20.;
                elem.point = Point::new(self.x, self.y);
                for child in block.children.iter_mut() {
                    self.layout_elem(child);
                    elem.size.height += child.size.height;
                }
                elem.size.width = self.width;
                self.y += 20.;
            }
            ElemType::Lines(lines) => {
                lines.elem_lines = Vec::new();
                let mut curr_line = ElemLine {height: 0., point: Point::new(0.,0.), elem_indexes: Vec::new()};
                let mut point = Point::new(self.x, self.y);
                elem.point = point;
                let mut height = 0.;
                let mut word_x = 0.;
                for (index, inline_elem) in lines.inline_elems.iter_mut().enumerate() {
                    if word_x + inline_elem.size.width > self.width {
                        (point, height) = self.add_line(&mut curr_line, point, height);
                        lines.elem_lines.push(curr_line);
                        word_x = 0.;
                        curr_line = ElemLine {height: 0., point: Point::new(0.,0.), elem_indexes: Vec::new()};
                    }
                    curr_line.height = f64::max(curr_line.height, inline_elem.size.height);
                    inline_elem.x = word_x;
                    word_x += inline_elem.size.width;
                    curr_line.elem_indexes.push(index);
                }
                (point, height) = self.add_line(&mut curr_line, point, height);
                lines.elem_lines.push(curr_line);
                elem.size.height = height;
                self.y = point.y;
            }
        }
    }
    fn add_line(&mut self, curr_line: &mut ElemLine, mut point: Point, mut height: f64) -> (Point, f64) {
        if point.y + curr_line.height > self.colum_height {
            self.x += self.column_width;
            height += self.colum_height - point.y;
            point.y = 0.;
        }
        curr_line.point.y = point.y;
        curr_line.point.x = self.x;
        point.y += curr_line.height;
        height += curr_line.height;
        (point, height)
    }
}
