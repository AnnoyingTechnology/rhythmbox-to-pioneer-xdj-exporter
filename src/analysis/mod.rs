//! Audio analysis layer
//!
//! This module provides audio analysis capabilities through a trait-based
//! abstraction. Phase 1 uses stub implementations, Phase 2 adds real analysis.
//!
//! Audio analysis is powered by stratum-dsp for both BPM and key detection.
//! Waveform generation uses symphonia for audio decoding.

mod real;
mod stratum;
mod stub;
mod traits;
pub mod waveform;

pub use real::RealAnalyzer;
pub use stub::StubAnalyzer;
pub use traits::{AnalysisResult, AudioAnalyzer, BeatGrid, WaveformData};
