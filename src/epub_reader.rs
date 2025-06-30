use std::collections::HashMap;
use std::io::Cursor;
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use std::vec;
use floem::{IntoView, View, ViewId};
use floem::event::EventPropagation;
use floem::peniko::{Blob, Format, Image};
use floem::prelude::{Color, container, create_rw_signal, create_signal, dyn_view, label, RwSignal, scroll, ScrollExt, SignalGet, SignalUpdate};
use floem::reactive::{create_effect, WriteSignal};
use floem::views::{button, Decorators, empty, h_stack, v_stack};
use floem_renderer::text::{Attrs, FamilyOwned, LineHeightValue};
use image::ImageFormat;
use image::io::Reader as ImageReader;
use lightningcss::stylesheet::{ParserOptions, StyleSheet};
use rayon::prelude::*;
use rayon::prelude::IntoParallelRefIterator;
use rbook::{Ebook, Epub};
use rbook::epub::Toc;
use regex::Regex;
use roxmltree::Document;
use rustc_data_structures::fx::FxHashMap;
use sha2::{Digest, Sha256};
use threadpool::ThreadPool;

use crate::book_elem::{BookElemFactory, CharGlyph, Elem, get_size, HTMLPage, ImageElem, ImagePromise, InlineContent, InlineElem, MemUsage, ParseState};
use crate::glyph_interner::GlyphCache;

use crate::IO::epub::{remove_dtd};
use crate::IO::library::{read_book_position, update_book_path, update_last_read, write_book_position};
use crate::library::{Page, Signals};
use crate::renderer::html_renderer::HtmlRenderer;
use crate::toc::{hierarchical_toc_entry, toc_view, TocEntry};

pub fn create_epub_reader(path: &str, library_path: &str, prev_page: Page, signals: Signals) -> impl View {
    let epub = Epub::new(path).unwrap();
    let id = epub.metadata().unique_identifier().unwrap().value();
    let position = read_book_position(library_path, id);
    let start_index_signal = create_rw_signal(position.1);

    update_last_read(library_path, id);
    update_book_path(library_path, id, path);

    let sections: Vec<String> = epub.spine().elements().iter()
        .map(|elem| epub.manifest().by_id(elem.name()).unwrap().value().to_string()).collect();
    let html_text: Vec<String> = epub.reader().iter()
        .map(|cont| {
            let text = cont.unwrap().to_string();
            let re = Regex::new(r#"(?i)<!DOCTYPE[^>]*>"#).unwrap();
            let cleaned = re.replace(&text, "").into_owned();
            cleaned
        }).collect();

    let now = Instant::now();
    let image_map = process_images(&epub);

    println!("Elapsed image processing time: {}", now.elapsed().as_millis());
    //let image_map: HashMap<String, ImageElem> = HashMap::new();

    let font_family = "Liberation Serif".to_string();
    let f = &[FamilyOwned::Name(font_family)];
    let base_font = Attrs::new()
        .font_size(20.)
        .family(f)
        .line_height(LineHeightValue::Normal(1.5))
        .color(Color::rgb8(43, 43, 43))
        ;
    let cache = GlyphCache::new();



    let documents: Vec<Document> = html_text.par_iter()
        .map(|section| {

            Document::parse(section).unwrap()
        }).collect();
    let css_strings: Vec<String> = epub.manifest().all_by_media_type("text/css").iter()
        .map(|css_name| epub.read_file(css_name.value()).unwrap())
        .collect();
    let style_sheets: Vec<StyleSheet> = css_strings.iter()
        .map(|css_string| StyleSheet::parse(css_string, ParserOptions::default()).unwrap())
        .collect();
    //let style_sheets = Vec::new();
    let now = Instant::now();
    let mut book_factory = BookElemFactory::new(cache, image_map, &base_font);
    let elems: Vec<HTMLPage> = documents.iter().zip(&sections)
        .map(|document| {
            book_factory.parse_root(document.0.root_element(), base_font, document.1.clone(), &style_sheets, document.0)
        })
        .collect();
    let mut pages: HashMap<String, HTMLPage> = HashMap::with_capacity(elems.len());
    let mut link_index: HashMap<String, FxHashMap<String, Vec<usize>>> = HashMap::with_capacity(elems.len());
    for (elem, url) in elems.into_iter().zip(sections.clone()) {
        link_index.insert(url.clone(), elem.locations.clone());
        pages.insert(url, elem);
    }
    drop(style_sheets);
    println!("Elapsed parsing time: {}", now.elapsed().as_millis());
    println!("Style time: {}", book_factory.style_time / 1_000_000);
    let (get_at_end, set_at_end)        = create_signal(0);
    let (get_go_on, set_go_on)          = create_signal(false);
    let section_index                   = create_rw_signal(position.0);
    let (current_url)  = create_rw_signal(sections[position.0].clone());

    let cache_size = book_factory.cache.total_memory_usage();
    println!("Cache size: {cache_size}");
    let mut size = 0;
    let mut mem_usage = MemUsage {char_size: 0, inline_size: 0, line_size: 0, elem_size: 0};
    for page in pages.values() {
        get_size(&page.root, &mut mem_usage)
    }
    println!("Inline size: {}", std::mem::size_of::<InlineElem>());
    println!("Inline content: {}", std::mem::size_of::<InlineContent>());
    println!("Char size: {}", std::mem::size_of::<CharGlyph>());
    println!("String size: {}", std::mem::size_of::<String>());
    println!("Elem Size: {}", mem_usage.elem_size / 1_000_000);
    println!("Line Size: {}", mem_usage.line_size / 1_000_000);
    println!("Size: {}", mem_usage.char_size / 1_000_000);
    println!("Inline Size: {}", mem_usage.inline_size / 1_000_000);
    let mut html_renderer = HtmlRenderer::new(start_index_signal, book_factory.cache, pages, current_url, set_at_end, get_go_on);
    html_renderer = html_renderer.style(|style| style.flex_grow(1.0).margin(40).width_full());


    let toc_on_click = Rc::new(move |link: String| {
        println!("Clicked toc link: {link}");
        let parts: Vec<&str> = link.split("#").collect();
        current_url.set(parts[0].to_string());
        if parts.len() == 1 {
            start_index_signal.set(Vec::new());
            return;
        }
        let ids = link_index.get(parts[0]).unwrap();
        match ids.get(parts[1]) {
            None => {start_index_signal.set(Vec::new())}
            Some(index) => {start_index_signal.set(index.clone())}
        }

    });
    let show_sidebar = create_rw_signal(true);
    let toggle_button = button(label (move || {"Toggle TOC"}))
        .on_click(move |_| {
            let show_sidebar = show_sidebar.clone();
             show_sidebar.update(|v| *v = !*v);
            EventPropagation::Continue
        });
    let back_button = button(label(move || { "Back" }))
        .on_click(move |_| {
            signals.active_page.set(prev_page);
            EventPropagation::Continue
        });
    let top_panel = h_stack((back_button, toggle_button)).style(move |s| s.border_bottom(1).flex_shrink(0.).flex_grow(0.));
    let toc = create_toc(epub.toc().elements());
    //let toc_view = v_stack((toc_view(toc, toc_on_click, 0),)).scroll()
       //     .style(|s| s.border_right(1).width(321).height_full());

    let toc_view = dyn_view(move ||
        if show_sidebar.get() {
            container(v_stack((crate::toc::toc_view(toc.clone(), toc_on_click.clone(), 0),)).scroll()
                .style(|s| s.border_right(1).width(340)))
        }
        else {
            container(empty())
        }
    );
    let main_area = h_stack((
        toc_view,
        html_renderer
    )).style(move |s| s.flex_grow(1.0).min_height(0));

    let stack= v_stack((top_panel, main_area,)).style(move |s| s.flex_grow(1.0).height_full().flex_col());
    let lib_path = library_path.to_string();
    let id = id.to_string();
    let cloned_sections = sections.clone();
    create_effect(move |_| {
       let start_index = start_index_signal.get();
        let url = current_url.get_untracked();
        let mut counter = 0;
        for section in &cloned_sections {
            if section.eq(&url) {
                break;
            }
            counter += 1;
        }

        write_book_position(&lib_path, &id, counter, start_index);
    });
    
    create_effect(move |_| {
        let at_ends = get_at_end.get();
        if (at_ends == -1) || (at_ends == 1) {
            section_index.update(|idx| {
                let url = current_url.get_untracked();
                let mut counter = 0;
                for section in &sections {
                    if section.eq(&url) {
                        *idx = counter;
                    }
                    counter += 1;
                }
                if at_ends == -1 {
                    if *idx == 0 {
                        set_go_on.set(false);
                        return;
                    }
                    set_go_on.set(true);
                    *idx -= 1;
                    current_url.set(sections[*idx].clone());
                }
                if at_ends == 1 {
                    if *idx == sections.len() - 1 {
                        set_go_on.set(false);
                        return;
                    }
                    set_go_on.set(true);
                    *idx += 1;
                    current_url.set(sections[*idx].clone());
                }
            });
        }
    });
    //container(stack).style(move |s| s.flex_grow(1.0).background(Color::WHITE)).into_view()
    stack
}

fn create_toc(elems: Vec<&rbook::xml::Element>) -> Vec<TocEntry> {
    elems.iter().map(|elem| TocEntry {title: elem.name().to_string(), link: elem.value().to_string(), children: create_toc(elem.children())}).collect()
}


fn process_images(epub: &Epub) -> HashMap<String, ImageElem> {
    let mut image_map: HashMap<String, ImageElem> = HashMap::new();
    let pool = ThreadPool::new(8);
    let image_types = ["jpeg", "jpg", "png", "gif", "webp"];

    for elem in epub.manifest().elements() {
        
        let image_path      = elem.value();
        let file_extension  = image_path.split(".").skip(1).next().unwrap();
        if !image_types.contains(&file_extension) { continue; }
        let image_type = match file_extension {
            "jpeg"  => ImageFormat::Jpeg,
            "jpg"   => ImageFormat::Jpeg,
            "png"   => ImageFormat::Png,
            "gif"   => ImageFormat::Gif,
            "webp"  => ImageFormat::WebP,
            _       => continue
        };

        let image_bytes = epub.read_bytes_file(image_path).unwrap();
        let image_size  = ImageReader::with_format(Cursor::new(&image_bytes), image_type).into_dimensions().unwrap();
        let width       = image_size.0 as u16;
        let height      = image_size.1 as u16;

        let image_promise: ImagePromise = Arc::new(RwLock::new(None));
        let image = ImageElem { width, height, image_promise: image_promise.clone() };
        image_map.insert(image_path.to_string(), image);
        pool.execute(move || {
            
            let data = Arc::new(ImageReader::with_format(Cursor::new(image_bytes), image_type).decode().unwrap().to_rgba8().into_raw());
            let mut hasher  = Sha256::new();
            let blob        = Blob::new(data.clone());
            hasher.update(&blob);
            let hash        = hasher.finalize().to_vec();
            let image       = Image::new(blob.clone(), Format::Rgba8, width as u32, height as u32);
            *image_promise.write().unwrap() = Some((image.clone(), hash));
        });
    }
    image_map
}
