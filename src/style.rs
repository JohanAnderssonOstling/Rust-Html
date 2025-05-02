use std::collections::HashMap;

use floem_renderer::text::Attrs;
use lightningcss::properties::font::{AbsoluteFontSize, AbsoluteFontWeight, FontSize, FontStyle, FontWeight, RelativeFontSize};
use lightningcss::properties::Property;
use lightningcss::properties::text::TextAlign;
use lightningcss::rules::CssRule;
use lightningcss::selector::Component;
use lightningcss::stylesheet::StyleSheet;
use lightningcss::values::length::{LengthPercentage, LengthPercentageOrAuto, LengthValue};
use roxmltree::Node;

use crate::book_elem::ParseState;

pub struct Margins {
    pub top: f64, pub right: f64, pub bottom: f64, pub left: f64,
}
pub enum CSSValue {
    Length(LengthPercentageOrAuto),
    FontWeight(FontWeight),
    TextAlign(TextAlign),
    TextStyle(floem_renderer::text::Style),
}
pub struct Style {
    pub font_size: Option<FontSize>,
    pub properties: HashMap<String, CSSValue>
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
            _ => ()
        }
        style
    }
    pub fn insert(&mut self,key: &str, value: CSSValue) {
        self.properties.insert(key.to_string(), value);
    }
}

struct MatchedRule<'a,'b> {
    specificity: u32,
    source_order: usize,
    declarations: &'a [Property<'b>]
}
pub fn apply_style_sheet(style_sheet: & StyleSheet, node: &Node, style: &mut Style) {
    let mut matched_rules: Vec<MatchedRule> = Vec::new();

    for (index, rule) in style_sheet.rules.0.iter().enumerate() {
        match rule {
            CssRule::Style(style_rule) => {
                for selector in &style_rule.selectors.0 {
                    let components: Vec<_> = selector.iter().collect();
                    if selector_matches(&components, node) {
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

pub fn resolve_style(style_sheets: &Vec<StyleSheet>, node: &Node, font: &mut Attrs, mut parse_state: ParseState) -> (Margins, ParseState){
    let mut style = Style::new(node.tag_name().name());
    let mut margins = Margins {top: 0., right: 0., bottom: 0., left: 0.};
    for style_sheet in style_sheets {
        apply_style_sheet(style_sheet, &node, &mut style);
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

fn selector_matches(selectors: &Vec<&Component<>>, node: &Node) -> bool {
        for part in selectors.iter() {
            
            match part {
                Component::ExplicitUniversalType => {}
                Component::LocalName(name)  if node.tag_name().name() != name.lower_name.0.as_ref() => return false,
                Component::ID(id)           if node.attribute("id").unwrap_or_default() != id.0.as_ref()  => return false,
                Component::Class(class_selector) => {
                    if let Some(class_attr) = node.attribute("class") {
                        let classes: Vec<&str> = class_attr.split_whitespace().collect();
                        if !classes.contains(&class_selector.as_ref()) { return false; }
                    }
                    else { return false; }
                }
                //Component::AttributeInNoNamespaceExists { .. } => {}
                //Component::AttributeInNoNamespace { .. } => {}
                //Component::AttributeOther(_) => {}
                Component::Negation(negated_selectors) => {}
                _ => ()
            }
        }
        return true;

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
            return match unit {
                "px"    => value as f64,
                "em"    => value as f64 * font_size,
                "rem"   => (value * parse_state.root_font_size) as f64,
                "pt"    => dim.to_px().unwrap() as f64,
                _ => {
                    println!("Unsupported unit: {unit}");
                    0.
                }
            };
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