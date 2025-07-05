use floem_renderer::text::Attrs;
use lightningcss::properties::font::{AbsoluteFontSize, AbsoluteFontWeight, FontSize, FontStyle, FontWeight, RelativeFontSize};
use lightningcss::properties::text::TextAlign;
use lightningcss::properties::Property;
use lightningcss::rules::CssRule;
use lightningcss::stylesheet::StyleSheet;
use lightningcss::values::length::{LengthPercentage, LengthPercentageOrAuto, LengthValue};
use roxmltree::{Document, Node};
use scraper::{ElementRef, Html};
use std::fmt::Pointer;
use std::ops::Deref;
use std::hash::{Hash, Hasher};
use rustc_data_structures::fx::FxHashMap;
use crate::book_elem::ParseState;
use crate::styling::selector_matching::{can_selector_match, selector_matches2, selector_matches_scraper};

// Pre-computed font size scaling factors for performance
const FONT_SIZE_SCALES: [f64; 8] = [
    0.5787037037, // 1.2^-3 (xxx-small relative to medium)
    0.6944444444, // 1.2^-2 (xx-small relative to medium) 
    0.8333333333, // 1.2^-1 (x-small relative to medium)
    1.0,          // 1.2^0  (medium)
    1.2,          // 1.2^1  (large relative to medium)
    1.44,         // 1.2^2  (x-large relative to medium)
    1.728,        // 1.2^3  (xx-large relative to medium)
    2.0736,       // 1.2^4  (xxx-large relative to medium)
];

const RELATIVE_FONT_SCALES: [f64; 2] = [
    0.8333333333, // smaller (1/1.2)
    1.2,          // larger
];

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyKey {
    MarginTop,
    MarginRight, 
    MarginBottom,
    MarginLeft,
    PaddingTop,
    PaddingRight,
    PaddingBottom, 
    PaddingLeft,
    FontWeight,
    TextAlign,
    FontStyle,
}

impl PropertyKey {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "margin-top" => Some(Self::MarginTop),
            "margin-right" => Some(Self::MarginRight),
            "margin-bottom" => Some(Self::MarginBottom),
            "margin-left" => Some(Self::MarginLeft),
            "padding-top" => Some(Self::PaddingTop),
            "padding-right" => Some(Self::PaddingRight),
            "padding-bottom" => Some(Self::PaddingBottom),
            "padding-left" => Some(Self::PaddingLeft),
            "font-weight" => Some(Self::FontWeight),
            "text-align" => Some(Self::TextAlign),
            "font-style" => Some(Self::FontStyle),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct Style {
    pub font_size: Option<FontSize>,
    pub properties: FxHashMap<PropertyKey, CSSValue>
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
        let mut style = Style {properties: FxHashMap::default(), font_size: None};
        match node_tag {
            "p" => {
                style.insert(PropertyKey::MarginTop, create_em(1.));
                style.insert(PropertyKey::MarginBottom, create_em(1.));
            }
            "h1" => {
                println!("In h1");
                style.font_size = Some(create_font_size(2.0));
                style.insert(PropertyKey::MarginTop, create_em(0.67));
                style.insert(PropertyKey::MarginBottom, create_em(0.67));
            }
            "h2" => {
                style.font_size = Some(create_font_size(1.5));
                style.insert(PropertyKey::MarginTop, create_em(0.83));
                style.insert(PropertyKey::MarginBottom, create_em(0.83));
            }
            "h3" => {
                style.font_size = Some(create_font_size(1.17));
                style.insert(PropertyKey::MarginTop, create_em(1.));
                style.insert(PropertyKey::MarginBottom, create_em(1.));
            }
            "h4" => {
                style.insert(PropertyKey::MarginTop, create_em(1.33));
                style.insert(PropertyKey::MarginBottom, create_em(1.33));
            }
            "h5" => {
                style.font_size = Some(create_font_size(0.83));
                style.insert(PropertyKey::MarginTop, create_em(1.67));
                style.insert(PropertyKey::MarginBottom, create_em(1.67));
            }
            "h6" => {
                style.font_size = Some(create_font_size(0.67));
                style.insert(PropertyKey::MarginTop, create_em(2.33));
                style.insert(PropertyKey::MarginBottom, create_em(2.33));
            }
            "dd" => {
                style.insert(PropertyKey::MarginLeft, create_px(40.))
            }
            "th" => {
                style.insert(PropertyKey::FontWeight, CSSValue::FontWeight(FontWeight::Absolute(AbsoluteFontWeight::Bold)));
                style.insert(PropertyKey::TextAlign, CSSValue::TextAlign(TextAlign::Center))
            }
            "blockquote" => {
                style.insert(PropertyKey::MarginTop, create_em(1.));
                style.insert(PropertyKey::MarginBottom, create_em(1.));
                style.insert(PropertyKey::MarginLeft, create_px(40.));
                style.insert(PropertyKey::MarginRight, create_px(40.));
            }
            "em" => {
                println!("In em");
                style.insert(PropertyKey::FontStyle, CSSValue::TextStyle(floem_renderer::text::Style::Italic));
            }
            "strong" => {
                style.insert(PropertyKey::FontWeight, CSSValue::FontWeight(FontWeight::Absolute(AbsoluteFontWeight::Bold)))
            }
            _ => ()
        }
        style
    }
    pub fn insert(&mut self, key: PropertyKey, value: CSSValue) {
        self.properties.insert(key, value);
    }
    
    pub fn insert_str(&mut self, key: &str, value: CSSValue) {
        if let Some(prop_key) = PropertyKey::from_str(key) {
            self.properties.insert(prop_key, value);
        }
    }
    
    pub fn with_capacity(capacity: usize) -> Self {
        Style {
            font_size: None,
            properties: FxHashMap::with_capacity_and_hasher(capacity, Default::default()),
        }
    }
    
    pub fn apply_property(&mut self, property: &Property) {
        match property {
            Property::MarginTop(value) => self.insert(PropertyKey::MarginTop, CSSValue::Length(value.clone())),
            Property::MarginRight(value) => self.insert(PropertyKey::MarginRight, CSSValue::Length(value.clone())),
            Property::MarginBottom(value) => self.insert(PropertyKey::MarginBottom, CSSValue::Length(value.clone())),
            Property::MarginLeft(value) => self.insert(PropertyKey::MarginLeft, CSSValue::Length(value.clone())),
            Property::PaddingTop(value) => self.insert(PropertyKey::PaddingTop, CSSValue::Length(value.clone())),
            Property::PaddingRight(value) => self.insert(PropertyKey::PaddingRight, CSSValue::Length(value.clone())),
            Property::PaddingBottom(value) => self.insert(PropertyKey::PaddingBottom, CSSValue::Length(value.clone())),
            Property::PaddingLeft(value) => self.insert(PropertyKey::PaddingLeft, CSSValue::Length(value.clone())),
            Property::FontWeight(value) => self.insert(PropertyKey::FontWeight, CSSValue::FontWeight(value.clone())),
            Property::TextAlign(value) => self.insert(PropertyKey::TextAlign, CSSValue::TextAlign(value.clone())),
            Property::FontStyle(value) => self.insert(PropertyKey::FontStyle, CSSValue::TextStyle(
                match value {
                    FontStyle::Normal => floem_renderer::text::Style::Normal,
                    FontStyle::Italic => floem_renderer::text::Style::Italic,
                    FontStyle::Oblique(_) => floem_renderer::text::Style::Italic,
                }
            )),
            Property::FontSize(value) => self.font_size = Some(value.clone()),
            _ => {} // Ignore unsupported properties
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
    let node_tag = node.tag_name().name();
    let node_id = node.attribute("id");
    let node_class = node.attribute("class");

    for (index, rule) in style_sheet.rules.0.iter().enumerate() {
        match rule {
            CssRule::Style(style_rule) => {
                for selector in &style_rule.selectors.0 {
                    // Fast pre-filtering: check if selector could possibly match this node
                    if can_selector_match(selector, node_tag, node_id, node_class) {
                        if selector_matches2(selector, node, parse_state, document) {
                            matched_rules.push(MatchedRule {
                                specificity: selector.specificity(),
                                source_order: index,
                                declarations: &style_rule.declarations.declarations
                            })
                        }
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
                        style.insert(PropertyKey::MarginTop, CSSValue::Length(margins.top.clone()));
                        style.insert(PropertyKey::MarginRight, CSSValue::Length(margins.right.clone()));
                        style.insert(PropertyKey::MarginBottom, CSSValue::Length(margins.bottom.clone()));
                        style.insert(PropertyKey::MarginLeft, CSSValue::Length(margins.left.clone()));
                    }
                    Property::Padding(paddings) => {
                        style.insert(PropertyKey::PaddingTop, CSSValue::Length(paddings.top.clone()));
                        style.insert(PropertyKey::PaddingRight, CSSValue::Length(paddings.right.clone()));
                        style.insert(PropertyKey::PaddingBottom, CSSValue::Length(paddings.bottom.clone()));
                        style.insert(PropertyKey::PaddingLeft, CSSValue::Length(paddings.left.clone()));
                    }
                    
                    _ => ()
                }
                continue
            }
            match decl {
                Property::MarginTop(value) => {style.insert(PropertyKey::MarginTop, CSSValue::Length(value.clone()));}
                Property::MarginRight(value) => {style.insert(PropertyKey::MarginRight, CSSValue::Length(value.clone()));}
                Property::MarginBottom(value) => {style.insert(PropertyKey::MarginBottom, CSSValue::Length(value.clone()));}
                Property::MarginLeft(value) => {style.insert(PropertyKey::MarginLeft, CSSValue::Length(value.clone()));}
                Property::PaddingTop(value) => {style.insert(PropertyKey::PaddingTop, CSSValue::Length(value.clone()));}
                Property::PaddingRight(value) => {style.insert(PropertyKey::PaddingRight, CSSValue::Length(value.clone()));}
                Property::PaddingBottom(value) => {style.insert(PropertyKey::PaddingBottom, CSSValue::Length(value.clone()));}
                Property::PaddingLeft(value) => {style.insert(PropertyKey::PaddingLeft, CSSValue::Length(value.clone()));}
                Property::FontSize(font_size) => {style.font_size = Some(font_size.to_owned());}
                Property::FontWeight(font_weight) => {style.insert(PropertyKey::FontWeight, CSSValue::FontWeight(font_weight.clone()))}
                Property::TextAlign(text_align) => {style.insert(PropertyKey::TextAlign, CSSValue::TextAlign(text_align.clone()))}
                Property::FontStyle(font_style) => {
                    let text_style = match font_style {
                        FontStyle::Normal => floem_renderer::text::Style::Normal,
                        FontStyle::Italic => floem_renderer::text::Style::Italic,
                        FontStyle::Oblique(_) => floem_renderer::text::Style::Oblique,
                    };
                    style.insert(PropertyKey::FontStyle, CSSValue::TextStyle(text_style));
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
                match key {
                    PropertyKey::MarginTop      => margins.top      += resolve_length(value, &parse_state, font_size),
                    PropertyKey::MarginRight    => margins.right    += resolve_length(value, &parse_state, font_size),
                    PropertyKey::MarginBottom   => margins.bottom   += resolve_length(value, &parse_state, font_size),
                    PropertyKey::MarginLeft     => margins.left     += resolve_length(value, &parse_state, font_size),
                    PropertyKey::PaddingTop     => margins.top      += resolve_length(value, &parse_state, font_size),
                    PropertyKey::PaddingRight   => margins.right    += resolve_length(value, &parse_state, font_size),
                    PropertyKey::PaddingBottom  => margins.bottom   += resolve_length(value, &parse_state, font_size),
                    PropertyKey::PaddingLeft    => margins.left     += resolve_length(value, &parse_state, font_size),
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
                match key {
                    PropertyKey::MarginTop      => margins.top      += resolve_length(value, &parse_state, font_size),
                    PropertyKey::MarginRight    => margins.right    += resolve_length(value, &parse_state, font_size),
                    PropertyKey::MarginBottom   => margins.bottom   += resolve_length(value, &parse_state, font_size),
                    PropertyKey::MarginLeft     => margins.left     += resolve_length(value, &parse_state, font_size),
                    PropertyKey::PaddingTop     => margins.top      += resolve_length(value, &parse_state, font_size),
                    PropertyKey::PaddingRight   => margins.right    += resolve_length(value, &parse_state, font_size),
                    PropertyKey::PaddingBottom  => margins.bottom   += resolve_length(value, &parse_state, font_size),
                    PropertyKey::PaddingLeft    => margins.left     += resolve_length(value, &parse_state, font_size),
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

fn apply_matched_rules(matched_rules: &[MatchedRule], style: &mut Style) {
    for matched_rule in matched_rules {
        for property in matched_rule.declarations {
            style.apply_property(property);
        }
    }
}

// Scraper-compatible style functions
pub fn apply_style_sheet_scraper(style_sheet: &StyleSheet, element: &ElementRef, style: &mut Style) {
    let mut matched_rules: Vec<MatchedRule> = Vec::with_capacity(16);
    let node_tag = element.value().name();
    let node_id = element.value().attr("id");
    let node_class = element.value().attr("class");

    for (index, rule) in style_sheet.rules.0.iter().enumerate() {
        match rule {
            CssRule::Style(style_rule) => {
                for selector in &style_rule.selectors.0 {
                    // Fast pre-filtering: check if selector could possibly match this node
                    if can_selector_match(selector, node_tag, node_id, node_class) {
                        // For now, simplified matching - just match basic selectors
                        if selector_matches_scraper(selector, element) {
                            matched_rules.push(MatchedRule {
                                specificity: selector.specificity(),
                                source_order: index,
                                declarations: &style_rule.declarations.declarations
                            })
                        }
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
    
    // Apply matched rules to style
    apply_matched_rules(&matched_rules, style);
}

pub fn resolve_style_scraper(style_sheets: &Vec<StyleSheet>, element: &ElementRef, font: &mut Attrs, mut parse_state: ParseState) -> (Margins, ParseState) {
    let mut style = Style::new(element.value().name());
    let mut margins = Margins {top: 0., right: 0., bottom: 0., left: 0.};
    
    for style_sheet in style_sheets {
        apply_style_sheet_scraper(style_sheet, element, &mut style);
    }
    
    if let Some(font_size) = &style.font_size {
        let resolved_font_size = resolve_font_size(font_size, &parse_state, font.font_size as f64).round();
        if resolved_font_size != 0. { *font = font.font_size(resolved_font_size as f32); }
    }
    
    let font_size = font.font_size as f64;
    for (key, value) in style.properties.iter() {
        match value {
            CSSValue::Length(value) => {
                match key {
                    PropertyKey::MarginTop      => margins.top      += resolve_length(value, &parse_state, font_size),
                    PropertyKey::MarginRight    => margins.right    += resolve_length(value, &parse_state, font_size),
                    PropertyKey::MarginBottom   => margins.bottom   += resolve_length(value, &parse_state, font_size),
                    PropertyKey::MarginLeft     => margins.left     += resolve_length(value, &parse_state, font_size),
                    PropertyKey::PaddingTop     => margins.top      += resolve_length(value, &parse_state, font_size),
                    PropertyKey::PaddingRight   => margins.right    += resolve_length(value, &parse_state, font_size),
                    PropertyKey::PaddingBottom  => margins.bottom   += resolve_length(value, &parse_state, font_size),
                    PropertyKey::PaddingLeft    => margins.left     += resolve_length(value, &parse_state, font_size),
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



fn resolve_font_size(value: &FontSize, parse_state: &ParseState, font_size: f64) -> f64 {
    match value {
        FontSize::Length(length) => {
            resolve_length_percentage(&length, parse_state, font_size, true)
        }
        FontSize::Absolute(absolute) => {
            let scale_index = match absolute {
                AbsoluteFontSize::XXSmall   => 0,
                AbsoluteFontSize::XSmall    => 1,
                AbsoluteFontSize::Small     => 2,
                AbsoluteFontSize::Medium    => 3,
                AbsoluteFontSize::Large     => 4,
                AbsoluteFontSize::XLarge    => 5,
                AbsoluteFontSize::XXLarge   => 6,
                AbsoluteFontSize::XXXLarge  => 7,
            };
            (parse_state.root_font_size as f64 * FONT_SIZE_SCALES[scale_index]).round()
        }
        FontSize::Relative(relative) => {
            let scale_index = match relative {
                RelativeFontSize::Smaller => 0,
                RelativeFontSize::Larger => 1,
            };
            (font_size * RELATIVE_FONT_SCALES[scale_index]).round()
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
