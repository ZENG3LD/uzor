//! Identity and scalar newtypes.
//!
//! All are `Copy`. Ids allocated by an engine (window, overlay slot, focus
//! scope, timer token) are minted only by that engine; ids chosen by the app
//! (ticket, region, tray item) are plain correlation values.

/// One OS window (native) or the page canvas (web).
///
/// Produced by the WindowEngine (sequential, never reused within a run);
/// consumed by every envelope, command, intent and snapshot that is
/// window-scoped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(pub u32);

impl WindowId {
    /// The first id the WindowEngine hands out.
    pub const FIRST: Self = Self(1);

    /// The raw value.
    pub const fn get(self) -> u32 {
        self.0
    }

    /// The id allocated after this one; `None` once `u32` is exhausted
    /// (ids are never wrapped, so they are never reused within a run).
    pub const fn next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(n) => Some(Self(n)),
            None => None,
        }
    }
}

/// Monotonic change counter.
///
/// Every engine bumps its own on each state change; the kernel bumps the
/// published one in the publish phase iff any engine revision moved. Readers
/// of a [`VisualSnapshot`](crate::VisualSnapshot) compare revisions to know
/// whether to re-read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(pub u64);

impl Revision {
    /// The revision of a state that never changed.
    pub const ZERO: Self = Self(0);

    /// The raw value.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// The following revision. Saturates at `u64::MAX` so the sequence stays
    /// monotonic (non-decreasing) even in the unreachable overflow case.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    /// Advance in place and return the new value.
    pub fn bump(&mut self) -> Self {
        *self = self.next();
        *self
    }
}

/// A point in time or a duration, in seconds, on the host's monotonic clock.
///
/// Produced by hosts (`performance.now()` / `Instant`) and passed into the
/// kernel as an argument: the crate never reads a clock itself, which keeps
/// every step deterministic.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Seconds(pub f64);

impl Seconds {
    /// Time zero / an empty duration.
    pub const ZERO: Self = Self(0.0);

    /// From milliseconds (the browser clock unit).
    pub fn from_millis(ms: f64) -> Self {
        Self(ms / 1000.0)
    }

    /// The raw value in seconds.
    pub const fn get(self) -> f64 {
        self.0
    }

    /// `self + dt` (a later instant).
    #[must_use]
    pub fn after(self, dt: Seconds) -> Self {
        Self(self.0 + dt.0)
    }

    /// `self - earlier`, clamped at zero.
    #[must_use]
    pub fn since(self, earlier: Seconds) -> Self {
        Self((self.0 - earlier.0).max(0.0))
    }
}

/// A [`Seconds`] instant with a total order, usable as a sorted-map key.
///
/// [`Seconds`] wraps an `f64` and is only `PartialOrd`; the CadenceEngine's
/// deadline wheel keys its entries by this newtype instead. Ordering is
/// IEEE 754 `totalOrder` (`f64::total_cmp`), so the order is total and
/// deterministic. Constructing one from NaN is refused ([`OrderedSeconds::new`]
/// returns `None`), so every stored key is a real (possibly infinite) instant
/// and the `total_cmp` order agrees with the numeric order except that
/// `-0.0 < +0.0`.
#[derive(Clone, Copy, Debug)]
pub struct OrderedSeconds(f64);

impl OrderedSeconds {
    /// Wrap an instant; `None` when it is NaN.
    pub fn new(t: Seconds) -> Option<Self> {
        if t.0.is_nan() {
            None
        } else {
            Some(Self(t.0))
        }
    }

    /// The instant as plain [`Seconds`].
    pub const fn seconds(self) -> Seconds {
        Seconds(self.0)
    }
}

impl PartialEq for OrderedSeconds {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for OrderedSeconds {}

impl PartialOrd for OrderedSeconds {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedSeconds {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// Handle of one armed deadline on the cadence engine's deadline wheel.
///
/// Produced by the CadenceEngine when a deadline is armed; consumed when it
/// fires or is disarmed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimerToken(pub u64);

/// App-chosen correlation value for a request whose answer arrives later
/// (clipboard read, screenshot, layout blob).
///
/// Produced by the app (or the InputEngine for paste); echoed back unchanged
/// in the matching input event or intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ticket(pub u64);

/// One paint region of a window, for per-region cadence and retained caching.
///
/// Chosen by the app when it declares regions; the value space matches
/// `uzor::render::RetainedScope::Region(u64)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegionId(pub u64);

/// One focus / keymap scope, opened with an overlay.
///
/// Produced by the InputEngine when a scope is pushed; consumed by the
/// KeymapEngine and the snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScopeId(pub u64);

/// One open overlay instance on a window's stack.
///
/// Produced by the OverlayEngine on open (distinct across re-opens of the
/// same app overlay id); used for deadlines, fades and capture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OverlaySlot(pub u64);

/// One entry of the native tray menu.
///
/// Chosen by the app in its tray spec; echoed back by the native host when
/// the entry is selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TrayItemId(pub u32);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revision_orders_and_increments() {
        let r0 = Revision::ZERO;
        let r1 = r0.next();
        assert!(r1 > r0);
        assert_eq!(r1.get(), 1);
        let mut r = r1;
        assert_eq!(r.bump(), Revision(2));
        assert_eq!(r, Revision(2));
        assert_eq!(Revision::default(), Revision::ZERO);
    }

    #[test]
    fn revision_saturates_monotonically() {
        let max = Revision(u64::MAX);
        assert_eq!(max.next(), max);
        assert!(max.next() >= max);
    }

    #[test]
    fn window_id_allocation() {
        let a = WindowId::FIRST;
        let b = a.next();
        assert_eq!(b, Some(WindowId(2)));
        assert!(b > Some(a));
        assert_eq!(WindowId(u32::MAX).next(), None);
        assert_eq!(a.get(), 1);
    }

    #[test]
    fn seconds_arithmetic() {
        let t = Seconds::from_millis(1500.0);
        assert_eq!(t, Seconds(1.5));
        assert_eq!(t.after(Seconds(0.5)), Seconds(2.0));
        assert_eq!(t.since(Seconds(1.0)), Seconds(0.5));
        assert_eq!(Seconds(1.0).since(t), Seconds::ZERO);
        assert!(Seconds(1.0) < t);
    }

    #[test]
    fn ordered_seconds_total_order() {
        assert!(OrderedSeconds::new(Seconds(f64::NAN)).is_none());
        let a = OrderedSeconds::new(Seconds(1.0)).map(OrderedSeconds::seconds);
        assert_eq!(a, Some(Seconds(1.0)));
        let mut v: Vec<OrderedSeconds> = [3.0, -1.0, f64::INFINITY, 0.5]
            .iter()
            .filter_map(|&x| OrderedSeconds::new(Seconds(x)))
            .collect();
        v.sort();
        let got: Vec<f64> = v.iter().map(|k| k.seconds().get()).collect();
        assert_eq!(got, vec![-1.0, 0.5, 3.0, f64::INFINITY]);
        assert_eq!(
            OrderedSeconds::new(Seconds(2.0)),
            OrderedSeconds::new(Seconds(2.0))
        );
    }
}
