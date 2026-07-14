#ifndef GEMELLI_SPOUT_BRIDGE_H
#define GEMELLI_SPOUT_BRIDGE_H

#include <cstdint>

#ifdef __cplusplus
extern "C" {
#endif

// Opaque handle to a Spout DirectX sender.
typedef struct SpoutBridgeHandle SpoutBridgeHandle;

// Create a Spout sender advertised under `name` (NUL-terminated UTF-8). The
// SDK copies `name` into a fixed 256-byte buffer (spoutSenderNames); longer
// names are truncated by the SDK, not by this bridge.
// Returns nullptr on failure (no D3D11 device, or name rejected).
SpoutBridgeHandle* spout_bridge_create(const char* name);

// Release the sender, its staging texture (if any), and the D3D11 device.
void spout_bridge_destroy(SpoutBridgeHandle* handle);

// Publish one BGRA8 frame using a specific send strategy. `pixels` must point
// to at least `pitch * height` readable bytes; `pitch` is the row stride in
// bytes (>= width*4). Pixels are copied out of `pixels` before this call
// returns; the bridge retains no pointer into the caller's buffer.
//
// `mode` selects the send strategy:
//   0 (StagingRowCopy) - DYNAMIC staging texture, filled with a row-by-row
//                         memcpy per scanline, then SpoutDX::SendTexture.
//   1 (SendImage)       - SpoutDX::SendImage, which uploads straight into
//                         the shared texture via UpdateSubresource (no
//                         staging texture).
//   2 (StagingSse)      - DYNAMIC staging texture, filled with SpoutCopy's
//                         SSE2 pitch-aware line copier (SpoutCopy::rgba2rgba
//                         is channel-agnostic, so no swizzle happens even
//                         though frames are BGRA), then SendTexture. When
//                         `pitch` is not a multiple of 16 bytes -- the SSE2
//                         copier issues aligned SIMD loads/stores that fault
//                         on such rows -- this mode automatically falls back
//                         to the mode 0 row-by-row memcpy for the copy step.
// Any other value falls back to mode 0.
//
// Returns a staged status code:
//    0  success
//   -1  bad arguments: null handle/pixels, zero width/height, or a pitch
//       narrower than one packed BGRA row (pitch < width*4)
//   -2  handle not initialized (spout_bridge_create's DirectX11/sender setup
//       did not complete)
//   -3  staging texture allocation failed (modes 0 and 2 only)
//   -4  Map() of the staging texture failed (modes 0 and 2 only)
//   -5  the underlying SpoutDX send call (SendTexture/SendImage) failed
int32_t spout_bridge_send_bgra_mode(SpoutBridgeHandle* handle,
                                     const uint8_t* pixels,
                                     uint32_t width,
                                     uint32_t height,
                                     uint32_t pitch,
                                     uint32_t mode);

#ifdef __cplusplus
}
#endif

#endif // GEMELLI_SPOUT_BRIDGE_H
