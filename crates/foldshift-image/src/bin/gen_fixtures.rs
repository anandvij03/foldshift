use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use std::fs;
use std::path::Path;

fn create_jpeg_with_exif(width: u32, height: u32, orientation: u8) -> Vec<u8> {
    let mut img = RgbImage::new(width, height);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        *pixel = Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8]);
    }
    let base = DynamicImage::ImageRgb8(img);
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
    let mut out = Vec::with_capacity(base_jpeg.len() + app1.len());
    out.extend_from_slice(&base_jpeg[..2]);
    out.extend_from_slice(&app1);
    out.extend_from_slice(&base_jpeg[2..]);
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out_dir = if args.len() > 1 {
        Path::new(&args[1]).to_path_buf()
    } else {
        Path::new("target/bench_fixtures").to_path_buf()
    };
    fs::create_dir_all(&out_dir).unwrap();

    // Fixture 1: 100x100 PNG (~10 KB)
    let mut s = RgbImage::new(100, 100);
    for p in s.pixels_mut() {
        *p = Rgb([100, 150, 200]);
    }
    DynamicImage::ImageRgb8(s)
        .save(out_dir.join("small_100x100.png"))
        .unwrap();

    // Fixture 2: 1920x1080 JPEG (~1-2 MB)
    let mut m = RgbImage::new(1920, 1080);
    for (x, y, p) in m.enumerate_pixels_mut() {
        *p = Rgb([(x % 255) as u8, (y % 255) as u8, 128]);
    }
    DynamicImage::ImageRgb8(m)
        .save(out_dir.join("medium_1920x1080.jpg"))
        .unwrap();

    // Fixture 3: 4000x3000 JPEG with orientation 6 (~5-7 MB)
    let l_bytes = create_jpeg_with_exif(4000, 3000, 6);
    fs::write(out_dir.join("large_4000x3000_orient6.jpg"), l_bytes).unwrap();

    println!("Fixtures generated successfully in {}", out_dir.display());
}
