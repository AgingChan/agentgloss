use anyhow::Context;

fn main() -> anyhow::Result<()> {
    let svg = std::fs::read_to_string("branding/agentgloss-icon-square.svg")?;
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default())?;
    let scale = 128.0 / tree.size().width();
    let mut pixmap = tiny_skia::Pixmap::new(128, 128).context("allocate icon pixmap")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    for (x, y) in [(0, 0), (127, 0), (0, 127), (127, 127)] {
        let pixel = pixmap.pixel(x, y).context("corner pixel")?;
        anyhow::ensure!(
            pixel.alpha() == 255 && (pixel.red(), pixel.green(), pixel.blue()) == (16, 24, 39),
            "icon corner ({x},{y}) is not opaque ink: rgba=({},{},{},{})",
            pixel.red(),
            pixel.green(),
            pixel.blue(),
            pixel.alpha()
        );
    }
    let png = pixmap.encode_png().context("encode icon PNG")?;
    std::fs::write("vscode/media/icon.png", png)?;
    println!("wrote vscode/media/icon.png (128x128, opaque ink corners)");
    Ok(())
}
