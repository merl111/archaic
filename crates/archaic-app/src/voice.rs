//! Bounded native microphone capture. No stream exists until Record is pressed.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    io::Write,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
pub struct Recorder {
    stream: cpal::Stream,
    samples: Arc<Mutex<Vec<i16>>>,
    failed: Arc<AtomicBool>,
    rate: u32,
}
pub struct Clip {
    pub file: tempfile::NamedTempFile,
    pub duration_ms: u64,
}
impl Recorder {
    pub fn start() -> Result<Self, &'static str> {
        let device = cpal::default_host()
            .default_input_device()
            .ok_or("voice-unavailable")?;
        let supported = device
            .default_input_config()
            .map_err(|_| "voice-unavailable")?;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let rate = config.sample_rate;
        if !(8000..=96000).contains(&rate) || config.channels == 0 || config.channels > 16 {
            return Err("voice-unavailable");
        }
        let samples = Arc::new(Mutex::new(Vec::new()));
        let failed = Arc::new(AtomicBool::new(false));
        let stream = match format {
            cpal::SampleFormat::F32 => {
                input::<f32>(&device, config, samples.clone(), failed.clone())
            }
            cpal::SampleFormat::I16 => {
                input::<i16>(&device, config, samples.clone(), failed.clone())
            }
            cpal::SampleFormat::U16 => {
                input::<u16>(&device, config, samples.clone(), failed.clone())
            }
            _ => return Err("voice-unavailable"),
        }?;
        stream.play().map_err(|_| "voice-unavailable")?;
        Ok(Self {
            stream,
            samples,
            failed,
            rate,
        })
    }
    pub fn duration(&self) -> u64 {
        self.samples
            .lock()
            .map(|s| s.len() as u64 * 1000 / self.rate as u64)
            .unwrap_or(0)
    }
    pub fn stop(self) -> Result<Clip, &'static str> {
        drop(self.stream);
        if self.failed.load(Ordering::Relaxed) {
            return Err("voice-unavailable");
        }
        let samples = self.samples.lock().map_err(|_| "voice-invalid")?;
        if samples.is_empty() {
            return Err("voice-invalid");
        }
        let bytes = wav(&samples, self.rate);
        let mut file = tempfile::Builder::new()
            .suffix(".wav")
            .tempfile()
            .map_err(|_| "file-save-failed")?;
        file.write_all(&bytes).map_err(|_| "file-save-failed")?;
        Ok(Clip {
            file,
            duration_ms: samples.len() as u64 * 1000 / self.rate as u64,
        })
    }
}
fn input<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    samples: Arc<Mutex<Vec<i16>>>,
    failed: Arc<AtomicBool>,
) -> Result<cpal::Stream, &'static str>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let channels = config.channels as usize;
    let limit = config.sample_rate as usize * 120;
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                if let Ok(mut samples) = samples.try_lock() {
                    let remaining = limit.saturating_sub(samples.len());
                    for frame in data.chunks_exact(channels).take(remaining) {
                        let mono = frame.iter().map(|v| v.to_sample::<f32>()).sum::<f32>()
                            / channels as f32;
                        samples.push((mono.clamp(-1., 1.) * i16::MAX as f32) as i16);
                    }
                }
            },
            move |_| failed.store(true, Ordering::Relaxed),
            None,
        )
        .map_err(|_| "voice-unavailable")
}
fn wav(samples: &[i16], rate: u32) -> Vec<u8> {
    let len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(len as usize + 44);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(len + 36).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&len.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}
#[cfg(test)]
mod tests {
    #[test]
    fn wav_header_and_payload_agree_without_opening_microphone() {
        let wav = super::wav(&[0, -32768, 32767], 48000);
        assert_eq!(wav.len(), 50);
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 6);
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 48000);
    }
}
