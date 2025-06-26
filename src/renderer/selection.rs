use crate::renderer::html_renderer::{HtmlRenderer, Selection, RenderState};
impl HtmlRenderer {
    pub fn get_selection(&self) -> Option<Selection> {
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
    pub fn hit(&self, render_state: &RenderState, gx0: f64, gy0: f64, gx1: f64, gy1: f64,) -> bool{
        let glyph_col_index = self.get_col_index(gx0);
        let selection = render_state.selection.as_ref().unwrap();
        if selection.start_col < glyph_col_index && glyph_col_index < selection.end_col {
            return true;
        }
        let is_first_col        = selection.start_col == glyph_col_index;
        let is_last_col         = selection.end_col == glyph_col_index;
        let is_first_line       = gy0 < selection.start_selection.y && gy1 > selection.start_selection.y;
        let is_last_line        = gy0 < selection.end_selection.y && gy1 > selection.end_selection.y;
        let is_after_first_line = gy0 > selection.start_selection.y;
        let is_before_last_line = gy0 < selection.end_selection.y;
        let is_first_x          = gx1 >= selection.start_selection.x;
        let is_last_x           = gx1 <= selection.end_selection.x;
        // Single column
        if is_first_col && is_last_col {
            // Single line
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
            else if is_after_first_line && is_before_last_line {
                return true;
            }
        }

        // First column
        else if is_first_col {
            if is_first_line {
                if is_first_x {return true}
            }
            else if is_after_first_line {
                return true;
            }
        }
        //End column
        else if is_last_col {
            if is_last_line {
                if is_last_x {return true}
            }
            else if is_before_last_line {
                return true
            }
        }

        false
    }
}
