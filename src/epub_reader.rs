use std::cell::RefCell;
use std::io::Cursor;
use std::rc::Rc;
use std::time::Instant;
use floem::{IntoView, View, ViewId};
use floem::prelude::SignalUpdate;
use floem::reactive::RwSignal;
use floem::views::Decorators;
use image::{DynamicImage, ImageFormat};
use image::io::Reader as ImageReader;
use rayon::prelude::IntoParallelRefIterator;
use rbook::{Ebook, Epub};
use roxmltree::{Document, Node};
use crate::html_renderer::HtmlRenderer;
pub struct EpubReader {
    id: ViewId,
    section_index: usize,
}
impl  View for EpubReader { fn id(&self) -> ViewId { self.id } }
impl EpubReader {
    pub fn new (path: &str) -> Self {
        let id = ViewId::new();
        let epub = Epub::new(path).unwrap();
        let mut style_sheets: Vec<String> = Vec::new();
        for elem in epub.manifest().all_by_media_type("text/css").iter() {
            style_sheets.push(epub.read_file(elem.value()).unwrap());
        }
        let sections: Vec<String> = epub.reader().iter()
            .map(|cont| cont.unwrap().to_string()).collect();
        let now = Instant::now();
        let mut images: Vec<String> = epub.manifest().all_by_media_type("image/gif").iter()
            .map(|elem| elem.value().to_string()).collect();

        /*images.par_iter().for_each(|elem| {
            //println!("Image: {}", elem.value());
            let img_bytes: Vec<u8> = epub.read_bytes_file(elem.value()).unwrap();
            let img = ImageReader::with_format(Cursor::new(img_bytes), ImageFormat::Gif).decode().unwrap();
        });*/
        let document = Document::parse(&sections[15]);
        println!("Elapsed: {}", now.elapsed().as_millis());
        //let html_content = epub.reader().set_current_page(20).unwrap().unwrap().to_string();
        //let document = Document::parse(&html_content).unwrap();
        let mut html_renderer = HtmlRenderer::new(&document.unwrap());
        html_renderer = html_renderer.style(|style| style.width_full());
        id.set_children(vec![html_renderer.into_view()]);
        Self {id, section_index: 0}
    }
}

pub struct EpubBook {
    epub: Epub,
    pub contents: Vec<String>,
}

impl EpubBook {
    pub fn new(path: &str) -> Self {
        let epub = Epub::new(path).unwrap();
        let mut reader = epub.reader();
        let spine = epub.spine().elements();
        let toc = epub.toc().elements();
        let t = epub.manifest().all_by_media_type("text/css");
        for elem in t {
            println!("{}",elem.value());
        }
        let mut section_contents = Vec::new();
        let mut count = 0;
        
        for cont in &reader {
            let content = cont.unwrap();
            section_contents.push(content.to_string());
            count += 1;
        }
        Self { epub, contents: section_contents }
    }

    fn load_image(&self, node: Node) -> (DynamicImage, String) {
        let img_path 	= node.attribute("src").unwrap().to_string();
        let format = img_path.split(".").last().unwrap().to_string();
        println!("Loading imgage {}", img_path);
        let img_bytes = self.epub.read_bytes_file(img_path.replace("../", "")).unwrap();
        let img = ImageReader::new(Cursor::new(img_bytes)).with_guessed_format().unwrap().decode().unwrap();
        (img, format)
    }
}

fn collect_text(node: Node) -> String {
    let mut text = String::new();
    for child in node.children() {
        if child.text().is_some() {
            text.push_str(child.text().unwrap());
        }
    }
    text
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_parsing() {
        let epub_book = EpubBook::new("/home/johan/Hem/Downloads/A Concise History of Switzerland -- Clive H_ Church; Randolph C_ Head -- 2013 -- Cambridge University Press -- 9780521143820 -- 046e534312eeda8990a82749ebab90fc -- Anna’s Archive.epub");
        
    }
}
