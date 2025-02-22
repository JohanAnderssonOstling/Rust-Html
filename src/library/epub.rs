use rbook::{Ebook, Epub};

pub fn get_epub (path: &str) -> String{
    let epub        = Epub::new(&path).unwrap();
    let title       = match epub.metadata().title() {
        None            => {path.split("/").last().unwrap().to_string()}
        Some(title)     => {title.value().to_string()}
    };
    title
}