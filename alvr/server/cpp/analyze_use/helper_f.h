#pragma once

#include <iostream>
#include <fstream> 
#include <chrono>
#include <cstdlib>


#ifndef HELPERS_H
#define HELPERS_H
#include "../platform/win32/NvEncoderD3D11.h"

extern int frame_count;
extern int save_frame_feq;

void add_frame_count();
int get_frame_count();
int get_save_frame_feq();


void SaveTextureAsBytes(ID3D11DeviceContext* context, ID3D11Texture2D* texture, bool FFRed, uint64_t m_targetTimestampNs);


#endif