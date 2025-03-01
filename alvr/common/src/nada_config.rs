pub const NADA_PARAM_PRIO :f64 = 1.0;            //Weight of priority of the flow | 1.0
/**
 * Min and Max rate of application supported by media encoder | 150 Kbps & 1.5 Mbps
 **/ 
pub const RMCAT_CC_DEFAULT_RMIN : i64 = 2_000_000;    // 5Mbps
pub const RMCAT_CC_DEFAULT_RMAX : i64 = 150_000_000;  //150Mbps
pub const INITIAL_RATE : i64 = 15_000_000;  //15Mbps
pub const NADA_PARAM_XREF : i64 = 20;     //Reference congestion level | 20ms
pub const NADA_PARAM_KAPPA : f64 = 0.5;       //Scaling parameter for gradual rate update | 0.5
pub const NADA_PARAM_ETA:f64 = 2.0;           //Scaling parameter for gradual rate update | 2.0
pub const NADA_PARAM_TAU:i64 = 500;       //Upper bound of RTT in gradual rate update |  500ms
pub const NADA_PARAM_DELTA: i64 = 100;    //Target feedback interval | 100ms 
pub const NADA_PARAM_DELTA_US : i64 = 100_000; //in Nano second
pub const NADA_PARAM_DFILT : i64 = 120;   //Bound on filtering delay | 120ms 
pub const NADA_PARAM_DFILT_US : i64 = 120_000; //in Nano second
pub const NADA_PARAM_LOGWIN : i64 = 500;  //Observation time window in for calculating packet summary statistics at receiver  | 500ms       |
pub const NADA_PARAM_QEPS : i64 = 10;    //Threshold for determining queuing delay build up at receiver| 10ms
pub const NADA_PARAM_QEPS_US : i64 = 10_000; //in Nano second

pub const NADA_PARAM_QTH  : i64 = 50;    //Delay threshold for non-linear warping | 50ms 
pub const NADA_PARAM_QMAX : i64 = 400;   //Delay upper bound for non-linear warping| 400ms
pub const NADA_PARAM_DLOSS: i64 = 10; //1_000; //Delay penalty for loss  | 1.0s
pub const NADA_PARAM_DMARK: i64 = 200;   //Delay penalty for ECN marking | 200ms 

pub const NADA_PARAM_GAMMA_MAX : f64 = 0.5; //Upper bound on rate increase ratio for accelerated ramp-up | 50%
pub const NADA_PARAM_QBOUND : i64 = 50;   //Upper bound on self-inflicted queuing delay during ramp up | 50ms

pub const NADA_PARAM_FPS: f64 = 30.0; //Frame rate of incoming video | 30
pub const NADA_PARAM_BETA_S: f64 = 0.1;   //Scaling parameter for modulating outgoing sending rate |  0.1
pub const NADA_PARAM_BETA_V: f64 = 0.1;  //Scaling parameter for modulating video encoder target rate | 0.1 
pub const NADA_PARAM_ALPHA: f64 = 0.1; // Smoothing factor of loss and marking ratios | 0.1 


//Added by Ze
pub const NADA_PARAM_LAMBDA :f64 = 0.5; 
pub const NADA_PARAM_MULTILOSS : f64 = 7.0;
pub const NADA_PARAM_PLRREF : f64 = 0.01;
pub const NADA_PARAM_XMAX : f64 = 500.0;

//History Size
pub const NADA_RTT_HISTORY_SIZE : usize = 15;


//RTCP NADA Feedback Report, from NADA Receiver
pub struct NADAFeedbackReport{
    pub rmode:i8, 
    pub x_curr:f64, 
    pub r_recv:i64,
    
    //Only For Debuging NADA Receiver
    pub d_queue:i64,
    pub d_tilde:f64,
    pub p_loss: f64,
}

impl NADAFeedbackReport {
pub fn new(
    rmode:i8, x_curr:f64, r_recv:i64, d_queue:i64, d_tilde:f64, p_loss: f64) -> Self {
    Self {
        rmode:rmode,
        x_curr:x_curr,
        r_recv:r_recv, 

        //Only For Debuging NADA Receiver
        d_queue:d_queue,
        d_tilde: d_tilde,
        p_loss: p_loss,
    }
}
}