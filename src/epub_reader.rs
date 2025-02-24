use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, RwLock};
use std::vec;
use floem::{IntoView, View, ViewId};
use floem::event::EventPropagation;
use floem::peniko::{Blob, Format, Image};
use floem::prelude::{container, create_rw_signal, create_signal, label, RwSignal, SignalGet, SignalUpdate};
use floem::reactive::{create_effect, WriteSignal};
use floem::views::{button, Decorators, h_stack, v_stack};
use floem_renderer::text::{Attrs, FamilyOwned, LineHeightValue};
use image::ImageFormat;
use image::io::Reader as ImageReader;
use rayon::prelude::*;
use rayon::prelude::IntoParallelRefIterator;
use rbook::{Ebook, Epub};
use roxmltree::Document;
use sha2::{Digest, Sha256};
use threadpool::ThreadPool;

use crate::book_elem::{BookElemFactory, Elem, ImageElem, ImagePromise};
use crate::glyph_cache::GlyphCache;
use crate::html_renderer::HtmlRenderer;
use crate::IO::epub::{remove_dtd};
use crate::IO::library::{read_book_position, update_book_path, update_last_read, write_book_position};
use crate::library::{Page, Signals};

pub fn create_epub_reader(path: &str, library_path: &str, prev_page: Page, signals: Signals) -> impl View {
    let epub = Epub::new(path).unwrap();
    let id = epub.metadata().unique_identifier().unwrap().value();
    let position = read_book_position(library_path, id);
    let start_index_signal = create_rw_signal(position.1);

    update_last_read(library_path, id);
    update_book_path(library_path, id, path);
    println!("OPening epub");

    let sections: Vec<String> = epub.spine().elements().iter()
        .map(|elem| epub.manifest().by_id(elem.name()).unwrap().value().to_string()).collect();
    let html_text: Vec<String> = epub.reader().iter()
        .map(|cont| cont.unwrap().to_string()).collect();

    //let image_map = process_images(&epub);
    let image_map: HashMap<String, ImageElem> = HashMap::new();

    let base_font = Attrs::new().font_size(20.).family(&[FamilyOwned::Serif]).line_height(LineHeightValue::Normal(1.4));
    let cache = GlyphCache::new();

    /*let cleaned_files: Vec<String> = html_text.par_iter()
        .map(|section| remove_dtd(section)).collect();*/

    let documents: Vec<Document> = html_text.par_iter()
        .map(|section| Document::parse(&section).unwrap()).collect();
    let mut book_factory = BookElemFactory::new(cache, image_map);
    let elems: Vec<Elem> = documents.iter().zip(&sections)
        .map(|document| book_factory.parse_root(document.0.root_element(), base_font, document.1.clone()))
        .collect();
    let mut pages: HashMap<String, Elem> = HashMap::with_capacity(elems.len());
    for (elem, url) in elems.into_iter().zip(sections.clone()) {
        pages.insert(url, elem);
    }
    let (get_at_end, set_at_end)        = create_signal(0);
    let (current_url, set_current_url)  = create_signal(sections[0].clone());
    let (get_go_on, set_go_on)          = create_signal(false);
    let section_index                   = create_rw_signal(position.0);

    let mut html_renderer = HtmlRenderer::new(start_index_signal, book_factory.cache, pages, current_url, set_at_end, get_go_on);
    html_renderer = html_renderer.style(|style| style.flex_grow(1.0).margin(40));

    let back_button = button(label(move || { "Back" }))
        .on_click(move |_| {
            signals.active_page.set(prev_page);
            EventPropagation::Continue
        });

    let top_panel = h_stack((back_button, )).style(move |s| s.height(20).border_bottom(1));

    let stack= v_stack((top_panel, html_renderer)).style(move |s| s.flex_grow(1.0));
    let lib_path = library_path.to_string();
    let id = id.to_string();
    create_effect(move |_| {
       let start_index = start_index_signal.get();
        write_book_position(&lib_path, &id, section_index.get(), start_index);
    });


    create_effect(move |_| {
        let at_ends = get_at_end.get();

        if (at_ends == -1) || (at_ends == 1) {
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
    container(stack).style(move |s| s.flex_grow(1.0)).into_view()
}


fn process_images(epub: &Epub) -> HashMap<String, ImageElem> {
    let mut image_map: HashMap<String, ImageElem> = HashMap::new();
    let pool = ThreadPool::new(8);
    let image_types = ["jpeg", "png", "gif", "webp"];

    for elem in epub.manifest().elements() {
        let image_path      = elem.value();
        let file_extension  = image_path.split(".").skip(1).next().unwrap();
        if !image_types.contains(&file_extension) { continue; }
        let image_type = match file_extension {
            "jpeg"  => ImageFormat::Jpeg,
            "jpg"   => ImageFormat::Jpeg,
            "png"   => ImageFormat::Png,
            "gif"   => ImageFormat::Gif,
            _       => continue
        };
        let image_bytes = epub.read_bytes_file(image_path).unwrap();
        let image_size  = ImageReader::with_format(Cursor::new(&image_bytes), image_type).into_dimensions().unwrap();
        let width       = image_size.0;
        let height      = image_size.1;

        let image_promise: ImagePromise = Arc::new(RwLock::new(None));
        let image = ImageElem { width, height, image_promise: image_promise.clone() };
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
