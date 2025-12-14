//! ANLZ (Analysis) file writer
//!
//! Writes .DAT and .EXT analysis files containing waveforms and beatgrids.
//! Based on Deep Symmetry's Analysis Files documentation and rekordcrate.

mod writer;

pub use writer::{write_dat_file, write_ext_file};
