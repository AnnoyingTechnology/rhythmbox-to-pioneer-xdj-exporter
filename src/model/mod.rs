//! Unified data model for music library representation
//!
//! This module defines data structures that are independent of
//! both input (Rhythmbox) and output (Pioneer) formats.

mod library;
mod playlist;
mod track;

pub use library::Library;
pub use playlist::{Playlist, PlaylistEntry};
pub use track::{MusicalKey, Track};
