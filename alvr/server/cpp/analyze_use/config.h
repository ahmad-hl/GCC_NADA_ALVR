#pragma once
#include <string>

extern bool save_rframe_lock;
extern bool save_eframe_lock;
extern std::string filename_s;

extern std::string get_path_head();
extern bool get_eframe_lock();
extern bool get_rframe_lock();