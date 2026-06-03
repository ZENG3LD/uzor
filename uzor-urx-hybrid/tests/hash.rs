//! fnv1a content_hash sanity. No GPU needed.

use uzor_urx_cpu::Pixmap;
use uzor_urx_hybrid::region_tex::fnv1a_64;

#[test]
fn empty_slice_has_known_value() {
    // fnv1a-64 offset basis.
    assert_eq!(fnv1a_64(&[]), 0xcbf29ce484222325);
}

#[test]
fn identical_pixmaps_hash_identically() {
    let mut a = Pixmap::new(64, 32);
    let mut b = Pixmap::new(64, 32);
    a.fill([100, 50, 25, 200]);
    b.fill([100, 50, 25, 200]);
    assert_eq!(fnv1a_64(a.pixels()), fnv1a_64(b.pixels()));
}

#[test]
fn one_byte_diff_changes_hash() {
    let mut a = Pixmap::new(64, 32);
    a.fill([100, 50, 25, 200]);
    let h1 = fnv1a_64(a.pixels());
    a.set_pixel(0, 0, [0, 0, 0, 0]);
    let h2 = fnv1a_64(a.pixels());
    assert_ne!(h1, h2);
}

#[test]
fn hash_doesnt_depend_on_size_if_same_content() {
    let a = Pixmap::new(0, 0);
    // Empty hash is the basis.
    assert_eq!(fnv1a_64(a.pixels()), 0xcbf29ce484222325);
}
