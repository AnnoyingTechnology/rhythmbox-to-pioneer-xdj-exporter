//! PDB (Pioneer Database) file writer
//!
//! Writes export.pdb files compatible with Rekordbox-exported USB devices.
//! Based on Deep Symmetry's analysis and rekordcrate's parser implementation.

mod writer;
mod types;
mod strings;

pub use writer::{write_pdb, write_pdb_ext, TrackMetadata};
pub use types::{TableType, FileType};

// Phase 1: Minimal table implementations
// Phase 2: Full table support with all metadata
