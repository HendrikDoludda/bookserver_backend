use epub_parser::Epub;
use image::ImageFormat;
use natord::compare;
use pdfium_render::prelude::*;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

use crate::{error_types::CoverImageError, models::BookFormat};

const COVER_DIR: &str = "./cover_images/";
const DEFAULT_IMAGE: &str = "example-cover.png";

pub fn get_cover_image(
    path: &PathBuf,
    format: BookFormat,
    _hash: &str,
) -> Result<PathBuf, CoverImageError> {
    match format {
        BookFormat::Pdf => get_pdf_cover_image(path, None, _hash),
        BookFormat::Cbz => get_cbz_cover_image(path, _hash),
        BookFormat::Epub => get_epub_cover_image(path, _hash),
        BookFormat::ImageComic => get_image_comic_cover_image(path, _hash),
        BookFormat::None => Ok(get_fallback_cover()),
    }
}

fn get_fallback_cover() -> PathBuf {
    Path::new(COVER_DIR).join(DEFAULT_IMAGE)
}

fn get_cbz_cover_image(path: &PathBuf, _hash: &str) -> Result<PathBuf, CoverImageError> {
    let cbz = File::open(path).map_err(|_| CoverImageError::FileCouldNotBeRead)?;
    let mut archive = ZipArchive::new(cbz).map_err(|_| CoverImageError::ZipArchiveFailure)?;

    let first_name = archive
        .file_names()
        .filter_map(|name| {
            let lower = name.to_ascii_lowercase();
            if lower.ends_with(".jpg")
                || lower.ends_with(".jpeg")
                || lower.ends_with(".png")
                || lower.ends_with(".webp")
            {
                Some((lower, name))
            } else {
                None
            }
        })
        .min_by(|a,b| a.0.cmp(&b.0))
        .map(|(_, original)| original.to_string())
        .ok_or(CoverImageError::SortingFailed)?;

    let mut file = archive
        .by_name(&first_name)
        .map_err(|_| CoverImageError::InvalidFileName)?;

    let mut image_data = Vec::new();
    file.read_to_end(&mut image_data)
        .map_err(|_| CoverImageError::PageToImageConversionFailed)?;

    let format = image::guess_format(&image_data).map_err(|_| CoverImageError::ImageFormatFailed)?;
    let extension = format
    .extensions_str()
    .first()
    .copied()
    .unwrap_or("png");

    let final_path = get_cover_output_path(_hash, extension)?;

    let result = create_image_at_location(&final_path, &image_data)?;

    Ok(result)
}

//TODO: Pdfium needs to be bundled to this project in order for it to work on all operating systems
//should also be included so that on a mac you can run this without symlinking
fn get_pdf_cover_image(
    path: &PathBuf,
    password: Option<&str>,
    _hash: &str,
) -> Result<PathBuf, CoverImageError> {
    let pdfium = Pdfium::default();

    let document = pdfium
        .load_pdf_from_file(path, password)
        .map_err(|_| CoverImageError::PdfToDocumentError)?;

    let render_config = PdfRenderConfig::new()
        .set_target_width(1000)
        .set_maximum_height(2000)
        .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);

    let page = document
        .pages()
        .get(0)
        .map_err(|_| CoverImageError::PageCouldNotBeRetrieved)?;

    let bitmap = page
        .render_with_config(&render_config)
        .map_err(|_| CoverImageError::PageToImageConversionFailed)?;

    let image = bitmap.as_image().into_rgb8();

    let extension = "jpg";
    let final_path = get_cover_output_path(_hash, &extension)?;
    if final_path.exists() {
        return Ok(final_path);
    }

    image::DynamicImage::ImageRgb8(image)
        .save_with_format(&final_path, ImageFormat::Jpeg)
        .map_err(|_| CoverImageError::CoverCreationFailed)?;
    Ok(final_path)
}

fn get_epub_cover_image(path: &PathBuf, _hash: &str) -> Result<PathBuf, CoverImageError> {
    let epub = Epub::parse(path).map_err(|_| CoverImageError::FileNotFound)?;
    let cover = epub
        .images
        .first()
        .ok_or(CoverImageError::CoverCreationFailed)?;
    let image_bytes = &cover.content;
    let image_type = &cover.media_type;

    // Extract extension from media type
    let extension = image_type
        .split('/')
        .nth(1)
        .ok_or(CoverImageError::CoverCreationFailed)?;

    let final_path = get_cover_output_path(_hash, &extension.to_string())?;
    if final_path.exists() {
        return Ok(final_path);
    }

    let result = create_image_at_location(&final_path, image_bytes)?;
    Ok(result)
}

fn get_image_comic_cover_image(path: &PathBuf, _hash: &str) -> Result<PathBuf, CoverImageError> {
    let first = fs::read_dir(path)
        .map_err(|_| CoverImageError::GeneralError)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| is_image_file(p))
        .min_by(|a, b| {
            compare(
                a.file_name().and_then(|n| n.to_str()).unwrap_or_default(),
                b.file_name().and_then(|n| n.to_str()).unwrap_or_default(),
            )
        })
        .ok_or(CoverImageError::FileNotFound)?;
    
    let extension = first
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("unknown")
        .to_string();

    let final_path = get_cover_output_path(_hash, &extension)?;
    if final_path.exists() {
        return Ok(final_path);
    }

    let result = copy_image_to_cover_location(first.as_path(), &final_path.as_path())?;

    Ok(result)
}

fn is_image_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase()),
        Some(ref ext) if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp")
    )
}

fn create_image_at_location(path: &Path, data: &[u8]) -> Result<PathBuf, CoverImageError> {
    fs::write(&path, data).map_err(|_| CoverImageError::CoverCreationFailed)?;
    Ok(path.to_path_buf())
}

fn copy_image_to_cover_location(
    original_image: &Path,
    final_file: &Path,
) -> Result<PathBuf, CoverImageError> {
    std::fs::copy(original_image, final_file).map_err(|_| CoverImageError::CoverCreationFailed)?;
    Ok(final_file.to_path_buf())
}

fn get_cover_output_path(_hash: &str, extension: &str) -> Result<PathBuf, CoverImageError> {
    fs::create_dir_all(COVER_DIR).map_err(|_| CoverImageError::GeneralError)?;

    let final_file = Path::new(COVER_DIR).join(_hash).with_extension(extension);
    Ok(final_file)
}
