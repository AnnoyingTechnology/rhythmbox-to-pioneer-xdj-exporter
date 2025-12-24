//! Audio analysis layer
//!
//! This module provides audio analysis capabilities through a trait-based
//! abstraction. Phase 1 uses stub implementations, Phase 2 adds real analysis.
//!
//! Audio analysis is powered by stratum-dsp for both BPM and key detection.

mod real;
mod stratum;
mod stub;
mod traits;

pub use real::RealAnalyzer;
pub use stub::StubAnalyzer;
pub use traits::{AnalysisResult, AudioAnalyzer, BeatGrid, WaveformData};

// Phase 2 TODO: Uncomment when implementing
// mod waveform;
