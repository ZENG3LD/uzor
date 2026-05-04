//! Multi-window registry backed by a `HashMap<WindowId, WindowState<A>>`.

use std::collections::HashMap;

use winit::window::WindowId;

use super::state::WindowState;

/// Registry of all live windows for an application.
pub struct WindowManager<A> {
    windows: HashMap<WindowId, WindowState<A>>,
}

impl<A> WindowManager<A> {
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
        }
    }

    pub fn insert(&mut self, state: WindowState<A>) -> WindowId {
        let id = state.id();
        self.windows.insert(id, state);
        id
    }

    pub fn remove(&mut self, id: WindowId) -> Option<WindowState<A>> {
        self.windows.remove(&id)
    }

    pub fn get(&self, id: WindowId) -> Option<&WindowState<A>> {
        self.windows.get(&id)
    }

    pub fn get_mut(&mut self, id: WindowId) -> Option<&mut WindowState<A>> {
        self.windows.get_mut(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&WindowId, &WindowState<A>)> {
        self.windows.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&WindowId, &mut WindowState<A>)> {
        self.windows.iter_mut()
    }

    pub fn count(&self) -> usize {
        self.windows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    pub fn ids(&self) -> impl Iterator<Item = WindowId> + '_ {
        self.windows.keys().copied()
    }

    pub fn drain_closed(&mut self) -> Vec<WindowState<A>> {
        let ids: Vec<WindowId> = self
            .windows
            .iter()
            .filter_map(|(id, s)| s.close_requested.then_some(*id))
            .collect();
        ids.into_iter()
            .filter_map(|id| self.windows.remove(&id))
            .collect()
    }
}

impl<A> Default for WindowManager<A> {
    fn default() -> Self {
        Self::new()
    }
}
