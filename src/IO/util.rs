use image::ImageFormat;

const IMAGE_TYPES: [&str; 4] = ["jpeg", "png", "gif", "webp"];


pub fn get_image_type (image_path: &str) -> Option<ImageFormat> {
    let file_extension  = image_path.split(".").skip(1).next().unwrap();
    match file_extension {
        "jpeg"  => Some(ImageFormat::Jpeg),
        "jpg"   => Some(ImageFormat::Jpeg),
        "png"   => Some(ImageFormat::Png),
        "gif"   => Some(ImageFormat::Gif),
        _       => None
    }
}