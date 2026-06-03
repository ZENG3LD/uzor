//! Image registry — opaque `ImageId` → decoded premul RGBA8 pixmap.
//!
//! On CPU there is no atlas — every image is just a `Vec<u8>` blitted
//! at draw time. Atlas packing is a GPU concern (see urx-hybrid /
//! urx-wgpu). Process-global registry; Arc-keyed handles so registry
//! mutation doesn't tear cached draws.
//!
//! Memory bound: caller responsible (clear handles when done). LRU
//! eviction could be layered on later — for now consumers control
//! lifetimes explicitly via `unregister`.

use std::sync::{Arc, Mutex};

use uzor_urx_core::scene::ImageId;

/// Decoded image — premultiplied RGBA8 row-major, no padding.
#[derive(Debug)]
pub struct ImageData {
    pub width:  u32,
    pub height: u32,
    pub bytes:  Vec<u8>, // length == w * h * 4, channels: R,G,B,A premultiplied
}

impl ImageData {
    pub fn from_raw_premul(width: u32, height: u32, bytes: Vec<u8>) -> Result<Self, ImageError> {
        if (width as usize) * (height as usize) * 4 != bytes.len() {
            return Err(ImageError::SizeMismatch {
                expected: (width as usize) * (height as usize) * 4,
                got: bytes.len(),
            });
        }
        Ok(Self { width, height, bytes })
    }

    /// Premultiply straight-alpha RGBA8 bytes in-place, then wrap.
    pub fn from_raw_straight(width: u32, height: u32, mut bytes: Vec<u8>) -> Result<Self, ImageError> {
        if (width as usize) * (height as usize) * 4 != bytes.len() {
            return Err(ImageError::SizeMismatch {
                expected: (width as usize) * (height as usize) * 4,
                got: bytes.len(),
            });
        }
        for px in bytes.chunks_exact_mut(4) {
            let a = px[3] as u32;
            px[0] = ((px[0] as u32 * a + 127) / 255) as u8;
            px[1] = ((px[1] as u32 * a + 127) / 255) as u8;
            px[2] = ((px[2] as u32 * a + 127) / 255) as u8;
        }
        Ok(Self { width, height, bytes })
    }
}

#[derive(Debug)]
pub enum ImageError {
    SizeMismatch { expected: usize, got: usize },
    #[cfg(feature = "image-decode")]
    Decode(String),
}

pub type ImageDataArc = Arc<ImageData>;

/// Process-global image registry. Single source of truth for
/// CPU/Hybrid backends.
pub struct ImageRegistry {
    next_id: u64,
    entries: Vec<(ImageId, ImageDataArc)>,
}

impl Default for ImageRegistry {
    fn default() -> Self {
        Self { next_id: 1, entries: Vec::new() }
    }
}

impl ImageRegistry {
    pub fn register(&mut self, data: ImageData) -> ImageId {
        let id = ImageId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        self.entries.push((id, Arc::new(data)));
        id
    }

    pub fn unregister(&mut self, id: ImageId) -> bool {
        if let Some(pos) = self.entries.iter().position(|(eid, _)| *eid == id) {
            self.entries.swap_remove(pos);
            true
        } else { false }
    }

    pub fn get(&self, id: ImageId) -> Option<ImageDataArc> {
        self.entries.iter().find(|(eid, _)| *eid == id).map(|(_, d)| d.clone())
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
}

static GLOBAL: Mutex<Option<ImageRegistry>> = Mutex::new(None);

fn with_global<R>(f: impl FnOnce(&mut ImageRegistry) -> R) -> R {
    let mut g = GLOBAL.lock().unwrap();
    let reg = g.get_or_insert_with(ImageRegistry::default);
    f(reg)
}

/// Register an image into the process-global registry. Returns the
/// opaque `ImageId` that scenes can reference via `DrawCommand::Image`.
pub fn register_image(data: ImageData) -> ImageId {
    with_global(|reg| reg.register(data))
}

/// Look up the decoded bytes for an `ImageId`. Returns `None` if the
/// id was never registered or has been unregistered.
pub fn lookup_image(id: ImageId) -> Option<ImageDataArc> {
    with_global(|reg| reg.get(id))
}

pub fn unregister_image(id: ImageId) -> bool {
    with_global(|reg| reg.unregister(id))
}

#[doc(hidden)]
pub fn _clear_global_for_tests() {
    let mut g = GLOBAL.lock().unwrap();
    *g = None;
}

#[cfg(feature = "image-decode")]
pub fn decode_and_register(bytes: &[u8]) -> Result<ImageId, ImageError> {
    use image::ImageReader;
    let img = ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| ImageError::Decode(e.to_string()))?
        .decode()
        .map_err(|e| ImageError::Decode(e.to_string()))?;
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let raw = rgba.into_raw();
    let data = ImageData::from_raw_straight(w, h, raw)?;
    Ok(register_image(data))
}
