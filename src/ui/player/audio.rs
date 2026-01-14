use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use anyhow::anyhow;
use cpal::{
    StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use ringbuf::{HeapCons, traits::Consumer};

pub struct AudioPlayer {
    _host: cpal::Host,
    device: cpal::Device,
    config: StreamConfig,
    sample_rate: u32,
    stream: Option<cpal::Stream>,
}

impl AudioPlayer {
    pub fn new() -> anyhow::Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("no avilable output device");

        let stream_config = device
            .supported_output_configs()?
            .next()
            .ok_or(anyhow!("failed to find supported config"))?
            .with_max_sample_rate();

        let sample_rate = stream_config.sample_rate();

        let config = stream_config.config();
        Ok(Self {
            _host: host,
            device,
            config,
            sample_rate,
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

    pub fn spawn(
        &mut self,
        mut consumer: HeapCons<f32>,
        count: Arc<AtomicUsize>,
        signal: Arc<AtomicBool>,
    ) {
        let channels = self.config.channels;

        let stream = self
            .device
            .build_output_stream(
                &self.config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let r_lenth = consumer.pop_slice(data);
                    for sample in &mut data[..r_lenth] {
                        *sample *= 0.3;
                    }
                    for sample in &mut data[r_lenth..] {
                        *sample = 0.0;
                    }
                    signal.store(true, Ordering::Release);

                    count.store(
                        count.load(Ordering::Relaxed) + r_lenth / channels as usize,
                        Ordering::Relaxed,
                    );
                },
                move |err| {
                    println!("error when playing: {}", err);
                },
                None,
            )
            .unwrap();

        stream.play().unwrap();
        self.stream = Some(stream);
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        println!("DEBUG: dropped player");
    }
}
