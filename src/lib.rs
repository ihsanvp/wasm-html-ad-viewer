extern crate js_sys;
extern crate mime_guess;
extern crate pathdiff;
extern crate wasm_bindgen;
extern crate zip;

mod utils;

use std::{io::Cursor, path::Path};
use wasm_bindgen::prelude::*;
use zip::ZipArchive;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

// macro_rules! console_log {
//     ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
// }

#[wasm_bindgen]
pub fn initialize() {
    utils::set_panic_hook();
}

#[wasm_bindgen(js_name = parseFile)]
pub fn parse_file(buf: Vec<u8>) -> String {
    let url: String;

    let reader = Cursor::new(buf);
    let mut zip = ZipArchive::new(reader).unwrap();

    let files = utils::get_files_from_zip(&mut zip);
    let html = files
        .iter()
        .find(|&x| x.1.ends_with(".html"))
        .unwrap()
        .to_owned();
    let root_dir = Path::new(html.1.as_str()).parent().unwrap();
    let mut z = zip.clone();
    let mut html_file = z.by_index(html.0).unwrap();
    let html_content = utils::get_text_from_zipfile(&mut html_file);

    let files = utils::resolve_zip_files(files, root_dir);

    if html_content.contains("gwd-image") {
        url = utils::parse_gwd(&html_content, &mut zip, &files);
    } else {
        url = utils::parse_adobe_an(&html_content, &mut zip, &files);
    }

    url
}
