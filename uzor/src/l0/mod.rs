//! `l0` — pure agnostic surface across every composite.
//!
//! The composites under `crate::ui::widgets::composite::*` are paint
//! pipelines + per-widget data types.  Each composite already
//! exposes a private internal draw function that takes only
//! `(RenderContext, rect, state, view, settings, kind)` — no
//! `LayoutManager`, no `InputCoordinator`, no `ContextManager`.
//!
//! The L1 / L2 / L3 wrappers (`register_layout_manager_*`,
//! `register_input_coordinator_*`, `register_context_manager_*`)
//! sit on top of those internals and add the framework plumbing.
//! Embedders that drive their own input pipeline (tessera, future
//! tui / web runtimes, custom L0 apps) only need the internals.
//!
//! This module re-exports them as one flat list.  No new logic — just
//! a stable, intention-revealing public surface.
//!
//! ## What's *not* here
//!
//! - `panel` — the current paint path calls `coord.widget_state(...)`
//!   inside the body to compute edge-handle hover; it's a real L1
//!   dependency.  Until panel is refactored to take hover state as a
//!   plain parameter it stays out of `l0`.  Use the L1 / L2 / L3
//!   wrappers in the meantime.

pub mod chrome {
    pub use crate::ui::widgets::composite::chrome::{
        // pure paint
        render::draw_chrome,
        // pure geometry
        render::measure,
        input::{chrome_hit_test, handle_chrome_action},
        // data types
        types::{
            ChromeHit, ChromeView, ChromeTabConfig, ChromeRenderKind,
            ChromeAction, ChromeColors, ChromeResponse,
        },
        state::{ChromeState, TabState},
        settings::ChromeSettings,
        style::ChromeStyle,
        theme::{ChromeTheme, DefaultChromeTheme},
    };

    /// Slot-driven chrome (alternative to `draw_chrome` —
    /// configurator + per-slot pure paint).  Embedders supply a
    /// `ChromeLayout` and own interaction state outside.
    pub mod layout {
        pub use crate::ui::widgets::composite::chrome::layout::{
            // entry points
            draw_chrome_layout, chrome_layout_hit_test,
            // configurator
            ChromeLayout, Slot,
            TabsConfig, SearchConfig, ToolbarSlotConfig,
            // hit-test result
            ChromeHitPath, ChromeZone, ChromeHitKind,
        };
    }
}

pub mod modal {
    pub use crate::ui::widgets::composite::modal::{
        render::{draw_modal, draw_body_overflow_chevrons, body_rect, measure_chrome},
        input::modal_header_hit,
        types::{BackdropKind, FooterBtnStyle, ModalView, ModalRenderKind},
        state::ModalState,
        settings::ModalSettings,
    };
}

pub mod popup {
    pub use crate::ui::widgets::composite::popup::{
        render::{draw_popup, body_rect},
        types::{BackdropKind, PopupView, PopupViewKind, PopupRenderKind},
        state::PopupState,
        settings::PopupSettings,
    };
}

pub mod dropdown {
    pub use crate::ui::widgets::composite::dropdown::{
        render::{draw_dropdown, measure_flat},
        types::{
            DropdownItem, SubmenuTrigger, SubmenuWidth, DropdownItemRight,
            DropdownViewKind, DropdownView, DropdownRenderKind,
        },
        state::DropdownState,
        settings::DropdownSettings,
    };
}

pub mod context_menu {
    pub use crate::ui::widgets::composite::context_menu::{
        render::{draw_context_menu, measure},
        types::{ContextMenuItem, ContextMenuView, ContextMenuRenderKind},
        state::ContextMenuState,
        settings::ContextMenuSettings,
    };
}

pub mod toolbar {
    pub use crate::ui::widgets::composite::toolbar::{
        render::{draw_toolbar, measure_horizontal, measure_vertical},
        types::{
            SplitButtonHoverZone, ToolbarItem, ToolbarSection,
            ToolbarView, TabConfig, ChromeStripView, ToolbarRenderKind,
        },
        state::ToolbarState,
        settings::ToolbarSettings,
    };
}

pub mod sidebar {
    pub use crate::ui::widgets::composite::sidebar::{
        render::{draw_sidebar, begin_body, end_body, measure, body_viewport, SidebarBodyViewport},
        types::{
            SidebarHeader, HeaderAction, SidebarTab,
            SidebarHeaderMode, SidebarView, SidebarRenderKind,
        },
        state::SidebarState,
        settings::SidebarSettings,
    };
}

pub mod blackbox_panel {
    pub use crate::ui::widgets::composite::blackbox_panel::{
        render::draw_blackbox,
        types::{BlackboxEvent, BlackboxEventResult, BlackboxView, BlackboxRenderKind, BlackboxHandler},
        settings::BlackboxPanelSettings,
    };
}

/// Style manager — pure-stdlib service that maps token names to
/// colours / sizes / textures, plus the built-in mirage palettes.
/// No L1 / L2 / L3 / WM dependency — backed by `HashMap<String, _>`.
pub mod style {
    pub use crate::layout::styles::{
        StyleManager,
        TextureKind,
        Preset,
        MirageDarkPreset,
        MirageLightPreset,
    };
}

/// Atomic widget paint surface — pure render functions for the
/// building blocks of composites.  No event handlers, no L1 / L2 /
/// L3 dependencies.  Tessera consumes these with its own
/// interaction-state map and dispatch.
pub mod atomic {
    pub mod button {
        pub use crate::ui::widgets::atomic::button::{
            // pure paint
            render::draw_button,
            // data types
            settings::ButtonSettings,
            state::{ButtonState, SplitButtonHoverZone},
            style::{
                ButtonStyle,
                DefaultButtonStyle, CompactButtonStyle, FlatButtonStyle,
                ToolbarButtonStyle, ToolbarLabelStyle,
                PrimaryButtonStyle, PrimaryRoundedButtonStyle,
                GhostOutlineButtonStyle, GhostOutlineRoundedButtonStyle,
                DangerButtonStyle, SidebarTabStyle, HorizontalTabStyle,
                UtilityButtonStyle,
                DropdownMenuRowStyle,
                RoundedDropdownMenuRowStyle, FlatDropdownMenuRowStyle,
            },
            theme::{ButtonTheme, DefaultButtonTheme},
            types::{
                ButtonType, ActionVariant, ButtonStyle as ButtonStyleEnum,
                ButtonContent,
            },
        };
        // Re-export ButtonView / ButtonResult — they live alongside
        // draw_button in render.rs.
        pub use crate::ui::widgets::atomic::button::render::{
            ButtonView, ButtonResult,
        };
    }

    pub mod text_input {
        pub use crate::ui::widgets::atomic::text_input::{
            // pure paint
            render::{draw_input, draw_input_cursor, cursor_from_char_positions},
            // data types
            settings::TextInputSettings,
            state::{
                InputCapability, TextFieldState, TextFieldStore,
                TextAction,
            },
            style::{TextInputStyle, DefaultTextInputStyle},
            theme::{TextInputTheme, DefaultTextInputTheme},
            types::{InputType, TextInputType},
        };
        // The behavior.rs layer is L1/L2-flavoured (event mapping,
        // confirm/cancel semantics) — kept out of l0.  Embedders
        // build their own dispatch on top of TextFieldStore.
    }
}
