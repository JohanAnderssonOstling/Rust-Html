use std::fs;
use itertools::{Either, Itertools};

pub fn get_library(path: &str) -> (Vec<String>, Vec<String>) {
    fs::read_dir(path)
        .unwrap()
        .filter_map(|res| res.ok())
        .filter_map(|entry| {
            let path = entry.path();
            let path_str = path.to_str().unwrap().to_string();

            if path.is_dir() && !path_str.contains('.') {
                println!("Is dir");
                Some(Either::Right(path_str)) // Directory path
                
            } else if path.extension().map_or(false, |ext| ext == "epub") {
                Some(Either::Left(path_str)) // Book path
            } else {
                None
            }
        })
        .partition_map(|either| either)
}