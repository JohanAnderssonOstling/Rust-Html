use crate::book_elem::{Elem, ElemType};

pub struct Layout {
    x: f64,
    y: f64,
}

impl Layout {
    fn layout_elem(&mut self, elem: Elem, column_width: f64, colum_height: f64) {
        match elem.elem_type {
            ElemType::Block(block) =>{
                
            }
            ElemType::Lines(lines) => {
                for line in lines.elem_lines.iter() {
                    
                }
            }
        }
    }
}
