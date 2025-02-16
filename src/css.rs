use std::collections::HashMap;

enum Value {
    Keyword(String),
    Length(f64, Unit),
    Number(f64),
    
}

enum Unit { Px, Em}

pub struct StyleSheet {
    tag_selectors:      HashMap<String, (String, String)>,
    class_selectors:    HashMap<String, (String, String)>,
    id_selectors:       HashMap<String, (String, String)>,
}

impl StyleSheet {
    pub fn resolve_tag(&self, tag: &str) {
        if !self.tag_selectors.contains_key(tag) {
            
        }
    }
}