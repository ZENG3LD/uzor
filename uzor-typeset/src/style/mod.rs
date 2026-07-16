//! Style resolution (design doc §5): [`property`]'s tri-state `master ->
//! layout -> instance` resolution algebra + [`theme`]'s 3-tier design
//! tokens (brand/design/component).

pub mod property;
pub mod theme;

pub use property::{resolve_property_chain, PropertyState, ResolvedProperty, RootPropertyState};
pub use theme::{BrandTokens, ColorRole, ComponentStyle, DesignTokens, FigureThemeTokens, FontFileRef, FontRole, TextStyle, Theme};
