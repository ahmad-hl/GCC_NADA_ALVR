#include "config.h"

bool save_rframe_lock = true;
bool save_eframe_lock = true;

bool get_eframe_lock() {
    return save_eframe_lock;
}

bool get_rframe_lock() {
    return save_rframe_lock;
}

std::string filename_s = "C:\\Users\\Ze\\Desktop\\mobisys\\frame_data\\";

std::string get_path_head(){
    return filename_s;
}