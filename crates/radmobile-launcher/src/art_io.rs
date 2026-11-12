//! Reading and writing the launcher's pictures ([`radmobile_core::art`]) as PNG.

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

use radmobile_core::art::Image;

pub fn save_png(path: &Path, image: &Image) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    {
        let file = BufWriter::new(File::create(&tmp)?);
        let mut enc = png::Encoder::new(file, image.width, image.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()?.write_image_data(&image.rgba)?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Any 8-bit PNG (MAME's snapshots are RGB), as RGBA.
pub fn load_png(path: &Path) -> anyhow::Result<Image> {
    let mut decoder = png::Decoder::new(BufReader::new(File::open(path)?));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size().unwrap_or_default()];
    let info = reader.next_frame(&mut buf)?;
    let samples = info.color_type.samples();
    let mut rgba = Vec::with_capacity((info.width * info.height * 4) as usize);
    for px in buf[..info.buffer_size()].chunks_exact(samples) {
        let pixel = match samples {
            1 => [px[0], px[0], px[0], 255],
            2 => [px[0], px[0], px[0], px[1]],
            3 => [px[0], px[1], px[2], 255],
            _ => [px[0], px[1], px[2], px[3]],
        };
        rgba.extend_from_slice(&pixel);
    }
    Ok(Image {
        width: info.width,
        height: info.height,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pictures_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/b.png");
        let (img, _) = radmobile_core::art::bezel(64, 36);
        save_png(&path, &img).unwrap();
        assert_eq!(load_png(&path).unwrap(), img);
        assert!(!path.with_extension("tmp").exists());
    }
}
