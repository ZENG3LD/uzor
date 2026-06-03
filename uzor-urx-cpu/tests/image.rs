//! Image registry + draw — covers 1:1 blit, scaled bilinear,
//! src_rect crop, missing-id fallback.

use uzor_urx_core::math::{Affine, Rect};
use uzor_urx_core::scene::{DrawCommand, Scene};
use uzor_urx_cpu::{CpuBackend, ImageData, Pixmap, register_image, unregister_image};

fn s() -> CpuBackend { CpuBackend::new() }

fn red_blue_strip(w: u32, h: u32) -> ImageData {
    // Left half red, right half blue, straight alpha 255.
    let mut bytes = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            if x < w / 2 {
                bytes[i..i+4].copy_from_slice(&[255, 0, 0, 255]);
            } else {
                bytes[i..i+4].copy_from_slice(&[0, 0, 255, 255]);
            }
        }
    }
    ImageData::from_raw_straight(w, h, bytes).unwrap()
}

#[test]
fn image_blit_1to1_preserves_pixels() {
    let id = register_image(red_blue_strip(20, 10));
    let mut p = Pixmap::new(40, 40);
    let mut scene = Scene::new();
    scene.push(DrawCommand::Image {
        src: id,
        src_rect: None,
        dest: Rect::new(10.0, 15.0, 30.0, 25.0), // 20×10 at (10,15)
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    // Left half = red. Centre of left half: (15, 20).
    let l = p.get_pixel(15, 20);
    assert!(l[0] > 200 && l[2] < 30, "left half red, got {:?}", l);
    // Right half = blue.
    let r = p.get_pixel(25, 20);
    assert!(r[2] > 200 && r[0] < 30, "right half blue, got {:?}", r);
    // Outside dest — empty.
    assert_eq!(p.get_pixel(5, 5), [0, 0, 0, 0]);
    unregister_image(id);
}

#[test]
fn image_scale_2x_bilinear_smooths_seam() {
    let id = register_image(red_blue_strip(4, 4));
    let mut p = Pixmap::new(40, 40);
    let mut scene = Scene::new();
    scene.push(DrawCommand::Image {
        src: id,
        src_rect: None,
        dest: Rect::new(5.0, 5.0, 37.0, 37.0), // 4×4 stretched to 32×32
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    // Around the seam (centre of dest = x=21), bilinear should produce
    // a non-pure colour blend somewhere in the transition.
    let mut found_blend = false;
    for x in 18_u32..=24 {
        let c = p.get_pixel(x, 20);
        if c[0] > 20 && c[2] > 20 {
            found_blend = true;
            break;
        }
    }
    assert!(found_blend, "bilinear must produce a R+B blend around the seam");
    unregister_image(id);
}

#[test]
fn image_src_rect_crops_correctly() {
    let id = register_image(red_blue_strip(20, 10));
    let mut p = Pixmap::new(20, 20);
    let mut scene = Scene::new();
    // Take only the right half (blue) of the source, draw at (2,5)-(12,15).
    scene.push(DrawCommand::Image {
        src: id,
        src_rect: Some(Rect::new(10.0, 0.0, 20.0, 10.0)),
        dest:     Rect::new(2.0,  5.0, 12.0, 15.0),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    let c = p.get_pixel(7, 10);
    assert!(c[2] > 200 && c[0] < 30, "cropped right-half should be blue, got {:?}", c);
    unregister_image(id);
}

#[test]
fn image_unknown_id_does_not_panic() {
    use uzor_urx_core::scene::ImageId;
    let mut p = Pixmap::new(10, 10);
    let mut scene = Scene::new();
    scene.push(DrawCommand::Image {
        src: ImageId(99999_999),
        src_rect: None,
        dest: Rect::new(0.0, 0.0, 10.0, 10.0),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    assert_eq!(p.get_pixel(5, 5), [0, 0, 0, 0]);
}

#[test]
fn image_premul_alpha_blend_on_existing_pixels() {
    // 50%-alpha green over solid red.
    let w = 4_u32; let h = 4_u32;
    let mut bytes = vec![0u8; (w * h * 4) as usize];
    for px in bytes.chunks_exact_mut(4) {
        px.copy_from_slice(&[0, 255, 0, 128]); // 50% green
    }
    let id = register_image(ImageData::from_raw_straight(w, h, bytes).unwrap());
    let mut p = Pixmap::new(20, 20);
    let mut scene = Scene::new();
    // Solid red background.
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 20.0, 20.0),
        radii: None,
        brush: uzor_urx_core::math::Brush::Solid(uzor_urx_core::math::Color::rgba8(255, 0, 0, 255)),
        transform: Affine::IDENTITY,
    });
    // Half-alpha green image on top.
    scene.push(DrawCommand::Image {
        src: id,
        src_rect: None,
        dest: Rect::new(4.0, 4.0, 16.0, 16.0),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    let c = p.get_pixel(10, 10);
    // Premul src-over: dst = src + dst*(1-src_a)
    // src_premul: [0, 128, 0, 128]; src_a=128
    // dst_after = src + dst*(255-128)/255 = [0, 128, 0, 128] + [255, 0, 0, 255]*0.498
    //           ≈ [127, 128, 0, 255]
    assert!(c[0] > 100 && c[0] < 160, "blended red ≈127, got {:?}", c);
    assert!(c[1] > 100 && c[1] < 160, "blended green ≈128, got {:?}", c);
    unregister_image(id);
}
