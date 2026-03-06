//! Example demonstrating button animation recipes
//!
//! Run with: cargo run --example button_recipes

use uzor_animation::recipes::buttons::*;
use uzor_animation::Easing;

fn main() {
    println!("Button Animation Recipes Example\n");

    // Using presets
    println!("=== Preset Animations ===");

    let hover = material_hover();
    println!("Material Hover: {}ms", hover.duration_ms());

    let press = ios_press();
    println!("iOS Press: {}ms", press.duration_ms());

    let ripple = material_ripple();
    println!("Material Ripple: {}ms", ripple.duration_ms());

    let elastic = elastic_scale_hover();
    println!("Elastic Scale: ~{}ms", elastic.duration_ms());

    let lift = lift_shadow();
    println!("Lift Shadow: {}ms", lift.duration_ms());

    // Using builders for customization
    println!("\n=== Custom Animations with Builders ===");

    let custom_hover = HoverBuilder::new()
        .duration_ms(300)
        .opacity(0.0, 0.12)
        .easing(Easing::EaseInOutQuad)
        .build();
    println!("Custom Hover: {}ms", custom_hover.duration_ms());

    let custom_press = PressBuilder::new()
        .duration_ms(80)
        .scale(0.92)
        .easing(Easing::EaseOutQuart)
        .build();
    println!("Custom Press: {}ms", custom_press.duration_ms());

    let custom_elastic = ElasticScaleBuilder::new()
        .stiffness(250.0)
        .damping(18.0)
        .target_scale(1.08)
        .build();
    println!("Custom Elastic: ~{}ms", custom_elastic.duration_ms());

    // Direct construction
    println!("\n=== Direct Construction ===");

    let direct_hover = ButtonAnimation::Hover {
        duration_ms: 180,
        easing: Easing::EaseOutCubic,
        opacity_from: 0.0,
        opacity_to: 0.06,
    };
    println!("Direct Hover: {}ms", direct_hover.duration_ms());

    // All available presets
    println!("\n=== All Available Presets ===");
    let presets = vec![
        ("material_hover", material_hover()),
        ("fast_subtle_hover", fast_subtle_hover()),
        ("ios_press", ios_press()),
        ("simple_press", simple_press()),
        ("bounce_press", bounce_press()),
        ("ios_release", ios_release()),
        ("framer_spring_press", framer_spring_press()),
        ("material_ripple", material_ripple()),
        ("elastic_scale_hover", elastic_scale_hover()),
        ("subtle_elastic_hover", subtle_elastic_hover()),
        ("glow_pulse", glow_pulse()),
        ("fast_glow_pulse", fast_glow_pulse()),
        ("underline_slide", underline_slide()),
        ("underline_slide_center", underline_slide_center()),
        ("fill_sweep", fill_sweep()),
        ("fill_sweep_vertical", fill_sweep_vertical()),
        ("border_draw", border_draw()),
        ("fast_border_draw", fast_border_draw()),
        ("magnetic_pull", magnetic_pull()),
        ("strong_magnetic_pull", strong_magnetic_pull()),
        ("lift_shadow", lift_shadow()),
        ("dramatic_lift", dramatic_lift()),
    ];

    for (name, anim) in presets {
        println!("  - {}: {}ms", name, anim.duration_ms());
    }

    println!("\n✓ All animations created successfully!");
}
