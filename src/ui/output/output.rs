use std::{ops::Range, path::PathBuf};

use anyhow::anyhow;

pub fn output(
    path: &PathBuf,
    out_path: &PathBuf,
    target_video_ix: usize,
    target_audio_ix: usize,
    time_range: &Range<f64>,
) -> anyhow::Result<()> {
    println!(
        "DEBUG: run output, path: {:?}, stream_ix: {}, time_range: {:?}",
        path, target_video_ix, time_range
    );
    // open source & seek to start point
    let mut input = ffmpeg_next::format::input(&path)?;
    let ts = (ffmpeg_next::sys::AV_TIME_BASE as f64 * time_range.start) as i64;
    input.seek(ts, ..ts)?;

    let mut output = ffmpeg_next::format::output(out_path)?;

    // create video stream
    let video_out_ix;
    {
        let v = input
            .stream(target_video_ix)
            .ok_or(anyhow!("failed to get target stream"))?;
        let mut v_out_stream = output.add_stream(None)?;
        v_out_stream.set_parameters(v.parameters());
        video_out_ix = v_out_stream.index();
    }
    // create audio stream
    let audio_out_ix;
    {
        let a = input
            .stream(target_audio_ix)
            .ok_or(anyhow!("failed to get target stream"))?;
        let mut a_out_stream = output.add_stream(None)?;
        a_out_stream.set_parameters(a.parameters());
        audio_out_ix = a_out_stream.index();
    }

    output.write_header()?;
    let video_out_tb = output
        .stream(video_out_ix)
        .ok_or(anyhow!("failed to get timebase"))?
        .time_base();
    let audio_out_tb = output
        .stream(audio_out_ix)
        .ok_or(anyhow!("failed to get timebase"))?
        .time_base();

    let mut v_offset_pts: Option<i64> = None;
    let mut v_offset_dts: Option<i64> = None;
    let mut a_offset_pts: Option<i64> = None;
    let mut a_offset_dts: Option<i64> = None;

    let mut output_state = (false, false);
    for (stream, mut packet) in input.packets() {
        let pkt_pts = packet.pts().unwrap_or(0);
        let pkt_dts = packet.dts().unwrap_or(pkt_pts);
        let frame_time = pkt_pts as f64 / stream.time_base().denominator() as f64;
        let this_ix = stream.index();
        // when video frame out the range
        if this_ix == target_video_ix && frame_time > time_range.end {
            output_state.0 = true;
        } else if this_ix == target_audio_ix && frame_time > time_range.end {
            output_state.1 = true;
        }
        if this_ix != target_video_ix && this_ix != target_audio_ix {
            continue;
        }
        println!(
            "OutputState {:?}, frame_time {}, target_time {}",
            output_state, frame_time, time_range.end
        );
        if output_state == (true, true) {
            break;
        }
        if this_ix == target_video_ix {
            if v_offset_pts.is_none() {
                v_offset_pts = packet.pts();
                v_offset_dts = packet.dts();
            }
            packet.set_pts(Some(pkt_pts - v_offset_pts.unwrap()));
            packet.set_dts(Some(pkt_dts - v_offset_dts.unwrap()));
            packet.set_stream(video_out_ix);
            packet.rescale_ts(stream.time_base(), video_out_tb);
        } else {
            if a_offset_pts.is_none() {
                a_offset_pts = packet.pts();
                a_offset_dts = packet.dts();
            }
            packet.set_pts(Some(pkt_pts - a_offset_pts.unwrap()));
            packet.set_dts(Some(pkt_dts - a_offset_dts.unwrap()));
            packet.set_stream(audio_out_ix);
            packet.rescale_ts(stream.time_base(), audio_out_tb);
        }
        packet.set_position(-1);
        packet.write_interleaved(&mut output)?;
    }

    output.write_trailer()?;
    Ok(())
}
