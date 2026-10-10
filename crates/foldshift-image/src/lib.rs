use std::fs;
use std::io::{BufWriter, Seek, Write};
use std::path::{Path, PathBuf};

use foldshift_core::{FcError, Inspection, MediaKind, Result};
use image::codecs::jpeg::JpegEncoder;
use image::metadata::Orientation;
use image::{DynamicImage, GenericImageView, ImageDecoder, ImageFormat, ImageReader};
use serde_json::json;

pub const SUPPORTED_EXTENSIONS: &[&str] =
    &["png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff"];

#[derive(Debug, Clone, Copy)]
pub struct EncodeOptions {
    pub quality: u8,
    pub force: bool,
}

impl Default for EncodeOptions {
    fn default() -> Self {
        Self {
            quality: 82,
            force: false,
        }
    }
}

pub fn inspect(path: &Path) -> Result<Inspection> {
    ensure_input(path)?;
    let reader = ImageReader::open(path)
        .map_err(FcError::Io)?
        .with_guessed_format()
        .map_err(FcError::Io)?;
    let format = reader
        .format()
        .ok_or_else(|| FcError::UnsupportedFormat(path.display().to_string()))?;
    let mut decoder = reader
        .into_decoder()
        .map_err(|error| FcError::Conversion(error.to_string()))?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let (mut width, mut height) = decoder.dimensions();
    if matches!(
        orientation,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    ) {
        std::mem::swap(&mut width, &mut height);
    }
    let color_type = decoder.color_type();
    let metadata = fs::metadata(path)?;

    Ok(
        Inspection::new(path, format_name(format), MediaKind::Image, metadata.len())
            .with_property("width", json!(width))
            .with_property("height", json!(height))
            .with_property("color", json!(format!("{:?}", color_type)))
            .with_property("orientation", json!(format!("{:?}", orientation))),
    )
}

pub fn convert(input: &Path, output: &Path, options: EncodeOptions) -> Result<()> {
    validate_encode_options(options)?;
    let _target_format = preflight_target(output, options.force)?;
    ensure_input(input)?;
    guard_still_image(input)?;
    let image = decode(input)?;
    encode_atomic(&image, output, options)
}

pub fn resize(
    input: &Path,
    output: &Path,
    width: Option<u32>,
    height: Option<u32>,
    options: EncodeOptions,
) -> Result<(u32, u32)> {
    validate_encode_options(options)?;
    if width.is_none() && height.is_none() {
        return Err(FcError::InvalidOption(
            "resize requires --width, --height, or both".into(),
        ));
    }
    if width == Some(0) || height == Some(0) {
        return Err(FcError::InvalidOption(
            "resize dimensions must be greater than zero".into(),
        ));
    }
    let _target_format = preflight_target(output, options.force)?;
    ensure_input(input)?;
    guard_still_image(input)?;

    let image = decode(input)?;
    let (source_width, source_height) = image.dimensions();
    let max_width =
        width.unwrap_or_else(|| scale_dimension(source_width, source_height, height.unwrap()));
    let max_height =
        height.unwrap_or_else(|| scale_dimension(source_height, source_width, width.unwrap()));
    let resized = image.thumbnail(max_width, max_height);
    let dimensions = resized.dimensions();
    encode_atomic(&resized, output, options)?;
    Ok(dimensions)
}

pub fn batch_convert(
    inputs: &[PathBuf],
    target_ext: &str,
    output_dir: &Path,
    pattern: Option<&str>,
    options: EncodeOptions,
    cancel_token: &foldshift_core::batch::CancellationToken,
    progress: impl FnMut(foldshift_core::batch::BatchProgress<'_>),
) -> Result<foldshift_core::batch::BatchSummary> {
    if !output_dir.exists() {
        fs::create_dir_all(output_dir)?;
    }
    let items =
        foldshift_core::batch::prepare_batch_items(inputs, output_dir, pattern, target_ext)?;

    let summary =
        foldshift_core::batch::execute_sequential_batch(&items, cancel_token, progress, |item| {
            cancel_token.check_cancelled()?;
            convert(&item.input, &item.output, options)?;
            let written = fs::metadata(&item.output).map(|m| m.len()).unwrap_or(0);
            Ok(written)
        });

    Ok(summary)
}

#[derive(Debug, Clone, Copy)]
pub struct BatchResizeOptions {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub quality: u8,
    pub force: bool,
}

impl Default for BatchResizeOptions {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            quality: 82,
            force: false,
        }
    }
}

pub fn batch_resize(
    inputs: &[PathBuf],
    target_ext: Option<&str>,
    output_dir: &Path,
    pattern: Option<&str>,
    options: BatchResizeOptions,
    cancel_token: &foldshift_core::batch::CancellationToken,
    progress: impl FnMut(foldshift_core::batch::BatchProgress<'_>),
) -> Result<foldshift_core::batch::BatchSummary> {
    if !output_dir.exists() {
        fs::create_dir_all(output_dir)?;
    }
    if options.width.is_none() && options.height.is_none() {
        return Err(FcError::InvalidOption(
            "batch resize requires --width, --height, or both".into(),
        ));
    }
    if options.width == Some(0) || options.height == Some(0) {
        return Err(FcError::InvalidOption(
            "resize dimensions must be greater than zero".into(),
        ));
    }

    let mut items = Vec::with_capacity(inputs.len());
    let mut seen_outputs = std::collections::HashSet::new();

    for (idx, input) in inputs.iter().enumerate() {
        let item_id = idx + 1;
        let ext = match target_ext {
            Some(e) => e.to_string(),
            None => input
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("png")
                .to_string(),
        };

        let output =
            foldshift_core::batch::resolve_output_path(input, output_dir, pattern, &ext, item_id)?;

        if !seen_outputs.insert(output.clone()) {
            return Err(FcError::InvalidOption(format!(
                "intra-batch destination collision detected: multiple inputs resolve to '{}'",
                output.display()
            )));
        }

        items.push(foldshift_core::batch::BatchItem {
            id: item_id,
            input: input.clone(),
            output,
        });
    }

    let summary =
        foldshift_core::batch::execute_sequential_batch(&items, cancel_token, progress, |item| {
            cancel_token.check_cancelled()?;
            resize(
                &item.input,
                &item.output,
                options.width,
                options.height,
                EncodeOptions {
                    quality: options.quality,
                    force: options.force,
                },
            )?;
            let written = fs::metadata(&item.output).map(|m| m.len()).unwrap_or(0);
            Ok(written)
        });

    Ok(summary)
}

fn scale_dimension(primary: u32, other: u32, requested_other: u32) -> u32 {
    ((primary as u64) * (requested_other as u64) / (other as u64)).clamp(1, u32::MAX as u64) as u32
}

fn decode(input: &Path) -> Result<DynamicImage> {
    let reader = ImageReader::open(input)
        .map_err(FcError::Io)?
        .with_guessed_format()
        .map_err(FcError::Io)?;
    let mut decoder = reader
        .into_decoder()
        .map_err(|error| FcError::Conversion(error.to_string()))?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder)
        .map_err(|error| FcError::Conversion(error.to_string()))?;
    image.apply_orientation(orientation);
    Ok(image)
}

fn validate_encode_options(options: EncodeOptions) -> Result<()> {
    if !(1..=100).contains(&options.quality) {
        return Err(FcError::InvalidOption(
            "quality must be between 1 and 100".into(),
        ));
    }
    Ok(())
}

fn preflight_target(output: &Path, force: bool) -> Result<ImageFormat> {
    let format = format_from_output(output)?;
    if output.exists() && !force {
        return Err(FcError::OutputExists(output.to_path_buf()));
    }
    Ok(format)
}

fn ensure_input(path: &Path) -> Result<()> {
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(FcError::InputMissing(path.to_path_buf()));
        }
        Err(err) => return Err(FcError::Io(err)),
    };
    if metadata.is_dir() {
        return Err(FcError::InvalidOption(format!(
            "input path is a directory: {}",
            path.display()
        )));
    }
    Ok(())
}

fn guard_still_image(path: &Path) -> Result<()> {
    let file = fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);

    let guessed = ImageReader::new(&mut reader)
        .with_guessed_format()
        .map_err(FcError::Io)?;
    let format = match guessed.format() {
        Some(f) => f,
        None => match format_from_output(path) {
            Ok(f) => f,
            Err(_) => return Ok(()),
        },
    };

    reader.seek(std::io::SeekFrom::Start(0))?;

    match format {
        ImageFormat::Gif => {
            let mut options = gif::DecodeOptions::new();
            options.skip_frame_decoding(true);
            let mut decoder = options
                .read_info(&mut reader)
                .map_err(|e| FcError::Conversion(e.to_string()))?;
            let has_first = decoder
                .next_frame_info()
                .map_err(|e| FcError::Conversion(e.to_string()))?
                .is_some();
            let has_second = decoder
                .next_frame_info()
                .map_err(|e| FcError::Conversion(e.to_string()))?
                .is_some();
            if has_first && has_second {
                return Err(FcError::UnsupportedContent(
                    "animated GIF with multiple frames is not supported".into(),
                ));
            }
        }
        ImageFormat::Png => {
            let decoder = image::codecs::png::PngDecoder::new(&mut reader)
                .map_err(|e| FcError::Conversion(e.to_string()))?;
            if decoder
                .is_apng()
                .map_err(|e| FcError::Conversion(e.to_string()))?
            {
                return Err(FcError::UnsupportedContent(
                    "animated PNG (APNG) is not supported".into(),
                ));
            }
        }
        ImageFormat::WebP => {
            let decoder = image::codecs::webp::WebPDecoder::new(&mut reader)
                .map_err(|e| FcError::Conversion(e.to_string()))?;
            if decoder.has_animation() {
                return Err(FcError::UnsupportedContent(
                    "animated WebP is not supported".into(),
                ));
            }
        }
        ImageFormat::Tiff => {
            let decoder = tiff::decoder::Decoder::new(&mut reader)
                .map_err(|e| FcError::Conversion(e.to_string()))?;
            if decoder.more_images() {
                return Err(FcError::UnsupportedContent(
                    "multi-page TIFF is not supported".into(),
                ));
            }
        }
        _ => {}
    }

    Ok(())
}

fn has_transparent_pixels(image: &DynamicImage) -> bool {
    if !image.color().has_alpha() {
        return false;
    }
    match image {
        DynamicImage::ImageRgba8(buf) => buf.pixels().any(|p| p[3] != u8::MAX),
        DynamicImage::ImageRgba16(buf) => buf.pixels().any(|p| p[3] != u16::MAX),
        DynamicImage::ImageRgba32F(buf) => buf.pixels().any(|p| p[3] < 1.0),
        DynamicImage::ImageLumaA8(buf) => buf.pixels().any(|p| p[1] != u8::MAX),
        DynamicImage::ImageLumaA16(buf) => buf.pixels().any(|p| p[1] != u16::MAX),
        _ => image.pixels().any(|(_, _, p)| p[3] != u8::MAX),
    }
}

fn encode_atomic(image: &DynamicImage, output: &Path, options: EncodeOptions) -> Result<()> {
    validate_encode_options(options)?;
    if output.exists() && !options.force {
        return Err(FcError::OutputExists(output.to_path_buf()));
    }
    let format = format_from_output(output)?;

    if format == ImageFormat::Jpeg && has_transparent_pixels(image) {
        return Err(FcError::InvalidOption(
            "JPEG cannot preserve transparency; choose PNG or WebP".into(),
        ));
    }

    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        fs::create_dir_all(parent)?;
    }
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    {
        let mut writer = BufWriter::new(temporary.as_file_mut());
        match format {
            ImageFormat::Jpeg => {
                let mut encoder = JpegEncoder::new_with_quality(&mut writer, options.quality);
                match image {
                    DynamicImage::ImageLuma8(buf) => {
                        encoder
                            .encode_image(buf)
                            .map_err(|error| FcError::Conversion(error.to_string()))?;
                    }
                    DynamicImage::ImageLuma16(_) => {
                        let luma8 = image.to_luma8();
                        encoder
                            .encode_image(&luma8)
                            .map_err(|error| FcError::Conversion(error.to_string()))?;
                    }
                    _ if matches!(image.color(), image::ColorType::L8 | image::ColorType::La8) => {
                        let luma8 = image.to_luma8();
                        encoder
                            .encode_image(&luma8)
                            .map_err(|error| FcError::Conversion(error.to_string()))?;
                    }
                    DynamicImage::ImageRgb8(buf) => {
                        encoder
                            .encode_image(buf)
                            .map_err(|error| FcError::Conversion(error.to_string()))?;
                    }
                    _ => {
                        let rgb8 = image.to_rgb8();
                        encoder
                            .encode_image(&rgb8)
                            .map_err(|error| FcError::Conversion(error.to_string()))?;
                    }
                }
            }
            other => image
                .write_to(&mut writer, other)
                .map_err(|error| FcError::Conversion(error.to_string()))?,
        }
        writer.flush()?;
    }
    temporary.as_file().sync_all()?;
    let result = if options.force {
        temporary.persist(output)
    } else {
        temporary.persist_noclobber(output)
    };
    result.map_err(|error| {
        if error.error.kind() == std::io::ErrorKind::AlreadyExists {
            FcError::OutputExists(output.to_path_buf())
        } else {
            FcError::Io(error.error)
        }
    })?;
    Ok(())
}

fn format_from_output(path: &Path) -> Result<ImageFormat> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "png" => Ok(ImageFormat::Png),
        "jpg" | "jpeg" => Ok(ImageFormat::Jpeg),
        "webp" => Ok(ImageFormat::WebP),
        "gif" => Ok(ImageFormat::Gif),
        "bmp" => Ok(ImageFormat::Bmp),
        "tif" | "tiff" => Ok(ImageFormat::Tiff),
        _ => Err(FcError::UnsupportedFormat(extension)),
    }
}

fn format_name(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "PNG",
        ImageFormat::Jpeg => "JPEG",
        ImageFormat::WebP => "WebP",
        ImageFormat::Gif => "GIF",
        ImageFormat::Bmp => "BMP",
        ImageFormat::Tiff => "TIFF",
        _ => "unsupported image",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_test_image(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(image::RgbImage::from_pixel(w, h, image::Rgb([40, 90, 150])))
    }

    #[test]
    fn converts_every_supported_format_and_reopens() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("input.png");
        DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            12,
            8,
            image::Rgb([40, 90, 150]),
        ))
        .save(&input)
        .unwrap();
        for extension in SUPPORTED_EXTENSIONS {
            let output = dir.path().join(format!("output.{extension}"));
            convert(&input, &output, EncodeOptions::default()).unwrap();
            assert_eq!(decode(&output).unwrap().dimensions(), (12, 8));
        }
    }

    #[test]
    fn overwrite_requires_force_and_failed_encoding_preserves_output() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("existing.jpg");
        fs::write(&output, b"original").unwrap();
        let transparent = DynamicImage::new_rgba8(4, 4);
        assert!(matches!(
            encode_atomic(&transparent, &output, EncodeOptions::default()),
            Err(FcError::OutputExists(_))
        ));
        assert!(
            encode_atomic(
                &transparent,
                &output,
                EncodeOptions {
                    force: true,
                    ..Default::default()
                }
            )
            .is_err()
        );
        assert_eq!(fs::read(&output).unwrap(), b"original");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        encode_atomic(
            &DynamicImage::new_rgb8(3, 2),
            &output,
            EncodeOptions {
                force: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(decode(&output).unwrap().dimensions(), (3, 2));
    }

    #[test]
    fn resize_preserves_aspect_ratio_and_rejects_zero() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("input.png");
        let output = dir.path().join("output.png");
        DynamicImage::new_rgb8(120, 80).save(&input).unwrap();
        assert!(resize(&input, &output, Some(0), None, EncodeOptions::default()).is_err());
        assert_eq!(
            resize(
                &input,
                &output,
                Some(30),
                Some(30),
                EncodeOptions::default()
            )
            .unwrap(),
            (30, 20)
        );
    }

    #[test]
    fn corrupt_input_creates_no_output() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("bad.png");
        let output = dir.path().join("output.png");
        fs::write(&input, b"not an image").unwrap();
        assert!(convert(&input, &output, EncodeOptions::default()).is_err());
        assert!(!output.exists());
    }

    #[test]
    fn proportional_dimension_never_becomes_zero() {
        assert_eq!(scale_dimension(1, 10_000, 1), 1);
    }

    #[test]
    fn recognizes_supported_extensions() {
        for extension in SUPPORTED_EXTENSIONS {
            let path = PathBuf::from(format!("test.{extension}"));
            assert!(format_from_output(&path).is_ok(), "{extension}");
        }
    }

    fn create_animated_gif() -> Vec<u8> {
        let mut buffer = Vec::new();
        {
            let mut encoder =
                gif::Encoder::new(&mut buffer, 2, 2, &[0, 0, 0, 255, 255, 255]).unwrap();
            let frame1 = gif::Frame::from_indexed_pixels(2, 2, [0, 1, 0, 1], None);
            let frame2 = gif::Frame::from_indexed_pixels(2, 2, [1, 0, 1, 0], None);
            encoder.write_frame(&frame1).unwrap();
            encoder.write_frame(&frame2).unwrap();
        }
        buffer
    }

    fn create_single_frame_gif() -> Vec<u8> {
        let mut buffer = Vec::new();
        {
            let mut encoder =
                gif::Encoder::new(&mut buffer, 2, 2, &[0, 0, 0, 255, 255, 255]).unwrap();
            let frame1 = gif::Frame::from_indexed_pixels(2, 2, [0, 1, 0, 1], None);
            encoder.write_frame(&frame1).unwrap();
        }
        buffer
    }

    fn create_multipage_tiff() -> Vec<u8> {
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut tiff = tiff::encoder::TiffEncoder::new(&mut cursor).unwrap();
            let image1 = [10u8, 20, 30, 40, 50, 60];
            tiff.write_image::<tiff::encoder::colortype::RGB8>(2, 1, &image1)
                .unwrap();
            let image2 = [70u8, 80, 90, 100, 110, 120];
            tiff.write_image::<tiff::encoder::colortype::RGB8>(2, 1, &image2)
                .unwrap();
        }
        cursor.into_inner()
    }

    fn create_singlepage_tiff() -> Vec<u8> {
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut tiff = tiff::encoder::TiffEncoder::new(&mut cursor).unwrap();
            let image1 = [10u8, 20, 30, 40, 50, 60];
            tiff.write_image::<tiff::encoder::colortype::RGB8>(2, 1, &image1)
                .unwrap();
        }
        cursor.into_inner()
    }

    fn create_animated_png() -> Vec<u8> {
        let mut buffer = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut buffer, 2, 2);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_animated(2, 0).unwrap();
            let mut writer = encoder.write_header().unwrap();
            let frame1 = [255, 0, 0, 255].repeat(4);
            let frame2 = [0, 255, 0, 255].repeat(4);
            writer.write_image_data(&frame1).unwrap();
            writer.write_image_data(&frame2).unwrap();
        }
        buffer
    }

    fn create_animated_webp() -> Vec<u8> {
        let vp8_payload = [
            0xd0, 0x01, 0x00, 0x9d, 0x01, 0x2a, 0x02, 0x00, 0x02, 0x00, 0x02, 0x00, 0x34, 0x25,
            0xa0, 0x02, 0x74, 0xba, 0x01, 0xf8, 0x00, 0x03, 0xb0, 0x00, 0xfe, 0xf0, 0xc4, 0x0b,
            0xff, 0x20, 0xb9, 0x61, 0x75, 0xc8, 0xd7, 0xff, 0x20, 0x3f, 0xe4, 0x07, 0xfc, 0x80,
            0xff, 0xf8, 0xf2, 0x00,
        ];
        let mut anmf_chunk = Vec::new();
        anmf_chunk.extend_from_slice(&[0, 0, 0]);
        anmf_chunk.extend_from_slice(&[0, 0, 0]);
        anmf_chunk.extend_from_slice(&[1, 0, 0]);
        anmf_chunk.extend_from_slice(&[1, 0, 0]);
        anmf_chunk.extend_from_slice(&[100, 0, 0]);
        anmf_chunk.push(0);
        anmf_chunk.extend_from_slice(b"VP8 ");
        anmf_chunk.extend_from_slice(&(vp8_payload.len() as u32).to_le_bytes());
        anmf_chunk.extend_from_slice(&vp8_payload);

        let mut data = Vec::new();
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&[0, 0, 0, 0]);
        data.extend_from_slice(b"WEBP");
        data.extend_from_slice(b"VP8X");
        data.extend_from_slice(&10u32.to_le_bytes());
        data.push(0x02);
        data.extend_from_slice(&[0, 0, 0]);
        data.extend_from_slice(&[1, 0, 0]);
        data.extend_from_slice(&[1, 0, 0]);
        data.extend_from_slice(b"ANIM");
        data.extend_from_slice(&6u32.to_le_bytes());
        data.extend_from_slice(&[0, 0, 0, 0]);
        data.extend_from_slice(&[0, 0]);
        data.extend_from_slice(b"ANMF");
        data.extend_from_slice(&(anmf_chunk.len() as u32).to_le_bytes());
        data.extend_from_slice(&anmf_chunk);

        let riff_size = (data.len() - 8) as u32;
        data[4..8].copy_from_slice(&riff_size.to_le_bytes());
        data
    }

    #[test]
    fn rejects_animated_gif_and_preserves_existing_output_on_force() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("animated.gif");
        let output = dir.path().join("output.png");
        fs::write(&input, create_animated_gif()).unwrap();
        fs::write(&output, b"pre-existing output").unwrap();

        let result = convert(
            &input,
            &output,
            EncodeOptions {
                force: true,
                ..Default::default()
            },
        );
        assert!(matches!(result, Err(FcError::UnsupportedContent(_))));
        assert_eq!(fs::read(&output).unwrap(), b"pre-existing output");

        let resize_result = resize(
            &input,
            &output,
            Some(10),
            None,
            EncodeOptions {
                force: true,
                ..Default::default()
            },
        );
        assert!(matches!(resize_result, Err(FcError::UnsupportedContent(_))));
        assert_eq!(fs::read(&output).unwrap(), b"pre-existing output");
    }

    #[test]
    fn supports_single_frame_gif() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("single.gif");
        let output = dir.path().join("output.png");
        fs::write(&input, create_single_frame_gif()).unwrap();

        convert(&input, &output, EncodeOptions::default()).unwrap();
        assert_eq!(decode(&output).unwrap().dimensions(), (2, 2));

        let resized_output = dir.path().join("resized.png");
        let dims = resize(
            &input,
            &resized_output,
            Some(4),
            None,
            EncodeOptions::default(),
        )
        .unwrap();
        assert_eq!(dims, (4, 4));
    }

    #[test]
    fn rejects_multipage_tiff_and_preserves_existing_output_on_force() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("multipage.tiff");
        let output = dir.path().join("output.png");
        fs::write(&input, create_multipage_tiff()).unwrap();
        fs::write(&output, b"pre-existing tiff output").unwrap();

        let result = convert(
            &input,
            &output,
            EncodeOptions {
                force: true,
                ..Default::default()
            },
        );
        assert!(matches!(result, Err(FcError::UnsupportedContent(_))));
        assert_eq!(fs::read(&output).unwrap(), b"pre-existing tiff output");
    }

    #[test]
    fn supports_singlepage_tiff() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("single.tiff");
        let output = dir.path().join("output.png");
        fs::write(&input, create_singlepage_tiff()).unwrap();

        convert(&input, &output, EncodeOptions::default()).unwrap();
        assert_eq!(decode(&output).unwrap().dimensions(), (2, 1));
    }

    #[test]
    fn rejects_animated_png() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("animated.png");
        let output = dir.path().join("output.webp");
        fs::write(&input, create_animated_png()).unwrap();

        let result = convert(&input, &output, EncodeOptions::default());
        assert!(matches!(result, Err(FcError::UnsupportedContent(_))));
        assert!(!output.exists());
    }

    #[test]
    fn rejects_animated_webp() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("animated.webp");
        let output = dir.path().join("output.png");
        fs::write(&input, create_animated_webp()).unwrap();

        let result = convert(&input, &output, EncodeOptions::default());
        assert!(matches!(result, Err(FcError::UnsupportedContent(_))));
        assert!(!output.exists());
    }

    #[test]
    fn detects_animated_content_regardless_of_extension() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("misnamed_gif.png");
        let output = dir.path().join("output.png");
        fs::write(&input, create_animated_gif()).unwrap();

        let result = convert(&input, &output, EncodeOptions::default());
        assert!(matches!(result, Err(FcError::UnsupportedContent(_))));
        assert!(!output.exists());
    }

    #[test]
    fn rejects_directory_input() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("some_directory");
        fs::create_dir(&input).unwrap();
        let output = dir.path().join("output.png");

        let result = convert(&input, &output, EncodeOptions::default());
        assert!(matches!(result, Err(FcError::InvalidOption(_))));
        assert!(!output.exists());
    }

    #[test]
    fn preflight_validation_has_no_filesystem_side_effects() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("input.png");
        DynamicImage::new_rgb8(10, 10).save(&input).unwrap();

        let nested_out = dir.path().join("new_dir").join("output.png");
        assert!(!nested_out.parent().unwrap().exists());

        // 1. Invalid quality rejects without creating parent dir
        let res = convert(
            &input,
            &nested_out,
            EncodeOptions {
                quality: 105,
                ..Default::default()
            },
        );
        assert!(matches!(res, Err(FcError::InvalidOption(_))));
        assert!(!nested_out.parent().unwrap().exists());

        // 2. Unsupported format rejects without creating parent dir
        let bad_ext = dir.path().join("new_dir").join("output.unsupported");
        let res = convert(&input, &bad_ext, EncodeOptions::default());
        assert!(matches!(res, Err(FcError::UnsupportedFormat(_))));
        assert!(!bad_ext.parent().unwrap().exists());

        // 3. Resize with zero dimension rejects without creating parent dir
        let res = resize(&input, &nested_out, Some(0), None, EncodeOptions::default());
        assert!(matches!(res, Err(FcError::InvalidOption(_))));
        assert!(!nested_out.parent().unwrap().exists());

        // 4. Missing input rejects without creating parent dir
        let missing = dir.path().join("missing.png");
        let res = convert(&missing, &nested_out, EncodeOptions::default());
        assert!(matches!(res, Err(FcError::InputMissing(_))));
        assert!(!nested_out.parent().unwrap().exists());

        // 5. Animated input rejects without creating parent dir
        let anim = dir.path().join("anim.gif");
        fs::write(&anim, create_animated_gif()).unwrap();
        let res = convert(&anim, &nested_out, EncodeOptions::default());
        assert!(matches!(res, Err(FcError::UnsupportedContent(_))));
        assert!(!nested_out.parent().unwrap().exists());
    }

    #[test]
    fn library_quality_validation_independent_of_cli() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("input.png");
        let output = dir.path().join("output.jpg");
        DynamicImage::new_rgb8(10, 10).save(&input).unwrap();

        assert!(matches!(
            convert(
                &input,
                &output,
                EncodeOptions {
                    quality: 0,
                    ..Default::default()
                }
            ),
            Err(FcError::InvalidOption(_))
        ));

        assert!(matches!(
            convert(
                &input,
                &output,
                EncodeOptions {
                    quality: 101,
                    ..Default::default()
                }
            ),
            Err(FcError::InvalidOption(_))
        ));
    }

    fn create_jpeg_with_exif_orientation(width: u32, height: u32, orientation: u8) -> Vec<u8> {
        let base = DynamicImage::new_rgb8(width, height);
        let mut base_jpeg = Vec::new();
        base.write_to(&mut std::io::Cursor::new(&mut base_jpeg), ImageFormat::Jpeg)
            .unwrap();

        #[rustfmt::skip]
        let app1 = [
            0xFF, 0xE1, 0x00, 0x22,
            b'E', b'x', b'i', b'f', 0x00, 0x00,
            b'I', b'I', 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00,
            0x01, 0x00,
            0x12, 0x01, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00,
            orientation, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];

        let mut output = Vec::with_capacity(base_jpeg.len() + app1.len());
        output.extend_from_slice(&base_jpeg[..2]); // SOI
        output.extend_from_slice(&app1);
        output.extend_from_slice(&base_jpeg[2..]);
        output
    }

    #[test]
    fn exif_orientation_applied_on_decode_and_reported_on_inspect() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("portrait.jpg");
        // Create 12x8 JPEG with orientation 6 (Rotate90)
        let jpeg_bytes = create_jpeg_with_exif_orientation(12, 8, 6);
        fs::write(&input, jpeg_bytes).unwrap();

        let inspection = inspect(&input).unwrap();
        assert_eq!(
            inspection.properties.get("orientation").unwrap(),
            "Rotate90"
        );
        // Effective dimensions after rotation
        assert_eq!(inspection.properties.get("width").unwrap(), 8);
        assert_eq!(inspection.properties.get("height").unwrap(), 12);

        // Convert to PNG and check output pixel dimensions
        let output = dir.path().join("upright.png");
        convert(&input, &output, EncodeOptions::default()).unwrap();
        let decoded = decode(&output).unwrap();
        assert_eq!(decoded.dimensions(), (8, 12));
    }

    #[test]
    fn grayscale_jpeg_and_16bit_downsampling() {
        let dir = tempfile::tempdir().unwrap();

        // 8-bit Grayscale
        let gray8_input = dir.path().join("gray8.png");
        DynamicImage::ImageLuma8(image::GrayImage::from_pixel(10, 10, image::Luma([128])))
            .save(&gray8_input)
            .unwrap();
        let gray_jpg = dir.path().join("gray.jpg");
        convert(&gray8_input, &gray_jpg, EncodeOptions::default()).unwrap();
        let insp = inspect(&gray_jpg).unwrap();
        assert_eq!(insp.properties.get("color").unwrap(), "L8");

        // 16-bit Grayscale downsampled to 8-bit
        let gray16_input = dir.path().join("gray16.png");
        DynamicImage::ImageLuma16(image::ImageBuffer::from_pixel(10, 10, image::Luma([32000])))
            .save(&gray16_input)
            .unwrap();
        let gray16_jpg = dir.path().join("gray16.jpg");
        convert(&gray16_input, &gray16_jpg, EncodeOptions::default()).unwrap();
        let insp16 = inspect(&gray16_jpg).unwrap();
        assert_eq!(insp16.properties.get("color").unwrap(), "L8");

        // 16-bit RGB downsampled to 8-bit
        let rgb16_input = dir.path().join("rgb16.png");
        DynamicImage::ImageRgb16(image::ImageBuffer::from_pixel(
            10,
            10,
            image::Rgb([32000, 10000, 5000]),
        ))
        .save(&rgb16_input)
        .unwrap();
        let rgb16_jpg = dir.path().join("rgb16.jpg");
        convert(&rgb16_input, &rgb16_jpg, EncodeOptions::default()).unwrap();
        let insp_rgb = inspect(&rgb16_jpg).unwrap();
        assert_eq!(insp_rgb.properties.get("color").unwrap(), "Rgb8");
    }

    #[test]
    fn transparency_rules_and_opaque_alpha_handling() {
        let dir = tempfile::tempdir().unwrap();

        // 1. Fully opaque alpha channel converts to JPEG without error
        let opaque_input = dir.path().join("opaque.png");
        DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            8,
            8,
            image::Rgba([100, 150, 200, 255]),
        ))
        .save(&opaque_input)
        .unwrap();
        let opaque_jpg = dir.path().join("opaque.jpg");
        assert!(convert(&opaque_input, &opaque_jpg, EncodeOptions::default()).is_ok());

        // 2. Image with truly transparent pixels is rejected for JPEG
        let transparent_input = dir.path().join("transparent.png");
        let mut img = image::RgbaImage::from_pixel(8, 8, image::Rgba([100, 150, 200, 255]));
        img.put_pixel(0, 0, image::Rgba([100, 150, 200, 0]));
        DynamicImage::ImageRgba8(img)
            .save(&transparent_input)
            .unwrap();
        let transparent_jpg = dir.path().join("transparent.jpg");
        let err = convert(
            &transparent_input,
            &transparent_jpg,
            EncodeOptions::default(),
        );
        assert!(
            matches!(err, Err(FcError::InvalidOption(msg)) if msg.contains("JPEG cannot preserve transparency"))
        );

        // 3. PNG and WebP preserve transparency
        let transparent_webp = dir.path().join("transparent.webp");
        assert!(
            convert(
                &transparent_input,
                &transparent_webp,
                EncodeOptions::default()
            )
            .is_ok()
        );
        let decoded_webp = decode(&transparent_webp).unwrap();
        assert_eq!(decoded_webp.color(), image::ColorType::Rgba8);
    }

    #[test]
    fn has_transparent_pixels_checks_native_buffers_accurately() {
        // RGB8 has no alpha
        let rgb = DynamicImage::new_rgb8(2, 2);
        assert!(!has_transparent_pixels(&rgb));

        // RGBA8 fully opaque
        let rgba_opaque = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            2,
            2,
            image::Rgba([0, 0, 0, 255]),
        ));
        assert!(!has_transparent_pixels(&rgba_opaque));

        // RGBA8 with one pixel having alpha 254
        let mut rgba_trans = image::RgbaImage::from_pixel(2, 2, image::Rgba([0, 0, 0, 255]));
        rgba_trans.put_pixel(1, 1, image::Rgba([0, 0, 0, 254]));
        assert!(has_transparent_pixels(&DynamicImage::ImageRgba8(
            rgba_trans
        )));

        // RGBA16 fully opaque
        let rgba16_opaque = DynamicImage::ImageRgba16(image::ImageBuffer::from_pixel(
            2,
            2,
            image::Rgba([0, 0, 0, u16::MAX]),
        ));
        assert!(!has_transparent_pixels(&rgba16_opaque));

        // RGBA16 with one pixel having alpha u16::MAX - 1
        let mut rgba16_trans =
            image::ImageBuffer::from_pixel(2, 2, image::Rgba([0, 0, 0, u16::MAX]));
        rgba16_trans.put_pixel(0, 0, image::Rgba([0, 0, 0, u16::MAX - 1]));
        assert!(has_transparent_pixels(&DynamicImage::ImageRgba16(
            rgba16_trans
        )));

        // LumaA8 fully opaque vs transparent
        let luma_a_opaque = DynamicImage::ImageLumaA8(image::ImageBuffer::from_pixel(
            2,
            2,
            image::LumaA([128, 255]),
        ));
        assert!(!has_transparent_pixels(&luma_a_opaque));

        let mut luma_a_trans = image::ImageBuffer::from_pixel(2, 2, image::LumaA([128, 255]));
        luma_a_trans.put_pixel(0, 0, image::LumaA([128, 100]));
        assert!(has_transparent_pixels(&DynamicImage::ImageLumaA8(
            luma_a_trans
        )));
    }

    #[test]
    fn same_file_policy_and_alias_preservation() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("in_place.png");
        let initial_img = DynamicImage::new_rgb8(10, 10);
        initial_img.save(&file).unwrap();
        let initial_bytes = fs::read(&file).unwrap();

        // 1. Same-file conversion without force must fail with OutputExists
        let res = convert(&file, &file, EncodeOptions::default());
        assert!(matches!(res, Err(FcError::OutputExists(_))));
        assert_eq!(fs::read(&file).unwrap(), initial_bytes);

        // 2. Same-file alias (e.g. ./in_place.png) without force must fail with OutputExists
        let alias = dir.path().join(".").join("in_place.png");
        let res = convert(&file, &alias, EncodeOptions::default());
        assert!(matches!(res, Err(FcError::OutputExists(_))));
        assert_eq!(fs::read(&file).unwrap(), initial_bytes);

        // 3. Same-file resize with force succeeds atomically
        let resize_res = resize(
            &file,
            &file,
            Some(5),
            None,
            EncodeOptions {
                force: true,
                ..Default::default()
            },
        );
        assert_eq!(resize_res.unwrap(), (5, 5));
        let decoded = decode(&file).unwrap();
        assert_eq!(decoded.dimensions(), (5, 5));

        // 4. Same-file conversion with force that fails encoding preserves the original file
        let transparent_file = dir.path().join("transparent.png");
        let mut img = image::RgbaImage::from_pixel(4, 4, image::Rgba([100, 150, 200, 255]));
        img.put_pixel(0, 0, image::Rgba([100, 150, 200, 0])); // transparent pixel
        DynamicImage::ImageRgba8(img)
            .save(&transparent_file)
            .unwrap();
        let trans_before = fs::read(&transparent_file).unwrap();

        // Trying to convert transparent.png to transparent.jpg in-place or invalid options
        let err = convert(
            &transparent_file,
            &transparent_file,
            EncodeOptions {
                quality: 101, // invalid quality
                force: true,
            },
        );
        assert!(matches!(err, Err(FcError::InvalidOption(_))));
        assert_eq!(fs::read(&transparent_file).unwrap(), trans_before);
    }

    #[test]
    fn batch_convert_and_resize_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let in1 = dir.path().join("img1.png");
        let in2 = dir.path().join("img2.png");
        let in3 = dir.path().join("img3.png");

        make_test_image(10, 10).save(&in1).unwrap();
        make_test_image(20, 20).save(&in2).unwrap();
        make_test_image(30, 30).save(&in3).unwrap();

        let out_dir = dir.path().join("converted");
        let cancel_token = foldshift_core::batch::CancellationToken::new();
        let mut progress_count = 0;

        // 1. Batch convert with pattern
        let summary = batch_convert(
            &[in1.clone(), in2.clone(), in3.clone()],
            "jpg",
            &out_dir,
            Some("{index:02}_{stem}.{ext}"),
            EncodeOptions::default(),
            &cancel_token,
            |_| {
                progress_count += 1;
            },
        )
        .unwrap();

        assert_eq!(summary.total, 3);
        assert_eq!(summary.succeeded, 3);
        assert_eq!(summary.failed, 0);
        assert!(summary.is_all_success());
        assert!(out_dir.join("01_img1.jpg").exists());
        assert!(out_dir.join("02_img2.jpg").exists());
        assert!(out_dir.join("03_img3.jpg").exists());
        assert!(progress_count > 0);

        // 2. Batch resize
        let resize_dir = dir.path().join("resized");
        let resize_summary = batch_resize(
            &[in1.clone(), in2.clone(), in3.clone()],
            Some("png"),
            &resize_dir,
            Some("small_{stem}.{ext}"),
            BatchResizeOptions {
                width: Some(5),
                ..Default::default()
            },
            &cancel_token,
            |_| {},
        )
        .unwrap();

        assert_eq!(resize_summary.total, 3);
        assert_eq!(resize_summary.succeeded, 3);
        assert!(resize_dir.join("small_img1.png").exists());
        let insp = inspect(&resize_dir.join("small_img1.png")).unwrap();
        assert_eq!(insp.properties.get("width").unwrap(), &5);
    }

    #[test]
    fn batch_convert_independent_failures_and_cancellation() {
        let dir = tempfile::tempdir().unwrap();
        let good1 = dir.path().join("good1.png");
        let corrupt = dir.path().join("corrupt.png");
        let good2 = dir.path().join("good2.png");

        make_test_image(10, 10).save(&good1).unwrap();
        fs::write(&corrupt, b"not an image at all").unwrap();
        make_test_image(10, 10).save(&good2).unwrap();

        let out_dir = dir.path().join("batch_fail_out");
        let cancel_token = foldshift_core::batch::CancellationToken::new();

        // Independent failure test: corrupt file fails, good files succeed!
        let summary = batch_convert(
            &[good1.clone(), corrupt.clone(), good2.clone()],
            "jpg",
            &out_dir,
            None,
            EncodeOptions::default(),
            &cancel_token,
            |_| {},
        )
        .unwrap();

        assert_eq!(summary.total, 3);
        assert_eq!(summary.succeeded, 2);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.cancelled, 0);
        assert!(summary.has_failures_or_cancellations());
        assert!(out_dir.join("good1.jpg").exists());
        assert!(!out_dir.join("corrupt.jpg").exists());
        assert!(out_dir.join("good2.jpg").exists());

        // Cancellation test
        let cancel_out_dir = dir.path().join("batch_cancel_out");
        let cancel_tok = foldshift_core::batch::CancellationToken::new();
        let tok_clone = cancel_tok.clone();

        let cancel_summary = batch_convert(
            &[good1.clone(), good2.clone()],
            "jpg",
            &cancel_out_dir,
            None,
            EncodeOptions::default(),
            &cancel_tok,
            |ev| {
                if let foldshift_core::batch::BatchProgress::ItemCompleted { index, .. } = ev
                    && index == 1
                {
                    tok_clone.cancel();
                }
            },
        )
        .unwrap();

        assert_eq!(cancel_summary.total, 2);
        assert_eq!(cancel_summary.succeeded, 1);
        assert_eq!(cancel_summary.cancelled, 1);
        assert!(cancel_out_dir.join("good1.jpg").exists());
        assert!(!cancel_out_dir.join("good2.jpg").exists());
    }
}
