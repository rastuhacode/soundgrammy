use super::*;
use resvg::{tiny_skia, usvg};
fn png(tree: &usvg::Tree, size: u32, background: bool) -> Result<Vec<u8>> {
    let mut pixmap = tiny_skia::Pixmap::new(size, size).ok_or("Invalid icon size")?;
    if background {
        pixmap.fill(tiny_skia::Color::from_rgba8(0, 136, 204, 255));
    }
    let scale = size as f32 / tree.size().width();
    resvg::render(
        tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok(pixmap.encode_png()?)
}
fn existing(tree: &usvg::Tree, directory: &Path, check: bool) -> Result {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            existing(tree, &path, check)?;
        } else if path.extension().is_some_and(|ext| ext == "png") {
            let bytes = fs::read(&path)?;
            if bytes.len() < 24 {
                return Err(format!("Invalid PNG: {}", path.display()).into());
            }
            let width = u32::from_be_bytes(bytes[16..20].try_into()?);
            let height = u32::from_be_bytes(bytes[20..24].try_into()?);
            if width == height {
                let image = png(
                    tree,
                    width,
                    path.components().any(|v| v.as_os_str() == "ios"),
                )?;
                if !check {
                    fs::write(path, image)?;
                }
            }
        }
    }
    Ok(())
}
pub fn generate(check: bool) -> Result {
    let svg = fs::read(root().join("native/icons/original/soundgrammy-original.svg"))?;
    let tree = usvg::Tree::from_data(&svg, &usvg::Options::default())?;
    existing(&tree, &root().join("native/icons"), check)?;
    existing(
        &tree,
        &root().join("native/gen/android/app/src/main/res"),
        check,
    )?;
    let sizes = [16, 24, 32, 48, 64, 128, 256];
    let mut ico = vec![0, 0, 1, 0, sizes.len() as u8, 0];
    let images = sizes
        .map(|size| png(&tree, size, false))
        .into_iter()
        .collect::<Result<Vec<_>>>()?;
    let mut offset = 6 + sizes.len() * 16;
    for (size, image) in sizes.into_iter().zip(&images) {
        ico.extend_from_slice(&[size as u8, size as u8, 0, 0, 1, 0, 32, 0]);
        ico.extend_from_slice(&(image.len() as u32).to_le_bytes());
        ico.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += image.len();
    }
    for image in images {
        ico.extend(image);
    }
    let mut chunks = Vec::new();
    for (kind, size) in [
        ("icp4", 16),
        ("icp5", 32),
        ("icp6", 64),
        ("ic07", 128),
        ("ic08", 256),
        ("ic09", 512),
        ("ic10", 1024),
    ] {
        let image = png(&tree, size, false)?;
        chunks.extend_from_slice(kind.as_bytes());
        chunks.extend_from_slice(&((image.len() + 8) as u32).to_be_bytes());
        chunks.extend(image);
    }
    let mut icns = b"icns".to_vec();
    icns.extend_from_slice(&((chunks.len() + 8) as u32).to_be_bytes());
    icns.extend(chunks);
    if !check {
        fs::write(root().join("native/icons/icon.ico"), ico)?;
        fs::write(root().join("native/icons/icon.icns"), icns)?;
    }
    println!(
        "Native icon generation {}.",
        if check { "verified" } else { "completed" }
    );
    Ok(())
}
