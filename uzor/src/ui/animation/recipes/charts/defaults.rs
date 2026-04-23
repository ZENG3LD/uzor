//! Default parameter structs for chart animations

use crate::animation::easing::Easing;
use crate::animation::spring::Spring;

/// Default parameters for bar growth animation
#[derive(Debug, Clone, Copy)]
pub struct BarGrowDefaults {
    pub duration_ms: u64,
    pub stagger_delay_ms: u64,
    pub easing: Easing,
}

impl Default for BarGrowDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 600,
            stagger_delay_ms: 50,
            easing: Easing::EaseOutCubic,
        }
    }
}

/// Default parameters for bar update animation
#[derive(Debug, Clone, Copy)]
pub struct BarUpdateDefaults {
    pub stiffness: f64,
    pub damping: f64,
    pub stagger_delay_ms: u64,
}

impl Default for BarUpdateDefaults {
    fn default() -> Self {
        Self {
            stiffness: 120.0,
            damping: 14.0,
            stagger_delay_ms: 20,
        }
    }
}

impl BarUpdateDefaults {
    pub fn to_spring(&self) -> Spring {
        Spring::new()
            .stiffness(self.stiffness)
            .damping(self.damping)
    }
}

/// Default parameters for line draw-in animation
#[derive(Debug, Clone, Copy)]
pub struct LineDrawInDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
}

impl Default for LineDrawInDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 1000,
            easing: Easing::Linear,
        }
    }
}

/// Default parameters for candlestick reveal animation
#[derive(Debug, Clone, Copy)]
pub struct CandlestickRevealDefaults {
    pub wick_duration_ms: u64,
    pub body_duration_ms: u64,
    pub stagger_delay_ms: u64,
    pub wick_easing: Easing,
    pub body_easing: Easing,
}

impl Default for CandlestickRevealDefaults {
    fn default() -> Self {
        Self {
            wick_duration_ms: 200,
            body_duration_ms: 300,
            stagger_delay_ms: 30,
            wick_easing: Easing::EaseOutQuad,
            body_easing: Easing::EaseOutCubic,
        }
    }
}

/// Default parameters for number counter animation
#[derive(Debug, Clone, Copy)]
pub struct NumberCounterDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
    pub decimals: u8,
}

impl Default for NumberCounterDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 1000,
            easing: Easing::EaseOutCubic,
            decimals: 2,
        }
    }
}

/// Default parameters for data morph animation
#[derive(Debug, Clone, Copy)]
pub struct DataMorphDefaults {
    pub duration_ms: u64,
    pub easing: Easing,
}

impl Default for DataMorphDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 500,
            easing: Easing::EaseInOutCubic,
        }
    }
}

/// Default parameters for area fill animation
#[derive(Debug, Clone, Copy)]
pub struct AreaFillDefaults {
    pub line_duration_ms: u64,
    pub fill_duration_ms: u64,
    pub fill_delay_ms: u64,
    pub line_easing: Easing,
    pub fill_easing: Easing,
}

impl Default for AreaFillDefaults {
    fn default() -> Self {
        Self {
            line_duration_ms: 1500,
            fill_duration_ms: 800,
            fill_delay_ms: 1000,
            line_easing: Easing::Linear,
            fill_easing: Easing::EaseInOutQuad,
        }
    }
}

/// Default parameters for pie slice growth animation
#[derive(Debug, Clone, Copy)]
pub struct PieSliceGrowDefaults {
    pub duration_ms: u64,
    pub stagger_delay_ms: u64,
    pub easing: Easing,
}

impl Default for PieSliceGrowDefaults {
    fn default() -> Self {
        Self {
            duration_ms: 1000,
            stagger_delay_ms: 100,
            easing: Easing::EaseOutBack,
        }
    }
}

/// Default parameters for heatmap fade animation
#[derive(Debug, Clone, Copy)]
pub struct HeatmapFadeDefaults {
    pub cell_duration_ms: u64,
    pub stagger_delay_ms: u64,
    pub easing: Easing,
}

impl Default for HeatmapFadeDefaults {
    fn default() -> Self {
        Self {
            cell_duration_ms: 300,
            stagger_delay_ms: 20,
            easing: Easing::EaseOutQuad,
        }
    }
}

/// Default parameters for ticker flash animation
#[derive(Debug, Clone, Copy)]
pub struct TickerFlashDefaults {
    pub flash_duration_ms: u64,
    pub fade_duration_ms: u64,
    pub easing: Easing,
}

impl Default for TickerFlashDefaults {
    fn default() -> Self {
        Self {
            flash_duration_ms: 200,
            fade_duration_ms: 400,
            easing: Easing::EaseOutCubic,
        }
    }
}
