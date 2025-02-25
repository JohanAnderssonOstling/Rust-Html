use std::fs;
use std::fs::File;
use std::io::{BufReader, BufWriter, Cursor, Read, Write};
use image::{DynamicImage, ImageFormat};
use image::imageops::FilterType;
use zip::{ZipArchive, ZipWriter};
use image::io::Reader as ImageReader;
use rbook::{Ebook, Epub};
use regex::Regex;
use zip::result::ZipResult;
use uuid::{uuid, Uuid};
use zip::write::{ FileOptions};
use crate::IO::library::{get_thumbnail, write_thumbnail};
use crate::IO::util::get_image_type;

#[derive(Clone)]
pub struct Book {
    pub title: String,
    pub cover: Option<Vec<u8>>,
    pub path: String,
}




pub fn get_epub (path: &str) -> String{
    let epub        = Epub::new(&path).unwrap();
    let title       = match epub.metadata().title() {
        None            => {path.split("/").last().unwrap().to_string()}
        Some(title)     => {title.value().to_string()}
    };
    title
}

pub fn get_book_cover(lib_path: &str, path: &str) -> Book {
    let epub        = Epub::new(&path).unwrap();
    let title       = match epub.metadata().title() {
        None            => {path.split("/").last().unwrap().to_string()}
        Some(title)     => {title.value().to_string()}
    };
    println!("path: {path}");
    let id = epub.metadata().unique_identifier().unwrap().value();
    let cover: Option<Vec<u8>>       = match get_thumbnail(lib_path, id) {
        Ok(thumbnail) => {Some(thumbnail)}
        Err(_) => {
            let image_path = get_cover_path(&epub);
            //if image_path.is_none() {None}
            let image_type  = get_image_type(&image_path.unwrap());
            let image_bytes = epub.read_bytes_file(image_path.unwrap()).unwrap();
            let image = ImageReader::with_format(Cursor::new(&image_bytes), image_type.unwrap()).decode().unwrap();
            let resized_image = image.resize(300, 500, FilterType::Lanczos3);
            let mut output = Vec::new();
            resized_image.write_to(&mut Cursor::new(&mut output), ImageFormat::Jpeg).unwrap();
            write_thumbnail(lib_path, id, output);
            Some(image_bytes)
        }
    };

    let path = path.to_string();
    Book {title, cover, path}
}

fn get_cover_path(epub:&Epub) -> Option<&str> {
    match epub.cover_image() {
        Some(image) => { Some(image.value()) }
        None => {
            let mut cover_img = None;
            for img_element in epub.manifest().images() {
                if img_element.name().to_lowercase().contains("cover") || img_element.value().to_lowercase().contains("cover") {
                    cover_img = Some(img_element.value());
                }
            }
            cover_img
        }
    }
}



pub fn remove_dtd(xml: &String) -> String {
    let regex = Regex::new(r#"<!DOCTYPE[^>]*>"#).unwrap();
    let cleaned = regex.replace(&xml, "").to_string();
    println!("{cleaned}");
    cleaned
}

