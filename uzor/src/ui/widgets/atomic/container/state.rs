//! Container state placeholder.
//!
//! **mlc audit finding:** containers are fully stateless. No dedicated state
//! struct exists in mlc for any of the 16 container variants. Hover / active
//! booleans are owned by the surrounding modal or panel state and passed in at
//! render time. Scroll position lives in `composite/scroll_container/`, not here.
//!
//! `ContainerState` is kept as an explicit zero-size type so the module API
//! remains uniform with other widgets that do carry state.

#[derive(Debug, Default, Clone, Copy)]
pub struct ContainerState;
