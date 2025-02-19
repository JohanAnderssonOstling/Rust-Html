use std::collections::HashMap;
use std::time::Instant;

use floem::{IntoView, View, ViewId};
use floem::views::Decorators;
use floem_renderer::text::{Attrs, LineHeightValue};
use image::DynamicImage;
use rayon::prelude::*;
use rayon::prelude::IntoParallelRefIterator;
use rbook::{Ebook, Epub};
use roxmltree::Document;

use crate::book_elem::{BookElemFactory, Elem, GlyphCache};
use crate::html_renderer::HtmlRenderer;

pub struct EpubReader {
    id: ViewId,
    section_index: usize,
    //elems: Vec<Elem>,
    images: Vec<DynamicImage>

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
        let t: Vec<String> = epub.spine().elements().iter()
            .map(|elem| epub.manifest().by_id(elem.name()).unwrap().value().to_string()).collect();
        let sections: Vec<String> = epub.reader().iter()
            .map(|cont| cont.unwrap().to_string()).collect();
        let mut images: Vec<String> = epub.manifest().all_by_media_type("image/gif").iter()
            .map(|elem| elem.value().to_string()).collect();

        let img_bytes: Vec<Vec<u8>> = images.iter()
            .map(|image_path| epub.read_bytes_file(image_path).unwrap())
            .collect();

        /*let images: Vec<DynamicImage> = img_bytes.par_iter()
            .map(|img_bytes| ImageReader::with_format(Cursor::new(img_bytes), ImageFormat::Gif).decode().unwrap())
            .collect();*/
        let images: Vec<DynamicImage> = Vec::new();

        let base_font = Attrs::new().font_size(20.).line_height(LineHeightValue::Normal(1.2));
        let cache = GlyphCache::new();
        let documents: Vec<Document> = sections.par_iter()
            .map(|section| Document::parse(section).unwrap())
            .collect();
        let mut book_factory = BookElemFactory::new(cache);
        let now = Instant::now();

        let elems: Vec<Elem> =  documents.iter()
            .map(|document|
                book_factory.parse_root(document.root_element(), base_font))
            .collect();
        let mut pages: HashMap<String, Elem> = HashMap::with_capacity(elems.len());
        for (elem, url) in elems.into_iter().zip(t.clone()) {
            println!("url: {}", &url);
            pages.insert(url, elem);
        }
        println!("Elapsed: {}", now.elapsed().as_millis());
        let document = Document::parse(&sections[15]).unwrap();
        println!("Current url: {}", t[15].clone());
        let mut html_renderer = HtmlRenderer::new(&document, book_factory.cache, pages, t[15].clone());
        html_renderer = html_renderer.style(|style| style.width_full());
        id.set_children(vec![html_renderer.into_view()]);
        Self {id, section_index: 0, images}
    }
}

