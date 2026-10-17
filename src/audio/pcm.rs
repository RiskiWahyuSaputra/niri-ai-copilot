//! Util PCM: format target whisper.cpp (s16le mono 16 kHz), pembungkusan WAV,
//! chunker, dan level meter RMS.

/// Sample rate yang diminta dari PipeWire.
pub const SAMPLE_RATE: u32 = 16_000;
/// Jumlah channel: mono.
pub const CHANNELS: u16 = 1;
/// Bit per sample: 16 (signed little-endian).
pub const BITS_PER_SAMPLE: u16 = 16;
/// Byte per detik audio.
pub const BYTES_PER_SECOND: u32 = SAMPLE_RATE * (BITS_PER_SAMPLE as u32 / 8) * CHANNELS as u32;
/// Byte per 100 ms (periode baca).
pub const BYTES_PER_100MS: usize = (BYTES_PER_SECOND as usize) / 10;

/// Bungkus PCM s16le mono menjadi file WAV lengkap.
pub fn wav_from_s16_mono(samples: &[u8]) -> Vec<u8> {
    let data_len = samples.len() as u32;
    let mut out = Vec::with_capacity(44 + samples.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&CHANNELS.to_le_bytes());
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(BYTES_PER_SECOND).to_le_bytes());
    let block_align = CHANNELS * (BITS_PER_SAMPLE / 8);
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&BITS_PER_SAMPLE.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(samples);
    out
}

/// Level RMS 0.0–1.0 dari PCM s16le (untuk indikator volume).
pub fn rms_level(samples: &[u8]) -> f32 {
    if samples.len() < 2 {
        return 0.0;
    }
    let mut sum: f64 = 0.0;
    let mut count: u64 = 0;
    for pair in samples.chunks_exact(2) {
        let v = i16::from_le_bytes([pair[0], pair[1]]) as f64 / i16::MAX as f64;
        sum += v * v;
        count += 1;
    }
    if count == 0 {
        return 0.0;
    }
    let rms = (sum / count as f64).sqrt() as f32;
    rms.clamp(0.0, 1.0)
}

/// Chunker: mengumpulkan byte mentah menjadi potongan berukuran tetap.
pub struct Chunker {
    buf: Vec<u8>,
    chunk_bytes: usize,
    /// Total byte yang sudah diterima sejak awal stream (untuk timestamp).
    total_bytes: u64,
}

impl Chunker {
    pub fn new(chunk_seconds: u32) -> Self {
        let bytes = BYTES_PER_SECOND as usize * chunk_seconds.max(1) as usize;
        Self {
            buf: Vec::with_capacity(bytes + BYTES_PER_100MS),
            chunk_bytes: bytes,
            total_bytes: 0,
        }
    }

    pub fn chunk_bytes(&self) -> usize {
        self.chunk_bytes
    }

    /// Total byte stream yang telah diproses.
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    /// Masukkan data; kembalikan potongan-potongan penuh yang siap diproses.
    pub fn push(&mut self, data: &[u8]) -> Vec<Vec<u8>> {
        self.total_bytes += data.len() as u64;
        self.buf.extend_from_slice(data);
        let mut chunks = Vec::new();
        while self.buf.len() >= self.chunk_bytes {
            let rest = self.buf.split_off(self.chunk_bytes);
            chunks.push(std::mem::replace(&mut self.buf, rest));
        }
        chunks
    }

    /// Ambil sisa buffer (dipakai saat stream berhenti).
    pub fn flush(&mut self) -> Option<Vec<u8>> {
        if self.buf.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.buf))
        }
    }

    /// Majukan penghitung stream tanpa menyimpan data (digunakan saat pause).
    pub fn skip(&mut self, n: usize) {
        self.total_bytes += n as u64;
    }
}

/// Konversi posisi byte stream ke milidetik sejak awal sesi.
pub fn bytes_to_ms(bytes: u64) -> i64 {
    (bytes * 1000 / BYTES_PER_SECOND as u64) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_valid() {
        let pcm = vec![0u8; 320];
        let wav = wav_from_s16_mono(&pcm);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(wav.len(), 44 + 320);
        let data_len = u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]);
        assert_eq!(data_len, 320);
    }

    #[test]
    fn rms_of_silence_and_signal() {
        let silence = vec![0u8; 640];
        assert_eq!(rms_level(&silence), 0.0);
        let loud: Vec<u8> = std::iter::repeat([0xff, 0x7f]).take(320).flatten().collect();
        assert!(rms_level(&loud) > 0.9);
    }

    #[test]
    fn chunker_emits_exact_chunks() {
        let mut c = Chunker::new(1); // 32000 byte
        let half = vec![1u8; 16_000];
        assert!(c.push(&half).is_empty());
        let out = c.push(&half);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), 32_000);

        let mut c = Chunker::new(1);
        let out = c.push(&vec![1u8; 32_000]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), 32_000);
        let out = c.push(&vec![1u8; 48_000]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), 32_000);
        let rest = c.flush().expect("sisa");
        assert_eq!(rest.len(), 16_000);
        assert_eq!(c.total_bytes(), 80_000);
    }

    #[test]
    fn bytes_to_ms_conversion() {
        assert_eq!(bytes_to_ms(32_000), 1000);
        assert_eq!(bytes_to_ms(0), 0);
    }
}
