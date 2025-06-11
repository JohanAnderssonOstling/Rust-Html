use std::fmt::format;
use std::{fs, io};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader};
use itertools::{Either, Itertools};
use zip::write::FileOptions;

pub fn get_library(path: &str) -> (Vec<String>, Vec<String>) {
    println!("path: {path}");
    fs::read_dir(path)
        .unwrap()
        .filter_map(|res| res.ok())
        .filter_map(|entry| {
            let path = entry.path();
            let path_str = path.to_str().unwrap().to_string();
            if path.is_dir() && !path_str.contains('.') { Some(Either::Right(path_str)) } 
            else if path.extension().map_or(false, |ext| ext == "epub") { Some(Either::Left(path_str)) } 
            else { None }
        })
        .partition_map(|either| either)
}

pub fn get_thumbnail(lib_path: &str, id: &str) -> io::Result<Vec<u8>,>{
    let cleaned_id      = id.replace("/", "|");
    let thumbnail_dir   = format!("{lib_path}/.bookrium/thumbnails");
    let thumbnail_path  = format!("{thumbnail_dir}/{cleaned_id}.jpg");
    fs::create_dir_all(&thumbnail_dir).unwrap();
    fs::read(thumbnail_path)
}

pub fn write_book_position(lib_path: &str, id: &str, section_index: usize, elem_index: Vec<usize>) {
    let write_path      = format!("{}/.bookrium/positions/{}", lib_path, id);
    let mut position    = section_index.to_string();
    position.push('\n');
    for elem in elem_index {
        position.push_str(&elem.to_string());
        position.push('\n');
    }
    fs::write(write_path, position).unwrap()
}

pub fn write_thumbnail(lib_path: &str, id: &str, thumbnail: Vec<u8>) {
    let cleaned_id      = id.replace("/", "|");
    let thumbnail_path  = format!("{lib_path}/.bookrium/thumbnails/{cleaned_id}.jpg");
    fs::write(thumbnail_path, &thumbnail).unwrap()
}

pub fn read_book_position(lib_path: &str, id: &str) -> (usize, Vec<usize>) {
    let pos_dir     = format!("{lib_path}/.bookrium/positions");
    let pos_path    = format!("{pos_dir}/{id}");
    println!("path: {}", pos_dir);
    fs::create_dir_all(&pos_dir).unwrap();
    match fs::read_to_string(&pos_path) {
        Ok(position)   => {
            let mut lines = position.lines();
            let section_index: usize = lines.next().unwrap().parse().unwrap();
            let mut elem_index: Vec<usize> = Vec::new();
            for line in lines {
                elem_index.push(line.parse().unwrap());
            }
            (section_index, elem_index)
        }
        Err(_)      => {
            File::create(&pos_path).unwrap();
            write_book_position(lib_path, id, 0, Vec::new());
            (0,Vec::new())
        }
    }
}

pub fn update_book_path(library_path: &str, id: &str, book_path: &str) {
    let path_dir    = format!("{library_path}/.bookrium/book_paths");
    let write_path  = format!("{path_dir}/{id}.txt");
    fs::create_dir_all(&path_dir).unwrap();
    fs::write(write_path, book_path).unwrap();
}

pub fn update_last_read(library_path: &str, id: &str) {
    let last_read_path  = format!("{library_path}/.bookrium/last_read.txt");
    let file = OpenOptions::new().read(true).write(true).create(true).open(&last_read_path).unwrap();
    let reader          = BufReader::new(file);
    let mut hashes: Vec<String> = reader.lines()
        .map(|e| e.unwrap()).take(20)
        .filter(|pos| pos != &id).collect();
    hashes.insert(0, id.to_string());
    fs::write(last_read_path, hashes.join("\n")).unwrap();
}