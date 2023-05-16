use pathdiff::diff_paths;
use regex::{Captures, Regex};
use std::io::Cursor;
use std::path::Path;
use web_sys::{Blob, Url};
use zip::read::ZipFile;
use zip::ZipArchive;

pub fn set_panic_hook() {
    // When the `console_error_panic_hook` feature is enabled, we can call the
    // `set_panic_hook` function at least once during initialization, and then
    // we will get better error messages if our code ever panics.
    //
    // For more details see
    // https://github.com/rustwasm/console_error_panic_hook#readme
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

type Zip = ZipArchive<Cursor<Vec<u8>>>;

pub fn get_files_from_zip(zip: &mut Zip) -> Vec<(usize, String)> {
    let mut filename_list: Vec<(usize, String)> = Vec::new();

    for idx in 0..zip.len() {
        let file = zip.by_index(idx).unwrap();
        let name = file.name().to_owned();
        if !name.ends_with("/") {
            filename_list.push((idx, name));
        }
    }

    filename_list
        .into_iter()
        .filter(|x| {
            !String::from(
                Path::new(x.1.as_str())
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap(),
            )
            .contains("._")
        })
        .collect()
}

pub fn get_text_from_zipfile(file: &mut ZipFile) -> String {
    std::io::read_to_string(file).unwrap()
}

pub fn get_binary_from_zipfile(file: &mut ZipFile) -> Vec<u8> {
    let mut buf = Vec::with_capacity(file.size() as usize);
    std::io::copy(file, &mut buf).unwrap();
    buf
}

pub fn create_object_url(bytes: Vec<u8>, blob_type: &str) -> String {
    let uint8arr = js_sys::Uint8Array::new(&unsafe { js_sys::Uint8Array::view(&bytes) }.into());
    let array = js_sys::Array::new();
    array.push(&uint8arr.buffer());
    let blob = Blob::new_with_u8_array_sequence_and_options(
        &array,
        web_sys::BlobPropertyBag::new().type_(blob_type),
    )
    .unwrap();
    let url = Url::create_object_url_with_blob(&blob).unwrap();

    url
}

pub fn resolve_zip_files(files: Vec<(usize, String)>, root_dir: &Path) -> Vec<(usize, String)> {
    files
        .iter()
        .map(|x| {
            (
                x.0,
                String::from(
                    diff_paths(Path::new(x.1.as_str()), root_dir)
                        .unwrap()
                        .to_str()
                        .unwrap(),
                ),
            )
        })
        .collect()
}

pub fn parse_adobe_an(html: &String, zip: &mut Zip, files: &Vec<(usize, String)>) -> String {
    let script_search = Regex::new(r#"src="(?P<path>.*?)""#).unwrap();
    let manifest_search = Regex::new(r#"manifest:((.|\n)*)\[((.|\n)*)],((.|\n)*?)};"#).unwrap();
    let asset_search =
        Regex::new(r#"\{(.*)src(.*?)"(?P<src>.*?)("|\?)(.*)id(.*?)"(?P<id>.*?)"(.*)}"#).unwrap();

    let final_html = script_search
        .replace_all(html.as_str(), |caps: &Captures| {
            let current_path = caps["path"].to_string();
            match files.iter().find(|x| x.1.contains(current_path.as_str())) {
                Some(matched_file) => {
                    let mut z = zip.clone();
                    let mut matched_zip = z.by_index(matched_file.0).unwrap();
                    let mut content = get_text_from_zipfile(&mut matched_zip);
                    content = manifest_search
                        .replace_all(content.as_str(), |cap_manifest: &Captures| {
                            let initial_manifest = cap_manifest[0].to_string();

                            asset_search
                                .replace_all(&initial_manifest.as_str(), |asset_cap: &Captures| {
                                    let result: String;
                                    let asset = files.iter().find(|x| {
                                        x.1.contains(asset_cap["src"].to_string().as_str())
                                    });

                                    if asset.is_some() {
                                        let asset = asset.unwrap();
                                        let mime = mime_guess::from_path(&asset.1)
                                            .first_or_text_plain()
                                            .to_string();
                                        let mime_parts = mime.split("/").collect::<Vec<&str>>();
                                        let t = mime_parts[0];
                                        let ext = mime_parts[1];
                                        let mut asset_zip = zip.by_index(asset.0).unwrap();
                                        let bytes = get_binary_from_zipfile(&mut asset_zip);
                                        let url = create_object_url(bytes, &mime);

                                        result = format!(
                                        "{{src: \"{}\", id: \"{}\", type: \"{}\", ext: \"{}\" }}",
                                        url,
                                        asset_cap["id"].to_string().as_str(),
                                        t,
                                        ext
                                    )
                                    } else {
                                        result = asset_cap[0].to_string()
                                    }

                                    result
                                })
                                .to_string()
                        })
                        .to_string();

                    let content_mime = mime_guess::from_path(matched_file.1.as_str())
                        .first_or_text_plain()
                        .to_string();
                    let url = create_object_url(content.as_bytes().to_vec(), content_mime.as_str());
                    format!("src=\"{}\"", url)
                }
                _ => caps[0].to_string(),
            }
        })
        .to_string();

    let url = create_object_url(final_html.as_bytes().to_vec(), "text/html");
    url
}

pub fn parse_gwd(html: &String, zip: &mut Zip, files: &Vec<(usize, String)>) -> String {
    let image_search = Regex::new(r#"<gwd-image((.|\n)*?)source="(?P<src>.*?)""#).unwrap();
    let final_html = image_search
        .replace_all(html, |caps: &Captures| {
            let mut initial_match = caps[0].to_string();
            let src = caps["src"].to_string();
            let image = files.iter().find(|x| x.1 == src);

            if image.is_some() {
                let image = image.unwrap();
                let mut z = zip.clone();
                let mut image_file = z.by_index(image.0).unwrap();
                let bytes = get_binary_from_zipfile(&mut image_file);
                let mime = mime_guess::from_path(image.1.as_str())
                    .first_or_octet_stream()
                    .to_string();
                let url = create_object_url(bytes, mime.as_str());
                initial_match = initial_match.replace(src.as_str(), &url);
            }

            initial_match
        })
        .to_string();

    let url = create_object_url(final_html.as_bytes().to_vec(), "text/html");
    url
}
