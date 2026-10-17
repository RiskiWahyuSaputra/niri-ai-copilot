//! Penangkapan audio melalui PipeWire.
//!
//! Implementasi memakai `pw-record` (bagian dari paket `pipewire`) sebagai
//! subprocess, membaca PCM mentah dari stdout secara streaming. Pendekatan ini
//! ringan (tidak menambah dependensi binding C besar) dan stabil di Arch Linux.
//!
//! Alur: `capture` (thread pembaca) → chunk WAV → antrean STT (backpressure
//! melalui bounded channel).

pub mod capture;
pub mod devices;
pub mod pcm;

pub use devices::{list_sources, AudioDevice};
