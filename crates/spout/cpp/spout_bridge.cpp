// C++ bridge exposing a minimal Spout DirectX sender to Rust FFI. Sender-only,
// CPU-pixel path: gemelli hands a BGRA8 CPU buffer per frame
// (gemelli-core::Frame), so we use spoutDX::SendImage rather than the GPU
// texture-handle path used by richer Spout integrations.

#include "spout_bridge.h"

#include "SpoutDX.h"

struct SpoutBridgeHandle {
    spoutDX sender;
};

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

    if (!handle->sender.SetSenderName(name)) {
        handle->sender.CloseDirectX11();
        delete handle;
        return nullptr;
    }

    // gemelli-core::Frame is BGRA8; advertising the matching sender format
    // makes SendImage's UpdateSubresource copy land in the right channel order.
    handle->sender.SetSenderFormat(DXGI_FORMAT_B8G8R8A8_UNORM);

    return handle;
}

bool spout_bridge_send_bgra(SpoutBridgeHandle* handle,
                            const uint8_t* pixels,
                            uint32_t width,
                            uint32_t height,
                            uint32_t pitch) {
    if (!handle || !pixels || width == 0 || height == 0) {
        return false;
    }
    // Reject a caller-declared stride narrower than one packed BGRA row.
    if (static_cast<uint64_t>(pitch) < static_cast<uint64_t>(width) * 4) {
        return false;
    }

    return handle->sender.SendImage(pixels, width, height, pitch);
}

void spout_bridge_destroy(SpoutBridgeHandle* handle) {
    if (!handle) {
        return;
    }
    handle->sender.ReleaseSender();
    handle->sender.CloseDirectX11();
    delete handle;
}

} // extern "C"
