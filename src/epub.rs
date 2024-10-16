use std::io::Cursor;

use image::DynamicImage;
use image::io::Reader as ImageReader;
use rbook::{Ebook, Epub};
use roxmltree::Node;

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
        let mut section_contents = Vec::new();
        let mut count = 0;
        
        for cont in &reader
        {
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
