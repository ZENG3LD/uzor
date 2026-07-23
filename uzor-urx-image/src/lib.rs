//! URX image registry — opaque `ImageId` -> decoded premultiplied
//! RGBA8 bytes, shared canonical store consumed by every URX backend.
//!
//! Relocated VERBATIM from `uzor-urx-cpu/src/image_reg.rs` (URX Wave 4
//! design, `docs/uzor-engines/plans/urx-wave4-vello-parity-design-2026-07-25.md`
//! §0.4/§10 Commit 1) — mechanical move, zero logic change, same
//! discipline as Wave 2's `uzor-urx-glyph` extraction. `uzor-urx-cpu`'s
//! own `src/image_reg.rs` becomes a thin `pub use uzor_urx_image::*;`
//! re-export shim so `crate::image_reg::lookup_image` call sites in
//! `image_draw.rs`/`backend.rs` don't change at all.
//!
//! **Why this needed to move at all** (§0.4's architectural finding):
//! if `uzor-urx-wgpu` built its OWN independent image registry (its own
//! `next_id` counter, its own `Vec`), the SAME logical image registered
//! once by a producer would get TWO DIFFERENT `ImageId` values
//! depending on which backend's registry assigned it — breaking the
//! "one Scene, N backends" model this whole family is built on. Images
//! need the identical shared-crate treatment Wave 2 already gave
//! glyphs, for identical reasons.
//!
//! On CPU there is no atlas — every image is just a `Vec<u8>` blitted
//! at draw time. Atlas packing is a GPU concern (`uzor-urx-wgpu`'s own
//! `image_cache.rs`, Wave 4 Commit 2+). Process-global registry;
//! Arc-keyed handles so registry mutation doesn't tear cached draws.
//!
//! Memory bound: caller responsible (clear handles when done). LRU
//! eviction could be layered on later — for now consumers control
//! lifetimes explicitly via `unregister_image`.
//!
//! Wave 4 status at close (Commits 2-6): `uzor-urx-wgpu`'s
//! `image_cache::NativeImageCache` (non-optional dependency on this
//! crate) resolves `ImageId`s registered here into per-image GPU
//! textures; `DrawCommand::Image` (`uzor-urx-core::scene`) is now
//! rendered for real on both `uzor-urx-cpu` (`image_draw.rs`, already
//! live) and `uzor-urx-wgpu` (`encode.rs::encode_image` +
//! `pipelines::image`, Commit 3) against this SAME registry — verified
//! end-to-end by the parity fixture `image_axis_aligned_and_rotated`
//! (`uzor-urx-wgpu/tests/{fixtures,parity}.rs`, Commit 5).

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
/// CPU/Hybrid/WGPU backends.
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

#[cfg(test)]
mod tests {
    use super::*;

    // The pre-extraction `uzor-urx-cpu/src/image_reg.rs` carried no
    // `#[cfg(test)]` tests of its own (its only exercise was via
    // `uzor-urx-cpu/tests/image.rs`'s end-to-end
    // registry+`image_draw.rs`+`Pixmap` rendering flow — that coverage
    // stays there, since this crate has no rendering dependency at
    // all, by design). These are new, minimal, registry-only smoke
    // tests added alongside the Wave 4 Commit 1 relocation so this
    // crate has SOME direct coverage of its own, rather than reporting
    // a vacuous 0/0 test run.
    //
    // Deliberately NEVER call `_clear_global_for_tests()` here — `cargo
    // test` runs a crate's tests in PARALLEL by default, and that
    // function wipes EVERY entry in the shared process-global registry,
    // not just the calling test's own; a concurrently-running test's
    // still-in-flight `lookup_image` could spuriously miss. Every test
    // below is self-contained instead: it registers its OWN id
    // (`next_id` is monotonic, so two concurrent `register_image` calls
    // never collide) and unregisters only that id when done.

    fn sample(w: u32, h: u32, fill: [u8; 4]) -> ImageData {
        let mut bytes = vec![0u8; (w * h * 4) as usize];
        for px in bytes.chunks_exact_mut(4) {
            px.copy_from_slice(&fill);
        }
        ImageData::from_raw_premul(w, h, bytes).expect("size matches by construction")
    }

    #[test]
    fn register_lookup_unregister_round_trip() {
        let id = register_image(sample(4, 4, [10, 20, 30, 255]));
        let got = lookup_image(id).expect("just-registered id must resolve");
        assert_eq!((got.width, got.height), (4, 4));
        assert_eq!(&got.bytes[0..4], &[10, 20, 30, 255]);
        assert!(unregister_image(id), "unregister must report the id existed");
        assert!(lookup_image(id).is_none(), "a lookup after unregister must miss");
        assert!(!unregister_image(id), "unregistering an already-removed id must report false, not panic");
    }

    #[test]
    fn lookup_of_never_registered_id_is_none_not_a_panic() {
        // `next_id` starts at 1 and only grows — 0 is never issued by
        // `ImageRegistry::register`, so this id is guaranteed unused
        // regardless of how many other tests have registered images in
        // this same process.
        assert!(lookup_image(ImageId(0)).is_none());
    }

    #[test]
    fn distinct_registrations_get_distinct_ids() {
        let a = register_image(sample(2, 2, [1, 1, 1, 255]));
        let b = register_image(sample(2, 2, [2, 2, 2, 255]));
        assert_ne!(a, b, "two separate register_image calls must never collide on the same ImageId");
        unregister_image(a);
        unregister_image(b);
    }

    #[test]
    fn from_raw_premul_rejects_size_mismatch() {
        let err = ImageData::from_raw_premul(4, 4, vec![0u8; 10]).unwrap_err();
        assert!(matches!(err, ImageError::SizeMismatch { expected: 64, got: 10 }));
    }

    #[test]
    fn from_raw_straight_premultiplies_in_place() {
        // Straight [200, 100, 50, 128] at ~50% alpha must premultiply
        // down (not stay at the straight values).
        let data = ImageData::from_raw_straight(1, 1, vec![200, 100, 50, 128]).unwrap();
        assert!(data.bytes[0] < 150 && data.bytes[0] > 90, "R must be scaled by alpha, got {}", data.bytes[0]);
        assert_eq!(data.bytes[3], 128, "alpha channel itself is untouched by premultiply");
    }
}
