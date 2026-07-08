#ifndef GEMELLI_SPOUT_BRIDGE_H
#define GEMELLI_SPOUT_BRIDGE_H

#include <cstdint>

#ifdef __cplusplus
extern "C" {
#endif

// Opaque handle to a Spout DirectX sender.
typedef struct SpoutBridgeHandle SpoutBridgeHandle;

// Create a Spout sender advertised under `name` (NUL-terminated UTF-8).
// Returns nullptr on failure (no D3D11 device or name rejected).
SpoutBridgeHandle* spout_bridge_create(const char* name);

// Send one BGRA8 frame. `pixels` must point to at least `pitch * height`
// readable bytes; `pitch` is the row stride in bytes (>= width*4). Pixels are
// copied before returning. Returns true on success.
bool spout_bridge_send_bgra(SpoutBridgeHandle* handle,
                            const uint8_t* pixels,
                            uint32_t width,
                            uint32_t height,
                            uint32_t pitch);

// Release the sender and its D3D11 device.
void spout_bridge_destroy(SpoutBridgeHandle* handle);

#ifdef __cplusplus
}
#endif

#endif // GEMELLI_SPOUT_BRIDGE_H
