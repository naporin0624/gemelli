#pragma once

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Opaque handle. The real struct (device + command queue + Syphon server) is
// defined only in syphon_bridge.mm — Rust and any other C caller only ever
// see a pointer to an incomplete type.
typedef struct SyphonBridgeHandle SyphonBridgeHandle;

// Creates a Metal-backed Syphon server advertised under `server_name`.
// Returns NULL on failure (no default Metal device, or SyphonMetalServer
// construction failed) — check Console.app for the corresponding NSLog.
SyphonBridgeHandle* syphon_bridge_create(const char* server_name);

// Publishes one BGRA8 frame. `pixels` is row-major, tightly packed or not —
// `bytes_per_row` tells the bridge the real stride so callers may pass
// exactly `width * 4` (tightly packed, as this project's core always does)
// without the bridge assuming it.
//
// Returns true on success, false on failure (invalid arguments, IOSurface
// allocation failure, or Metal texture creation failure).
bool syphon_bridge_send_rgba(SyphonBridgeHandle* handle,
                              const uint8_t* pixels,
                              uint32_t width,
                              uint32_t height,
                              uint32_t bytes_per_row);

// Stops the Syphon server and releases all resources. `handle` must not be
// used after this call. Safe to call with NULL (no-op).
void syphon_bridge_destroy(SyphonBridgeHandle* handle);

#ifdef __cplusplus
}
#endif
