// Cargo.toml:
// [dependencies]
// cssparser = "0.27.2"  // or the latest version
// anyhow = "1.0"
/*
use std::collections::HashMap;
use anyhow::Result;
use cssparser::{Parser, ParserInput, ToCss, Token};

/// Represents a CSS rule with one or more selectors and declarations.
/// Only the "margin" property is supported in this example.
#[derive(Debug)]
pub struct CSSRule {
    /// A list of selectors. Each selector may be compound,
    /// e.g. "div.container#main".
    pub selectors: Vec<String>,
    /// Declarations for the rule (only "margin" supported).
    pub declarations: HashMap<String, String>,
}

/// A handler that stores parsed CSS rules.
#[derive(Debug)]
pub struct CSSHandler {
    rules: Vec<CSSRule>,
}

impl CSSHandler {
    /// Creates a new CSSHandler by parsing the provided CSS string.
    pub fn new(css: &str) -> Result<Self> {
        let rules = Self::parse_css(css)?;
        Ok(Self { rules })
    }

    /// Parses the CSS string into a list of CSSRule objects using rust-cssparser.
    fn parse_css(css: &str) -> Result<Vec<CSSRule>> {
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        let mut rules = Vec::new();

        while !parser.is_exhausted() {
            parser.skip_whitespace();
            // Parse the selectors (everything until the opening '{')
            let selectors = Self::parse_selectors(&mut parser)?;
            // Parse the declaration block.
            let declarations = Self::parse_declarations(&mut parser)?;
            rules.push(CSSRule { selectors, declarations });
            parser.skip_whitespace();
        }
        Ok(rules)
    }

    /// Parses selectors by reading tokens until the opening '{'
    /// and splitting the collected text on commas.
    fn parse_selectors(parser: &mut Parser) -> Result<Vec<String>> {
        let mut selectors_text = String::new();
        while let Ok(token) = parser.next() {
            match token {
                // Stop when the declaration block starts.
                Token::Delim('{') => break,
                _ => {
                    selectors_text.push_str(token.to_css_string().as_str());
                    selectors_text.push(' ');
                }
            }
        }
        let selectors: Vec<String> = selectors_text
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Ok(selectors)
    }

    /// Parses a declaration block (tokens inside `{ ... }`) and returns
    /// a map of property names to values.
    fn parse_declarations(parser: &mut Parser) -> Result<HashMap<String, String>> {
        let mut declarations = HashMap::new();
        // parse_nested_block consumes tokens until the matching '}'.
        parser.parse_nested_block(|input| {
            while !input.is_exhausted() {
                input.skip_whitespace();
                // Expect an identifier for the property name.
                let property = input.expect_ident().map_err(|e| e.to_owned())?;
                input.skip_whitespace();
                input.expect_colon().map_err(|e| e.to_owned())?;
                input.skip_whitespace();

                // Collect tokens until a semicolon is encountered.
                let mut value_tokens = Vec::new();
                while let Ok(token) = input.next() {
                    if let Token::Semicolon = token {
                        break;
                    } else {
                        value_tokens.push(token.to_css_string());
                    }
                }
                let value = value_tokens.join(" ").trim().to_string();
                declarations.insert(property.to_string(), value);
                input.skip_whitespace();
            }
            Ok(())
        }).map_err(|e| anyhow::anyhow!("Error parsing declarations: {:?}", e))?;
        Ok(declarations)
    }

    /// Parses a compound selector into its components:
    /// (optional element type, vector of classes, optional id)
    ///
    /// For example, given "div.container#main":
    ///   - tag: Some("div")
    ///   - classes: vec!["container"]
    ///   - id: Some("main")
    fn parse_compound_selector(selector: &str) -> (Option<String>, Vec<String>, Option<String>) {
        let mut tag: Option<String> = None;
        let mut classes: Vec<String> = Vec::new();
        let mut id: Option<String> = None;

        let mut i = 0;
        let bytes = selector.as_bytes();
        while i < bytes.len() {
            let c = bytes[i] as char;
            if c == '#' {
                // Parse an ID.
                i += 1;
                let start = i;
                while i < bytes.len() {
                    let c2 = bytes[i] as char;
                    if c2 == '.' || c2 == '#' {
                        break;
                    }
                    i += 1;
                }
                let id_str = &selector[start..i];
                if !id_str.is_empty() {
                    id = Some(id_str.to_string());
                }
            } else if c == '.' {
                // Parse a class.
                i += 1;
                let start = i;
                while i < bytes.len() {
                    let c2 = bytes[i] as char;
                    if c2 == '.' || c2 == '#' {
                        break;
                    }
                    i += 1;
                }
                let class_str = &selector[start..i];
                if !class_str.is_empty() {
                    classes.push(class_str.to_string());
                }
            } else if c.is_whitespace() {
                // Skip whitespace.
                i += 1;
            } else {
                // Parse element type.
                let start = i;
                while i < bytes.len() {
                    let c2 = bytes[i] as char;
                    if c2 == '.' || c2 == '#' || c2.is_whitespace() {
                        break;
                    }
                    i += 1;
                }
                let element_str = &selector[start..i];
                if !element_str.is_empty() {
                    tag = Some(element_str.to_string());
                }
            }
        }
        (tag, classes, id)
    }

    /// Checks if a given compound selector matches the element.
    /// The selector may specify:
    /// - an element type (if present, must match),
    /// - one or more classes (all must be present),
    /// - an id (if present, must match).
    fn matches_selector(selector: &str, element: &Element) -> bool {
        let (tag, classes, id) = Self::parse_compound_selector(selector);
        if let Some(ref t) = tag {
            if t != &element.element_type {
                return false;
            }
        }
        for class in classes {
            if !element.classes.contains(&class) {
                return false;
            }
        }
        if let Some(ref selector_id) = id {
            if let Some(ref elem_id) = element.id {
                if selector_id != elem_id {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }

    /// Calculates specificity for a compound selector.
    /// - Each ID adds 100 points.
    /// - Each class adds 10 points.
    /// - An element type adds 1 point.
    fn calculate_specificity(selector: &str) -> u32 {
        let (tag, classes, id) = Self::parse_compound_selector(selector);
        let mut specificity = 0;
        if id.is_some() {
            specificity += 100;
        }
        specificity += 10 * classes.len() as u32;
        if tag.is_some() {
            specificity += 1;
        }
        specificity
    }

    /// Computes the margin for a given element based on matching rules.
    /// When multiple rules match, the rule with the highest specificity wins.
    pub fn get_computed_margin(&self, element: &Element) -> Option<String> {
        let mut best_specificity = 0;
        let mut computed_margin = None;

        for rule in &self.rules {
            for selector in &rule.selectors {
                if Self::matches_selector(selector, element) {
                    let specificity = Self::calculate_specificity(selector);
                    // In case of equal specificity, the later rule wins.
                    if specificity >= best_specificity {
                        best_specificity = specificity;
                        if let Some(margin) = rule.declarations.get("margin") {
                            computed_margin = Some(margin.clone());
                        }
                    }
                }
            }
        }
        computed_margin
    }
}

/// A simple representation of an HTML element.
#[derive(Debug)]
pub struct Element {
    /// The tag name (e.g. "div", "section").
    pub element_type: String,
    /// A list of classes (without the dot, e.g. "container").
    pub classes: Vec<String>,
    /// Optional element ID (without the hash, e.g. "main").
    pub id: Option<String>,
}

mod tests {
    use crate::css::{CSSHandler, Element};

    #[test]
    fn main() -> anyhow::Result<()> {
        let css = r#"
        /* Simple selectors */
        div {
            margin: 10px;
        }
        .container {
            margin: 20px;
        }
        #main {
            margin: 30px;
        }
        /* Compound selector: requires element type, class, and id */
        div.container#main {
            margin: 40px;
        }
        /* Another compound example */
        span.highlight {
            margin: 15px;
        }
    "#;

        let handler = CSSHandler::new(css)?;

        // Define some sample elements.
        let element1 = Element {
            element_type: "div".into(),
            classes: vec![],
            id: None,
        };

        let element2 = Element {
            element_type: "section".into(),
            classes: vec!["container".into()],
            id: None,
        };

        let element3 = Element {
            element_type: "div".into(),
            classes: vec!["container".into()],
            id: Some("main".into()),
        };

        let element4 = Element {
            element_type: "span".into(),
            classes: vec!["highlight".into()],
            id: None,
        };

        println!("Element1 computed margin: {:?}", handler.get_computed_margin(&element1)); // expected: 10px
        println!("Element2 computed margin: {:?}", handler.get_computed_margin(&element2)); // expected: 20px
        println!("Element3 computed margin: {:?}", handler.get_computed_margin(&element3)); // expected: 40px (compound selector wins)
        println!("Element4 computed margin: {:?}", handler.get_computed_margin(&element4)); // expected: 15px

        Ok(())
    }
}


*/