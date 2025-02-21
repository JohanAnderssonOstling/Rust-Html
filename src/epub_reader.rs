use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use crossbeam::channel;
use floem::{IntoView, View, ViewId};
use floem::context::EventCx;
use floem::event::{Event, EventPropagation};
use floem::keyboard::{Key, NamedKey};
use floem::kurbo::Size;
use floem::peniko::{Blob, Format, Image};
use floem::prelude::{create_rw_signal, create_signal, RwSignal, SignalGet, SignalUpdate};
use floem::reactive::{create_effect, ReadSignal, WriteSignal};
use floem::views::Decorators;
use floem_renderer::text::{Attrs, LineHeightValue};
use image::io::Reader as ImageReader;
use image::{DynamicImage, ImageFormat};
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

pub struct EpubReader {
    id: ViewId,
    section_index: RwSignal<usize>,
    write_current_url: WriteSignal<String>,
    start_index_signal: RwSignal<Vec<usize>>,
    read_at_end: ReadSignal<bool>,
    read_at_start: ReadSignal<bool>
}
impl  View for EpubReader {
    fn id(&self) -> ViewId { self.id }
    fn event_after_children(&mut self, cx: &mut EventCx, event: &Event) -> EventPropagation {
        match &event {
            Event::KeyDown(event) => {
                match event.key.logical_key {
                    Key::Named(NamedKey::ArrowRight) => {
                        let at_end = self.read_at_end.get();
                        println!("At end: {}", at_end);
                    }
                    _ => ()
                }
            }
            _ => ()
        }
        EventPropagation::Continue
    }
}
impl EpubReader {
    pub fn new (path: &str) -> Self {
        let id = ViewId::new();
        let start_index: Vec<usize> = Vec::new();
        let start_index_signal = create_rw_signal(start_index);
        let (read_at_end, write_at_end) = create_signal(false);
        let (read_at_start, write_at_start) = create_signal(false);



        let epub = Epub::new(path).unwrap();
        let mut style_sheets: Vec<String> = Vec::new();
        for elem in epub.manifest().all_by_media_type("text/css").iter() {
            style_sheets.push(epub.read_file(elem.value()).unwrap());
        }
        let sections: Vec<String> = epub.spine().elements().iter()
            .map(|elem| epub.manifest().by_id(elem.name()).unwrap().value().to_string()).collect();
        let html_text: Vec<String> = epub.reader().iter()
            .map(|cont| cont.unwrap().to_string()).collect();

        let mut image_map: HashMap<String, ImageElem> = HashMap::new();
        let pool = ThreadPool::new(8);
        let mut rng = rand::thread_rng(); // Create a random number generator

        let image_types = ["jpeg", "png", "gif", "webp"];
        let mut index: u8 = 0;

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
            let image_bytes = epub.read_bytes_file(image_path).unwrap();
            let image_size = ImageReader::with_format(Cursor::new(&image_bytes), image_type).into_dimensions().unwrap();

            let size = Size::new(image_size.0 as f64, image_size.1 as f64);
            let width = image_size.0;
            let height = image_size.1;

            //let hash = vec![index / 2];
            let mut bytes = [0u8; 8]; // Fixed-size byte array (8 bytes)
            rng.fill(&mut bytes);
            let hash = bytes.to_vec();
            let image_promise: ImagePromise = Arc::new(RwLock::new(None));
            let image = ImageElem {width, height, hash, image_promise: image_promise.clone()};
            image_map.insert(image_path.to_string(), image);
            index += 1;
            let image_promise_clone = image_promise.clone();
            let image_bytes_clone = image_bytes.clone();
            pool.execute(move || {
                let data = Arc::new(ImageReader::with_format(Cursor::new(image_bytes_clone), image_type).decode().unwrap().to_rgba8().into_raw());
                let mut hasher = Sha256::new();

                let blob = Blob::new(data.clone());
                hasher.update(&blob);
                let hash = hasher.finalize().to_vec();

                let image = Image::new(blob.clone(), Format::Rgba8, width, height);
                *image_promise_clone.write().unwrap() = Some((image.clone(), hash));
                println!("Decoding finished");
            });
        }

        let base_font = Attrs::new().font_size(20.).line_height(LineHeightValue::Normal(1.2));
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
        let (read_current_url, write_current_url) = create_signal(sections[0].clone());
        let (get_go_on, set_go_on) = create_signal(false);

        let mut html_renderer = HtmlRenderer::new(book_factory.cache, pages, read_current_url, set_at_end, get_go_on);
        html_renderer = html_renderer.style(|style| style.width_full());
        id.set_children(vec![html_renderer.into_view()]);
        let section_index = create_rw_signal(0);
        let epub_reader = Self {id, section_index, start_index_signal, read_at_start, read_at_end, write_current_url}.keyboard_navigable();



        create_effect(move |_| {
            let at_ends = get_at_end.get();
            //let mut index = epub_reader.section_index.get();
            println!("Index: {index}");
            //set_go_on.set(false);

            if (at_ends == -1) || (at_ends == 1) {
                set_go_on.set(false);
                if at_ends == -1 {
                    epub_reader.section_index.update(|idx| {
                        if *idx == 0 {
                            set_go_on.set(false);
                            return;
                        }
                        set_go_on.set(true);
                        *idx -= 1;
                        epub_reader.write_current_url.set(sections[*idx].clone());

                    });
                }
                if at_ends == 1 {
                    epub_reader.section_index.update(|idx| {
                        if *idx == sections.len() - 1 {
                            set_go_on.set(false);
                            return;
                        }
                        set_go_on.set(true);
                        *idx += 1;
                        epub_reader.write_current_url.set(sections[*idx].clone());
                    });
                }
                //println!("At ends: {at_ends} Index: {index}" );
            }
        });


        epub_reader
    }
}

