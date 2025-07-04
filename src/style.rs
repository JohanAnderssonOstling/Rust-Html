use floem_renderer::text::Attrs;
use lightningcss::properties::font::{AbsoluteFontSize, AbsoluteFontWeight, FontSize, FontStyle, FontWeight, RelativeFontSize};
use lightningcss::properties::text::TextAlign;
use lightningcss::properties::Property;
use lightningcss::rules::CssRule;
use lightningcss::selector::{Combinator, Component, Selector};
use lightningcss::stylesheet::StyleSheet;
use lightningcss::values::length::{LengthPercentage, LengthPercentageOrAuto, LengthValue};
use roxmltree::{Document, Node};
use std::collections::HashMap;
use std::fmt::Pointer;
use std::ops::Deref;
use std::hash::{Hash, Hasher};
use rustc_data_structures::fx::FxHashMap;
use crate::book_elem::ParseState;

pub struct Margins {
    pub top: f64, pub right: f64, pub bottom: f64, pub left: f64,
}
#[derive(Clone)]
pub enum CSSValue {
    Length(LengthPercentageOrAuto),
    FontWeight(FontWeight),
    TextAlign(TextAlign),
    TextStyle(floem_renderer::text::Style),
}

#[derive(Clone)]
pub struct Style {
    pub font_size: Option<FontSize>,
    pub properties: HashMap<String, CSSValue>
}

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct StyleCacheKey {
    pub tag_name: String,
    pub id: Option<String>,
    pub class: Option<String>,
    pub ancestor_tags: Vec<String>,
}

pub struct StyleCache {
    cache: FxHashMap<StyleCacheKey, Style>,
}

impl StyleCache {
    pub fn new() -> Self {
        StyleCache {
            cache: FxHashMap::default(),
        }
    }
    
    pub fn get(&self, key: &StyleCacheKey) -> Option<&Style> {
        self.cache.get(key)
    }
    
    pub fn insert(&mut self, key: StyleCacheKey, style: Style) {
        self.cache.insert(key, style);
    }
    
    pub fn clear(&mut self) {
        self.cache.clear();
    }
}
fn create_px(value: f32) -> CSSValue {
    CSSValue::Length(LengthPercentageOrAuto::LengthPercentage(LengthPercentage::Dimension(LengthValue::Px(value))))
}
fn create_em(value: f32) -> CSSValue {
    CSSValue::Length(LengthPercentageOrAuto::LengthPercentage(LengthPercentage::Dimension(LengthValue::Em(value))))
}
fn create_font_size(value: f32) -> FontSize {
    FontSize::Length(LengthPercentage::Dimension(LengthValue::Em(value)))
}
impl Style {
    pub fn new(node_tag: &str) -> Style {
        let mut style = Style {properties: HashMap::new(), font_size: None};
        match node_tag {
            "p" => {
                style.insert("margin-top", create_em(1.));
                style.insert("margin-bottom", create_em(1.));
            }
            "h1" => {
                style.font_size = Some(create_font_size(2.0));
                style.insert("margin-top", create_em(0.67));
                style.insert("margin-bottom", create_em(0.67));
            }
            "h2" => {
                style.font_size = Some(create_font_size(1.5));
                style.insert("margin-top", create_em(0.83));
                style.insert("margin-bottom", create_em(0.83));
            }
            "h3" => {
                style.font_size = Some(create_font_size(1.17));
                style.insert("margin-top", create_em(1.));
                style.insert("margin-bottom", create_em(1.));
            }
            "h4" => {
                style.insert("margin-top", create_em(1.33));
                style.insert("margin-bottom", create_em(1.33));
            }
            "h5" => {
                style.font_size = Some(create_font_size(0.83));
                style.insert("margin-top", create_em(1.67));
                style.insert("margin-bottom", create_em(1.67));
            }
            "h6" => {
                style.font_size = Some(create_font_size(0.67));
                style.insert("margin-top", create_em(2.33));
                style.insert("margin-bottom", create_em(2.33));
            }
            "dd" => {
                style.insert("margin-left", create_px(40.))
            }
            "th" => {
                style.insert("font-weight", CSSValue::FontWeight(FontWeight::Absolute(AbsoluteFontWeight::Bold)));
                style.insert("text-align", CSSValue::TextAlign(TextAlign::Center))
            }
            "blockquote" => {
                style.insert("margin-top", create_em(1.));
                style.insert("margin-bottom", create_em(1.));
                style.insert("margin-left", create_px(40.));
                style.insert("margin-right", create_px(40.));
            }
            _ => ()
        }
        style
    }
    pub fn insert(&mut self,key: &str, value: CSSValue) {
        self.properties.insert(key.to_string(), value);
    }
    
    pub fn with_capacity(capacity: usize) -> Self {
        Style {
            font_size: None,
            properties: HashMap::with_capacity(capacity),
        }
    }
}

struct MatchedRule<'a,'b> {
    specificity: u32,
    source_order: usize,
    declarations: &'a [Property<'b>]
}
pub fn apply_style_sheet(style_sheet: & StyleSheet, node: &Node, style: &mut Style, parse_state: &ParseState, document: &Document) {
    let mut matched_rules: Vec<MatchedRule> = Vec::with_capacity(16);

    for (index, rule) in style_sheet.rules.0.iter().enumerate() {
        match rule {
            CssRule::Style(style_rule) => {
                for selector in &style_rule.selectors.0 {

                    if selector_matches2(selector, node, parse_state, document) {
                        matched_rules.push(MatchedRule {
                            specificity: selector.specificity(),
                            source_order: index,
                            declarations: &style_rule.declarations.declarations
                        })
                    }
                }
            }
            _ => (),
        }
    }
    matched_rules.sort_by(|a, b| {
        a.specificity.cmp(&b.specificity)
            .then_with(|| a.source_order.cmp(&b.source_order))
    });
    for matched_rule in matched_rules {
        for decl in matched_rule.declarations.iter() {
            if decl.property_id().is_shorthand() {
                match decl {
                    Property::Margin(margins) => {
                        style.insert("margin-top",      CSSValue::Length(margins.top.clone()));
                        style.insert("margin-right",    CSSValue::Length(margins.right.clone()));
                        style.insert("margin-bottom",   CSSValue::Length(margins.bottom.clone()));
                        style.insert("margin-left",     CSSValue::Length(margins.left.clone()));
                    }
                    Property::Padding(paddings) => {
                        style.insert("padding-top",     CSSValue::Length(paddings.top.clone()));
                        style.insert("padding-right",   CSSValue::Length(paddings.right.clone()));
                        style.insert("padding-bottom",  CSSValue::Length(paddings.bottom.clone()));
                        style.insert("padding-left",    CSSValue::Length(paddings.left.clone()));
                    }
                    
                    _ => ()
                }
                continue
            }
            match decl {
                Property::MarginTop(value) | Property::MarginRight(value) | Property::MarginBottom(value) | Property::MarginLeft(value)
                | Property::PaddingTop(value) | Property::PaddingRight(value) | Property::PaddingBottom(value) | Property::PaddingLeft(value)
                    => {style.insert(decl.property_id().name(), CSSValue::Length(value.clone()));}
                Property::FontSize(font_size) => {style.font_size = Some(font_size.to_owned());}
                Property::FontWeight(font_weight) => {style.insert(decl.property_id().name(), CSSValue::FontWeight(font_weight.clone()))}
                Property::TextAlign(text_align) => {style.insert(decl.property_id().name(), CSSValue::TextAlign(text_align.clone()))}
                Property::FontStyle(font_style) => {
                    let text_style = match font_style {
                        FontStyle::Normal => floem_renderer::text::Style::Normal,
                        FontStyle::Italic => floem_renderer::text::Style::Italic,
                        FontStyle::Oblique(_) => floem_renderer::text::Style::Oblique,
                    };
                    style.insert(decl.property_id().name(), CSSValue::TextStyle(text_style));
                }
                
                _ => ()
            }
        }
    }
}

pub fn resolve_style_cached(style_sheets: &Vec<StyleSheet>, node: &Node, font: &mut Attrs, mut parse_state: ParseState, document: &Document, cache: &mut StyleCache) -> (Margins, ParseState) {
    let cache_key = StyleCacheKey {
        tag_name: node.tag_name().name().to_string(),
        id: node.attribute("id").map(|s| s.to_string()),
        class: node.attribute("class").map(|s| s.to_string()),
        ancestor_tags: parse_state.ancestors.iter()
            .filter_map(|id| document.get_node(*id))
            .map(|n| n.tag_name().name().to_string())
            .collect(),
    };
    
    if let Some(cached_style) = cache.get(&cache_key) {
        return apply_cached_style(cached_style, font, parse_state);
    }
    
    let mut style = Style::new(node.tag_name().name());
    let mut margins = Margins {top: 0., right: 0., bottom: 0., left: 0.};
    
    for style_sheet in style_sheets {
        apply_style_sheet(style_sheet, &node, &mut style, &parse_state, document);
    }
    
    cache.insert(cache_key, style.clone());
    apply_cached_style(&style, font, parse_state)
}

fn apply_cached_style(style: &Style, font: &mut Attrs, mut parse_state: ParseState) -> (Margins, ParseState) {
    let mut margins = Margins {top: 0., right: 0., bottom: 0., left: 0.};
    
    if let Some(font_size) = &style.font_size {
        let resolved_font_size = resolve_font_size(font_size, &parse_state, (font.font_size as f64)).round();
        if resolved_font_size != 0. { *font = font.font_size(resolved_font_size as f32); }
    }
    
    let font_size = font.font_size as f64;
    for (key, value) in style.properties.iter() {
        match value {
            CSSValue::Length(value) => {
                match key.as_str() {
                    "margin-top"        => margins.top      += resolve_length(value, &parse_state, font_size),
                    "margin-right"      => margins.right    += resolve_length(value, &parse_state, font_size),
                    "margin-bottom"     => margins.bottom   += resolve_length(value, &parse_state, font_size),
                    "margin-left"       => margins.left     += resolve_length(value, &parse_state, font_size),
                    "padding-top"       => margins.top      += resolve_length(value, &parse_state, font_size),
                    "padding-right"     => margins.right    += resolve_length(value, &parse_state, font_size),
                    "padding-bottom"    => margins.bottom   += resolve_length(value, &parse_state, font_size),
                    "padding-left"      => margins.left     += resolve_length(value, &parse_state, font_size),
                    _ => ()
                }
            }
            CSSValue::FontWeight(value) => {parse_state.font_weight = resolve_font_weight(value);}
            CSSValue::TextAlign(value)  => parse_state.text_align = value.clone(),
            CSSValue::TextStyle(text_style)    => parse_state.text_style = *text_style,
        }
    }
    (margins, parse_state)
}

pub fn resolve_style(style_sheets: &Vec<StyleSheet>, node: &Node, font: &mut Attrs, mut parse_state: ParseState, document: &Document) -> (Margins, ParseState){
    let mut style = Style::new(node.tag_name().name());
    let mut margins = Margins {top: 0., right: 0., bottom: 0., left: 0.};
    for style_sheet in style_sheets {
        apply_style_sheet(style_sheet, &node, &mut style, &parse_state, document);
    }
    if let Some(font_size) = &style.font_size {
        let resolved_font_size = resolve_font_size(font_size, &parse_state, (font.font_size as f64)).round();
        if resolved_font_size != 0. { *font = font.font_size(resolved_font_size as f32); }
    }
    let font_size = font.font_size as f64;
    for (key, value) in style.properties.iter() {
        match value {
            CSSValue::Length(value) => {
                match key.as_str() {
                    "margin-top"        => margins.top      += resolve_length(value, &parse_state, font_size),
                    "margin-right"      => margins.right    += resolve_length(value, &parse_state, font_size),
                    "margin-bottom"     => margins.bottom   += resolve_length(value, &parse_state, font_size),
                    "margin-left"       => margins.left     += resolve_length(value, &parse_state, font_size),
                    "padding-top"       => margins.top      += resolve_length(value, &parse_state, font_size),
                    "padding-right"     => margins.right    += resolve_length(value, &parse_state, font_size),
                    "padding-bottom"    => margins.bottom   += resolve_length(value, &parse_state, font_size),
                    "padding-left"      => margins.left     += resolve_length(value, &parse_state, font_size),
                    _ => (println!("Unresolved key: {key}"))
                }
            }
            CSSValue::FontWeight(value) => {parse_state.font_weight = resolve_font_weight(value);}
            CSSValue::TextAlign(value)  => parse_state.text_align = value.clone(),
            CSSValue::TextStyle(text_style)    => parse_state.text_style = *text_style,
        }
    }
    (margins, parse_state)
}

fn selector_matches(selectors: &Vec<&Component<>>, node: &Node, parse_state: &ParseState, document: &Document) -> bool {

    // Parse the selector into sequences separated by combinators
    let mut sequences = Vec::new();
    let mut current_sequence = Vec::new();
    let mut combinators = Vec::new();
    
    for component in selectors {
        match component {
            Component::Combinator(combinator) => {
                if !current_sequence.is_empty() {
                    sequences.push(current_sequence.clone());
                    current_sequence.clear();
                }
                println!("FOund combinator");
                combinators.push(combinator.clone());
            }
            _ => {
                current_sequence.push(*component);
            }
        }
    }
    
    if !current_sequence.is_empty() {
        sequences.push(current_sequence);
    }
    
    // If no combinators, just match the single sequence against current node
    if combinators.is_empty() {
        return matches_sequence(&sequences[0], node);
    }
    
    // Start with the rightmost sequence (the one that should match the current node)
    if !matches_sequence(sequences.last().unwrap(), node) {
        return false;
    }
    
    // Work backwards through combinators
    let mut current_ancestors = parse_state.ancestors.clone();
    
    for (i, combinator) in combinators.iter().enumerate().rev() {
        let target_sequence = &sequences[i];
        
        match combinator {
            Combinator::Child => {
                // Direct parent must match
                if let Some(parent_id) = current_ancestors.last() {
                    if let Some(parent_node) = document.get_node(*parent_id) {
                        if matches_sequence(target_sequence, &parent_node) {
                            current_ancestors.pop(); // Move up one level
                            continue;
                        }
                    }
                }
                return false;
            }
            Combinator::Descendant => {
                // Any ancestor must match
                let mut found = false;
                while let Some(ancestor_id) = current_ancestors.pop() {
                    if let Some(ancestor_node) = document.get_node(ancestor_id) {
                        if matches_sequence(target_sequence, &ancestor_node) {
                            found = true;
                            break;
                        }
                    }
                }
                if !found {
                    return false;
                }
            }
            _ => {
                // Unsupported combinator
                return false;
            }
        }
    }
    
    true
}

fn matches_sequence(sequence: &Vec<&Component<>>, node: &Node) -> bool {
    for component in sequence {
        match component {
            Component::ExplicitUniversalType => {} // Matches any element
            Component::LocalName(name) => {
                if node.tag_name().name() != name.lower_name.0.as_ref() {
                    return false;
                }
            }
            Component::ID(id) => {
                if node.attribute("id").unwrap_or_default() != id.0.as_ref() {
                    return false;
                }
            }
            Component::Class(class_selector) => {
                if let Some(class_attr) = node.attribute("class") {
                    let classes: Vec<&str> = class_attr.split_whitespace().collect();
                    if !classes.contains(&class_selector.as_ref()) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            Component::AttributeInNoNamespace { local_name, value, .. } => {
                if let Some(attr) = node.attribute(local_name.0.as_ref()) {
                    if !attr.eq(value.0.as_ref()) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            Component::AttributeInNoNamespaceExists { local_name, .. } => {
                if node.attribute(local_name.0.as_ref()).is_none() {
                    return false;
                }
            }
            // Add other component types as needed
            _ => {
                // For now, ignore unsupported components
            }
        }
    }
    true
}

fn resolve_font_size(value: &FontSize, parse_state: &ParseState, font_size: f64) -> f64 {
    match value {
        FontSize::Length(length) => {
            resolve_length_percentage(&length, parse_state, font_size, true)
        }
        FontSize::Absolute(absolute) => {
            let steps = match  absolute {
                AbsoluteFontSize::XXSmall   => -3,
                AbsoluteFontSize::XSmall    => -2,
                AbsoluteFontSize::Small     => -1,
                AbsoluteFontSize::Medium    => 0,
                AbsoluteFontSize::Large     => 1,
                AbsoluteFontSize::XLarge    => 2,
                AbsoluteFontSize::XXLarge   => 3,
                AbsoluteFontSize::XXXLarge  => 4,
            };
            ((parse_state.root_font_size * 1.2f32.powi(steps)) as f64).round()
        }
        FontSize::Relative(relative) => {
            let steps = match relative {
                RelativeFontSize::Smaller => -1,
                RelativeFontSize::Larger => -1,
            };
            font_size * 1.2f64.powi(steps).round()
        }
    }
}
fn resolve_font_weight(font_weight: &FontWeight) -> u16{
    match font_weight {
        FontWeight::Absolute(absolute_value) => {
            match absolute_value {
                AbsoluteFontWeight::Weight(weight) => *weight as u16,
                AbsoluteFontWeight::Normal => 400,
                AbsoluteFontWeight::Bold => 700,
            }
        }
        FontWeight::Bolder => {400}
        FontWeight::Lighter => {400}
    }

}

fn resolve_length_percentage(length: &LengthPercentage, parse_state: &ParseState, font_size: f64, is_font: bool) -> f64{
    match length {
        LengthPercentage::Dimension(dim) => {
            let (value, unit) = dim.to_unit_value();
            match unit {
                "px"    => value as f64,
                "em"    => font_size * value as f64,
                "rem"   => (value * parse_state.root_font_size) as f64,
                "pt"    => value as f64,
                "ex"    => value as f64 * font_size * 0.5,
                _ => {
                    println!("Unsupported unit: {unit}");
                    0.
                }
            }
        }
        LengthPercentage::Percentage(percentage) => {
            
            match is_font {
                true => font_size * (percentage.0 as f64),
                false => parse_state.width * (percentage.0 as f64)
            }
        }
        LengthPercentage::Calc(_) => {0.}
    }
}

fn resolve_length(value: &LengthPercentageOrAuto, parse_state: &ParseState, font_size: f64) -> f64 {
    match value {
        LengthPercentageOrAuto::Auto => {0.}
        LengthPercentageOrAuto::LengthPercentage(length) => {
            resolve_length_percentage(length, parse_state, font_size, false)
        }
    }
}

fn sequence_matches(component: &Component, node: &Node) -> bool {
    match component {
        Component::ExplicitUniversalType => {true} // Matches any element
        Component::LocalName(name) => {
            node.tag_name().name() == name.lower_name.0.as_ref()
        }
        Component::ID(id) => {
            node.attribute("id").unwrap_or_default() == id.0.as_ref()
        }
        Component::Class(class_selector) => {
            if let Some(class_attr) = node.attribute("class") {
                let classes: Vec<&str> = class_attr.split_whitespace().collect();
                classes.contains(&class_selector.as_ref())

            } else {
                false
            }
        }
        Component::AttributeInNoNamespace { local_name, value, .. } => {
            if let Some(attr) = node.attribute(local_name.0.as_ref()) {
                attr.eq(value.0.as_ref())
            } else {
                false
            }
        }
        Component::AttributeInNoNamespaceExists { local_name, .. } => {
            println!("No namespace");
            node.attribute(local_name.0.as_ref()).is_some()
        }
        // Add other component types as needed
        _ => {
            println!("Unsopported: {:#?}", component);
            return true;
        }
    }
}

fn selector_matches2(selector: &Selector, node: &Node, parse_state: &ParseState, document: &Document) -> bool{
    let mut iter = selector.iter();

    
        //let node2 = document.get_node(*parse_state.ancestors.last().unwrap()).unwrap();
        let sequences = iter.by_ref();
        for sequence in sequences {
            if !sequence_matches(sequence, node) {
                return false;
            }
        }


        if let Some(combinator) = iter.next_sequence() {
            match combinator {
                Combinator::Child => {
                    let parent_id = parse_state.ancestors.last().unwrap();
                    let parent = document.get_node(*parent_id).unwrap();
                    let sequences = iter.by_ref();
                    for sequence in sequences {
                        if !sequence_matches(sequence, &parent) {
                            return false;
                        }
                    }
                }
                Combinator::Descendant => {
                    let mut matchs = false;
                    let sequences: Vec<_> = iter.cloned().collect();
                    //println!("Length: {}", parse_state.ancestors.len());
                    for id in parse_state.ancestors.iter() {
                        let mut matched = true;
                        let parent = document.get_node(*id).unwrap();
                        for sequence in &sequences {
                            //println!("Looking");
                           // println!("Sequences: {:#?}", sequence);
                            if !sequence_matches(sequence, &parent) {
                                //println!("Found");
                                matched = false;
                                break;
                            }
                        }
                        if matched {
                            matchs = true;
                            break;
                        }

                    }
                    if !matchs {
                        
                        return false;
                    }
                    
                }
                _ => {}
            }
        
       
    }
    true
}



mod tests {
    use lightningcss::rules::CssRule;
    use lightningcss::selector::{Combinator, Component};
    use lightningcss::stylesheet::{ParserOptions, StyleSheet};

    fn print_selectors(css: &str) {
        let style_sheet = StyleSheet::parse(css, ParserOptions::default()).unwrap();

        for rule in &style_sheet.rules.0 {
            if let CssRule::Style(style_rule) = rule {
                println!("style rule: {:#?}", style_rule.selectors);
                for selector in &style_rule.selectors.0 {
                    println!("selector: {:#?}", selector);
                    let mut parts = selector.iter();
                    let mut iter = selector.iter();
                    loop {
                        let sequence: Vec<_> = iter.by_ref().take_while(|component| {
                            !matches!(component, Component::Combinator(_))
                        }).collect();

                        println!("Sequence: {:?}", sequence);

                        // Move to the next sequence (leftward) and get the combinator if present
                        if let Some(combinator) = iter.next_sequence() {
                           match combinator {
                               Combinator::Child => {}
                               Combinator::Descendant => {}
                               Combinator::NextSibling => {}
                               Combinator::LaterSibling => {}
                               Combinator::PseudoElement => {}
                               Combinator::SlotAssignment => {}
                               Combinator::Part => {}
                               Combinator::DeepDescendant => {}
                               Combinator::Deep => {}
                           }
                        } else {
                            break; // No more sequences
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn print_selector() {
        let css = "h1.chapter-title .chapter-number {
                      color: #333333;
                      font-weight: bold;
                      display: block;
                      font-size: 90%;
                   }";

        print_selectors(css);
    }
}
