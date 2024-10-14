use crate::{gcc_config::GCC_INIT_CONFIGURED_BITRATE, gcc_delay_based_controller::{self, AimdRateControl, BandwidthUsage, BitrateEstimator, RateControlInput, TrendlineEstimator}};

use chrono::{Utc};
pub struct GccBandwidthEstimator{
    pub trendline_manager : TrendlineEstimator,
    pub aimd_manager : AimdRateControl,
    pub rate_control_input_manager : RateControlInput,
    pub bitrate_estimator_manager : BitrateEstimator,
    pub last_frame_send_timestamp : f64,
    pub last_frame_arrival_timestamp : f64,
}
impl GccBandwidthEstimator{
    pub fn new()-> Self {
        Self { 
                trendline_manager: TrendlineEstimator::new(),
                aimd_manager: AimdRateControl::new(true),
                rate_control_input_manager: RateControlInput::new(BandwidthUsage::kBwNormal, Some(GCC_INIT_CONFIGURED_BITRATE)),
                bitrate_estimator_manager: BitrateEstimator::new(),
                last_frame_send_timestamp: 0.,
                last_frame_arrival_timestamp: 0., 
            }
    }

    pub fn Update(&mut self, current_frame_send_timestamp: f64, current_frame_arrival_timestamp: f64, current_frame_size: i64)-> f64{
        let mut send_delta_ms= 0.0;
        let mut recv_delta_ms = 0.0;
        
        if self.last_frame_send_timestamp!=0.{
            send_delta_ms = (current_frame_send_timestamp - self.last_frame_send_timestamp)*0.001;
            recv_delta_ms = (current_frame_arrival_timestamp - self.last_frame_arrival_timestamp)*0.001;
        }

        self.last_frame_send_timestamp = current_frame_send_timestamp;
        self.last_frame_arrival_timestamp = current_frame_arrival_timestamp;
        let send_time_ms = (current_frame_send_timestamp*0.001) as i64;
        let arrival_time_ms = (current_frame_arrival_timestamp*0.001) as i64;
        let packet_size = current_frame_size;
        
        self.trendline_manager.UpdateTrendline(recv_delta_ms, send_delta_ms, send_time_ms, arrival_time_ms, packet_size);

        if self.trendline_manager.hypothesis_ == BandwidthUsage::kBwNormal{
            self.rate_control_input_manager.bw_state = BandwidthUsage::kBwNormal;
        }else if self.trendline_manager.hypothesis_ == BandwidthUsage::kBwOverusing{
            self.rate_control_input_manager.bw_state = BandwidthUsage::kBwOverusing;
        }else{
            self.rate_control_input_manager.bw_state = BandwidthUsage::kBwUnderusing;
        }

        self.bitrate_estimator_manager.Update(arrival_time_ms, packet_size as usize, false);

        self.rate_control_input_manager.estimated_throughput = Some(self.bitrate_estimator_manager.bitrate().unwrap());

        let at_time = (Utc::now().timestamp_micros() as f64 * 0.001) as i64;

        self.aimd_manager.Update(&self.rate_control_input_manager, at_time);

        let target_bitrate_bps = self.aimd_manager.current_bitrate_;

        return target_bitrate_bps;
    }

    pub fn get_target_bitrate_bps(&mut self)->f64{
        return self.aimd_manager.current_bitrate_;
    }
}