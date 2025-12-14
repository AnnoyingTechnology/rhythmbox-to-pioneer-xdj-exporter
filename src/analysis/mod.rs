//! Audio analysis layer
//!
//! This module provides audio analysis capabilities through a trait-based
//! abstraction. Phase 1 uses stub implementations, Phase 2 adds real analysis.

mod traits;
mod stub;

pub use traits::{AudioAnalyzer, AnalysisResult, BeatGrid, WaveformData};
pub use stub::StubAnalyzer;

// Phase 2: Uncomment when implementing real analysis
// mod audio;
// mod beat;
// mod key;
// mod waveform;
// pub use audio::AudioDecoder;
