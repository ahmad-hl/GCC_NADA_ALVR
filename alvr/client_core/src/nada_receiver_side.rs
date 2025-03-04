use alvr_common::SlidingWindowAverage;
use alvr_common::*;
use alvr_packets::RateUpdateMode;
use chrono::Utc;
use std::time::{Duration, Instant};


pub struct NadaReceiver{
    pub d_base : i64, //Estimated baseline delay 
    pub d_tilde: f64, //Equivalent delay after non-linear warping 
    pub d_queue: i64, //Estimated queueing delay  
    pub p_loss : f64, //Estimated packet loss ratio 
    pub p_mark: f64, //Estimated packet ECN marking ratio
    pub r_recv : i64, //Receiving rate  (bps)
    pub t_last : i64, //Last time receiving a feedback
    pub d_queue_history : SlidingWindowAverage<i64>,
    pub x_curr : f64, //Aggregate congestion signal 
    pub rmode :  RateUpdateMode, //Rate update mode: (0 = accelerated ramp-up | 1 = gradual)      
    
    //last send & arrival times
    pub last_t_send : i64, //last send timestamp
    pub last_t_arrival : i64, //last arrival timestamp

    //To compute r_recv, p_loss
    pub total_received_bytes : usize,
    total_num_packets: u32,
    total_packets_lost: u32,
    pub receive_rate_timer: Instant,
}

impl NadaReceiver{
    pub fn new() -> Self {
        Self { 
            d_base: i64::MAX, 
            d_tilde: 0.0,
            d_queue: 0,
            p_loss: 0.0, 
            p_mark: 0.0,
            r_recv: 0, 
            t_last: Utc::now().timestamp_micros(), 
            d_queue_history: SlidingWindowAverage::new(
                0,
                15,
            ),
            x_curr: 0.0,
            rmode: RateUpdateMode::GradualUpdate,

                //last send & arrival times
            last_t_send: 0,
            last_t_arrival: 0, 
            
            //receive timer
            total_received_bytes: 0,
            total_num_packets: 0,
            total_packets_lost: 0,
            receive_rate_timer: Instant::now(), 
        }
    }

    /****
        Obtain one-way delay measurement: d_fwd = t_curr - t_sent
        update baseline delay: d_base = min(d_base, d_fwd)
        update queuing delay:  d_queue = d_fwd - d_base
     *****/
    pub fn compute_oneway_delay(&mut self, frame_send_timestamp: i64, frame_arrival_timestamp: i64){
        let d_fwd = frame_arrival_timestamp - frame_send_timestamp;
        self.d_base = std::cmp::min(self.d_base, d_fwd);
        self.d_queue = d_fwd -  self.d_base;
        self.d_queue_history.submit_sample(self.d_queue);

        //initialize last timestamps
        self.last_t_send = frame_send_timestamp;
        self.last_t_arrival = frame_arrival_timestamp;
    }

    /** When packet losses are observed, the estimated queuing delay follows
        a non-linear warping inspired by the delay-adaptive congestion window
        backoff policy in [Budzisz-TON11]: **/
    fn compute_d_tilde(&mut self, had_packet_loss:bool){

        if had_packet_loss{
            if  self.d_queue <  NADA_PARAM_QTH{
                self.d_tilde =  self.d_queue as f64;

            } else if self.d_queue > NADA_PARAM_QTH &&  self.d_queue < NADA_PARAM_QMAX{
                let numerator = (NADA_PARAM_QTH - self.d_queue).pow(4);
                let denominator = (NADA_PARAM_QMAX - NADA_PARAM_QTH).pow(4);
                self.d_tilde = NADA_PARAM_QTH  as f64 * (numerator/ denominator) as f64;
            
            } else{
                self.d_tilde = 0.0;
            }
        }
    } 
    

    /** On time to send a new feedback report (t_curr - t_last > DELTA)
        calculate non-linear warping of delay d_tilde if packet loss exists
        calculate current aggregate congestion signal x_curr
        determine mode of rate adaptation for sender: rmode
        send RTCP feedback report containing: rmode, x_curr, and r_recv
        update t_last = t_curr  **/
    pub fn time_to_report_feedback(&mut self, had_packet_loss: bool, had_packet_mark: bool)-> bool{
        let t_curr = Utc::now().timestamp_micros();
        let time_diff = (t_curr - self.t_last)/1000;

        if time_diff > NADA_PARAM_DELTA{

            self.d_tilde = self.d_queue_history.get_average() as f64/1000.0; // /1000 for us to ms
            // Calculate non-linear warping of delay if packet loss exists
             if had_packet_loss {
                self.compute_d_tilde(had_packet_loss);
            }
            // Update packet marking ratio estimate
            if had_packet_mark {
                self.p_mark += 1.0; // Increment mark count (similarly, improve this logic as needed)
            }
            self.x_curr = self.d_tilde + self.p_mark * NADA_PARAM_DMARK as f64 + self.p_loss * NADA_PARAM_DLOSS as f64;
            self.determine_rate_adaptation_mode(had_packet_loss);

            true
        }
        else{
            false
        }
    }

    pub fn update_t_last(&mut self){
        let t_curr = Utc::now().timestamp_micros();
        self.t_last = t_curr;
    }

    /** Is there build-up of queuing delay?
     Check if d_fwd-d_base < QEPS for all previous
      delay samples within the observation window LOGWIN **/
    fn exists_queuing_delay_buildup(&mut self) -> bool {
        for value in self.d_queue_history.get_history_iter() {
            if *value > NADA_PARAM_QEPS_US {
                return true; 
            }
        }
        false 
    }

    /**
     Determine whether the network is underutilized 
     and recommend the corresponding rate adaptation mode 
    **/
    fn determine_rate_adaptation_mode(&mut self, had_packet_loss: bool){

        self.rmode = RateUpdateMode::AcceleratedRampUp;
        /* To operate in accelerated ramp-up mode:
        o  No recent packet losses within the observation window LOGWIN; and
        o  No build-up of queuing delay: d_fwd-d_base < QEPS for all previous
        delay samples within the observation window LOGWIN.*/
        if had_packet_loss{
            self.rmode = RateUpdateMode::GradualUpdate;
        }
        if self.exists_queuing_delay_buildup(){
            self.rmode = RateUpdateMode::GradualUpdate;
        }
    }

    pub fn update_receive_loss_rate(&mut self, received_bytes: usize){
        let total_num_packets = (received_bytes as f32 / 1500.0).ceil() as u32;
        let num_packets_lost = 0;
        // received bytes in nal + size of header (VideoPacketHeader)
        let size_of_timestamp = std::mem::size_of::<Duration>();
        let size_of_send_timestamp = std::mem::size_of::<i64>();
        self.total_received_bytes += received_bytes + size_of_timestamp + size_of_send_timestamp;
        
        // total & lost packets to compute p_inst
        self.total_packets_lost += num_packets_lost;
        self.total_num_packets += total_num_packets;

        let elapsed = self.receive_rate_timer.elapsed();
        if elapsed >= Duration::from_millis(NADA_PARAM_LOGWIN as u64){
            /* 5.1.3.  Estimation of receiving rate (r_recv):
             * NADA maintains a recent observation window with time span of LOGWIN,
             * and simply divides the total size of packets arriving during that window */
            let interval_in_sec = NADA_PARAM_LOGWIN as f64/1000.0 ;
            let received_rate_bps=((self.total_received_bytes * 8) as f64) / interval_in_sec;
            self.r_recv = received_rate_bps.round() as i64;


            /* 5.1.2.  Estimation of packet loss/marking ratio:
             * The instantaneous packet loss ratio p_inst is the ratio between 
             * the number of missing packets over the number of total transmitted packets 
             * within the recent observation window LOGWIN.  The packet loss ratio p_loss is
             *obtained after exponential smoothing:
                p_loss = ALPHA*p_inst + (1-ALPHA)*p_loss.   (10) */
            let p_inst = if self.total_num_packets > 0 {
                self.total_packets_lost as f64 / self.total_num_packets as f64
            } else {
                0.0 
            };

            self.p_loss = NADA_PARAM_ALPHA * p_inst + (1.0 - NADA_PARAM_ALPHA) * self.p_loss;
            
            //re-initialize
            self.total_received_bytes = 0;
            self.receive_rate_timer = Instant::now();
            self.total_packets_lost = 0;
            self.total_num_packets = 0;
        }

    }

}