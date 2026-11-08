use base64::{engine::general_purpose::STANDARD, Engine};
use image::{metadata::Orientation, DynamicImage, ImageDecoder, ImageEncoder, ImageFormat};
use serde::Serialize;
use std::{fs, io::BufReader, path::Path};

#[derive(Serialize)]
struct Entry {
    name: String,
    path: String,
}

#[derive(Serialize)]
struct Directory {
    path: String,
    parent: Option<String>,
    breadcrumbs: Vec<Entry>,
    folders: Vec<Entry>,
    images: Vec<Entry>,
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn breadcrumbs(path: &Path) -> Vec<Entry> {
    let mut ancestors: Vec<_> = path.ancestors().collect();
    ancestors.reverse();
    ancestors
        .into_iter()
        .map(|ancestor| Entry {
            name: ancestor
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| display_path(ancestor)),
            path: display_path(ancestor),
        })
        .collect()
}

#[tauri::command]
fn list_drives() -> Result<Vec<String>, String> {
    #[cfg(windows)]
    {
        let mask = unsafe { windows_sys::Win32::Storage::FileSystem::GetLogicalDrives() };
        if mask == 0 {
            return Err(format!(
                "Cannot list drives: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok((0..26)
            .filter(|index| mask & (1 << index) != 0)
            .map(|index| format!("{}:\\", (b'A' + index as u8) as char))
            .collect())
    }
    #[cfg(not(windows))]
    {
        Ok(vec!["/".to_string()])
    }
}

#[tauri::command]
fn read_directory(path: String) -> Result<Directory, String> {
    let path = Path::new(&path);
    if !path.is_absolute() {
        return Err("Please enter an absolute folder path.".into());
    }
    let entries =
        fs::read_dir(path).map_err(|error| format!("Cannot open {}: {error}", path.display()))?;
    let mut folders = Vec::new();
    let mut images = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("Cannot read folder entry: {error}"))?;
        let entry_path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let item = Entry {
            name,
            path: display_path(&entry_path),
        };
        if entry_path.is_dir() {
            folders.push(item);
        } else if entry_path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| {
                matches!(
                    ext.to_ascii_lowercase().as_str(),
                    "jpg" | "jpeg" | "png" | "bmp" | "gif" | "webp" | "tif" | "tiff"
                )
            })
        {
            images.push(item);
        }
    }
    folders.sort_by(|a, b| natord::compare_ignore_case(&a.name, &b.name));
    images.sort_by(|a, b| natord::compare_ignore_case(&a.name, &b.name));
    Ok(Directory {
        path: display_path(path),
        parent: path.parent().map(display_path),
        breadcrumbs: breadcrumbs(path),
        folders,
        images,
    })
}

type DecodedImage = (DynamicImage, Option<Vec<u8>>);

fn decode_scaled_jpeg(path: &Path, size: u32) -> Result<Option<DecodedImage>, String> {
    let file =
        fs::File::open(path).map_err(|error| format!("Cannot open {}: {error}", path.display()))?;
    let mut decoder = jpeg_decoder::Decoder::new(BufReader::new(file));
    decoder
        .read_info()
        .map_err(|error| format!("Cannot read {}: {error}", path.display()))?;
    let info = decoder
        .info()
        .ok_or_else(|| format!("Cannot read image information from {}", path.display()))?;
    if !matches!(
        info.pixel_format,
        jpeg_decoder::PixelFormat::L8 | jpeg_decoder::PixelFormat::RGB24
    ) {
        return Ok(None);
    }
    let (width, height) = decoder
        .scale(size as u16, size as u16)
        .map_err(|error| format!("Cannot scale {}: {error}", path.display()))?;
    let pixels = decoder
        .decode()
        .map_err(|error| format!("Cannot decode {}: {error}", path.display()))?;
    let width = u32::from(width);
    let height = u32::from(height);
    let mut image = match info.pixel_format {
        jpeg_decoder::PixelFormat::L8 => {
            image::GrayImage::from_raw(width, height, pixels).map(DynamicImage::ImageLuma8)
        }
        jpeg_decoder::PixelFormat::RGB24 => {
            image::RgbImage::from_raw(width, height, pixels).map(DynamicImage::ImageRgb8)
        }
        _ => unreachable!(),
    }
    .ok_or_else(|| format!("Invalid decoded image dimensions in {}", path.display()))?;
    let orientation = decoder
        .exif_data()
        .and_then(Orientation::from_exif_chunk)
        .unwrap_or(Orientation::NoTransforms);
    image.apply_orientation(orientation);
    Ok(Some((image, decoder.icc_profile())))
}

fn decode_image(path: &Path, thumbnail_size: Option<u32>) -> Result<DecodedImage, String> {
    let reader = image::ImageReader::open(path)
        .and_then(|reader| reader.with_guessed_format())
        .map_err(|error| format!("Cannot open {}: {error}", path.display()))?;
    if reader.format() == Some(ImageFormat::Jpeg) {
        if let Some(size) = thumbnail_size {
            if let Some(image) = decode_scaled_jpeg(path, size)? {
                return Ok(image);
            }
        }
    }
    let mut decoder = reader
        .into_decoder()
        .map_err(|error| format!("Cannot decode {}: {error}", path.display()))?;
    let profile = decoder.icc_profile().map_err(|error| {
        format!(
            "Cannot read colour profile from {}: {error}",
            path.display()
        )
    })?;
    let orientation = decoder
        .orientation()
        .map_err(|error| format!("Cannot read orientation from {}: {error}", path.display()))?;
    let mut image = DynamicImage::from_decoder(decoder)
        .map_err(|error| format!("Cannot decode {}: {error}", path.display()))?;
    image.apply_orientation(orientation);
    Ok((image, profile))
}

fn encode_png(
    image: DynamicImage,
    profile: Option<Vec<u8>>,
    path: &Path,
) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let mut encoder = image::codecs::png::PngEncoder::new(&mut output);
    if let Some(profile) = profile {
        encoder.set_icc_profile(profile).map_err(|error| {
            format!(
                "Cannot preserve colour profile for {}: {error}",
                path.display()
            )
        })?;
    }
    image
        .write_with_encoder(encoder)
        .map_err(|error| format!("Cannot convert {}: {error}", path.display()))?;
    Ok(output)
}

fn make_thumbnail(path: String, size: u32) -> Result<Vec<u8>, String> {
    let path = Path::new(&path);
    if !path.is_absolute() || !(256..=2048).contains(&size) {
        return Err("Invalid thumbnail path or size.".into());
    }
    let (mut image, profile) = decode_image(path, Some(size))?;
    if image.width() > size || image.height() > size {
        image = image.thumbnail(size, size);
    }
    encode_png(image, profile, path)
}

fn thumbnail_request(request: tauri::http::Request<Vec<u8>>) -> Result<Vec<u8>, String> {
    if request.method() != tauri::http::Method::GET {
        return Err("Invalid thumbnail request method.".into());
    }
    let query = request
        .uri()
        .query()
        .ok_or("Missing thumbnail parameters.")?;
    let mut path = None;
    let mut size = None;
    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        match key.as_ref() {
            "path" if path.is_none() => path = Some(value.into_owned()),
            "size" if size.is_none() => {
                size = Some(
                    value
                        .parse::<u32>()
                        .map_err(|_| "Invalid thumbnail size.")?,
                )
            }
            _ => return Err("Invalid thumbnail parameters.".into()),
        }
    }
    make_thumbnail(
        path.ok_or("Missing thumbnail path.")?,
        size.ok_or("Missing thumbnail size.")?,
    )
}

fn thumbnail_response(result: Result<Vec<u8>, String>) -> tauri::http::Response<Vec<u8>> {
    use tauri::http::{header, HeaderValue, Response, StatusCode};
    let (status, content_type, body) = match result {
        Ok(bytes) => (StatusCode::OK, "image/png", bytes),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            "text/plain; charset=utf-8",
            error.into_bytes(),
        ),
    };
    let mut response = Response::new(body);
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    response
}

fn convert_tiff(path: String, thumbnail: bool) -> Result<String, String> {
    let path = Path::new(&path);
    if !path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("tif") || ext.eq_ignore_ascii_case("tiff"))
    {
        return Err("Only TIFF images can be converted.".into());
    }
    let (mut image, profile) = decode_image(path, None)?;
    if thumbnail {
        image = image.thumbnail(1200, 1200);
    }
    let output = encode_png(image, profile, path)?;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(output)))
}

#[tauri::command]
async fn tiff_as_png(path: String, thumbnail: bool) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || convert_tiff(path, thumbnail))
        .await
        .map_err(|error| format!("TIFF conversion task failed: {error}"))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .register_asynchronous_uri_scheme_protocol("thumbnail", |_context, request, responder| {
            let task = tauri::async_runtime::spawn_blocking(move || thumbnail_request(request));
            tauri::async_runtime::spawn(async move {
                let result = task
                    .await
                    .map_err(|error| format!("Thumbnail task failed: {error}"))
                    .and_then(|result| result);
                responder.respond(thumbnail_response(result));
            });
        })
        .invoke_handler(tauri::generate_handler![
            list_drives,
            read_directory,
            tiff_as_png
        ])
        .run(tauri::generate_context!())
        .expect("error while running PolyPrint");
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, ImageBuffer, ImageFormat, Rgb, Rgba};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_directory() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("polyprint-test-{unique}"));
        fs::create_dir(&root).unwrap();
        root
    }

    #[test]
    fn reads_images_in_natural_order_and_folders_separately() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("polyprint-test-{unique}"));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("subfolder")).unwrap();
        for name in ["page10.JPG", "page2.png", "page1.tiff", "notes.txt"] {
            fs::write(root.join(name), []).unwrap();
        }
        let result = read_directory(display_path(&root)).unwrap();
        assert_eq!(
            result
                .images
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            ["page1.tiff", "page2.png", "page10.JPG"]
        );
        assert_eq!(result.folders[0].name, "subfolder");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_relative_paths_and_non_tiff_conversion() {
        assert!(read_directory("relative".into()).is_err());
        assert!(convert_tiff("page.png".into(), false).is_err());
        assert!(make_thumbnail("relative.jpg".into(), 512).is_err());
        assert!(make_thumbnail(display_path(&std::env::temp_dir()), 0).is_err());
    }

    #[test]
    fn creates_bounded_thumbnails_for_all_supported_formats() {
        let root = test_directory();
        let source = ImageBuffer::from_pixel(640, 480, Rgb([120u8, 40, 200]));
        for (extension, format) in [
            ("jpg", ImageFormat::Jpeg),
            ("png", ImageFormat::Png),
            ("bmp", ImageFormat::Bmp),
            ("gif", ImageFormat::Gif),
            ("webp", ImageFormat::WebP),
            ("tiff", ImageFormat::Tiff),
        ] {
            let path = root.join(format!("page.{extension}"));
            source.save_with_format(&path, format).unwrap();
            let bytes = make_thumbnail(display_path(&path), 256).unwrap();
            let result = image::load_from_memory_with_format(&bytes, ImageFormat::Png).unwrap();
            assert_eq!(result.dimensions(), (256, 192), "{extension}");
        }
        let alpha_path = root.join("alpha.png");
        ImageBuffer::from_pixel(20, 10, Rgba([255u8, 0, 0, 64]))
            .save(&alpha_path)
            .unwrap();
        let bytes = make_thumbnail(display_path(&alpha_path), 256).unwrap();
        let result = image::load_from_memory(&bytes).unwrap();
        assert_eq!(result.dimensions(), (20, 10));
        assert_eq!(result.get_pixel(0, 0).0[3], 64);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn jpeg_uses_reduced_decode_and_preserves_orientation() {
        let root = test_directory();
        let path = root.join("rotated.jpg");
        ImageBuffer::from_pixel(2048, 1536, Rgb([140u8, 100, 60]))
            .save(&path)
            .unwrap();
        let jpeg = fs::read(&path).unwrap();
        let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut rotated = vec![0xff, 0xd8, 0xff, 0xe1];
        rotated.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        rotated.extend_from_slice(exif);
        rotated.extend_from_slice(&jpeg[2..]);
        fs::write(&path, rotated).unwrap();

        let (decoded, _) = decode_scaled_jpeg(&path, 256).unwrap().unwrap();
        assert_eq!(decoded.dimensions(), (192, 256));
        assert!(decoded.width() * decoded.height() <= (2048 * 1536) / 64);
        let bytes = make_thumbnail(display_path(&path), 256).unwrap();
        let thumbnail = image::load_from_memory(&bytes).unwrap();
        assert_eq!(thumbnail.dimensions(), (192, 256));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn thumbnail_protocol_decodes_paths_and_disables_caching() {
        let root = test_directory();
        let path = root.join("manual & 100%.png");
        ImageBuffer::from_pixel(640, 480, Rgb([120u8, 40, 200]))
            .save(&path)
            .unwrap();
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("path", &display_path(&path))
            .append_pair("size", "256")
            .finish();
        let request = tauri::http::Request::builder()
            .uri(format!("thumbnail://localhost/?{query}"))
            .body(Vec::new())
            .unwrap();
        let response = thumbnail_response(thumbnail_request(request));
        assert_eq!(response.status(), tauri::http::StatusCode::OK);
        assert_eq!(
            response.headers()[tauri::http::header::CACHE_CONTROL],
            "no-store"
        );
        assert_eq!(
            image::load_from_memory(response.body())
                .unwrap()
                .dimensions(),
            (256, 192)
        );

        let invalid = tauri::http::Request::builder()
            .uri("thumbnail://localhost/?path=relative&size=0")
            .body(Vec::new())
            .unwrap();
        assert_eq!(
            thumbnail_response(thumbnail_request(invalid)).status(),
            tauri::http::StatusCode::BAD_REQUEST
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn breadcrumbs_preserve_drive_and_network_share_roots() {
        let drive = breadcrumbs(Path::new(r"C:\Manuals\Lathe"));
        assert_eq!(
            drive
                .iter()
                .map(|entry| entry.path.as_str())
                .collect::<Vec<_>>(),
            [r"C:\", r"C:\Manuals", r"C:\Manuals\Lathe"]
        );
        assert_eq!(drive[0].name, r"C:\");
        assert_eq!(drive[2].name, "Lathe");

        let network = breadcrumbs(Path::new(r"\\server\share\Manuals\Lathe"));
        assert_eq!(
            network
                .iter()
                .map(|entry| entry.path.as_str())
                .collect::<Vec<_>>(),
            [
                r"\\server\share\",
                r"\\server\share\Manuals",
                r"\\server\share\Manuals\Lathe"
            ]
        );
        assert_eq!(network[0].name, r"\\server\share\");
    }

    #[cfg(not(windows))]
    #[test]
    fn breadcrumbs_preserve_unix_root() {
        let crumbs = breadcrumbs(Path::new("/manuals/lathe"));
        assert_eq!(
            crumbs
                .iter()
                .map(|entry| entry.path.as_str())
                .collect::<Vec<_>>(),
            ["/", "/manuals", "/manuals/lathe"]
        );
        assert_eq!(crumbs[0].name, "/");
    }
}
