//! Image and Mermaid media pipeline.

use crate::config::{MermaidEngine, Protocol as ProtocolChoice, Theme};
use crate::tui::TuiOptions;
use anyhow::Context;
use image::{DynamicImage, ImageFormat};
use ratatui::layout::Size;
use ratatui_image::FontSize;
use ratatui_image::Resize;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::Protocol;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

pub type MediaKey = String;

const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;
const MAX_DIMENSION: u32 = 10_000;
const MAX_PIXELS: u64 = 40_000_000;
const CACHE_VERSION: u32 = 1;

struct MediaResult {
    key: MediaKey,
    result: Result<Protocol, String>,
}

pub struct MediaManager {
    picker: Picker,
    engine: MermaidEngine,
    theme: Theme,
    cache_dir: PathBuf,
    protocols: HashMap<MediaKey, Protocol>,
    errors: HashMap<MediaKey, String>,
    pending: HashSet<MediaKey>,
    hash_cache: HashMap<PathBuf, (SystemTime, u64, String)>,
    tx: Sender<MediaResult>,
    rx: Receiver<MediaResult>,
}

impl MediaManager {
    pub fn new(options: &TuiOptions) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            picker: Picker::halfblocks(),
            engine: options.mermaid_engine,
            theme: options.theme,
            cache_dir: options.cache_dir.clone(),
            protocols: HashMap::new(),
            errors: HashMap::new(),
            pending: HashSet::new(),
            hash_cache: HashMap::new(),
            tx,
            rx,
        }
    }

    /// Select a terminal graphics protocol from the environment. This is
    /// deliberately non-interactive: querying stdio can leave a reader thread
    /// competing with the TUI event loop for keyboard input.
    pub fn configure(&mut self, options: &TuiOptions) {
        let forced = match options.protocol {
            ProtocolChoice::Auto => None,
            ProtocolChoice::Kitty => Some(ProtocolType::Kitty),
            ProtocolChoice::Iterm2 => Some(ProtocolType::Iterm2),
            ProtocolChoice::Sixel => Some(ProtocolType::Sixel),
            ProtocolChoice::Halfblocks => Some(ProtocolType::Halfblocks),
        };
        let protocol = forced.unwrap_or_else(detect_protocol);
        self.picker = picker_for(protocol);
    }

    pub fn protocol_name(&self) -> String {
        format!("{:?}", self.picker.protocol_type())
    }

    pub fn get(&self, key: &str) -> Option<&Protocol> {
        self.protocols.get(key)
    }

    pub fn error(&self, key: &str) -> Option<&str> {
        self.errors.get(key).map(String::as_str)
    }

    pub fn poll(&mut self) {
        while let Ok(result) = self.rx.try_recv() {
            self.pending.remove(&result.key);
            match result.result {
                Ok(protocol) => {
                    self.protocols.insert(result.key, protocol);
                }
                Err(error) => {
                    self.errors.insert(result.key, error);
                }
            }
        }
    }

    /// Wait for a media key to finish; used by tests and preflight checks.
    pub fn wait_for(&mut self, key: &str, timeout: Duration) -> bool {
        let start = Instant::now();
        loop {
            self.poll();
            if self.protocols.contains_key(key) || self.errors.contains_key(key) {
                return true;
            }
            if start.elapsed() >= timeout {
                return false;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    pub fn ensure_image(
        &mut self,
        document_path: &Path,
        src: &str,
        target: (u16, u16),
    ) -> MediaKey {
        let target = (target.0.max(1), target.1.max(1));
        let stamp = self.image_stamp(document_path, src);
        let key = format!(
            "img:{}:{}:{}x{}:{}:{}",
            CACHE_VERSION,
            src,
            target.0,
            target.1,
            self.protocol_name(),
            stamp
        );
        if self.known(&key) {
            return key;
        }
        self.pending.insert(key.clone());
        let picker = self.picker.clone();
        let tx = self.tx.clone();
        let document_path = document_path.to_path_buf();
        let src = src.to_string();
        let job_key = key.clone();
        thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                load_image(&picker, &document_path, &src, target)
                    .and_then(|image| encode(&picker, image, target))
                    .map_err(|error| format!("{error:#}"))
            }))
            .unwrap_or_else(|_| Err("media worker panicked".to_string()));
            let _ = tx.send(MediaResult {
                key: job_key,
                result,
            });
        });
        key
    }

    pub fn ensure_mermaid(&mut self, source: &str, target: (u16, u16)) -> MediaKey {
        let target = (target.0.max(1), target.1.max(1));
        let source_hash = hex::encode(Sha256::digest(source.as_bytes()));
        let key = format!(
            "mermaid:{}:{:?}:{:?}:{}:{}x{}:{}",
            CACHE_VERSION,
            self.engine,
            self.theme,
            source_hash,
            target.0,
            target.1,
            self.protocol_name()
        );
        if self.known(&key) {
            return key;
        }
        self.pending.insert(key.clone());
        let picker = self.picker.clone();
        let tx = self.tx.clone();
        let source = source.to_string();
        let engine = self.engine;
        let theme = self.theme;
        let job_key = key.clone();
        thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                load_mermaid(&picker, engine, theme, &source, target)
                    .and_then(|image| encode(&picker, image, target))
                    .map_err(|error| format!("{error:#}"))
            }))
            .unwrap_or_else(|_| Err("media worker panicked".to_string()));
            let _ = tx.send(MediaResult {
                key: job_key,
                result,
            });
        });
        key
    }

    fn known(&self, key: &str) -> bool {
        self.protocols.contains_key(key)
            || self.errors.contains_key(key)
            || self.pending.contains(key)
    }

    /// Reuse the content hash while a file's metadata is unchanged, so the UI
    /// does not re-read and re-hash every visible image on every frame.
    fn image_stamp(&mut self, document_path: &Path, src: &str) -> String {
        let Ok(path) = resolve_image_path(document_path, src) else {
            return "missing".to_string();
        };
        let Ok(metadata) = std::fs::metadata(&path) else {
            return "missing".to_string();
        };
        let Ok(modified) = metadata.modified() else {
            return "missing".to_string();
        };
        let size = metadata.len();
        if size > MAX_FILE_BYTES {
            return format!("oversize:{size}");
        }
        if let Some((cached_modified, cached_size, cached_hash)) = self.hash_cache.get(&path)
            && *cached_modified == modified
            && *cached_size == size
        {
            return cached_hash.clone();
        }
        let Ok(bytes) = std::fs::read(&path) else {
            return "missing".to_string();
        };
        let hash = hex::encode(Sha256::digest(&bytes));
        self.hash_cache.insert(path, (modified, size, hash.clone()));
        hash
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Drop encoded protocol state after a width/height change.
    pub fn clear_encodings(&mut self) {
        self.protocols.clear();
        self.errors.clear();
    }
}

fn encode(picker: &Picker, image: DynamicImage, target: (u16, u16)) -> anyhow::Result<Protocol> {
    let size = Size::new(target.0, target.1);
    picker
        .new_protocol(image, size, Resize::Fit(None))
        .context("encode media for terminal")
}

fn load_image(
    picker: &Picker,
    document_path: &Path,
    src: &str,
    target: (u16, u16),
) -> anyhow::Result<DynamicImage> {
    let path = resolve_image_path(document_path, src)?;
    let metadata =
        std::fs::metadata(&path).with_context(|| format!("cannot stat {}", path.display()))?;
    if metadata.len() > MAX_FILE_BYTES {
        anyhow::bail!("image is larger than 50 MB ({} bytes)", metadata.len());
    }
    if is_svg(&path) {
        let svg = std::fs::read_to_string(&path)?;
        rasterize_svg(picker, &svg, target)
    } else {
        let mut reader = image::ImageReader::open(&path)
            .with_context(|| format!("cannot open {}", path.display()))?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(MAX_DIMENSION);
        limits.max_image_height = Some(MAX_DIMENSION);
        limits.max_alloc = Some(MAX_PIXELS * 4 + 64 * 1024 * 1024);
        reader.limits(limits);
        let image = reader
            .decode()
            .with_context(|| format!("cannot decode {}", path.display()))?;
        check_dimensions(&image)?;
        Ok(image)
    }
}

fn load_mermaid(
    picker: &Picker,
    engine: MermaidEngine,
    theme: Theme,
    source: &str,
    target: (u16, u16),
) -> anyhow::Result<DynamicImage> {
    let mmdc = command_exists("mmdc").then(|| std::ffi::OsStr::new("mmdc"));
    let svg = render_mermaid_svg(engine, theme, source, mmdc)?;
    rasterize_svg(picker, &svg, target)
}

fn render_mermaid_svg(
    engine: MermaidEngine,
    theme: Theme,
    source: &str,
    mmdc: Option<&std::ffi::OsStr>,
) -> anyhow::Result<String> {
    match engine {
        MermaidEngine::Source => anyhow::bail!("Mermaid rendering is disabled"),
        MermaidEngine::Rust => mermaid_rs_renderer::render(source)
            .map_err(|error| anyhow::anyhow!("Mermaid parse error: {error}")),
        MermaidEngine::Mmdc => match mmdc {
            Some(program) => render_with_mmdc_program(program, theme, source),
            None => anyhow::bail!("mmdc is not installed"),
        },
        MermaidEngine::Auto => match mermaid_rs_renderer::render(source) {
            Ok(svg) => Ok(svg),
            Err(rust_error) => match mmdc {
                Some(program) => render_with_mmdc_program(program, theme, source)
                    .with_context(|| format!("Rust Mermaid renderer failed: {rust_error}")),
                None => anyhow::bail!("Mermaid parse error: {rust_error}"),
            },
        },
    }
}

fn render_with_mmdc_program(
    program: &std::ffi::OsStr,
    theme: Theme,
    source: &str,
) -> anyhow::Result<String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("agentgloss-mmdc-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let input = dir.join("diagram.mmd");
    let output = dir.join("diagram.svg");
    std::fs::write(&input, source)?;
    let theme = match theme {
        Theme::Dark => "dark",
        Theme::Light => "default",
        Theme::Auto => "default",
    };
    let input_arg = input.to_string_lossy().to_string();
    let output_arg = output.to_string_lossy().to_string();
    let result = (|| -> anyhow::Result<String> {
        let output = Command::new(program)
            .args([
                "-i",
                &input_arg,
                "-o",
                &output_arg,
                "-b",
                "transparent",
                "-t",
                theme,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .context("run mmdc")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mmdc exited with {}: {}", output.status, stderr.trim());
        }
        std::fs::read_to_string(&output_arg).context("read mmdc SVG")
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

fn rasterize_svg(picker: &Picker, svg: &str, target: (u16, u16)) -> anyhow::Result<DynamicImage> {
    let options = usvg::Options {
        fontdb: font_database(),
        font_family: "DejaVu Sans".to_string(),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(svg, &options).context("parse SVG")?;
    let size = tree.size();
    if size.width() <= 0.0 || size.height() <= 0.0 {
        anyhow::bail!("SVG has zero size");
    }
    if size.width() > MAX_DIMENSION as f32 || size.height() > MAX_DIMENSION as f32 {
        anyhow::bail!(
            "SVG dimensions {}x{} exceed the 10,000px limit",
            size.width(),
            size.height()
        );
    }
    let font = picker.font_size();
    let max_width = (target.0 as f32 * font.width as f32).clamp(1.0, 4096.0);
    let max_height = (target.1 as f32 * font.height as f32).clamp(1.0, 4096.0);
    let mut scale = (max_width / size.width())
        .min(max_height / size.height())
        .max(0.01);
    let mut width = (size.width() * scale).ceil().max(1.0) as u32;
    let mut height = (size.height() * scale).ceil().max(1.0) as u32;
    if width.max(height) > 4096 {
        let factor = 4096.0 / width.max(height) as f32;
        width = ((width as f32 * factor).floor() as u32).max(1);
        height = ((height as f32 * factor).floor() as u32).max(1);
        scale *= factor;
    }
    let mut pixmap = tiny_skia::Pixmap::new(width, height).context("allocate SVG raster")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let png = pixmap.encode_png().context("encode SVG raster")?;
    image::load_from_memory_with_format(&png, ImageFormat::Png).context("decode SVG raster")
}

fn font_database() -> Arc<fontdb::Database> {
    static FONTS: OnceLock<Arc<fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut database = fontdb::Database::new();
            database.load_system_fonts();
            Arc::new(database)
        })
        .clone()
}

fn picker_for(protocol: ProtocolType) -> Picker {
    let font = terminal_font_size().unwrap_or_else(|| FontSize::new(10, 20));
    #[allow(deprecated)]
    let mut picker = Picker::from_fontsize(font);
    picker.set_protocol_type(protocol);
    picker
}

fn terminal_font_size() -> Option<FontSize> {
    let size = crossterm::terminal::window_size().ok()?;
    font_size_from_pixels(size.columns, size.rows, size.width, size.height)
}

/// Derive a cell's pixel size from `TIOCGWINSZ` metrics. Many terminals leave
/// the pixel fields zero or report bogus values; implausible results fall back
/// to the 10x20 default rather than distorting every image.
fn font_size_from_pixels(
    columns: u16,
    rows: u16,
    width_px: u16,
    height_px: u16,
) -> Option<FontSize> {
    if columns == 0 || rows == 0 || width_px == 0 || height_px == 0 {
        return None;
    }
    let width = width_px / columns;
    let height = height_px / rows;
    ((4..=64).contains(&width) && (6..=128).contains(&height)).then(|| FontSize::new(width, height))
}

fn detect_protocol() -> ProtocolType {
    detect_protocol_from(|name| std::env::var(name).ok())
}

fn detect_protocol_from(get: impl Fn(&str) -> Option<String>) -> ProtocolType {
    if get("KITTY_WINDOW_ID").is_some()
        || get("GHOSTTY_RESOURCES_DIR").is_some()
        || get("TERM_PROGRAM").is_some_and(|value| {
            value.eq_ignore_ascii_case("kitty") || value.eq_ignore_ascii_case("ghostty")
        })
    {
        return ProtocolType::Kitty;
    }
    if get("WEZTERM_EXECUTABLE").is_some()
        || get("TERM_PROGRAM").is_some_and(|value| {
            value.eq_ignore_ascii_case("wezterm") || value.eq_ignore_ascii_case("iterm.app")
        })
    {
        return ProtocolType::Iterm2;
    }
    if get("WT_SESSION").is_some()
        || get("TERM").is_some_and(|value| {
            value.contains("sixel") || value.contains("foot") || value.contains("mlterm")
        })
    {
        return ProtocolType::Sixel;
    }
    ProtocolType::Halfblocks
}

fn resolve_image_path(document_path: &Path, src: &str) -> anyhow::Result<PathBuf> {
    let lower = src.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        anyhow::bail!("remote images are not fetched");
    }
    let raw = PathBuf::from(src);
    let path = if raw.is_absolute() {
        raw
    } else {
        document_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(raw)
    };
    let path = path.canonicalize().unwrap_or_else(|_| path.clone());
    if !path.exists() {
        anyhow::bail!("image not found: {}", path.display());
    }
    Ok(path)
}

fn is_svg(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
}

fn check_dimensions(image: &DynamicImage) -> anyhow::Result<()> {
    if image.width() > MAX_DIMENSION || image.height() > MAX_DIMENSION {
        anyhow::bail!(
            "image dimensions {}x{} exceed the 10,000px limit",
            image.width(),
            image.height()
        );
    }
    if u64::from(image.width()) * u64::from(image.height()) > MAX_PIXELS {
        anyhow::bail!(
            "image dimensions {}x{} exceed the 40-megapixel limit",
            image.width(),
            image.height()
        );
    }
    Ok(())
}

fn command_exists(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| {
        let candidate = dir.join(name);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            candidate
                .metadata()
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        }
        #[cfg(not(unix))]
        {
            candidate.is_file()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Keys, Protocol as ProtocolChoice};

    fn options(dir: &Path) -> TuiOptions {
        TuiOptions {
            keys: Keys::Vim,
            mouse: false,
            protocol: ProtocolChoice::Halfblocks,
            theme: Theme::Dark,
            mermaid_engine: MermaidEngine::Rust,
            cache_dir: dir.join("cache"),
        }
    }

    fn tiny_png(path: &Path) {
        let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]));
        let mut file = std::fs::File::create(path).unwrap();
        image.write_to(&mut file, ImageFormat::Png).unwrap();
    }

    fn wait(media: &mut MediaManager, key: &str) -> bool {
        media.wait_for(key, Duration::from_secs(10))
    }

    #[cfg(unix)]
    fn fake_mmdc(dir: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let script = dir.join("mmdc");
        std::fs::write(
            &script,
            "#!/bin/sh\nout=''\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"-o\" ]; then shift; out=\"$1\"; fi\n  shift\ndone\nprintf '%s' '<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"><rect width=\"10\" height=\"10\"/></svg>' > \"$out\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        script
    }

    #[test]
    fn resolves_and_encodes_local_png() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("spec.md");
        std::fs::write(&doc, "![x](red.png)\n").unwrap();
        tiny_png(&dir.path().join("red.png"));
        let mut media = MediaManager::new(&options(dir.path()));
        let key = media.ensure_image(&doc, "red.png", (20, 10));
        assert!(wait(&mut media, &key), "{:?}", media.error(&key));
        assert!(media.get(&key).is_some());
    }

    #[test]
    fn rejects_remote_and_missing_images() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("spec.md");
        std::fs::write(&doc, "").unwrap();
        let mut media = MediaManager::new(&options(dir.path()));
        let remote = media.ensure_image(&doc, "https://example.com/a.png", (20, 10));
        assert!(wait(&mut media, &remote));
        assert!(media.get(&remote).is_none());
        assert!(media.error(&remote).unwrap().contains("remote"));
        let missing = media.ensure_image(&doc, "missing.png", (20, 10));
        assert!(wait(&mut media, &missing));
        assert!(media.error(&missing).unwrap().contains("not found"));
    }

    #[test]
    fn rasterizes_svg() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("spec.md");
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50"><rect width="100" height="50" fill="#ff0000"/></svg>"##;
        std::fs::write(dir.path().join("box.svg"), svg).unwrap();
        let mut media = MediaManager::new(&options(dir.path()));
        let key = media.ensure_image(&doc, "box.svg", (20, 10));
        assert!(wait(&mut media, &key), "{:?}", media.error(&key));
        assert!(media.get(&key).is_some());
    }

    #[test]
    fn renders_a_simple_mermaid_diagram() {
        let dir = tempfile::tempdir().unwrap();
        let mut media = MediaManager::new(&options(dir.path()));
        let key = media.ensure_mermaid("flowchart LR\nA-->B\n", (30, 15));
        assert!(wait(&mut media, &key), "{:?}", media.error(&key));
        assert!(media.get(&key).is_some());
    }

    #[test]
    fn renders_sequence_and_class_diagrams() {
        let dir = tempfile::tempdir().unwrap();
        let mut media = MediaManager::new(&options(dir.path()));
        for source in [
            "sequenceDiagram\n  Alice->>Bob: Hi\n  Bob-->>Alice: Hello\n",
            "classDiagram\n  class Animal\n  Animal <|-- Dog\n",
        ] {
            let key = media.ensure_mermaid(source, (30, 15));
            assert!(wait(&mut media, &key), "{:?}", media.error(&key));
            assert!(media.get(&key).is_some());
        }
    }

    #[test]
    fn failed_mermaid_render_is_cached() {
        let dir = tempfile::tempdir().unwrap();
        let mut media = MediaManager::new(&options(dir.path()));
        let key = media.ensure_mermaid("not a diagram\n", (30, 15));
        assert!(wait(&mut media, &key));
        assert!(media.error(&key).is_some());
        let again = media.ensure_mermaid("not a diagram\n", (30, 15));
        assert_eq!(key, again);
        assert!(media.error(&again).is_some());
    }

    #[cfg(unix)]
    #[test]
    fn fake_mmdc_program_returns_svg() {
        let dir = tempfile::tempdir().unwrap();
        let script = fake_mmdc(dir.path());
        let svg =
            render_with_mmdc_program(script.as_os_str(), Theme::Dark, "flowchart LR\nA-->B\n")
                .unwrap();
        assert!(svg.contains("<svg"));
    }

    #[cfg(unix)]
    #[test]
    fn auto_engine_falls_back_to_mmdc_when_the_rust_parser_fails() {
        let dir = tempfile::tempdir().unwrap();
        let script = fake_mmdc(dir.path());
        let svg = render_mermaid_svg(
            MermaidEngine::Auto,
            Theme::Dark,
            "not a diagram\n",
            Some(script.as_os_str()),
        )
        .unwrap();
        assert!(svg.contains("<svg"));
        assert!(
            render_mermaid_svg(MermaidEngine::Auto, Theme::Dark, "not a diagram\n", None).is_err()
        );
    }

    #[test]
    fn decodes_webp_and_preserves_pixel_data() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("spec.md");
        let path = dir.path().join("blue.webp");
        let image = image::RgbImage::from_pixel(3, 2, image::Rgb([0, 0, 255]));
        image.save_with_format(&path, ImageFormat::WebP).unwrap();
        let mut media = MediaManager::new(&options(dir.path()));
        let key = media.ensure_image(&doc, "blue.webp", (20, 10));
        assert!(wait(&mut media, &key), "{:?}", media.error(&key));
        assert!(media.get(&key).is_some());

        let loaded = load_image(&media.picker, &doc, "blue.webp", (20, 10)).unwrap();
        assert_eq!((loaded.width(), loaded.height()), (3, 2));
        assert_eq!(loaded.to_rgba8().get_pixel(0, 0).0, [0, 0, 255, 255]);
    }

    #[test]
    fn enforces_file_and_pixel_limits() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("spec.md");
        let big = dir.path().join("big.png");
        std::fs::File::create(&big)
            .unwrap()
            .set_len(MAX_FILE_BYTES + 1)
            .unwrap();
        let mut media = MediaManager::new(&options(dir.path()));
        let stamp = media.image_stamp(&doc, "big.png");
        assert!(stamp.starts_with("oversize:"), "{stamp}");
        let key = media.ensure_image(&doc, "big.png", (20, 10));
        assert!(wait(&mut media, &key));
        assert!(media.error(&key).unwrap().contains("50 MB"));

        let huge = DynamicImage::new_luma8(8000, 5001);
        assert!(check_dimensions(&huge).is_err());
        assert!(check_dimensions(&DynamicImage::new_luma8(10_001, 1)).is_err());
    }

    #[test]
    fn explicit_protocol_overrides_detection() {
        let dir = tempfile::tempdir().unwrap();
        let mut opts = options(dir.path());
        opts.protocol = ProtocolChoice::Kitty;
        let mut media = MediaManager::new(&opts);
        media.configure(&opts);
        assert_eq!(media.protocol_name(), "Kitty");
        opts.protocol = ProtocolChoice::Iterm2;
        media.configure(&opts);
        assert_eq!(media.protocol_name(), "Iterm2");
        opts.protocol = ProtocolChoice::Sixel;
        media.configure(&opts);
        assert_eq!(media.protocol_name(), "Sixel");
        opts.protocol = ProtocolChoice::Halfblocks;
        media.configure(&opts);
        assert_eq!(media.protocol_name(), "Halfblocks");
    }

    #[test]
    fn mermaid_source_engine_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let mut opts = options(dir.path());
        opts.mermaid_engine = MermaidEngine::Source;
        let mut media = MediaManager::new(&opts);
        let key = media.ensure_mermaid("flowchart LR\nA-->B\n", (30, 15));
        assert!(wait(&mut media, &key));
        assert!(media.get(&key).is_none());
        assert!(media.error(&key).unwrap().contains("disabled"));
    }

    #[test]
    fn cache_key_changes_with_content_and_target_size() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("spec.md");
        let image_path = dir.path().join("red.png");
        tiny_png(&image_path);
        let mut media = MediaManager::new(&options(dir.path()));
        let first = media.ensure_image(&doc, "red.png", (20, 10));
        assert!(wait(&mut media, &first));
        assert!(media.get(&first).is_some());
        let duplicate = media.ensure_image(&doc, "red.png", (20, 10));
        assert_eq!(first, duplicate);

        let blue = image::RgbaImage::from_pixel(2, 2, image::Rgba([0, 0, 255, 255]));
        blue.save(&image_path).unwrap();
        let changed = media.ensure_image(&doc, "red.png", (20, 10));
        assert_ne!(first, changed);
        assert!(wait(&mut media, &changed));

        let resized = media.ensure_image(&doc, "red.png", (30, 10));
        assert_ne!(changed, resized);
        assert!(wait(&mut media, &resized));

        media.clear_encodings();
        assert!(media.get(&resized).is_none());
    }

    #[test]
    fn absolute_image_paths_and_raster_formats() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("spec.md");
        let mut media = MediaManager::new(&options(dir.path()));
        let formats = [
            ("a.jpg", ImageFormat::Jpeg),
            ("a.bmp", ImageFormat::Bmp),
            ("a.gif", ImageFormat::Gif),
        ];
        for (name, format) in formats {
            let path = dir.path().join(name);
            let image = image::RgbImage::from_pixel(2, 2, image::Rgb([10, 20, 30]));
            image.save_with_format(&path, format).unwrap();
            let key = media.ensure_image(&doc, &path.to_string_lossy(), (20, 10));
            assert!(wait(&mut media, &key), "{name}: {:?}", media.error(&key));
            assert!(media.get(&key).is_some());
        }
    }

    #[test]
    fn svg_raster_has_visible_pixels() {
        let dir = tempfile::tempdir().unwrap();
        let mut media = MediaManager::new(&options(dir.path()));
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><rect width="20" height="20" fill="#ff0000"/></svg>"##;
        let image = rasterize_svg(&media.picker, svg, (20, 10)).unwrap();
        assert!(image.width() > 0 && image.height() > 0);
        assert!(
            image
                .to_rgba8()
                .pixels()
                .any(|pixel| pixel.0[3] > 0 && pixel.0[0] > 0)
        );
        media.clear_encodings();
    }

    #[test]
    fn derives_font_size_from_terminal_pixels_when_plausible() {
        let font = font_size_from_pixels(240, 60, 1920, 1080).unwrap();
        assert_eq!((font.width, font.height), (8, 18));
        assert!(font_size_from_pixels(0, 60, 1920, 1080).is_none());
        assert!(font_size_from_pixels(240, 60, 0, 0).is_none());
        assert!(font_size_from_pixels(240, 60, 240, 60).is_none());
        assert!(font_size_from_pixels(2, 2, u16::MAX, u16::MAX).is_none());
    }

    #[test]
    fn cache_dir_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let media = MediaManager::new(&options(dir.path()));
        assert!(media.cache_dir().ends_with("cache"));
    }

    #[test]
    fn rejects_oversized_dimensions() {
        let image = DynamicImage::new_rgba8(MAX_DIMENSION + 1, 1);
        assert!(check_dimensions(&image).is_err());
    }

    #[test]
    fn detects_protocols_from_environment() {
        let kitty = detect_protocol_from(|key| (key == "KITTY_WINDOW_ID").then(|| "1".into()));
        assert!(matches!(kitty, ProtocolType::Kitty));
        let wez = detect_protocol_from(|key| (key == "WEZTERM_EXECUTABLE").then(|| "1".into()));
        assert!(matches!(wez, ProtocolType::Iterm2));
        let wt = detect_protocol_from(|key| (key == "WT_SESSION").then(|| "1".into()));
        assert!(matches!(wt, ProtocolType::Sixel));
        let foot = detect_protocol_from(|key| (key == "TERM").then(|| "xterm-foot".into()));
        assert!(matches!(foot, ProtocolType::Sixel));
        let plain = detect_protocol_from(|_| None);
        assert!(matches!(plain, ProtocolType::Halfblocks));
    }

    #[test]
    #[ignore = "diagnostic; run with --ignored --nocapture"]
    fn diag_mermaid_rasters() {
        let out = Path::new("/tmp/agentgloss-mermaid");
        std::fs::create_dir_all(out).unwrap();
        let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mermaid");
        let picker = picker_for(ProtocolType::Halfblocks);
        let font = picker.font_size();
        let mut paths: Vec<_> = std::fs::read_dir(fixture_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "mmd"))
            .collect();
        paths.sort();
        for path in paths {
            let source = std::fs::read_to_string(&path).unwrap();
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();
            match render_mermaid_svg(MermaidEngine::Rust, Theme::Dark, &source, None) {
                Ok(svg) => {
                    std::fs::write(out.join(format!("{stem}.svg")), &svg).unwrap();
                    let options = usvg::Options {
                        fontdb: font_database(),
                        ..usvg::Options::default()
                    };
                    let svg_size = usvg::Tree::from_str(&svg, &options)
                        .ok()
                        .map(|tree| tree.size());
                    match rasterize_svg(&picker, &svg, (80, 10)) {
                        Ok(image) => {
                            let (width, height) = (image.width(), image.height());
                            let rgba = image.to_rgba8();
                            let mut min = (u32::MAX, u32::MAX);
                            let mut max = (0u32, 0u32);
                            let mut ink = 0u32;
                            for (x, y, pixel) in rgba.enumerate_pixels() {
                                if pixel[3] > 0 {
                                    ink += 1;
                                    min = (min.0.min(x), min.1.min(y));
                                    max = (max.0.max(x), max.1.max(y));
                                }
                            }
                            image.save(out.join(format!("{stem}.png"))).unwrap();
                            println!(
                                "{stem}: svg={:?} raster={width}x{height} ink={ink} bbox={min:?}..{max:?}",
                                svg_size.map(|size| (size.width(), size.height()))
                            );
                        }
                        Err(error) => println!("{stem}: RASTER ERROR {error:#}"),
                    }
                }
                Err(error) => println!("{stem}: MERMAID ERROR {error:#}"),
            }
        }
        println!("picker font={}x{}", font.width, font.height);
    }
}
