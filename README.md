[Congestion Control for VR Cloud Gaming: Integration and Comparison in Real VR Gaming Environment](https://dl.acm.org/doi/abs/10.1145/3746027.3755439)  <br> 
To be published in the Proceedings of the 33rd ACM International Conference on Multimedia 2025 (ACM MM'25)  <br>
The system is built upon the [codebase of ALVR](https://github.com/alvr-org/ALVR). <br>
We are grateful to the ALVR team for their work, and we acknowledge and give them credit for their contributions.<br>
ALVR streams VR games from your PC to your VR headset via Wi-Fi. <br>
Please read more details about the supported VR Headsets, PC OS, requirements, and tools required on [ALVR](https://github.com/alvr-org/ALVR).

## Build from source

You can follow the guide [here](https://github.com/alvr-org/ALVR/wiki/Building-From-Source).

## System Architecture
![system Overview](figures/systemOverview.png)
We integrated GCC and NADA for adaptive game streaming and evaluated them against ALVR adaptive bitrate (ABR mode). This integration not only enables fair performance evaluation across benchmarks but also ensures game-agnostic VR cloud gaming through interoperation with SteamVR. 

The **Network Statistics** module feeds network performance metrics to the **Congestion Control** module to compute the target bitrate according to the network conditions. This module outputs the target bitrate and passes it to the **Video Encoder** module.
## System Performance 
### Bitrate to network throughput
![Target Bitrate over stable WiFi network (35 Mb/s)](figures/latency_30Mbps.png)
![Target bitrate response to varying bandwidth (highlighted in gray) over 5G mobile network](figures/latency_mobile.png)

### Motion-to-photon Latency 
![Motion to Photon Latency over stable WiFi network (35 Mb/s)](figures/latency_30Mbps.png)
![Motion to Photon Latency over 5G mobile network](figures/latency_mobile.png)

### Visual Quality (PSNR and SSIM)
![Peak Signal-to-Noise Ratio & Structural Similarity Index Measure (SSIM) over stable WiFi network](figures/ssim_psnr_30mbps.png)
![Peak Signal-to-Noise Ratio & Structural Similarity Index Measure (SSIM) over 5G mobile network](figures/ssim_psnr_mobile.png)

