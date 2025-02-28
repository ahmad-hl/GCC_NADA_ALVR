use std::{
    collections::{HashMap, VecDeque}, f64::{NAN, INFINITY}, 
};
use crate::gcc_config;


#[derive(PartialEq)]
pub enum BandwidthUsage {
    kBwNormal = 0,
    kBwUnderusing = 1,
    kBwOverusing = 2,
    kLast,
}


pub struct PacketTiming{
    pub arrival_time_ms:f64,
    pub smoothed_delay_ms:f64,
    pub raw_delay_ms:f64,
}
impl PacketTiming {
    fn new(arrival_time_ms: f64, smoothed_delay_ms: f64, raw_delay_ms: f64) -> Self {
        Self {
            arrival_time_ms,
            smoothed_delay_ms,
            raw_delay_ms,
        }
    }
}


pub fn LinearFitSlope(packets:& VecDeque<PacketTiming>)->Option<f64> {
  if packets.len()>2 {
            // Compute the "center of mass".
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        for packet in packets {
            sum_x += packet.arrival_time_ms;
            sum_y += packet.smoothed_delay_ms;
        }
        let x_avg = sum_x / packets.len() as f64;
        let y_avg = sum_y / packets.len() as f64;
        // Compute the slope k = \sum (x_i-x_avg)(y_i-y_avg) / \sum (x_i-x_avg)^2
        let mut numerator = 0.0;
        let mut denominator = 0.0;
        for packet in packets {
            let x = packet.arrival_time_ms;
            let y = packet.smoothed_delay_ms;
            numerator += (x - x_avg) * (y - y_avg);
            denominator += (x - x_avg) * (x - x_avg);
        }
        if denominator == 0.0{
            return Option::None;
        }
            
        return Some(numerator / denominator);
  }else{
    return Option::None;
  }
}










pub struct TrendlineEstimator{
    //TrendlineEstimatorSettings settings_;
    pub smoothing_coef_: f64,
    pub threshold_gain_: f64,
    // Used by the existing threshold.
    pub num_of_deltas_: i64,
    // Keep the arrival times small by using the change from the first packet.
    pub first_arrival_time_ms_: i64,
    // Exponential backoff filtering.
    pub accumulated_delay_: f64,
    pub smoothed_delay_: f64,
    // Linear least squares regression.
    

    pub k_up_:f64,
    pub k_down_:f64,
    pub overusing_time_threshold_: f64,
    pub threshold_: f64,
    pub prev_modified_trend_: f64,
    pub last_update_ms_: i64,
    pub prev_trend_:f64,
    pub time_over_using_:f64,
    pub overuse_counter_: i64,
    pub hypothesis_: BandwidthUsage,
    pub hypothesis_predicted_: BandwidthUsage,
    //NetworkStatePredictor* network_state_predictor_;
    pub delay_hist_:VecDeque<PacketTiming>,
    pub current_trend_for_testing:f64,
    pub current_threshold_for_testing:f64,
}
impl TrendlineEstimator{
    pub fn new() -> Self {
        Self {
            
            smoothing_coef_: 0.9,
            threshold_gain_: 4.0,
            // Used by the existing threshold.
            num_of_deltas_: 0,
            // Keep the arrival times small by using the change from the first packet.
            first_arrival_time_ms_: -1,
            // Exponential backoff filtering.
            accumulated_delay_: 0.0,
            smoothed_delay_: 0.0,
            // Linear least squares regression.
            k_up_:0.0087,
            k_down_:0.039,
            overusing_time_threshold_: 10.0,
            threshold_: 12.5,//12.5
            prev_modified_trend_: NAN,
            last_update_ms_: -1,
            prev_trend_:0.0,
            time_over_using_:-1.0,
            overuse_counter_: 0,
            hypothesis_: BandwidthUsage::kBwNormal,
            hypothesis_predicted_: BandwidthUsage::kBwNormal,
            //NetworkStatePredictor* network_state_predictor_;
            delay_hist_:VecDeque::new(),
            current_threshold_for_testing:0.0,
            current_trend_for_testing:0.0,

        }
    }

    pub fn UpdateThreshold(&mut self,modified_trend:f64,
        now_ms: i64) {
        if self.last_update_ms_ == -1{
            self.last_update_ms_ = now_ms;
        }
        

        if modified_trend.abs() > self.threshold_ + 15.0 {
        // Avoid adapting the threshold to big latency spikes, caused e.g.,
        // by a sudden capacity drop.
        self.last_update_ms_ = now_ms;
        return;
        }
        let k = if modified_trend.abs() < self.threshold_ {
            self.k_down_
        } else {
            self.k_up_
        };
        
        let kMaxTimeDeltaMs = 100;
        let time_delta_ms = std::cmp::min(now_ms - self.last_update_ms_, kMaxTimeDeltaMs);
        self.threshold_ += k * (modified_trend.abs() - self.threshold_) * time_delta_ms as f64;
        if self.threshold_>600.0 as f64{
            self.threshold_=600.0;
        }else if self.threshold_<6.0 as f64{
            self.threshold_=6.0;
        }
        self.last_update_ms_ = now_ms;
    }


    pub fn Detect( &mut self,trend: f64,ts_delta: f64, now_ms: i64) {
        if self.num_of_deltas_ < 2 {  
          self.hypothesis_ = BandwidthUsage::kBwNormal;
          return;
        }
        let modified_trend =
            std::cmp::min(self.num_of_deltas_, 60) as f64 * trend * self.threshold_gain_;
        self.prev_modified_trend_ = modified_trend;
        
        if modified_trend > self.threshold_ {
          if self.time_over_using_ == -1.0 {
            // Initialize the timer. Assume that we've been
            // over-using half of the time since the previous
            // sample.
            self.time_over_using_ = ts_delta / 2.0;
          } else {
            // Increment timer
            self.time_over_using_ += ts_delta;
          }
          self.overuse_counter_+=1;
          if (self.time_over_using_ > self.overusing_time_threshold_ && self.overuse_counter_ > 1) {
            if trend >= self.prev_trend_ {
              self.time_over_using_ = 0.0;
              self.overuse_counter_ = 0;
              self.hypothesis_ = BandwidthUsage::kBwOverusing;
            }
          }
        } else if modified_trend < -self.threshold_ {
          self.time_over_using_ = -1.0;
          self.overuse_counter_ = 0;
          self.hypothesis_ = BandwidthUsage::kBwUnderusing;
        } else {
          self.time_over_using_ = -1.0;
          self.overuse_counter_ = 0;
          self.hypothesis_ = BandwidthUsage::kBwNormal;
        }
        self.current_threshold_for_testing=self.threshold_;
        self.current_trend_for_testing=modified_trend;
        self.prev_trend_ = trend;
        self.UpdateThreshold(modified_trend, now_ms);
      }
      




    pub fn UpdateTrendline(&mut self,recv_delta_ms:f64,
        send_delta_ms:f64,
        send_time_ms:i64,
        arrival_time_ms:i64,
        packet_size:i64) {
                let delta_ms = recv_delta_ms - send_delta_ms;
                self.num_of_deltas_+=1;
                self.num_of_deltas_ = std::cmp::min(self.num_of_deltas_, 1000);
                if self.first_arrival_time_ms_ == -1{
                    self.first_arrival_time_ms_ = arrival_time_ms;
                }
                

                // Exponential backoff filter.
                self.accumulated_delay_ += delta_ms;
                self.smoothed_delay_ = self.smoothing_coef_ * self.smoothed_delay_ +
                (1.0 - self.smoothing_coef_) * self.accumulated_delay_;
                
                // Maintain packet window
                self.delay_hist_.push_back(PacketTiming::new((arrival_time_ms-self.first_arrival_time_ms_) as f64,self.smoothed_delay_,self.accumulated_delay_));
                if self.delay_hist_.len() > gcc_config::GCC_WINDOW_SIZE {
                    self.delay_hist_.pop_front();
                }
                

                // Simple linear regression.
                let mut  trend = self.prev_trend_;
                if self.delay_hist_.len() == gcc_config::GCC_WINDOW_SIZE {
                // Update trend_ if it is possible to fit a line to the data. The delay
                // trend can be seen as an estimate of (send_rate - capacity)/capacity.
                // 0 < trend < 1   ->  the delay increases, queues are filling up
                //   trend == 0    ->  the delay does not change
                //   trend < 0     ->  the delay decreases, queues are being emptied
                trend = LinearFitSlope(&self.delay_hist_).unwrap_or(trend);
                
                }
                self.Detect(trend, send_delta_ms, arrival_time_ms);
        }
                

                
}
          
pub struct LinkCapacityEstimator{
    pub estimate_kbps_:Option<f64>,
    pub deviation_kbps_:f64,
}
impl LinkCapacityEstimator{
    pub fn new()->Self{
        Self { estimate_kbps_: Option::None, deviation_kbps_: 0.4}
    }
    pub fn deviation_estimate_kbps(&mut self)->f64 {
        return (self.deviation_kbps_*self.estimate_kbps_.unwrap()).sqrt();
    }
    pub fn  UpperBound(&mut self)->f64 {
        if !self.estimate_kbps_.is_none()
        {
            return (self.estimate_kbps_.unwrap()+ 3.0 * self.deviation_estimate_kbps())*1000.0;//check the unit of deviation estimate kbps
        }
          
        return f64::INFINITY;
      }
      
      pub fn LowerBound(&mut self)->f64 {
        if !self.estimate_kbps_.is_none(){
            return f64::max(0.0, self.estimate_kbps_.unwrap()-3.0*self.deviation_estimate_kbps())*1000.0;//check the unit of deviation estimate kbps
        }
          
        return 0.0;
      }
      
      pub fn Reset(&mut self){
        self.estimate_kbps_=Option::None;
      }
      
      pub fn OnOveruseDetected(&mut self, acknowledged_rate:f64) {
        self.Update(acknowledged_rate, 0.05);
      }
      
      pub fn OnProbeRate(&mut self, probe_rate:f64) {
        self.Update(probe_rate, 0.5);
      }
      
      pub fn Update(&mut self,capacity_sample:f64, alpha:f64) {
        let mut sample_kbps = capacity_sample*0.001;
        if self.estimate_kbps_.is_none() {
          self.estimate_kbps_ = Some(sample_kbps);
        } else {
          self.estimate_kbps_ = Some((1.0 - alpha) * self.estimate_kbps_.unwrap() + alpha * sample_kbps);
        }
        // Estimate the variance of the link capacity estimate and normalize the
        // variance with the link capacity estimate.
        let norm = f64::max(self.estimate_kbps_.unwrap(), 1.0);
        let mut error_kbps = self.estimate_kbps_.unwrap() - sample_kbps;
        self.deviation_kbps_ =
            (1.0 - alpha) * self.deviation_kbps_ + alpha * error_kbps * error_kbps / norm;
        // 0.4 ~= 14 kbit/s at 500 kbit/s
        // 2.5f ~= 35 kbit/s at 500 kbit/s
        if self.deviation_kbps_>2.5 as f64{
            self.deviation_kbps_=2.5;
        }else if self.deviation_kbps_<0.4 as f64{
            self.deviation_kbps_=0.4;
        }
        
      }
      
      pub fn has_estimate(&mut self) ->bool {
        return !self.estimate_kbps_.is_none();
      }
      
      pub fn  estimate(&mut self) ->f64 {
        return self.estimate_kbps_.unwrap()*1000.0;
      }
      
      
}
pub struct NetworkStateEstimate{
    pub confidence:f64,
    pub update_time:i64,
    pub last_receive_time:i64,
    pub last_send_time:i64,
    pub link_capacity:f64,
    pub link_capacity_lower:f64,
    pub link_capacity_upper:f64,
    pub pre_link_buffer_delay:i64,
    pub post_link_buffer_delay:i64,
    pub propagation_delay:i64,
}
impl Default for NetworkStateEstimate {
    fn default() -> Self {
        Self {
            confidence:f64::NAN,
            update_time:i64::MIN ,
            last_receive_time:i64::MIN,
            last_send_time:i64::MIN,
            link_capacity:-std::f64::INFINITY,
            link_capacity_lower:-std::f64::INFINITY,
            link_capacity_upper:-std::f64::INFINITY,
            pre_link_buffer_delay:i64::MIN,
            post_link_buffer_delay:i64::MIN,
            propagation_delay:i64::MIN,
        }
    }
}

#[derive(PartialEq)]
pub enum RateControlState {
    kRcHold = 0,
    kRcIncrease = 1,
    kRcDecrease = 2,
    
}
pub struct RateControlInput{
    pub bw_state:BandwidthUsage,
    pub estimated_throughput:Option<f64>,
}
impl RateControlInput{
    pub fn new(bw_state:BandwidthUsage,
        estimated_throughput:Option<f64>)->Self{
            Self{
                bw_state:bw_state,
                estimated_throughput:estimated_throughput,
            }
            
        }
}

pub struct AimdRateControl{
    pub min_configured_bitrate_:f64,
    pub max_configured_bitrate_:f64,
    pub current_bitrate_:f64,
    pub latest_estimated_throughput_:f64,
    pub link_capacity_:LinkCapacityEstimator,
    pub network_estimate_:Option<NetworkStateEstimate>,
    pub rate_control_state_:RateControlState,
    pub time_last_bitrate_change_:i64,
    pub time_last_bitrate_decrease_:i64,
    pub time_first_throughput_estimate_:i64,
    pub bitrate_is_initialized_:bool,
    pub beta_:f64,
    pub in_alr_:bool,
    pub rtt_:i64,
    pub send_side_:bool,
    pub no_bitrate_increase_in_alr_:bool,
    pub last_decrease_:Option<f64>,
    pub esitmate_thr_testing:f64,

}
impl AimdRateControl{
    pub fn new(send_side:bool)->Self{
        Self{
            min_configured_bitrate_:gcc_config::GCC_MIN_CONFIGURED_BITRATE,
            max_configured_bitrate_:gcc_config::GCC_MAX_CONFIGURED_BITRATE,
            current_bitrate_:gcc_config::GCC_INIT_CONFIGURED_BITRATE,
            latest_estimated_throughput_:gcc_config::GCC_INIT_CONFIGURED_BITRATE,
            link_capacity_:LinkCapacityEstimator::new() ,
            rate_control_state_:RateControlState::kRcHold,
            time_last_bitrate_change_:i64::MIN,
            time_last_bitrate_decrease_:i64::MIN,
            time_first_throughput_estimate_:i64::MIN,
            bitrate_is_initialized_:false,
            beta_:gcc_config::GCC_DECREASE_COEF_BETA,
            in_alr_:false,
            rtt_:gcc_config::GCC_DEFAULT_RTT,
            send_side_:send_side,
            no_bitrate_increase_in_alr_:true,
            last_decrease_:Some(0.0),
            network_estimate_:Some(NetworkStateEstimate {
                ..Default::default()
            }),
            esitmate_thr_testing:0.0,
        }

    }
    pub fn ChangeState(&mut self,input:& RateControlInput,
        at_time:i64) {
            match input.bw_state {
                BandwidthUsage::kBwNormal => {
                    if self.rate_control_state_ == RateControlState::kRcHold {
                        self.time_last_bitrate_change_ = at_time;
                        self.rate_control_state_ = RateControlState::kRcIncrease;
                    }
                    else if self.rate_control_state_ == RateControlState::kRcDecrease {
                        self.rate_control_state_ = RateControlState::kRcHold;
                    }
                },
                BandwidthUsage::kBwOverusing => {
                    if self.rate_control_state_ != RateControlState::kRcDecrease {
                        self.rate_control_state_ = RateControlState::kRcDecrease;
                    }
                },
                BandwidthUsage::kBwUnderusing => {
                    self.rate_control_state_ = RateControlState::kRcHold;
                },
                _ => {
                    
                }
            }
        
    }
    pub fn GetNearMaxIncreaseRateBpsPerSecond(&mut self) -> f64 {
        //RTC_DCHECK(!current_bitrate_.IsZero());
        let kFrameInterval = gcc_config::GCC_FRAME_INTERVAL;
        let mut frame_size = self.current_bitrate_ * kFrameInterval;
        let kPacketSize = 1500*8;
        let packets_per_frame = frame_size as f64/ kPacketSize as f64;
        let  avg_packet_size = frame_size / packets_per_frame;
      
        // Approximate the over-use estimator delay to 100 ms.
        let mut response_time = (self.rtt_ + 100) as f64*0.001;
      
        response_time = response_time * 2.;
        let mut increase_rate_bps_per_second =
            avg_packet_size / response_time as f64;
        let kMinIncreaseRateBpsPerSecond = 4000.0;
        return f64::max(kMinIncreaseRateBpsPerSecond, increase_rate_bps_per_second);
      }

    pub fn MultiplicativeRateIncrease(&mut self,
        at_time:i64,
         last_time:i64,
         current_bitrate:f64) ->f64 {
      let mut alpha = gcc_config::GCC_INCREASE_COEF_ALPHA;
      if last_time==i64::MIN {
        let mut time_since_last_update = at_time - last_time;
        alpha = alpha.powf(((time_since_last_update as f64/1000.0).min(1.0))as f64);
      }
      let mut multiplicative_increase =
          f64::max(current_bitrate * (alpha - 1.0), 1000.0);
      return multiplicative_increase;
    }
    
    pub fn AdditiveRateIncrease(&mut self,at_time:i64,
                                                    last_time:i64) ->f64 {
      let mut time_period_seconds = ((at_time - last_time)as f64)/1000.0;
      let mut data_rate_increase_bps =
          self.GetNearMaxIncreaseRateBpsPerSecond() * time_period_seconds;
      return data_rate_increase_bps;
    }
    
    pub fn Update(&mut self,input:&RateControlInput,
         at_time:i64)-> f64 {
        // Set the initial bit rate value to what we're receiving the first half
        // second.
        // TODO(bugs.webrtc.org/9379): The comment above doesn't match to the code.
        if !self.bitrate_is_initialized_ {
        let kInitializationTime = 5000 as i64;
        //RTC_DCHECK_LE(kBitrateWindowMs, kInitializationTime.ms());
        if self.time_first_throughput_estimate_==i64::MIN {
        if input.estimated_throughput.is_some(){
            self.time_first_throughput_estimate_ = at_time;
        }
        
        } else if at_time - self.time_first_throughput_estimate_ >
        kInitializationTime &&
        input.estimated_throughput.is_some() {
        self.current_bitrate_ = input.estimated_throughput.unwrap();
        self.bitrate_is_initialized_ = true;
        }
        }

        self.ChangeBitrate(input, at_time);
        return self.current_bitrate_;
    }

    pub fn ClampBitrate(&mut self,new_bitrate: f64) ->f64 {
        let mut new_bitrate_r=new_bitrate;
        if self.network_estimate_.is_some()&&
            self.network_estimate_.as_mut().unwrap().link_capacity_upper!=-std::f64::INFINITY {
          let mut upper_bound = self.network_estimate_.as_mut().unwrap().link_capacity_upper;
          new_bitrate_r = f64::min(upper_bound, new_bitrate_r);
        }
        if self.network_estimate_.is_some()&& self.network_estimate_.as_mut().unwrap().link_capacity_lower!=-std::f64::INFINITY &&
            new_bitrate_r < self.current_bitrate_{
                new_bitrate_r = f64::min(
              self.current_bitrate_,
              f64::max(new_bitrate_r, self.network_estimate_.as_mut().unwrap().link_capacity_lower * self.beta_));
        }
        new_bitrate_r = f64::max(new_bitrate_r, self.min_configured_bitrate_);
        return new_bitrate_r;
      }

    pub fn ChangeBitrate(&mut self,input:& RateControlInput,
         at_time:i64) {
            let mut new_bitrate=Option::None;
            let mut estimated_throughput =
            input.estimated_throughput.unwrap_or(self.latest_estimated_throughput_);
            if input.estimated_throughput.is_some(){
                self.latest_estimated_throughput_ = input.estimated_throughput.unwrap();
            }
            

            // An over-use should always trigger us to reduce the bitrate, even though
            // we have not yet established our first estimate. By acting on the over-use,
            // we will end up with a valid estimate.
            if !self.bitrate_is_initialized_ &&
            input.bw_state != BandwidthUsage::kBwOverusing{
                return;
            }
            

            self.ChangeState(input, at_time);

            match self.rate_control_state_{
                RateControlState::kRcHold=>{},
                RateControlState::kRcIncrease => { 
                    if estimated_throughput > self.link_capacity_.UpperBound()
                    { 
                        self.link_capacity_.Reset(); 
                    }
                    let mut increase_limit = 1.5 * estimated_throughput + 10000.0;

                    if self.current_bitrate_ < increase_limit {
                        let mut increased_bitrate = -std::f64::INFINITY;
                        if self.link_capacity_.has_estimate() {
                            let additive_increase = self.AdditiveRateIncrease(at_time, self.time_last_bitrate_change_);
                            increased_bitrate = self.current_bitrate_ + additive_increase;
                        } else {
                            let multiplicative_increase = self.MultiplicativeRateIncrease(
                                at_time, self.time_last_bitrate_change_, self.current_bitrate_
                            );
                            increased_bitrate = self.current_bitrate_ + multiplicative_increase;
                        }
                        new_bitrate = Some(increased_bitrate.min(increase_limit));
                    }
                    self.time_last_bitrate_change_ = at_time;
                    },
                RateControlState::kRcDecrease => {
                    let mut decreased_bitrate = std::f64::INFINITY;

                    decreased_bitrate = estimated_throughput * self.beta_;
                    if decreased_bitrate > self.current_bitrate_  {
                        if self.link_capacity_.has_estimate() {
                            decreased_bitrate = self.beta_ * self.link_capacity_.estimate();
                        }
                    }

                    if decreased_bitrate < self.current_bitrate_ {
                        new_bitrate = Some(decreased_bitrate);
                    }

                    if self.bitrate_is_initialized_ && estimated_throughput < self.current_bitrate_ {
                        if let Some(new_br) = new_bitrate {
                            self.last_decrease_ = Some(self.current_bitrate_ - new_br);
                        } else {
                            self.last_decrease_ = Option::None;
                        }
                    }

                    if estimated_throughput < self.link_capacity_.LowerBound() {
                        self.link_capacity_.Reset();
                    }

                    self.bitrate_is_initialized_ = true;
                    self.esitmate_thr_testing=estimated_throughput;
                    self.link_capacity_.OnOveruseDetected(estimated_throughput);
                    self.rate_control_state_ = RateControlState::kRcHold;
                    self.time_last_bitrate_change_ = at_time;
                    self.time_last_bitrate_decrease_ = at_time;
                },
                _ => {

                },
                
            }
            if self.network_estimate_.is_none(){
                self.network_estimate_=Some(NetworkStateEstimate {
                    ..Default::default()
                });
            }
            if self.link_capacity_.LowerBound()!=0.0{
                if self.link_capacity_.LowerBound()<self.min_configured_bitrate_{
                    self.network_estimate_.as_mut().unwrap().link_capacity_lower=self.min_configured_bitrate_;
                }
                else{
                    self.network_estimate_.as_mut().unwrap().link_capacity_lower=self.link_capacity_.LowerBound();
                }
            }else {
                self.network_estimate_.as_mut().unwrap().link_capacity_lower=self.min_configured_bitrate_;
            }
            if self.link_capacity_.UpperBound()!=f64::INFINITY{
                if self.link_capacity_.UpperBound()>self.max_configured_bitrate_{
                    self.network_estimate_.as_mut().unwrap().link_capacity_upper=self.max_configured_bitrate_;
                }
                else{
                    self.network_estimate_.as_mut().unwrap().link_capacity_upper=self.link_capacity_.UpperBound();
                }
            }else {
                self.network_estimate_.as_mut().unwrap().link_capacity_upper=self.max_configured_bitrate_;
            }

            self.current_bitrate_ = self.ClampBitrate(new_bitrate.unwrap_or(self.current_bitrate_));
        }


        
    

}



pub struct BitrateEstimator{
    pub sum_ :i64,
    pub initial_window_ms_:i64,
    pub noninitial_window_ms_:i64,
    pub uncertainty_scale_:f64,
    pub uncertainty_scale_in_alr_:f64,
    pub small_sample_uncertainty_scale_:f64,
    pub small_sample_threshold_:usize,
    pub uncertainty_symmetry_cap_:usize,
    pub estimate_floor_:usize,
    pub current_window_ms_:i64,
    pub prev_time_ms_:i64,
    pub bitrate_estimate_kbps_:f64,
    pub bitrate_estimate_var_:f64,
}
impl BitrateEstimator{
    pub fn new()->Self{
        Self{
            sum_:0,
            initial_window_ms_:gcc_config::GCC_BITRATE_ESTIMATOR_INIT_WINDOW_MS,
            noninitial_window_ms_:gcc_config::GCC_BITRATE_ESTIMATOR_NONINIT_WINDOW_MS,
            uncertainty_scale_:10.0,
            uncertainty_scale_in_alr_:10.0,
            small_sample_uncertainty_scale_:10.0,
            small_sample_threshold_:0,
            uncertainty_symmetry_cap_:0,
            estimate_floor_:0,
            current_window_ms_:0,
            prev_time_ms_:-1,
            bitrate_estimate_kbps_:-1.0,
            bitrate_estimate_var_:50.0,
        }
    }
    pub fn UpdateWindow( &mut self,now_ms:i64,
         bytes:usize,
         rate_window_ms:i64,
         is_small_sample:&mut bool)->f64{
            if now_ms < self.prev_time_ms_ {
                self.prev_time_ms_ = -1;
                self.sum_ = 0;
                self.current_window_ms_ = 0;
              }
              if self.prev_time_ms_ >= 0 {
                self.current_window_ms_ += now_ms - self.prev_time_ms_;
                // Reset if nothing has been received for more than a full window.
                if now_ms - self.prev_time_ms_ > rate_window_ms {
                    self.sum_ = 0;
                    self.current_window_ms_ %= rate_window_ms;
                }
              }
              self.prev_time_ms_ = now_ms;
              let mut bitrate_sample = -1.0;
              if self.current_window_ms_ >= rate_window_ms {
                *is_small_sample = self.sum_ < self.small_sample_threshold_ as i64;
                bitrate_sample = 8.0 * self.sum_ as f64 / rate_window_ms as f64;
                self.current_window_ms_ -= rate_window_ms;
                self.sum_ = 0;
              }
              self.sum_ += bytes as i64;
              return bitrate_sample;
         }

    pub fn Update(&mut self, at_time:i64,  amount:usize,  in_alr:bool){
        let mut rate_window_ms = self.noninitial_window_ms_;
        // We use a larger window at the beginning to get a more stable sample that
        // we can use to initialize the estimate.
        if self.bitrate_estimate_kbps_ < 0.0{
            rate_window_ms = self.initial_window_ms_;
        }
            
        let mut is_small_sample = false;
        let mut bitrate_sample_kbps = self.UpdateWindow(at_time, amount,
                                                rate_window_ms, &mut is_small_sample);
        if bitrate_sample_kbps < 0.0
        {
            return;
        }
            
        if self.bitrate_estimate_kbps_ < 0.0{
            // This is the very first sample we get. Use it to initialize the estimate.
            self.bitrate_estimate_kbps_ = bitrate_sample_kbps;
            return;
        }
        // Optionally use higher uncertainty for very small samples to avoid dropping
        // estimate and for samples obtained in ALR.
        let mut scale = self.uncertainty_scale_;
        if is_small_sample && bitrate_sample_kbps < self.bitrate_estimate_kbps_ {
            scale = self.small_sample_uncertainty_scale_;
        } else if in_alr && bitrate_sample_kbps < self.bitrate_estimate_kbps_ {
            // Optionally use higher uncertainty for samples obtained during ALR.
            scale = self.uncertainty_scale_in_alr_;
        }
        // Define the sample uncertainty as a function of how far away it is from the
        // current estimate. With low values of uncertainty_symmetry_cap_ we add more
        // uncertainty to increases than to decreases. For higher values we approach
        // symmetry.
        let mut sample_uncertainty =
            scale * (self.bitrate_estimate_kbps_ - bitrate_sample_kbps).abs() /
            (self.bitrate_estimate_kbps_ +
            f64::min(bitrate_sample_kbps,
                        self.uncertainty_symmetry_cap_ as f64));

        let mut  sample_var = sample_uncertainty * sample_uncertainty;
        // Update a bayesian estimate of the rate, weighting it lower if the sample
        // uncertainty is large.
        // The bitrate estimate uncertainty is increased with each update to model
        // that the bitrate changes over time.
        let mut pred_bitrate_estimate_var = self.bitrate_estimate_var_ + 5.0;
        self.bitrate_estimate_kbps_ = (sample_var * self.bitrate_estimate_kbps_ +
                                    pred_bitrate_estimate_var * bitrate_sample_kbps) /
                                (sample_var + pred_bitrate_estimate_var);
        self.bitrate_estimate_kbps_ =
            f64::max(self.bitrate_estimate_kbps_, self.estimate_floor_ as f64);
        self.bitrate_estimate_var_ = sample_var * pred_bitrate_estimate_var /
                                (sample_var + pred_bitrate_estimate_var);
        //BWE_TEST_LOGGING_PLOT(1, "acknowledged_bitrate", at_time.ms(),
                                //bitrate_estimate_kbps_ * 1000);
    }
    pub fn bitrate(&mut self)->Option<f64>{
        if self.bitrate_estimate_kbps_<0.0{
            return Option::None;
        }
        else {
            return Some(self.bitrate_estimate_kbps_*1000.0);
        }
    }
    pub fn PeekRate(&mut self)->Option<f64>{
        if self.current_window_ms_>0{
            return Some(self.sum_ as f64/(self.current_window_ms_ as f64*0.001));
        }
        else {
            return Option::None;
        }
    }
    pub fn ExpectFastRateChange(&mut self){
        self.bitrate_estimate_var_+=200.0;
    }
}