use std::fs::File;
use std::io::{BufReader, Cursor};
use image::DynamicImage;
use zip::ZipArchive;
use image::io::Reader as ImageReader;
use rbook::{Ebook, Epub};
use regex::Regex;
use crate::IO::util::get_image_type;

#[derive(Clone)]
pub struct Book {
    pub title: String,
    pub cover: Option<Vec<u8>>
}

pub fn get_book_cover(path: &str) -> Book {
    let epub        = Epub::new(&path).unwrap();
    let title       = match epub.metadata().title() {
        None            => {path.split("/").last().unwrap().to_string()}
        Some(title)     => {title.value().to_string()}
    };
    let cover       = get_cover(&epub);
    Book {title, cover}
}


pub fn get_epub (path: &str) -> String{
    let epub        = Epub::new(&path).unwrap();
    let title       = match epub.metadata().title() {
        None            => {path.split("/").last().unwrap().to_string()}
        Some(title)     => {title.value().to_string()}
    };
    title
}

fn get_cover(epub:&Epub) -> Option<Vec<u8>> {
    let image_path = match epub.cover_image() {
        Some(image) => {Some(image.value())}
        None        => {
            let mut cover_img = None;
            for img_element in epub.manifest().images() {
                if img_element.name().to_lowercase().contains("cover") || img_element.value().to_lowercase().contains("cover") {
                    cover_img =  Some(img_element.value());
                }
            }
            cover_img
        }
    };
    if image_path.is_none() {return None}
    let image_type  = get_image_type(image_path.unwrap());
    if image_type.is_none() {return None}
    let image_bytes = epub.read_bytes_file(image_path.unwrap()).unwrap();
    //Some(ImageReader::with_format(Cursor::new(image_bytes), image_type.unwrap()).decode().unwrap())
    Some(image_bytes)
}

pub fn get_epub_uuid(path: &str) {
    let zip_file = File::open(path).unwrap();
    let mut zip = ZipArchive::new(BufReader::new(zip_file)).unwrap();

}

pub fn get_epub_title(path: &str) {
    let zip_file = File::open(path).unwrap();
    let mut zip = ZipArchive::new(BufReader::new(zip_file)).unwrap();

}

pub fn remove_dtd(xml: &String) -> String {
    let regex = Regex::new(r#"<!DOCTYPE[^>]*>"#).unwrap();
    let cleaned = regex.replace(&xml, "").to_string();
    println!("{cleaned}");
    cleaned
}