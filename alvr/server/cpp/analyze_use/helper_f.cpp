#include "helper_f.h"

int frame_count = 0;
int save_frame_feq = 500;

void add_frame_count(){
    frame_count++;
}

int get_frame_count(){
    return frame_count;
}

int get_save_frame_feq(){
    return save_frame_feq;
}


void SaveTextureAsBytes(ID3D11DeviceContext* context, ID3D11Texture2D* texture, bool FFRed, uint64_t m_targetTimestampNs)
{
    if(get_rframe_lock()&& frame_count%get_save_frame_feq()==0){
        ID3D11Device* device;
        texture->GetDevice(&device);
        // Get texture description
        D3D11_TEXTURE2D_DESC desc;
        texture->GetDesc(&desc);

        // Create staging texture
        D3D11_TEXTURE2D_DESC stagingDesc = desc;
        stagingDesc.Usage = D3D11_USAGE_STAGING;
        stagingDesc.BindFlags = 0;
        stagingDesc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
        ID3D11Texture2D* stagingTexture;
        device->CreateTexture2D(&stagingDesc, nullptr, &stagingTexture);

        // Copy texture to staging texture
        context->CopyResource(stagingTexture, texture);

        // Map staging texture to CPU memory
        D3D11_MAPPED_SUBRESOURCE mappedResource;
        context->Map(stagingTexture, 0, D3D11_MAP_READ, 0, &mappedResource);

        // Write texture to byte file
        std::string name;
        if(FFRed){
            name = "rframe_";
        }
        else{
            name = "oframe_";
        }
        name += std::to_string(get_frame_count());
        name += ".bytes";
        const char* filename = (filename_s+name).c_str();

        std::ofstream file(filename, std::ios::out | std::ios::binary);
        file.write((char*)mappedResource.pData, mappedResource.DepthPitch);
        file.close();
    }
}