// C++ bridge exposing a Spout DirectX sender to Rust FFI. Sender-only,
// CPU-pixel path: gemelli hands a BGRA8 CPU buffer per frame
// (gemelli-core::Frame). Three send strategies are exposed for
// gemelli-spout to A/B (see spout_bridge.h); StagingSse is gemelli-spout's
// production default.

#include "spout_bridge.h"

#include <cstring>

#include "SpoutCopy.h"
#include "SpoutDX.h"

struct SpoutBridgeHandle {
    spoutDX sender;
    spoutCopy copier;                    // SSE2/pitch-aware line copier (mode 2)
    ID3D11Device* device = nullptr;
    ID3D11DeviceContext* context = nullptr;
    ID3D11Texture2D* staging = nullptr;  // DYNAMIC upload texture, (re)allocated on resize
    uint32_t staging_width = 0;
    uint32_t staging_height = 0;
    bool initialized = false;
};

namespace {

void release_staging(SpoutBridgeHandle* handle) {
    if (handle->staging) {
        handle->staging->Release();
        handle->staging = nullptr;
    }
    handle->staging_width = 0;
    handle->staging_height = 0;
}

// (Re)allocates the DYNAMIC staging texture when the requested size differs
// from what is already allocated. Returns false if allocation fails.
bool ensure_staging(SpoutBridgeHandle* handle, uint32_t width, uint32_t height) {
    if (handle->staging && handle->staging_width == width && handle->staging_height == height) {
        return true;
    }
    release_staging(handle);

    D3D11_TEXTURE2D_DESC desc = {};
    desc.Width = width;
    desc.Height = height;
    desc.MipLevels = 1;
    desc.ArraySize = 1;
    desc.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    desc.SampleDesc.Count = 1;
    desc.SampleDesc.Quality = 0;
    desc.Usage = D3D11_USAGE_DYNAMIC;
    desc.BindFlags = D3D11_BIND_SHADER_RESOURCE;
    desc.CPUAccessFlags = D3D11_CPU_ACCESS_WRITE;

    HRESULT hr = handle->device->CreateTexture2D(&desc, nullptr, &handle->staging);
    if (FAILED(hr) || !handle->staging) {
        release_staging(handle);
        return false;
    }
    handle->staging_width = width;
    handle->staging_height = height;
    return true;
}

// Mode 0: DYNAMIC staging + row-by-row memcpy + SendTexture.
int32_t send_staging_row_copy(SpoutBridgeHandle* handle,
                               const uint8_t* pixels,
                               uint32_t width,
                               uint32_t height,
                               uint32_t pitch) {
    if (!ensure_staging(handle, width, height)) {
        return -3;
    }

    D3D11_MAPPED_SUBRESOURCE mapped;
    HRESULT hr = handle->context->Map(handle->staging, 0, D3D11_MAP_WRITE_DISCARD, 0, &mapped);
    if (FAILED(hr)) {
        return -4;
    }

    const uint8_t* src_row = pixels;
    uint8_t* dst_row = static_cast<uint8_t*>(mapped.pData);
    const size_t copy_width = static_cast<size_t>(width) * 4;
    for (uint32_t y = 0; y < height; ++y) {
        memcpy(dst_row, src_row, copy_width);
        src_row += pitch;
        dst_row += mapped.RowPitch;
    }
    handle->context->Unmap(handle->staging, 0);

    return handle->sender.SendTexture(handle->staging) ? 0 : -5;
}

// Mode 1: SendImage writes the CPU buffer straight into the shared texture
// via UpdateSubresource -- no staging texture, no CopyResource.
int32_t send_image(SpoutBridgeHandle* handle,
                    const uint8_t* pixels,
                    uint32_t width,
                    uint32_t height,
                    uint32_t pitch) {
    return handle->sender.SendImage(pixels, width, height, pitch) ? 0 : -5;
}

// Mode 2: DYNAMIC staging filled with SpoutCopy's SSE2 line copier
// (pitch-aware, channel-agnostic -- no swizzle), then SendTexture.
int32_t send_staging_sse(SpoutBridgeHandle* handle,
                          const uint8_t* pixels,
                          uint32_t width,
                          uint32_t height,
                          uint32_t pitch) {
    if (!ensure_staging(handle, width, height)) {
        return -3;
    }

    D3D11_MAPPED_SUBRESOURCE mapped;
    HRESULT hr = handle->context->Map(handle->staging, 0, D3D11_MAP_WRITE_DISCARD, 0, &mapped);
    if (FAILED(hr)) {
        return -4;
    }

    handle->copier.rgba2rgba(pixels, mapped.pData, width, height, pitch, mapped.RowPitch, false);
    handle->context->Unmap(handle->staging, 0);

    return handle->sender.SendTexture(handle->staging) ? 0 : -5;
}

} // namespace

extern "C" {

SpoutBridgeHandle* spout_bridge_create(const char* name) {
    if (!name) {
        return nullptr;
    }

    SpoutBridgeHandle* handle = new SpoutBridgeHandle();

    if (!handle->sender.OpenDirectX11()) {
        delete handle;
        return nullptr;
    }
    handle->device = handle->sender.GetDX11Device();
    handle->context = handle->sender.GetDX11Context();
    if (!handle->device || !handle->context) {
        handle->sender.CloseDirectX11();
        delete handle;
        return nullptr;
    }

    if (!handle->sender.SetSenderName(name)) {
        handle->sender.CloseDirectX11();
        delete handle;
        return nullptr;
    }

    // gemelli-core::Frame is BGRA8; advertising the matching sender format
    // means every send strategy above writes straight into a BGRA texture
    // with no channel swizzle.
    handle->sender.SetSenderFormat(DXGI_FORMAT_B8G8R8A8_UNORM);

    handle->initialized = true;
    return handle;
}

void spout_bridge_destroy(SpoutBridgeHandle* handle) {
    if (!handle) {
        return;
    }
    release_staging(handle);
    handle->sender.ReleaseSender();
    handle->sender.CloseDirectX11();
    delete handle;
}

int32_t spout_bridge_send_bgra_mode(SpoutBridgeHandle* handle,
                                     const uint8_t* pixels,
                                     uint32_t width,
                                     uint32_t height,
                                     uint32_t pitch,
                                     uint32_t mode) {
    if (!handle || !pixels || width == 0 || height == 0) {
        return -1;
    }
    // Reject a caller-declared stride narrower than one packed BGRA row.
    if (static_cast<uint64_t>(pitch) < static_cast<uint64_t>(width) * 4) {
        return -1;
    }
    if (!handle->initialized || !handle->device || !handle->context) {
        return -2;
    }

    switch (mode) {
        case 1:
            return send_image(handle, pixels, width, height, pitch);
        case 2:
            return send_staging_sse(handle, pixels, width, height, pitch);
        case 0:
        default:
            return send_staging_row_copy(handle, pixels, width, height, pitch);
    }
}

} // extern "C"
