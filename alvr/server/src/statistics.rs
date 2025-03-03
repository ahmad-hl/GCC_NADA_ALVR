use alvr_common::{SlidingWindowAverage, HEAD_ID};
use alvr_events::{EventType, GraphStatistics, NominalBitrateStats, StatisticsSummary};
use alvr_packets::ClientStatistics;
use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};
use csv::Writer;
use std::fs::OpenOptions;
use std::error::Error;
const FULL_REPORT_INTERVAL: Duration = Duration::from_millis(1000);
use chrono::{Utc, TimeZone, Local, format::{strftime, StrftimeItems}};
use crate::GCC_BANDWIDTH_ESTIMATOR;
pub struct HistoryFrame {
    target_timestamp: Duration,
    tracking_received: Instant,
    frame_present: Instant,
    frame_present_MTP: Instant,
    frame_composed: Instant,
    frame_composed_MTP: Instant,
    frame_encoded: Instant,
    frame_encoded_MTP: Instant,
    video_packet_bytes: usize,
    video_packet_bytes_MTP: usize,
    total_pipeline_latency: Duration,
    total_pipeline_latency_MTP: Duration,
    reported:bool,//wz repeat
    last_repeat_game_latency:Duration,//wz repeat
    frame_send_timestamp:i64,
    encode_times: i32,
    composition_times: i32,
    tracking_rece_times: i32,
    frame_present_times: i32,
    MTP_reported: bool
    
}

impl Default for HistoryFrame {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            target_timestamp: Duration::ZERO,
            tracking_received: now,
            frame_present: now,
            frame_present_MTP: now,
            frame_composed: now,
            frame_composed_MTP: now,
            frame_encoded: now,
            frame_encoded_MTP: now,
            video_packet_bytes: 0,//total size for this encoded frame
            video_packet_bytes_MTP: 0,
            total_pipeline_latency: Duration::ZERO,
            total_pipeline_latency_MTP: Duration::ZERO,
            reported: false,//wz repeat
            last_repeat_game_latency: Duration::ZERO,//wz repeat
            frame_send_timestamp:Utc::now().timestamp_micros(),
            encode_times: 0,
            composition_times: 0,
            tracking_rece_times: 0,
            frame_present_times: 0,
            MTP_reported: false,

            
        }
    }
}

#[derive(Default, Clone)]
struct BatteryData {
    gauge_value: f32,
    is_plugged: bool,
}

fn write_latency_to_csv(filename: &str, latency_values: [String; 19]) -> Result<(), Box<dyn Error>> {

    let file = OpenOptions::new().write(true).append(true).open(filename)?;
    let mut writer = Writer::from_writer(file);

    // Write the latency strings in the next row
    writer.write_record(&[
        &latency_values[0],
        &latency_values[1],
        &latency_values[2],
        &latency_values[3],
        &latency_values[4],
        &latency_values[5],
        &latency_values[6],
        &latency_values[7],
        &latency_values[8],
        &latency_values[9],

        &latency_values[10],
        &latency_values[11],
        &latency_values[12],
        &latency_values[13],
        &latency_values[14],
        &latency_values[15],
        &latency_values[16],
        &latency_values[17],
        &latency_values[18],

    ])?;

    Ok(())
}
fn write_pending_stats_to_csv(filename: &str, latency_values: [String; 9]) -> Result<(), Box<dyn Error>> {

    let file = OpenOptions::new().write(true).append(true).open(filename)?;
    let mut writer = Writer::from_writer(file);

    // Write the latency strings in the next row
    writer.write_record(&[
        &latency_values[0],
        &latency_values[1],
        &latency_values[2],
        &latency_values[3],
        &latency_values[4],
        &latency_values[5],
        &latency_values[6],
        &latency_values[7],
        &latency_values[8],

    ])?;

    Ok(())
}
pub struct StatisticsManager {
    history_buffer: VecDeque<HistoryFrame>,
    max_history_size: usize,
    last_full_report_instant: Instant,
    last_frame_present_instant: Instant,
    last_frame_present_interval: Duration,
    video_packets_total: usize,
    video_packets_partial_sum: usize,
    video_bytes_total: usize,
    video_bytes_partial_sum: usize,
    packets_lost_total: usize,
    packets_lost_partial_sum: usize,
    battery_gauges: HashMap<u64, BatteryData>,
    steamvr_pipeline_latency: Duration,
    total_pipeline_latency_average: SlidingWindowAverage<Duration>,
    last_vsync_time: Instant,
    frame_interval: Duration,
    last_nominal_bitrate_stats: NominalBitrateStats,
    gcc_target_bitrate_mbps: f64,
    video_bytes_partial_sum_pending: usize,
    last_time_bitrate_report_for_pending: Instant,


}

impl StatisticsManager {
    // history size used to calculate average total pipeline latency
    pub fn new(
        max_history_size: usize,
        nominal_server_frame_interval: Duration,
        steamvr_pipeline_frames: f32,
    ) -> Self {
        Self {
            history_buffer: VecDeque::new(),
            max_history_size,
            last_full_report_instant: Instant::now(),
            last_frame_present_instant: Instant::now(),
            last_frame_present_interval: Duration::ZERO,
            video_packets_total: 0,
            video_packets_partial_sum: 0,
            video_bytes_total: 0,
            video_bytes_partial_sum: 0,
            packets_lost_total: 0,
            packets_lost_partial_sum: 0,
            battery_gauges: HashMap::new(),
            steamvr_pipeline_latency: Duration::from_secs_f32(
                steamvr_pipeline_frames * nominal_server_frame_interval.as_secs_f32(),
            ),
            total_pipeline_latency_average: SlidingWindowAverage::new(
                Duration::ZERO,
                max_history_size,
            ),
            last_vsync_time: Instant::now(),
            frame_interval: nominal_server_frame_interval,
            last_nominal_bitrate_stats: NominalBitrateStats::default(),
            gcc_target_bitrate_mbps: 150.,
            video_bytes_partial_sum_pending: 0,
            last_time_bitrate_report_for_pending: Instant::now(),
        }
    }

    pub fn report_tracking_received(&mut self, target_timestamp: Duration) {
        if !self
            .history_buffer
            .iter()
            .any(|frame| frame.target_timestamp == target_timestamp)
        {
            self.history_buffer.push_front(HistoryFrame {
                target_timestamp,
                tracking_received: Instant::now(),
                ..Default::default()
            });
        }

        if self.history_buffer.len() > self.max_history_size {
            self.history_buffer.pop_back();
        }
    }

    pub fn report_frame_present(&mut self, target_timestamp: Duration, offset: Duration) {
        if let Some(frame) = self
            .history_buffer
            .iter_mut()
            .find(|frame| frame.target_timestamp == target_timestamp)
        {
            if frame.frame_present_times == 0 {
                let now = Instant::now() - offset;
                frame.frame_present_MTP = now;
            }
            let now = Instant::now() - offset;

            self.last_frame_present_interval =
                now.saturating_duration_since(self.last_frame_present_instant);
            self.last_frame_present_instant = now;

            frame.frame_present = now;
            frame.frame_present_times +=1;
        }
    }

    pub fn report_frame_composed(&mut self, target_timestamp: Duration, offset: Duration) {
        if let Some(frame) = self
            .history_buffer
            .iter_mut()
            .find(|frame| frame.target_timestamp == target_timestamp)
        {
            if frame.composition_times == 0{
                frame.frame_composed_MTP = Instant::now() - offset;
            }
            frame.frame_composed = Instant::now() - offset;
            frame.composition_times +=1;
        }
    }

    // returns encoding interval
    pub fn report_frame_encoded(
        &mut self,
        target_timestamp: Duration,
        bytes_count: usize,
    ) -> Duration {
        self.video_packets_total += 1;
        self.video_packets_partial_sum += 1;
        self.video_bytes_total += bytes_count;
        self.video_bytes_partial_sum += bytes_count;
        self.video_bytes_partial_sum_pending += bytes_count;

        if let Some(frame) = self
            .history_buffer
            .iter_mut()
            .find(|frame| frame.target_timestamp == target_timestamp)
        {
            if frame.encode_times == 0{
                frame.frame_encoded_MTP = Instant::now();
                frame.video_packet_bytes_MTP = bytes_count;
                let _ = frame.frame_encoded_MTP.saturating_duration_since(frame.frame_composed_MTP);
            }
            frame.frame_encoded = Instant::now();

            frame.video_packet_bytes = bytes_count;
            frame.encode_times +=1;
            frame
                .frame_encoded
                .saturating_duration_since(frame.frame_composed)
        } else {
            Duration::ZERO
        }
        
    }
    pub fn report_pending_stats(&mut self,target_timestamp: Duration){
        if let Some(frame) = self
            .history_buffer
            .iter_mut()
            .find(|frame| frame.target_timestamp == target_timestamp)
        {
            let target_ts_nanos=(frame.target_timestamp.as_nanos()).to_string();
            let game_latency_ms=(frame.frame_present.saturating_duration_since(frame.tracking_received).as_secs_f32()*1000.).to_string();
            let composite_latency_ms=(frame.frame_composed.saturating_duration_since(frame.frame_present).as_secs_f32()*1000.).to_string();
            let encode_latency_ms=(frame.frame_encoded.saturating_duration_since(frame.frame_composed).as_secs_f32()*1000.).to_string();
            let encoded_frame_size=frame.video_packet_bytes.to_string();
            let mut bitrate_mbps = "".to_string();
            if self.last_time_bitrate_report_for_pending + FULL_REPORT_INTERVAL < Instant::now() {
                self.last_time_bitrate_report_for_pending += FULL_REPORT_INTERVAL;

                let interval_secs = FULL_REPORT_INTERVAL.as_secs_f32();
                bitrate_mbps = (self.video_bytes_partial_sum_pending as f32 * 8.
                    / 1e6
                    / interval_secs).to_string();
                self.video_bytes_partial_sum_pending = 0;
            }
            let gcc_target_bitrate_mbps=(self.gcc_target_bitrate_mbps).to_string();
            let send_ts_ms=frame.frame_send_timestamp.to_string();
            let experiment_target_timestamp=Local::now().format("%Y%m%d%H%M%S").to_string();
            let latency_strings=[target_ts_nanos,game_latency_ms,composite_latency_ms,encode_latency_ms,encoded_frame_size,bitrate_mbps,gcc_target_bitrate_mbps,send_ts_ms,experiment_target_timestamp];
            let _ = write_pending_stats_to_csv("alvr_statistics_pending.csv", latency_strings);

        }
    }
    pub fn report_send_timestamp(&mut self,target_timestamp: Duration, send_ts: i64)
    {
        if let Some(frame) = self
            .history_buffer
            .iter_mut()
            .find(|frame| frame.target_timestamp == target_timestamp)
        {
            frame.frame_send_timestamp = send_ts;
        }

    }
    pub fn report_packet_loss(&mut self) {
        self.packets_lost_total += 1;
        self.packets_lost_partial_sum += 1;
    }

    pub fn report_battery(&mut self, device_id: u64, gauge_value: f32, is_plugged: bool) {
        *self.battery_gauges.entry(device_id).or_default() = BatteryData {
            gauge_value,
            is_plugged,
        };
    }

    pub fn report_nominal_bitrate_stats(&mut self, stats: NominalBitrateStats) {
        self.last_nominal_bitrate_stats = stats;
    }

    // Called every frame. Some statistics are reported once every frame
    // Returns network latency
    pub fn report_statistics(&mut self, client_stats: ClientStatistics) -> (Duration,String) {
        if let Some(frame) = self
            .history_buffer
            .iter_mut()
            .find(|frame| frame.target_timestamp == client_stats.target_timestamp)
        {
            frame.total_pipeline_latency = client_stats.total_pipeline_latency;

            let mut game_time_latency = frame
                .frame_present
                .saturating_duration_since(frame.tracking_received);

            let server_compositor_latency = frame
                .frame_composed
                .saturating_duration_since(frame.frame_present);

            let encoder_latency = frame
                .frame_encoded
                .saturating_duration_since(frame.frame_composed);

            // The network latency cannot be estiamed directly. It is what's left of the total
            // latency after subtracting all other latency intervals. In particular it contains the
            // transport latency of the tracking packet and the interval between the first video
            // packet is sent and the last video packet is received for a specific frame.
            // For safety, use saturating_sub to avoid a crash if for some reason the network
            // latency is miscalculated as negative.
            let network_latency = frame.total_pipeline_latency.saturating_sub(
                game_time_latency
                    + server_compositor_latency
                    + encoder_latency
                    + client_stats.video_decode
                    + client_stats.video_decoder_queue
                    + client_stats.rendering
                    + client_stats.vsync_queue,
            );

            let client_fps = 1.0
                / client_stats
                    .frame_interval
                    .max(Duration::from_millis(1))
                    .as_secs_f32();
            let server_fps = 1.0
                / self
                    .last_frame_present_interval
                    .max(Duration::from_millis(1))
                    .as_secs_f32();
            let mut bitrate_mbps = "".to_string();
            if self.last_full_report_instant + FULL_REPORT_INTERVAL < Instant::now() {
                self.last_full_report_instant += FULL_REPORT_INTERVAL;

                let interval_secs = FULL_REPORT_INTERVAL.as_secs_f32();
                bitrate_mbps = (self.video_bytes_partial_sum as f32 * 8.
                    / 1e6
                    / interval_secs).to_string();
                alvr_events::send_event(EventType::StatisticsSummary(StatisticsSummary {
                    video_packets_total: self.video_packets_total,
                    video_packets_per_sec: (self.video_packets_partial_sum as f32 / interval_secs)
                        as _,
                    video_mbytes_total: (self.video_bytes_total as f32 / 1e6) as usize,
                    video_mbits_per_sec: self.video_bytes_partial_sum as f32 * 8.
                        / 1e6
                        / interval_secs,
                    total_latency_ms: client_stats.total_pipeline_latency.as_secs_f32() * 1000.,
                    network_latency_ms: network_latency.as_secs_f32() * 1000.,
                    encode_latency_ms: encoder_latency.as_secs_f32() * 1000.,
                    decode_latency_ms: client_stats.video_decode.as_secs_f32() * 1000.,
                    packets_lost_total: self.packets_lost_total,
                    packets_lost_per_sec: (self.packets_lost_partial_sum as f32 / interval_secs)
                        as _,
                    client_fps: client_fps as _,
                    server_fps: server_fps as _,
                    battery_hmd: (self
                        .battery_gauges
                        .get(&HEAD_ID)
                        .cloned()
                        .unwrap_or_default()
                        .gauge_value
                        * 100.) as u32,
                    hmd_plugged: self
                        .battery_gauges
                        .get(&HEAD_ID)
                        .cloned()
                        .unwrap_or_default()
                        .is_plugged,
                }));
                
                self.video_packets_partial_sum = 0;
                self.video_bytes_partial_sum = 0;
                self.packets_lost_partial_sum = 0;
            }
            let return_bitrate_mbps = bitrate_mbps.clone();
            if frame.reported{
                game_time_latency=game_time_latency.saturating_sub(frame.last_repeat_game_latency);
                
                frame.total_pipeline_latency=frame.total_pipeline_latency.saturating_sub(frame.last_repeat_game_latency);
                //frame.total_pipeline_latency-=frame.last_repeat_game_latency;
            }
            frame.reported=true;
            frame.last_repeat_game_latency+=game_time_latency;

            // While not accurate, this prevents NaNs and zeros that would cause a crash or pollute
            // the graph
            let bitrate_bps = if network_latency != Duration::ZERO {
                frame.video_packet_bytes as f32 * 8.0 / network_latency.as_secs_f32()
            } else {
                0.0
            };

            // todo: use target timestamp in nanoseconds. the dashboard needs to use the first
            // timestamp as the graph time origin.
            alvr_events::send_event(EventType::GraphStatistics(GraphStatistics {
                total_pipeline_latency_s: frame.total_pipeline_latency.as_secs_f32(),
                game_time_s: game_time_latency.as_secs_f32(),
                server_compositor_s: server_compositor_latency.as_secs_f32(),
                encoder_s: encoder_latency.as_secs_f32(),
                network_s: network_latency.as_secs_f32(),
                decoder_s: client_stats.video_decode.as_secs_f32(),
                decoder_queue_s: client_stats.video_decoder_queue.as_secs_f32(),
                client_compositor_s: client_stats.rendering.as_secs_f32(),
                vsync_queue_s: client_stats.vsync_queue.as_secs_f32(),
                client_fps,
                server_fps,
                nominal_bitrate: self.last_nominal_bitrate_stats.clone(),
                actual_bitrate_bps: bitrate_bps,
            }));

            let gcc_target_bitrate_bps = GCC_BANDWIDTH_ESTIMATOR.lock().Update(frame.frame_send_timestamp as f64, client_stats.frame_arrival_timestamp as f64, frame.video_packet_bytes as i64);
            self.gcc_target_bitrate_mbps = gcc_target_bitrate_bps/1000./1000.;
            let timestamp_for_this_frame=(frame.target_timestamp.as_nanos()).to_string();
            let interval_trackingReceived_framePresentInVirtualDevice=(game_time_latency.as_secs_f32()*1000.).to_string();//game latency
            let interval_framePresentInVirtualDevice_frameComposited=(server_compositor_latency.as_secs_f32()*1000.).to_string();//composite latency
            let interval_frameComposited_VideoEncoded=(encoder_latency.as_secs_f32() * 1000.).to_string();//encode latency
            let interval_VideoReceivedByClient_VideoDecoded=(client_stats.video_decode.as_secs_f32() * 1000.).to_string();//decode latency
            let interval_network=((network_latency.as_secs_f32()*1000.).to_string());//network latency(interval_trackingsend_trackingreceived+interval_encodedVideoSend_encodedVideoReceived)
            //let decode_latency = (client_stats.video_decode.as_secs_f32()*1000.).to_string();
            let client_dequeue_latency=(client_stats.video_decoder_queue.as_secs_f32()*1000.).to_string();
            let client_rendering_latency=(client_stats.rendering.as_secs_f32()*1000.).to_string();
            let client_vsync_queue_latency=(client_stats.vsync_queue.as_secs_f32()*1000.).to_string();
            let interval_total_pipeline=(frame.total_pipeline_latency.as_secs_f32() * 1000.).to_string();//total pipeline latency wz repeat
            let total_size_for_this_encoded_frame_bytes=frame.video_packet_bytes.to_string();//bytes for this frame
            let frame_send_ts=frame.frame_send_timestamp.to_string();
            let frame_arrival_ts=client_stats.frame_arrival_timestamp.to_string();
            let server_fps=server_fps.to_string();
            let client_fps=client_fps.to_string();
            let gcc_target_bitrate_mbps_string = (gcc_target_bitrate_bps/1e6).to_string();
            let experiment_target_timestamp=Local::now().format("%Y%m%d%H%M%S").to_string();
            let latency_strings=[timestamp_for_this_frame,interval_trackingReceived_framePresentInVirtualDevice,interval_framePresentInVirtualDevice_frameComposited,interval_frameComposited_VideoEncoded,interval_VideoReceivedByClient_VideoDecoded,interval_network,
            client_dequeue_latency,client_rendering_latency,client_vsync_queue_latency,interval_total_pipeline,server_fps,client_fps,total_size_for_this_encoded_frame_bytes,gcc_target_bitrate_mbps_string,bitrate_mbps, client_stats.recv_bitrate_report_mbps.to_string(),frame_send_ts,
            frame_arrival_ts,experiment_target_timestamp];
            let _ = write_latency_to_csv("alvr_statistics.csv", latency_strings);
            (network_latency,return_bitrate_mbps)
        } else {
            (Duration::ZERO,"".to_string())
        }
    }

    pub fn video_pipeline_latency_average(&self) -> Duration {
        self.total_pipeline_latency_average.get_average()
    }

    pub fn tracker_pose_time_offset(&self) -> Duration {
        // This is the opposite of the client's StatisticsManager::tracker_prediction_offset().
        self.steamvr_pipeline_latency
            .saturating_sub(self.total_pipeline_latency_average.get_average())
    }

    // NB: this call is non-blocking, waiting should be done externally
    pub fn duration_until_next_vsync(&mut self) -> Duration {
        let now = Instant::now();

        // update the last vsync if it's too old
        while self.last_vsync_time + self.frame_interval < now {
            self.last_vsync_time += self.frame_interval;
        }

        (self.last_vsync_time + self.frame_interval).saturating_duration_since(now)
    }
}
