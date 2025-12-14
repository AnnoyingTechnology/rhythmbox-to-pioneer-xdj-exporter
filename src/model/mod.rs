//! Unified data model for music library representation
//!
//! This module defines data structures that are independent of
//! both input (Rhythmbox) and output (Pioneer) formats.

mod track;
mod playlist;
mod library;

pub use track::{Track, MusicalKey};
pub use playlist::{Playlist, PlaylistEntry};
pub use library::Library;
