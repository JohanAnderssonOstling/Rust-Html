use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader};
use itertools::{Either, Itertools};

pub fn get_library(path: &str) -> (Vec<String>, Vec<String>) {
    fs::read_dir(path)
        .unwrap()
        .filter_map(|res| res.ok())
        .filter_map(|entry| {
            let path = entry.path();
            let path_str = path.to_str().unwrap().to_string();

            if path.is_dir() && !path_str.contains('.') {
                Some(Either::Right(path_str)) // Directory path
                
            } else if path.extension().map_or(false, |ext| ext == "epub") {
                Some(Either::Left(path_str)) // Book path
            } else {
                None
            }
        })
        .partition_map(|either| either)
}

pub fn update_book_path(library_path: &str, id: &str, book_path: &str) {
    let path_dir    = format!("{library_path}/.bookrium/book_paths");
    let write_path  = format!("{path_dir}/{id}.txt");
    fs::create_dir_all(&path_dir).unwrap();
    fs::write(write_path, book_path).unwrap();
}

pub fn update_last_read(library_path: &str, id: &str) {
    let last_read_path  = format!("{library_path}/.bookrium/last_read.txt");
    println!("{last_read_path}");
    let reader          = BufReader::new(File::open(&last_read_path).unwrap());
    let mut hashes: Vec<String> = reader.lines()
        .map(|e| e.unwrap()).take(20)
        .filter(|pos| pos != &id).collect();
    hashes.insert(0, id.to_string());
    fs::write(last_read_path, hashes.join("\n")).unwrap();
}