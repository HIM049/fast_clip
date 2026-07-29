use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anyhow::{anyhow, bail};
use atomic_float::AtomicF32;
use cpal::{
    FromSample, SampleFormat, SizedSample, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use ringbuf::{HeapCons, traits::Consumer};

pub struct AudioPlayer {
    _host: cpal::Host,
    device: cpal::Device,
    config: StreamConfig,
    sample_format: SampleFormat,
    sample_rate: u32,
    channels: u16,
    stream: Option<cpal::Stream>,
}

impl AudioPlayer {
    pub fn new() -> anyhow::Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("no avilable output device");

        let stream_config = device
            .default_output_config()
            .map_err(|error| anyhow!("failed to find default output config: {error}"))?;

        let sample_rate = stream_config.sample_rate();
        let channels = stream_config.channels();
        let sample_format = stream_config.sample_format();

        let config = stream_config.config();
        eprintln!(
            "[DEBUG-audio-config] sample_format={sample_format} sample_rate={sample_rate} channels={channels}"
        );
        Ok(Self {
            _host: host,
            device,
            config,
            sample_format,
            sample_rate,
            channels,
            stream: None,
        })
    }

    pub fn play(&mut self) -> Result<(), cpal::PlayStreamError> {
        if let Some(s) = self.stream.as_mut() {
            s.play()?;
        }
        Ok(())
    }

    pub fn pause(&mut self) -> Result<(), cpal::PauseStreamError> {
        if let Some(s) = self.stream.as_mut() {
            s.pause()?;
        }
        Ok(())
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn spawn(
        &mut self,
        consumer: HeapCons<f32>,
        signal: Arc<AtomicBool>,
        gain: Arc<AtomicF32>,
    ) -> anyhow::Result<()> {
        macro_rules! build_stream {
            ($sample_type:ty) => {
                build_output_stream::<$sample_type>(
                    &self.device,
                    &self.config,
                    consumer,
                    signal,
                    gain,
                    move |err| eprintln!("audio output stream error: {err}"),
                )
            };
        }

        let stream = match self.sample_format {
            SampleFormat::I8 => build_stream!(i8),
            SampleFormat::I16 => build_stream!(i16),
            SampleFormat::I24 => build_stream!(cpal::I24),
            SampleFormat::I32 => build_stream!(i32),
            SampleFormat::I64 => build_stream!(i64),
            SampleFormat::U8 => build_stream!(u8),
            SampleFormat::U16 => build_stream!(u16),
            SampleFormat::U24 => build_stream!(cpal::U24),
            SampleFormat::U32 => build_stream!(u32),
            SampleFormat::U64 => build_stream!(u64),
            SampleFormat::F32 => build_stream!(f32),
            SampleFormat::F64 => build_stream!(f64),
            sample_format => bail!("unsupported audio output sample format: {sample_format}"),
        }
        .map_err(|error| anyhow!("failed to build audio output stream: {error}"))?;

        stream
            .play()
            .map_err(|error| anyhow!("failed to start audio output stream: {error}"))?;
        self.stream = Some(stream);
        Ok(())
    }
}

fn build_output_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut consumer: HeapCons<f32>,
    signal: Arc<AtomicBool>,
    gain: Arc<AtomicF32>,
    error_callback: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: SizedSample + FromSample<f32>,
{
    device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            let gain = gain.load(Ordering::Relaxed);
            write_samples(data, &mut consumer, gain);
            signal.store(true, Ordering::Release);
        },
        error_callback,
        None,
    )
}

fn write_samples<T>(data: &mut [T], consumer: &mut HeapCons<f32>, gain: f32)
where
    T: SizedSample + FromSample<f32>,
{
    for output in data {
        *output = consumer
            .try_pop()
            .map(|sample| T::from_sample(sample * gain))
            .unwrap_or(T::EQUILIBRIUM);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringbuf::{
        SharedRb,
        storage::Heap,
        traits::{Producer, Split},
    };

    #[test]
    fn converts_pcm_and_fills_missing_signed_samples_with_silence() {
        let buffer = SharedRb::<Heap<f32>>::new(4);
        let (mut producer, mut consumer) = buffer.split();
        producer.push_slice(&[-1.0, 0.0, 1.0]);

        let mut output = [i16::MIN; 4];
        write_samples(&mut output, &mut consumer, 1.0);

        assert_eq!(output, [i16::MIN, 0, i16::MAX, 0]);
    }

    #[test]
    fn fills_missing_unsigned_samples_with_equilibrium() {
        let buffer = SharedRb::<Heap<f32>>::new(1);
        let (_, mut consumer) = buffer.split();
        let mut output = [u16::MIN; 2];

        write_samples(&mut output, &mut consumer, 1.0);

        assert_eq!(output, [u16::MAX / 2 + 1; 2]);
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        println!("DEBUG: player dropped");
    }
}
