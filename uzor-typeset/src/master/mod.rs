//! Master templates — named region geometry + default styles +
//! placeholder slots (design doc §5), shared by both page and slide
//! slicing targets. [`page_master`] is P0/P1's own `PageMaster`, extended
//! this phase with header/footer margin boxes + a page-number token;
//! [`slide_master`]/[`placeholder`] are new P2 additions (their own
//! slicing consumer, `slice_slides`, is still P3 — see this crate's
//! `CLAUDE.md`).

pub mod page_master;
pub mod placeholder;
pub mod slide_master;

pub use page_master::{HeaderPlaceholder, Margins, PageMaster, PageNumberFormat, PageNumberStyle};
pub use placeholder::{PlaceholderKind, PlaceholderSlot};
pub use slide_master::{LayoutId, MasterId, PlaceholderFill, PlacedPlaceholder, SlideInstance, SlideLayout, SlideMaster};
