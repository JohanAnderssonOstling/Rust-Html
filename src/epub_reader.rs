use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, RwLock};

use floem::{IntoView, View, ViewId};
use floem::peniko::{Blob, Format, Image};
use floem::prelude::{create_rw_signal, create_signal, RwSignal, SignalGet, SignalUpdate};
use floem::reactive::{create_effect, WriteSignal};
use floem::views::Decorators;
use floem_renderer::text::{Attrs, Family, FamilyOwned, LineHeightValue};
use image::ImageFormat;
use image::io::Reader as ImageReader;
use rand::Rng;
use rayon::prelude::*;
use rayon::prelude::IntoParallelRefIterator;
use rbook::{Ebook, Epub};
use roxmltree::Document;
use sha2::{Digest, Sha256};
use threadpool::ThreadPool;

use crate::book_elem::{BookElemFactory, Elem, ImageElem, ImagePromise};
use crate::glyph_cache::GlyphCache;
use crate::html_renderer::HtmlRenderer;


impl View for EpubReader {
    fn id(&self) -> ViewId {
        self.id
    }
}

pub struct EpubReader {
    id: ViewId,
}

impl EpubReader {
    pub fn new (path: &str) -> Self {
        let id = ViewId::new();
        let start_index: Vec<usize> = Vec::new();
        let start_index_signal = create_rw_signal(start_index);
        let epub = Epub::new(path).unwrap();

        let sections: Vec<String> = epub.spine().elements().iter()
            .map(|elem| epub.manifest().by_id(elem.name()).unwrap().value().to_string()).collect();
        let html_text: Vec<String> = epub.reader().iter()
            .map(|cont| cont.unwrap().to_string()).collect();

        let image_map = process_images(&epub);

        let base_font = Attrs::new().font_size(20.).family(&[FamilyOwned::Serif]).line_height(LineHeightValue::Normal(1.2));
        let cache = GlyphCache::new();
        let documents: Vec<Document> = html_text.par_iter()
            .map(|section| Document::parse(section).unwrap()).collect();
        let mut book_factory = BookElemFactory::new(cache, image_map);
        let elems: Vec<Elem> =  documents.iter().zip(&sections)
            .map(|document| book_factory.parse_root(document.0.root_element(), base_font, document.1.clone()))
            .collect();
        let mut pages: HashMap<String, Elem> = HashMap::with_capacity(elems.len());
        for (elem, url) in elems.into_iter().zip(sections.clone()) {
            pages.insert(url, elem);
        }
        let (get_at_end, set_at_end) = create_signal(0);
        let (current_url, set_current_url) = create_signal(sections[0].clone());
        let (get_go_on, set_go_on) = create_signal(false);

        let mut html_renderer = HtmlRenderer::new(book_factory.cache, pages, current_url, set_at_end, get_go_on);
        html_renderer = html_renderer.style(|style| style.flex_grow(1.0).margin(40));
        id.set_children(vec![html_renderer.into_view()]);
        let section_index = create_rw_signal(0);

        let epub_reader = Self {id}.style(move |s| s.width_full().height_full().flex_grow(1.0));

        create_effect(move |_| {
            let at_ends = get_at_end.get();

            if (at_ends == -1) || (at_ends == 1) {
                set_go_on.set(false);
                if at_ends == -1 {
                    section_index.update(|idx| {
                        if *idx == 0 {
                            set_go_on.set(false);
                            return;
                        }
                        set_go_on.set(true);
                        *idx -= 1;
                        set_current_url.set(sections[*idx].clone());
                    });
                }
                if at_ends == 1 {
                    section_index.update(|idx| {
                        if *idx == sections.len() - 1 {
                            set_go_on.set(false);
                            return;
                        }
                        set_go_on.set(true);
                        *idx += 1;
                        set_current_url.set(sections[*idx].clone());
                    });
                }
            }
        });

        epub_reader
    }
    
}
fn process_images(epub: &Epub) -> HashMap<String, ImageElem> {
    let mut image_map: HashMap<String, ImageElem> = HashMap::new();
    let pool = ThreadPool::new(8);
    let image_types = ["jpeg", "png", "gif", "webp"];

    for elem in epub.manifest().elements() {
        let image_path = elem.value();
        let file_extension = image_path.split(".").skip(1).next().unwrap();
        if !image_types.contains(&file_extension) {continue}
        let image_type = match file_extension {
            "jpeg"  => ImageFormat::Jpeg,
            "jpg"   => ImageFormat::Jpeg,
            "png"   => ImageFormat::Png,
            "gif"   => ImageFormat::Gif,
            _ => continue
        };
        let image_bytes     = epub.read_bytes_file(image_path).unwrap();
        let image_size      = ImageReader::with_format(Cursor::new(&image_bytes), image_type).into_dimensions().unwrap();
        let width           = image_size.0;
        let height          = image_size.1;

        let image_promise: ImagePromise = Arc::new(RwLock::new(None));
        let image = ImageElem {width, height, image_promise: image_promise.clone()};
        image_map.insert(image_path.to_string(), image);
        pool.execute(move || {
            let data = Arc::new(ImageReader::with_format(Cursor::new(image_bytes), image_type).decode().unwrap().to_rgba8().into_raw());
            let mut hasher  = Sha256::new();
            let blob        = Blob::new(data.clone());
            hasher.update(&blob);
            let hash        = hasher.finalize().to_vec();
            let image       = Image::new(blob.clone(), Format::Rgba8, width, height);
            *image_promise.write().unwrap() = Some((image.clone(), hash));
        });
    }
    image_map
}
