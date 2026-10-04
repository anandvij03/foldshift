use std::fs;
use std::io::{BufWriter, Write};
use std::path::Path;

use foldshift_core::{FcError, Inspection, MediaKind, Result};
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, GenericImageView, ImageFormat, ImageReader};
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
    let image = reader
        .decode()
        .map_err(|error| FcError::Conversion(error.to_string()))?;
    let (width, height) = image.dimensions();
    let metadata = fs::metadata(path)?;

    let mut properties = std::collections::BTreeMap::new();
    properties.insert("width".into(), json!(width));
    properties.insert("height".into(), json!(height));
    properties.insert("color".into(), json!(format!("{:?}", image.color())));

    Ok(Inspection {
        path: path.to_path_buf(),
        format: format_name(format).into(),
        media_kind: MediaKind::Image,
        bytes: metadata.len(),
        properties,
    })
}

pub fn convert(input: &Path, output: &Path, options: EncodeOptions) -> Result<()> {
    ensure_input(input)?;
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
    ensure_input(input)?;
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

fn scale_dimension(primary: u32, other: u32, requested_other: u32) -> u32 {
    ((primary as u64) * (requested_other as u64) / (other as u64)).clamp(1, u32::MAX as u64) as u32
}

fn decode(input: &Path) -> Result<DynamicImage> {
    ImageReader::open(input)?
        .with_guessed_format()?
        .decode()
        .map_err(|error| FcError::Conversion(error.to_string()))
}

fn ensure_input(path: &Path) -> Result<()> {
    if !path.is_file() {
        return Err(FcError::InputMissing(path.to_path_buf()));
    }
    Ok(())
}

fn encode_atomic(image: &DynamicImage, output: &Path, options: EncodeOptions) -> Result<()> {
    if !(1..=100).contains(&options.quality) {
        return Err(FcError::InvalidOption(
            "quality must be between 1 and 100".into(),
        ));
    }
    if output.exists() && !options.force {
        return Err(FcError::OutputExists(output.to_path_buf()));
    }
    let format = format_from_output(output)?;
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
                if image.color().has_alpha() && image.to_rgba16().pixels().any(|p| p[3] != u16::MAX)
                {
                    return Err(FcError::InvalidOption(
                        "JPEG cannot preserve transparency; choose PNG or WebP".into(),
                    ));
                }
                let mut encoder = JpegEncoder::new_with_quality(&mut writer, options.quality);
                encoder
                    .encode_image(&DynamicImage::ImageRgb8(image.to_rgb8()))
                    .map_err(|error| FcError::Conversion(error.to_string()))?;
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
}
