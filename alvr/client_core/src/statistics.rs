use alvr_common::SlidingWindowAverage;
use alvr_packets::ClientStatistics;
use alvr_packets::RateUpdateMode;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
use chrono::{Utc, TimeZone};
use crate::NADA_RECEIVER;

const FULL_REPORT_INTERVAL: Duration = Duration::from_millis(1000);

struct HistoryFrame {
    input_acquired: Instant,
    video_packet_received: Instant,
    client_stats: ClientStatistics,
    frame_send_timestamp: i64,
}

pub struct StatisticsManager {
    history_buffer: VecDeque<HistoryFrame>,
    max_history_size: usize,
    prev_vsync: Instant,
    total_pipeline_latency_average: SlidingWindowAverage<Duration>,
    steamvr_pipeline_latency: Duration,
    recv_size_sum: usize,
    last_bitrate_report_instant: Instant,
}

impl StatisticsManager {
    pub fn new(
        max_history_size: usize,
        nominal_server_frame_interval: Duration,
        steamvr_pipeline_frames: f32,
    ) -> Self {
        Self {
            max_history_size,
            history_buffer: VecDeque::new(),
            prev_vsync: Instant::now(),
            total_pipeline_latency_average: SlidingWindowAverage::new(
                Duration::ZERO,
                max_history_size,
            ),
            steamvr_pipeline_latency: Duration::from_secs_f32(
                steamvr_pipeline_frames * nominal_server_frame_interval.as_secs_f32(),
            ),
            recv_size_sum:0,
            last_bitrate_report_instant: Instant::now(),
        }
    }

    pub fn report_input_acquired(&mut self, target_timestamp: Duration) {
        if !self
            .history_buffer
            .iter()
            .any(|frame| frame.client_stats.target_timestamp == target_timestamp)
        {
            self.history_buffer.push_front(HistoryFrame {
                input_acquired: Instant::now(),
                // this is just a placeholder because Instant does not have a default value
                video_packet_received: Instant::now(),
                client_stats: ClientStatistics {
                    target_timestamp,
                    ..Default::default()
                },
                frame_send_timestamp: 0,
            });
        }

        if self.history_buffer.len() > self.max_history_size {
            self.history_buffer.pop_back();
        }
    }

    pub fn report_video_packet_received(&mut self, target_timestamp: Duration, arrival_ts: i64,size:  usize,frame_send_timestamp: i64) {
        if let Some(frame) = self
            .history_buffer
            .iter_mut()
            .find(|frame| frame.client_stats.target_timestamp == target_timestamp)
        {
            frame.video_packet_received = Instant::now();
            frame.client_stats.frame_arrival_timestamp=arrival_ts;
            frame.frame_send_timestamp = frame_send_timestamp;
            self.recv_size_sum +=size;
            if self.last_bitrate_report_instant + FULL_REPORT_INTERVAL < Instant::now() {
                self.last_bitrate_report_instant += FULL_REPORT_INTERVAL;
                let interval_secs = FULL_REPORT_INTERVAL.as_secs_f32();
                frame.client_stats.recv_bitrate_report_mbps = (self.recv_size_sum as f32 * 8.
                / 1e6
                / interval_secs);
                self.recv_size_sum = 0;
            }

            // Lock NADA_RECEIVER & get ref, then call functions through ref
            {
                let mut nada_receiver = NADA_RECEIVER.lock();
                nada_receiver.compute_oneway_delay(frame_send_timestamp, arrival_ts);
                nada_receiver.update_receive_loss_rate(size);
                let is_feedback_on = nada_receiver.time_to_report_feedback(false, false);

                //if there is a feedback to report
                if is_feedback_on{
                    //send RTCP feedback report containing values of: rmode, x_curr, and r_recv
                    frame.client_stats.nada_feedback = true;
                    frame.client_stats.nada_xcurr = nada_receiver.x_curr;
                    frame.client_stats.nada_rmode = match nada_receiver.rmode {
                        RateUpdateMode::AcceleratedRampUp => 0,
                        RateUpdateMode::GradualUpdate => 1,
                        _ => 1,
                    };
                    frame.client_stats.nada_recv = nada_receiver.r_recv;

                    //To Debug NADA Receiver, report values of: t_last, d_fwd, d_tilde, d_queue, p_loss
                    frame.client_stats.plr = nada_receiver.p_loss;
                    frame.client_stats.d_fwd = nada_receiver.d_fwd;
                    frame.client_stats.d_tilde = nada_receiver.d_tilde;
                    frame.client_stats.d_queue = nada_receiver.d_queue;

                    //update t_last = t_curr
                    nada_receiver.update_t_last();
                }else{
                    frame.client_stats.nada_feedback = false;
                }
            }
        }
    }

    pub fn report_frame_decoded(&mut self, target_timestamp: Duration) {
        if let Some(frame) = self
            .history_buffer
            .iter_mut()
            .find(|frame| frame.client_stats.target_timestamp == target_timestamp)
        {
            frame.client_stats.video_decode =
                Instant::now().saturating_duration_since(frame.video_packet_received);
        }
    }

    pub fn report_compositor_start(&mut self, target_timestamp: Duration) {
        if let Some(frame) = self
            .history_buffer
            .iter_mut()
            .find(|frame| frame.client_stats.target_timestamp == target_timestamp)
        {
            frame.client_stats.video_decoder_queue = Instant::now().saturating_duration_since(
                frame.video_packet_received + frame.client_stats.video_decode,
            );
        }
    }

    // vsync_queue is the latency between this call and the vsync. it cannot be measured by ALVR and
    // should be reported by the VR runtime
    pub fn report_submit(&mut self, target_timestamp: Duration, vsync_queue: Duration) {
        let now = Instant::now();

        if let Some(frame) = self
            .history_buffer
            .iter_mut()
            .find(|frame| frame.client_stats.target_timestamp == target_timestamp)
        {
            frame.client_stats.rendering = now.saturating_duration_since(
                frame.video_packet_received
                    + frame.client_stats.video_decode
                    + frame.client_stats.video_decoder_queue,
            );
            frame.client_stats.vsync_queue = vsync_queue;
            frame.client_stats.total_pipeline_latency =
                now.saturating_duration_since(frame.input_acquired) + vsync_queue;
            self.total_pipeline_latency_average
                .submit_sample(frame.client_stats.total_pipeline_latency);

            let vsync = now + vsync_queue;
            frame.client_stats.frame_interval = vsync.saturating_duration_since(self.prev_vsync);
            self.prev_vsync = vsync;
        }
    }

    pub fn summary(&self, target_timestamp: Duration) -> Option<ClientStatistics> {
        self.history_buffer
            .iter()
            .find(|frame| frame.client_stats.target_timestamp == target_timestamp)
            .map(|frame| frame.client_stats.clone())
    }

    // latency used for head prediction
    pub fn average_total_pipeline_latency(&self) -> Duration {
        self.total_pipeline_latency_average.get_average()
    }

    // latency used for controllers/trackers prediction
    pub fn tracker_prediction_offset(&self) -> Duration {
        self.total_pipeline_latency_average
            .get_average()
            .saturating_sub(self.steamvr_pipeline_latency)
    }
}
