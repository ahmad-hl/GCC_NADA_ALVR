use chrono::{Utc,Local};
use alvr_common::*;
use alvr_common::SlidingWindowAverage;
use alvr_packets::RateUpdateMode;
use std::fs::OpenOptions;
use std::error::Error;
use csv::Writer;

pub struct NadaSender{
    pub r_ref: i64, //Reference rate based on network congestion
    pub rtt_history : SlidingWindowAverage<i64>, //Estimated round-trip-time  
    pub r_recv : i64,   //Receiving rate
    pub rmode : RateUpdateMode, //Rate update mode: (0 = accelerated ramp-up | 1 = gradual update)    
    pub x_curr : f64,  //Aggregate congestion signal 
    pub x_prev: f64,   //Prev value of aggregate congestion signal
    pub r_vin : i64,   //target rate for the live video encoder
    pub r_send : i64,  //actual sending rate for regulating traffic
    pub prev_r_vin : i64, 
    pub prev_r_send : i64,
    pub t_last:i64,    //Last time receiving a feedback 
    pub t_curr:i64,

    //Only for Debugging the NADA Receiver
    pub d_queue:i64, //Estimated queueing delay     
    pub d_tilde:f64, //Equivalent delay after non-linear warping
    pub p_loss: f64, //Estimated packet loss ratio
}
impl NadaSender {
    pub fn new()->Self{
        Self { 
            r_ref: INITIAL_RATE,
            rtt_history: SlidingWindowAverage::new(
                0,
                NADA_RTT_HISTORY_SIZE,
            ),
            r_recv: INITIAL_RATE, //30Mbps
            rmode: RateUpdateMode::GradualUpdate,
            x_curr: 0.0,
            x_prev: 0.0,
            r_vin: INITIAL_RATE,  //30Mbps
            r_send: INITIAL_RATE, //30Mbps
            prev_r_vin: INITIAL_RATE, //30Mbps
            prev_r_send: INITIAL_RATE, //30Mbps
            t_last: Utc::now().timestamp_micros(), 
            t_curr: Utc::now().timestamp_micros(), 

            //Only for Debugging the NADA Receiver
            d_queue: 0,
            d_tilde: 0.0,
            p_loss: 0.0,
        }
    }

    // Function to save data to CSV
    pub fn write_sender_values_to_csv(&self, filename: &str) -> Result<(), Box<dyn Error>> {
        let eval_rmode = match self.rmode {
            RateUpdateMode::AcceleratedRampUp => 0,
            RateUpdateMode::GradualUpdate => 1,
            _ => 2,
        };
        let linux_timestamp=Local::now().format("%Y%m%d%H%M%S").to_string();
        let rtt_average = self.rtt_history.get_average() as f64/ 1000.0; // /1000 to convert us to ms

        let nada_values = [
            self.r_ref.to_string(),
            rtt_average.to_string(),
            self.r_recv.to_string(),
            eval_rmode.to_string(),
            self.x_curr.to_string(),
            self.x_prev.to_string(),
            self.r_vin.to_string(),
            self.r_send.to_string(),
            self.prev_r_vin.to_string(),
            self.prev_r_send.to_string(),
            self.t_curr.to_string(),
            self.t_last.to_string(),
            //Only to debug Receiver values
            (self.d_queue as f64 /1000.0).to_string(),
            self.d_tilde.to_string(),
            self.p_loss.to_string(),
            linux_timestamp,
        ];

        // let _= write_nada_variable_values_to_csv(filename, nada_values);
        let file = OpenOptions::new().write(true).append(true).open(filename)?;
        let mut writer = Writer::from_writer(file);
        writer.write_record(&nada_values)?;

        Ok(())
    }

    fn update_accelerated_rampup(&mut self) -> i64{
        let rtt_average = self.rtt_history.get_average() / 1000; // /1000 to convert us to ms
        let res = (NADA_PARAM_QBOUND /(rtt_average + NADA_PARAM_DELTA + NADA_PARAM_DFILT)) as f64;
        //gamma: Rate increase multiplier in accelerated ramp-up mode 
        let gamma = if res < NADA_PARAM_GAMMA_MAX {
            res
        } else {
            NADA_PARAM_GAMMA_MAX
        };
        let updated_r_ref =  if self.r_ref as f64  > (1.0 + gamma) * self.r_recv as f64{
            self.r_ref 
        }else{
            let result = (1.0 + gamma) * (self.r_recv as f64);
            result.round() as i64 // Round the result and convert to i64
        };

        updated_r_ref
    }

    fn update_gradual(&mut self, delta_us: i64) -> i64{
        let delta = delta_us as f64/1000.0; // nano sec (us) to ms
        
        let right_side = NADA_PARAM_PRIO * NADA_PARAM_XREF as f64 * RMCAT_CC_DEFAULT_RMAX  as f64 / self.r_ref  as f64;
        let x_offset = self.x_curr - right_side;
        let x_diff   = self.x_curr - self.x_prev;
        let updated_r_ref = self.r_ref  as f64 - NADA_PARAM_KAPPA * (delta/ NADA_PARAM_TAU as f64) * (x_offset/ NADA_PARAM_TAU as f64) * self.r_ref  as f64
        - NADA_PARAM_KAPPA * NADA_PARAM_ETA * (x_diff / NADA_PARAM_TAU as f64) * self.r_ref  as f64;

        updated_r_ref.round() as i64
    }

    /** on receiving feedback report:
       1. obtain current timestamp from system clock: t_curr
       2. obtain values of rmode, x_curr, and r_recv from feedback report
       3. update estimation of rtt
       4. measure feedback interval: delta = t_curr - t_last
       if rmode == 0: update r_ref via accelerated ramp-up rules
       else:          update r_ref via gradual update rules
       6. clip rate r_ref within the range of [RMIN, RMAX]
       x_prev = x_curr & t_last = t_curr
     **/
    pub fn update_on_receive_feedback(&mut self, send_timestamp:i64, 
        feedback_report: NADAFeedbackReport, video_fps:f64){
        self.t_curr = Utc::now().timestamp_micros();
        self.rmode = match feedback_report.rmode{
            0 => RateUpdateMode::AcceleratedRampUp,
            1 => RateUpdateMode::GradualUpdate,
            _ => RateUpdateMode::GradualUpdate,
        };
        self.x_curr = feedback_report.x_curr;
        self.r_recv = feedback_report.r_recv;

        //Only To Debug NADA Receiver
        self.d_queue = feedback_report.d_queue;
        self.d_tilde = feedback_report.d_tilde;

        //update estimation of rtt
        let rtt = self.t_curr - send_timestamp; 
        self.rtt_history.submit_sample(rtt);
        
        //Measure feedback interval: delta = t_curr - t_last
        let delta_us = self.t_curr - self.t_last;
        let updated_r_ref;
        match self.rmode{
            RateUpdateMode::AcceleratedRampUp =>{
                updated_r_ref = self.update_accelerated_rampup( );
            }
            _ => {
                updated_r_ref = self.update_gradual(delta_us);
            }
        };
        
        self.r_ref  = updated_r_ref.clamp(RMCAT_CC_DEFAULT_RMIN, RMCAT_CC_DEFAULT_RMAX);
        self.t_last = self.t_curr;
        self.x_prev = self.x_curr;

        //Update target (r_vin) & sending (r_send) bitrates
        self.prev_r_vin = self.r_vin;
        self.prev_r_send = self.r_send;

        let use_shaping_buffer = true;
        if use_shaping_buffer {
            self.update_target_bitrate(0.0, video_fps);
            self.update_sending_bitrate(0.0, video_fps);
        } else{
            self.r_vin  = self.r_ref;
            self.r_send = self.r_ref;
        }
    }

    fn update_target_bitrate(&mut self, buffer_len:f64, video_fps: f64){
        self.prev_r_vin = self.r_vin;
        let buffer_len_ = if buffer_len == 0.0{
            self.prev_r_vin as f64  / 1500.0
            // 1.0
        }else{
            buffer_len
        };

        let result = self.r_ref as f64 - NADA_PARAM_BETA_V  * 8.0 * buffer_len_ * video_fps;
        self.r_vin  = result.round() as i64
    }

    fn update_sending_bitrate(&mut self, buffer_len:f64, video_fps: f64){
        let buffer_len_ = if buffer_len == 0.0{
            self.prev_r_vin as f64  / 1500.0
            // 1.0
        }else{
            buffer_len
        };

        let result = self.r_ref  as f64 - NADA_PARAM_BETA_S * 8.0 * buffer_len_ * video_fps;
        self.r_send = result.round() as i64
    }

    pub fn get_target_bitrate(&mut self) -> i64{
        self.r_vin
    }

    pub fn get_sending_bitrate(&mut self) -> i64{
        self.r_send
    }
}