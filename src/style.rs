use std::collections::HashMap;
use floem::taffy::LengthPercentageAuto;
use lightningcss::properties::{Property, PropertyId};
use lightningcss::properties::font::{FontSize, FontWeight};
use lightningcss::properties::text::TextAlign;
use lightningcss::rules::CssRule;
use lightningcss::rules::style::StyleRule;
use lightningcss::selector::{Component, Selector};
use lightningcss::stylesheet::StyleSheet;
use lightningcss::values::length::{LengthPercentage, LengthPercentageOrAuto, LengthValue};
use roxmltree::Node;
/*
pub struct Style<'a, 'c> {
    pub properties: HashMap<PropertyId<'a>, Property<'c>>
}
*/
 
pub enum CSSValue {
    Length(LengthPercentageOrAuto),
    FontWeight(FontWeight),
    TextAlign(TextAlign)
}
pub struct S {
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
impl  S {
    pub fn new(node_tag: &str) -> S {
        let mut style = S {properties: HashMap::new(), font_size: None};
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
            _ => ()
        }
        style
    }
    pub fn insert(&mut self,key: &str, value: CSSValue) {
        self.properties.insert(key.to_string(), value);
    }
}
/*
 impl <'a, 'c> Style<'a, 'c> {
    pub fn new() -> Style<'a, 'c>{

        Style {properties: HashMap::new()}
    }
}
*/

struct MatchedRule<'a,'b> {
    specificity: u32,
    source_order: usize,
    declarations: &'a [Property<'b>]
}
pub fn apply_style_sheet(style_sheet: & StyleSheet, node: &Node, style: &mut S) {
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
                        style.insert("margin-top", CSSValue::Length(margins.top.clone()));
                        style.insert("margin-right", CSSValue::Length(margins.right.clone()));
                        style.insert("margin-bottom", CSSValue::Length(margins.bottom.clone()));
                        style.insert("margin-left", CSSValue::Length(margins.left.clone()));
                    }
                    Property::Padding(paddings) => {
                        style.insert("padding-top", CSSValue::Length(paddings.top.clone()));
                        style.insert("padding-right", CSSValue::Length(paddings.right.clone()));
                        style.insert("padding-bottom", CSSValue::Length(paddings.bottom.clone()));
                        style.insert("padding-left", CSSValue::Length(paddings.left.clone()));
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
                _ => ()
            }
        }
    }
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

