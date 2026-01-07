use anyhow::anyhow;
use cpal::{
    StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use ringbuf::{HeapCons, traits::Consumer};

use crate::ui::player::frame::FrameAudio;

pub struct AudioPlayer {
    _host: cpal::Host,
    device: cpal::Device,
    config: StreamConfig,
    stream: Option<cpal::Stream>,
}

impl AudioPlayer {
    pub fn new() -> anyhow::Result<Self> {
        println!("new player");
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("no avilable output device");

        let stream_config = device
            .supported_output_configs()?
            .next()
            .ok_or(anyhow!("failed to find supported config"))?
            .with_max_sample_rate();

        let config = stream_config.config();
        Ok(Self {
            _host: host,
            device,
            config,
            stream: None,
        })
    }

    pub fn spawn(mut self, mut consumer: HeapCons<FrameAudio>) -> Self {
        println!("spawn player");

        let stream = self
            .device
            .build_output_stream(
                &self.config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    if let Some(f) = consumer.try_pop() {
                        let len = data.len().min(f.sample.len());
                        data[..len].copy_from_slice(&f.sample[..len]);
                        // println!("frame len {}, want len {}", len, data.len());
                    }
                },
                move |err| {
                    println!("error when playing: {}", err);
                },
                None,
            )
            .unwrap();

        stream.play().unwrap();
        self.stream = Some(stream);

        self
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        println!("dropped player");
    }
}
