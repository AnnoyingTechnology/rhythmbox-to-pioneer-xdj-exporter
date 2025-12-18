//! Audio analysis layer
//!
//! This module provides audio analysis capabilities through a trait-based
//! abstraction. Phase 1 uses stub implementations, Phase 2 adds real analysis.

mod stub;
mod traits;

pub use stub::StubAnalyzer;
pub use traits::{AnalysisResult, AudioAnalyzer, BeatGrid, WaveformData};

// Phase 2: Uncomment when implementing real analysis
// mod audio;
// mod beat;
// mod key;
// mod waveform;
// pub use audio::AudioDecoder;
