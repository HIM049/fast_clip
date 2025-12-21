use std::path::PathBuf;

use anyhow::anyhow;
use ffmpeg_next::{
    decoder,
    format::{self, context},
    software::scaling::{self},
};
use gpui::{Context, Entity};

use crate::ui::{app::MyApp, player_size::PlayerSize};

pub struct VideoDecoder {
    input: Option<context::Input>,
    stream_ix: usize,
    decoder: Option<decoder::Video>,

    size: Entity<PlayerSize>,
}

impl VideoDecoder {
    pub fn new(size_entity: Entity<PlayerSize>) -> Self {
        Self {
            input: None,
            stream_ix: 0,
            decoder: None,

            size: size_entity,
        }
    }

    pub fn open(&mut self, cx: &mut Context<MyApp>, path: PathBuf) -> anyhow::Result<()> {
        let i = ffmpeg_next::format::input(&path)?;
        let stream = i
            .streams()
            .best(ffmpeg_next::media::Type::Video)
            .ok_or(anyhow!("failed to find best"))?;

        let decoder = ffmpeg_next::codec::context::Context::from_parameters(stream.parameters())?
            .decoder()
            .video()?;

        let frame_rate = stream.avg_frame_rate();
        let frames = stream.frames();

        let orignal_width = decoder.width();
        let orignal_height = decoder.height();

        println!("frame rate: {}, frames: {}", frame_rate, frames);

        self.stream_ix = stream.index();
        self.input = Some(i);
        self.decoder = Some(decoder);

        self.size.update(cx, |p, _| {
            p.set_orignal((orignal_width, orignal_height));
        });

        Ok(())
    }

    pub fn run(&mut self, cx: &mut Context<MyApp>) -> Option<Vec<u8>> {
        let Some(input) = self.input.as_mut() else {
            return None;
        };
        let Some(decoder) = self.decoder.as_mut() else {
            return None;
        };
        // let output_size = self.size.read(cx);
        let src_w = decoder.width();
        let src_h = decoder.height();
        let dst_w = decoder.width();
        let dst_h = decoder.height();
        // let dst_w = output_size.output_size().0;
        // let dst_h = output_size.output_size().1;

        let mut decoded_frame = ffmpeg_next::frame::Video::empty();
        let mut scaled_frame = ffmpeg_next::frame::Video::new(format::Pixel::RGBA, dst_w, dst_h);
        let mut buffer = Vec::with_capacity((dst_w * dst_h * 4) as usize);

        let mut scaler_context = ffmpeg_next::software::scaling::Context::get(
            decoder.format(),
            src_w,
            src_h,
            format::Pixel::RGBA,
            dst_w,
            dst_h,
            scaling::Flags::BILINEAR,
        )
        .unwrap();

        // println!("packets {}", input.packets().count());

        // get packet
        for (stream, packet) in input.packets() {
            println!("into stream_ix {}", stream.index());
            // check whether stream index is target viedo
            if stream.index() != self.stream_ix {
                continue;
            }
            // try to send packet to decoder
            if decoder.send_packet(&packet).is_err() {
                println!("send packet");
                return None;
            }

            // try receive decoder
            if decoder.receive_frame(&mut decoded_frame).is_ok() {
                println!("received frame {}", stream.index());
                scaler_context
                    .run(&decoded_frame, &mut scaled_frame)
                    .unwrap();

                let data = scaled_frame.data(0);
                let stride = scaled_frame.stride(0);

                for y in 0..dst_h as usize {
                    let start = y * stride;
                    let end = start + (dst_w as usize * 4);
                    buffer.extend_from_slice(&data[start..end]);
                }
                break;
            }
        }
        Some(buffer)
    }
}
