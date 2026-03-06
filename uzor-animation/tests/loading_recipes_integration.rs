//! Integration tests for loading recipes
//!
//! Validates that all loading animation presets compile and have reasonable defaults.

use uzor_animation::recipes::loading::*;

#[test]
fn test_material_circular_preset() {
    let anim = material_circular();
    assert!(anim.is_infinite());
    assert_eq!(anim.duration_ms(), 2000);
    assert_eq!(anim.element_count(), 1);
}

#[test]
fn test_material_linear_preset() {
    let anim = material_linear();
    assert!(anim.is_infinite());
    assert_eq!(anim.duration_ms(), 2000);
}

#[test]
fn test_three_bounce_dots_preset() {
    let anim = three_bounce_dots();
    assert_eq!(anim.element_count(), 3);
    assert!(anim.is_infinite());
    assert_eq!(anim.duration_ms(), 1200);
}

#[test]
fn test_wave_bars_preset() {
    let anim = wave_bars();
    assert_eq!(anim.element_count(), 5);
    assert!(anim.is_infinite());
    assert_eq!(anim.duration_ms(), 1200);
}

#[test]
fn test_pulse_ring_preset() {
    let anim = pulse_ring();
    assert!(anim.is_infinite());
    assert_eq!(anim.duration_ms(), 1500);
}

#[test]
fn test_shimmer_preset() {
    let anim = shimmer();
    assert!(anim.is_infinite());
    assert_eq!(anim.duration_ms(), 1500);
}

#[test]
fn test_fading_dots_circle_preset() {
    let anim = fading_dots_circle();
    assert_eq!(anim.element_count(), 8);
    assert!(anim.is_infinite());
}

#[test]
fn test_ios_spinner_preset() {
    let anim = ios_spinner();
    assert_eq!(anim.element_count(), 12);
    assert!(anim.is_infinite());
}

#[test]
fn test_progress_ring_determinate_preset() {
    let anim = progress_ring_determinate();
    assert!(!anim.is_infinite());
    assert_eq!(anim.duration_ms(), 350);
}

#[test]
fn test_progress_bar_determinate_preset() {
    let anim = progress_bar_determinate();
    assert!(!anim.is_infinite());
    assert_eq!(anim.duration_ms(), 400);
}

#[test]
fn test_skeleton_pulse_preset() {
    let anim = skeleton_pulse();
    assert!(anim.is_infinite());
    assert_eq!(anim.duration_ms(), 2000);
}

#[test]
fn test_bouncing_ball_preset() {
    let anim = bouncing_ball();
    assert!(anim.is_infinite());
    assert_eq!(anim.duration_ms(), 600);
}

#[test]
fn test_ripple_rings_preset() {
    let anim = ripple_rings();
    assert_eq!(anim.element_count(), 4);
    assert!(anim.is_infinite());
    assert_eq!(anim.duration_ms(), 4000);
}

#[test]
fn test_wave_dots_vertical_preset() {
    let anim = wave_dots_vertical();
    assert_eq!(anim.element_count(), 4);
    assert!(anim.is_infinite());
    assert_eq!(anim.duration_ms(), 1400);
}

#[test]
fn test_path_draw_preset() {
    let path_len = 500.0;
    let anim = path_draw(path_len);
    assert!(anim.is_infinite());
    assert_eq!(anim.duration_ms(), 3000);
}

#[test]
fn test_progress_with_shimmer_preset() {
    let anim = progress_with_shimmer();
    assert!(!anim.is_infinite());
    assert_eq!(anim.duration_ms(), 400);
}

#[test]
fn test_basic_spinner_preset() {
    let anim = basic_spinner();
    assert!(anim.is_infinite());
    assert_eq!(anim.duration_ms(), 2000);
}

#[test]
fn test_spinkit_chase_preset() {
    let anim = spinkit_chase();
    assert_eq!(anim.element_count(), 6);
    assert!(anim.is_infinite());
}

#[test]
fn test_double_bounce_preset() {
    let anim = double_bounce();
    assert_eq!(anim.element_count(), 2);
    assert!(anim.is_infinite());
}

#[test]
fn test_spinner_builder() {
    let anim = SpinnerBuilder::new()
        .duration_ms(3000)
        .easing(uzor_animation::Easing::EaseInOutQuad)
        .build();

    assert_eq!(anim.duration_ms(), 3000);
    assert!(anim.is_infinite());
}

#[test]
fn test_pulse_dots_builder() {
    let anim = PulseDotsBuilder::new()
        .count(5)
        .stagger_delay_ms(150)
        .scale_range(0.2, 1.0)
        .build();

    assert_eq!(anim.element_count(), 5);
}

#[test]
fn test_bar_wave_builder() {
    let anim = BarWaveBuilder::new()
        .count(7)
        .stagger_delay_ms(80)
        .build();

    assert_eq!(anim.element_count(), 7);
}

#[test]
fn test_progress_ring_builder() {
    let anim = ProgressRingBuilder::new()
        .radius(60.0)
        .stroke_width(10.0)
        .build();

    assert!(!anim.is_infinite());
}

#[test]
fn test_shimmer_builder() {
    let anim = ShimmerBuilder::new()
        .duration_ms(2000)
        .gradient_range(150.0, -150.0)
        .build();

    assert_eq!(anim.duration_ms(), 2000);
}
