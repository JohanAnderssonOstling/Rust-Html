use lightningcss::selector::{Combinator, Component, Selector};
use roxmltree::{Document, Node};
use scraper::{Element, ElementRef};
use crate::book_elem::ParseState;

// Fast pre-filtering to reject selectors that can't possibly match
pub fn can_selector_match(selector: &Selector, node_tag: &str, node_id: Option<&str>, node_class: Option<&str>) -> bool {
    // Get the rightmost (target) selector components
    let mut iter = selector.iter();
    let target_components: Vec<_> = iter.by_ref().collect();

    for component in &target_components {
        match component {
            Component::LocalName(name) => {
                if node_tag != name.lower_name.0.as_ref() {
                    return false;
                }
            }
            Component::ID(id) => {
                if node_id.unwrap_or("") != id.0.as_ref() {
                    return false;
                }
            }
            Component::Class(class) => {
                if let Some(classes) = node_class {
                    if !classes.split_whitespace().any(|c| c == class.0.as_ref()) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            _ => {} // Other components require full matching
        }
    }
    true
}

pub fn sequence_matches(component: &Component, node: &Node) -> bool {
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
           // println!("Unsopported: {:#?}", component);
            return false;
        }
    }
}

pub fn selector_matches2(selector: &Selector, node: &Node, parse_state: &ParseState, document: &Document) -> bool{
    let mut iter = selector.iter();
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

// Scraper-based selector matching using direct DOM navigation
pub fn selector_matches_scraper(selector: &Selector, element: &ElementRef) -> bool {
    let mut iter = selector.iter();
    let sequences = iter.by_ref();
    for sequence in sequences {
        if !component_matches_scraper(sequence, element) {
            return false;
        }
    }


    
    if let Some(combinator) = iter.next_sequence() {
        match combinator {
            Combinator::Child => {

                let sequences = iter.by_ref();
                for sequence in sequences {
                    if !component_matches_scraper(sequence, &element.parent_element().unwrap()) {
                        return false;
                    }
                }
            }
            Combinator::Descendant => {
                let mut matchs = false;
                let sequences: Vec<_> = iter.cloned().collect();
                //println!("Length: {}", parse_state.ancestors.len());
                for ancestor in element.ancestors().filter_map(ElementRef::wrap) {
                    let mut matched = true;
                    for sequence in &sequences {
                        //println!("Looking");
                        // println!("Sequences: {:#?}", sequence);
                        if !component_matches_scraper(sequence, &ancestor) {
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

pub fn matches_sequence_scraper(sequence: &Vec<&Component>, element: &ElementRef) -> bool {
    for component in sequence {
        if !component_matches_scraper(component, element) {
            return false;
        }
    }
    true
}

pub fn component_matches_scraper(component: &Component, element: &ElementRef) -> bool {

    match component {
        Component::ExplicitUniversalType => true,
        Component::LocalName(name) => {
            element.value().name() == name.lower_name.0.as_ref()
        }
        Component::ID(id) => {
            element.value().attr("id").unwrap_or("") == id.0.as_ref()
        }
        Component::Class(class_selector) => {
            if let Some(class_attr) = element.value().attr("class") {
                let classes: Vec<&str> = class_attr.split_whitespace().collect();
                classes.contains(&class_selector.as_ref())
            } else {
                false
            }
        }
        Component::AttributeInNoNamespace { local_name, value, .. } => {
            if let Some(attr) = element.value().attr(local_name.0.as_ref()) {
                attr.eq(value.0.as_ref())
            } else {
                false
            }
        }
        Component::AttributeInNoNamespaceExists { local_name, .. } => {
            element.value().attr(local_name.0.as_ref()).is_some()
        }
        _ => false,
    }
}