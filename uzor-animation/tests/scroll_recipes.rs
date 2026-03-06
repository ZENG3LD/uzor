//! Integration tests for scroll animation recipes

use uzor_animation::recipes::scroll::presets::*;
use uzor_animation::recipes::scroll::builders::*;
use uzor_animation::recipes::scroll::types::*;
use uzor_animation::easing::Easing;
use std::time::Duration;

#[test]
fn test_progress_bar_horizontal_preset() {
    let anim = progress_bar_horizontal();

    match anim {
        ScrollAnimation::ProgressBar {
            scroll_start,
            scroll_end,
            easing,
            orientation,
        } => {
            assert_eq!(scroll_start, 0.0);
            assert_eq!(scroll_end, 10000.0);
            assert_eq!(easing, Easing::Linear);
            assert_eq!(orientation, ProgressBarOrientation::Horizontal);
        }
        _ => panic!("Expected ProgressBar variant"),
    }
}

#[test]
fn test_progress_ring_preset() {
    let anim = progress_ring();

    match anim {
        ScrollAnimation::ProgressBar { orientation, .. } => {
            assert_eq!(orientation, ProgressBarOrientation::Circular);
        }
        _ => panic!("Expected ProgressBar variant"),
    }
}

#[test]
fn test_parallax_hero_preset() {
    let anim = parallax_hero();

    match anim {
        ScrollAnimation::ParallaxLayers { layer_speeds, axis } => {
            assert_eq!(layer_speeds.len(), 3);
            assert_eq!(layer_speeds[0], 0.3);
            assert_eq!(layer_speeds[1], 0.6);
            assert_eq!(layer_speeds[2], 1.0);
            assert_eq!(axis, ParallaxAxis::Vertical);
        }
        _ => panic!("Expected ParallaxLayers variant"),
    }
}

#[test]
fn test_fade_in_on_enter_preset() {
    let anim = fade_in_on_enter();

    match anim {
        ScrollAnimation::FadeOnScroll {
            opacity_from,
            opacity_to,
            scroll_range,
            easing,
        } => {
            assert_eq!(opacity_from, 0.0);
            assert_eq!(opacity_to, 1.0);
            assert_eq!(scroll_range, (0.0, 0.3));
            assert_eq!(easing, Easing::EaseOutCubic);
        }
        _ => panic!("Expected FadeOnScroll variant"),
    }
}

#[test]
fn test_slide_up_on_enter_preset() {
    let anim = slide_up_on_enter();

    match anim {
        ScrollAnimation::RevealOnEnter {
            translate_y,
            opacity_from,
            entry_range,
            duration,
            easing,
        } => {
            assert_eq!(translate_y, 50.0);
            assert_eq!(opacity_from, 0.0);
            assert_eq!(entry_range, (0.0, 0.4));
            assert_eq!(duration, Duration::from_millis(600));
            assert_eq!(easing, Easing::EaseOutCubic);
        }
        _ => panic!("Expected RevealOnEnter variant"),
    }
}

#[test]
fn test_sticky_shrink_header_preset() {
    let anim = sticky_shrink_header();

    match anim {
        ScrollAnimation::StickyHeader {
            from_state,
            to_state,
            scroll_threshold,
            duration,
            easing,
        } => {
            assert_eq!(from_state.height, 80.0);
            assert_eq!(to_state.height, 48.0);
            assert_eq!(from_state.scale, 1.0);
            assert_eq!(to_state.scale, 0.9);
            assert_eq!(scroll_threshold, 100.0);
            assert_eq!(duration, Duration::from_millis(300));
            assert_eq!(easing, Easing::EaseInOutQuad);
        }
        _ => panic!("Expected StickyHeader variant"),
    }
}

#[test]
fn test_horizontal_pin_scroll_preset() {
    let anim = horizontal_pin_scroll();

    match anim {
        ScrollAnimation::HorizontalScroll {
            scroll_distance,
            vertical_range,
            pin,
            easing,
        } => {
            assert_eq!(scroll_distance, 2000.0);
            assert_eq!(vertical_range, (0.0, 3000.0));
            assert_eq!(pin, true);
            assert_eq!(easing, Easing::Linear);
        }
        _ => panic!("Expected HorizontalScroll variant"),
    }
}

#[test]
fn test_number_counter_preset() {
    let anim = number_counter();

    match anim {
        ScrollAnimation::NumberCounter {
            from,
            to,
            threshold,
            duration,
            easing,
        } => {
            assert_eq!(from, 0.0);
            assert_eq!(to, 100.0);
            assert_eq!(threshold, 0.8);
            assert_eq!(duration, Duration::from_millis(2000));
            assert_eq!(easing, Easing::EaseOutCubic);
        }
        _ => panic!("Expected NumberCounter variant"),
    }
}

#[test]
fn test_color_shift_sections_preset() {
    let anim = color_shift_sections();

    match anim {
        ScrollAnimation::ColorShift {
            color_stops,
            scroll_range,
            easing,
        } => {
            assert_eq!(color_stops.len(), 4);
            assert_eq!(color_stops[0].0, 0.0);
            assert_eq!(color_stops[3].0, 1.0);
            assert_eq!(scroll_range, (0.0, 5000.0));
            assert_eq!(easing, Easing::EaseInOutSine);
        }
        _ => panic!("Expected ColorShift variant"),
    }
}

#[test]
fn test_progress_bar_builder() {
    let anim = ProgressBarBuilder::new()
        .scroll_start(100.0)
        .scroll_end(2000.0)
        .orientation(ProgressBarOrientation::Vertical)
        .easing(Easing::EaseInOutQuad)
        .build();

    match anim {
        ScrollAnimation::ProgressBar {
            scroll_start,
            scroll_end,
            orientation,
            easing,
        } => {
            assert_eq!(scroll_start, 100.0);
            assert_eq!(scroll_end, 2000.0);
            assert_eq!(orientation, ProgressBarOrientation::Vertical);
            assert_eq!(easing, Easing::EaseInOutQuad);
        }
        _ => panic!("Expected ProgressBar variant"),
    }
}

#[test]
fn test_parallax_layers_builder() {
    let anim = ParallaxLayersBuilder::new()
        .layer_speeds(vec![0.2, 0.5, 0.8])
        .axis(ParallaxAxis::Horizontal)
        .build();

    match anim {
        ScrollAnimation::ParallaxLayers { layer_speeds, axis } => {
            assert_eq!(layer_speeds.len(), 3);
            assert_eq!(layer_speeds[0], 0.2);
            assert_eq!(layer_speeds[2], 0.8);
            assert_eq!(axis, ParallaxAxis::Horizontal);
        }
        _ => panic!("Expected ParallaxLayers variant"),
    }
}

#[test]
fn test_fade_builder_with_custom_values() {
    let anim = FadeOnScrollBuilder::new()
        .opacity_from(0.2)
        .opacity_to(0.9)
        .scroll_range(500.0, 1500.0)
        .easing(Easing::EaseInOutSine)
        .build();

    match anim {
        ScrollAnimation::FadeOnScroll {
            opacity_from,
            opacity_to,
            scroll_range,
            easing,
        } => {
            assert_eq!(opacity_from, 0.2);
            assert_eq!(opacity_to, 0.9);
            assert_eq!(scroll_range, (500.0, 1500.0));
            assert_eq!(easing, Easing::EaseInOutSine);
        }
        _ => panic!("Expected FadeOnScroll variant"),
    }
}

#[test]
fn test_reveal_builder_custom() {
    let anim = RevealOnEnterBuilder::new()
        .translate_y(100.0)
        .opacity_from(0.3)
        .entry_range(0.1, 0.5)
        .duration(Duration::from_millis(800))
        .easing(Easing::EaseOutQuart)
        .build();

    match anim {
        ScrollAnimation::RevealOnEnter {
            translate_y,
            opacity_from,
            entry_range,
            duration,
            easing,
        } => {
            assert_eq!(translate_y, 100.0);
            assert_eq!(opacity_from, 0.3);
            assert_eq!(entry_range, (0.1, 0.5));
            assert_eq!(duration, Duration::from_millis(800));
            assert_eq!(easing, Easing::EaseOutQuart);
        }
        _ => panic!("Expected RevealOnEnter variant"),
    }
}

#[test]
fn test_number_counter_builder_custom() {
    let anim = NumberCounterBuilder::new()
        .from(10.0)
        .to(500.0)
        .threshold(0.5)
        .duration(Duration::from_millis(3000))
        .easing(Easing::EaseInOutCubic)
        .build();

    match anim {
        ScrollAnimation::NumberCounter {
            from,
            to,
            threshold,
            duration,
            easing,
        } => {
            assert_eq!(from, 10.0);
            assert_eq!(to, 500.0);
            assert_eq!(threshold, 0.5);
            assert_eq!(duration, Duration::from_millis(3000));
            assert_eq!(easing, Easing::EaseInOutCubic);
        }
        _ => panic!("Expected NumberCounter variant"),
    }
}

#[test]
fn test_scroll_animation_is_active() {
    let anim = FadeOnScrollBuilder::new()
        .scroll_range(100.0, 500.0)
        .build();

    assert!(!anim.is_active(50.0));
    assert!(anim.is_active(100.0));
    assert!(anim.is_active(300.0));
    assert!(anim.is_active(500.0));
    assert!(!anim.is_active(600.0));
}

#[test]
fn test_scroll_range_extraction() {
    let anim = horizontal_pin_scroll();
    let (start, end) = anim.scroll_range();

    assert_eq!(start, 0.0);
    assert_eq!(end, 3000.0);
}

#[test]
fn test_parallax_builder_add_layer() {
    let anim = ParallaxLayersBuilder::new()
        .layer_speeds(vec![]) // Start empty
        .add_layer(0.2)
        .add_layer(0.5)
        .add_layer(0.8)
        .add_layer(1.0)
        .build();

    match anim {
        ScrollAnimation::ParallaxLayers { layer_speeds, .. } => {
            assert_eq!(layer_speeds.len(), 4);
            assert_eq!(layer_speeds[3], 1.0);
        }
        _ => panic!("Expected ParallaxLayers variant"),
    }
}

#[test]
fn test_color_shift_builder() {
    let anim = ColorShiftBuilder::new()
        .color_stops(vec![
            (0.0, (1.0, 0.0, 0.0)),
            (0.5, (0.0, 1.0, 0.0)),
            (1.0, (0.0, 0.0, 1.0)),
        ])
        .scroll_range(0.0, 3000.0)
        .easing(Easing::Linear)
        .build();

    match anim {
        ScrollAnimation::ColorShift {
            color_stops,
            scroll_range,
            easing,
        } => {
            assert_eq!(color_stops.len(), 3);
            assert_eq!(color_stops[0].1, (1.0, 0.0, 0.0)); // Red
            assert_eq!(color_stops[2].1, (0.0, 0.0, 1.0)); // Blue
            assert_eq!(scroll_range, (0.0, 3000.0));
            assert_eq!(easing, Easing::Linear);
        }
        _ => panic!("Expected ColorShift variant"),
    }
}
