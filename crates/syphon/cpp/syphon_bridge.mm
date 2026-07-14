#import "syphon_bridge.h"
#import <Metal/Metal.h>
#import <IOSurface/IOSurface.h>
#import <Syphon/Syphon.h>
#import <Foundation/Foundation.h>
// kCVPixelFormatType_32BGRA lives here — not pulled in transitively by
// Foundation.h (the reference bridge imports the Cocoa.h umbrella header
// instead, which happens to re-export it; this project only needs
// Foundation, so it imports CoreVideo directly rather than all of Cocoa).
#import <CoreVideo/CVPixelBuffer.h>

// Real definition of the handle declared opaque in the header. ARC manages
// the three id<...>-typed fields: `-fobjc-arc` gives this C++ struct a
// non-trivial constructor/destructor pair that retains on construction and
// releases on destruction, the same way the reference bridge's `SyphonBridge`
// struct does. No manual retain/release or __bridge cast is needed for the
// handle itself — only IOSurfaceCreate's NSDictionary argument needs one
// (Core Foundation's CFDictionaryRef vs. Foundation's NSDictionary*).
struct SyphonBridgeHandle {
    id<MTLDevice>       device;
    id<MTLCommandQueue> commandQueue;
    SyphonMetalServer*  server;

    // Cache reused by send_persistent (mode 1). `syphon_bridge_create` builds
    // this struct via aggregate init (`new SyphonBridgeHandle{device, queue,
    // server}`), which only sets the three fields above by position — every
    // field below MUST carry a default member initializer, or it holds
    // garbage and release_cache/ensure_cache read an uninitialized CF
    // pointer.
    IOSurfaceRef   surface = nullptr;      // CFRetained (owned)
    id<MTLTexture> texture = nil;          // wraps `surface`, keeping it alive
    uint32_t       width = 0;
    uint32_t       height = 0;

    // Most recently committed publish command buffer (ARC-managed, so the
    // default member initializer is enough — see syphon_bridge_destroy for
    // why this must be drained before tearing down `server`).
    id<MTLCommandBuffer> lastCommandBuffer = nil;
};

// Row-wise copy: the IOSurface's actual stride (page/tile aligned by the
// system) can differ from the caller's `bytes_per_row`, so a single memcpy
// over the whole buffer would misalign every row past the first.
static void copy_rows(IOSurfaceRef surface, const uint8_t* pixels, uint32_t width,
                       uint32_t height, uint32_t bytes_per_row) {
    IOSurfaceLock(surface, 0, nullptr);
    uint8_t* dstRow = static_cast<uint8_t*>(IOSurfaceGetBaseAddress(surface));
    size_t dstBytesPerRow = IOSurfaceGetBytesPerRow(surface);
    size_t copyWidth = (size_t)width * 4;

    const uint8_t* srcRow = pixels;
    for (uint32_t row = 0; row < height; row++) {
        memcpy(dstRow, srcRow, copyWidth);
        srcRow += bytes_per_row;
        dstRow += dstBytesPerRow;
    }
    IOSurfaceUnlock(surface, 0, nullptr);
}

// Releases the cached surface/texture and resets the cached geometry.
static void release_cache(SyphonBridgeHandle* h) {
    h->texture = nil;
    if (h->surface) {
        CFRelease(h->surface);
        h->surface = nullptr;
    }
    h->width = 0;
    h->height = 0;
}

// (Re)builds the cached IOSurface + MTLTexture when the requested geometry
// doesn't match what's cached. Only geometry gates the cache: the aligned
// pitch below is always recomputed from `width`, so unlike a raw stride
// comparison it can never validate against a stale, narrower pitch.
static bool ensure_cache(SyphonBridgeHandle* h, uint32_t width, uint32_t height) {
    if (h->surface && h->width == width && h->height == height) {
        return true;
    }
    release_cache(h);

    // Metal requires an IOSurface-backed texture's stride to satisfy the
    // platform row-alignment (16 bytes for BGRA8); the caller's stride is
    // the tightly-packed source layout, which a cropped width can leave
    // unaligned.
    size_t alignedBytesPerRow = IOSurfaceAlignProperty(kIOSurfaceBytesPerRow, (size_t)width * 4);

    NSDictionary* surfaceProps = @{
        (NSString*)kIOSurfaceWidth: @(width),
        (NSString*)kIOSurfaceHeight: @(height),
        (NSString*)kIOSurfaceBytesPerElement: @4,
        (NSString*)kIOSurfaceBytesPerRow: @(alignedBytesPerRow),
        (NSString*)kIOSurfacePixelFormat: @(kCVPixelFormatType_32BGRA),
        (NSString*)kIOSurfaceAllocSize: @(alignedBytesPerRow * (size_t)height),
    };

    IOSurfaceRef surface = IOSurfaceCreate((__bridge CFDictionaryRef)surfaceProps);
    if (!surface) {
        NSLog(@"[SyphonBridge] ensure_cache: IOSurfaceCreate failed (%ux%u)", width, height);
        return false;
    }

    MTLTextureDescriptor* desc =
        [MTLTextureDescriptor texture2DDescriptorWithPixelFormat:MTLPixelFormatBGRA8Unorm
                                                            width:width
                                                           height:height
                                                        mipmapped:NO];
    desc.usage = MTLTextureUsageShaderRead;
    desc.storageMode = MTLStorageModeShared;

    id<MTLTexture> texture = [h->device newTextureWithDescriptor:desc
                                                         iosurface:surface
                                                             plane:0];
    if (!texture) {
        NSLog(@"[SyphonBridge] ensure_cache: failed to wrap IOSurface as MTLTexture");
        CFRelease(surface);
        return false;
    }

    h->surface = surface;
    h->texture = texture;
    h->width = width;
    h->height = height;
    return true;
}

// Adds the shared error-logging completed handler, commits `cmdBuf`, and
// records it as `h->lastCommandBuffer` so `syphon_bridge_destroy` can wait
// for this frame's GPU work (and the completed handler above) to finish
// before tearing down the server. Shared by both publish paths below.
static void commit_and_track(SyphonBridgeHandle* h, id<MTLCommandBuffer> cmdBuf,
                              const char* site) {
    [cmdBuf addCompletedHandler:^(id<MTLCommandBuffer> completed) {
        if (completed.error) {
            NSLog(@"[SyphonBridge] %s: publish command buffer error: %@", site, completed.error);
        }
    }];
    [cmdBuf commit];
    h->lastCommandBuffer = cmdBuf;
}

// Commits a Syphon publish of the cached texture on a fresh command buffer.
//
// Single-buffered on purpose: Syphon blits this texture into its own
// surface inside this command buffer. The next frame's CPU copy could in
// principle overlap a still-in-flight blit, tearing one frame; at webcam
// rates the blit finishes orders of magnitude sooner. Matches the
// reference bridge, which is verified tearing-free on real hardware.
static bool publish_cached(SyphonBridgeHandle* h, uint32_t width, uint32_t height) {
    id<MTLCommandBuffer> cmdBuf = [h->commandQueue commandBuffer];
    [h->server publishFrameTexture:h->texture
                    onCommandBuffer:cmdBuf
                        imageRegion:NSMakeRect(0, 0, width, height)
                            flipped:YES];
    commit_and_track(h, cmdBuf, "publish_cached");
    return true;
}

// Mode 0: allocate a fresh IOSurface + MTLTexture every frame, row-copy,
// publish, release. Baseline strategy kept for A/B benchmarking against
// send_persistent below (see examples/bench_syphon_cpu.rs).
static bool send_perframe(SyphonBridgeHandle* h, const uint8_t* pixels,
                           uint32_t width, uint32_t height, uint32_t bytes_per_row) {
    // Metal requires an IOSurface-backed texture's stride to satisfy the
    // platform row-alignment (16 bytes for BGRA8); the caller's stride is
    // the tightly-packed source layout, which a cropped width can leave
    // unaligned.
    size_t surfaceBytesPerRow = IOSurfaceAlignProperty(kIOSurfaceBytesPerRow, (size_t)width * 4);

    NSDictionary* surfaceProps = @{
        (NSString*)kIOSurfaceWidth: @(width),
        (NSString*)kIOSurfaceHeight: @(height),
        (NSString*)kIOSurfaceBytesPerElement: @4,
        (NSString*)kIOSurfaceBytesPerRow: @(surfaceBytesPerRow),
        (NSString*)kIOSurfacePixelFormat: @(kCVPixelFormatType_32BGRA),
        (NSString*)kIOSurfaceAllocSize: @(surfaceBytesPerRow * (size_t)height),
    };

    IOSurfaceRef surface = IOSurfaceCreate((__bridge CFDictionaryRef)surfaceProps);
    if (!surface) {
        NSLog(@"[SyphonBridge] send_perframe: IOSurfaceCreate failed (%ux%u)", width, height);
        return false;
    }

    copy_rows(surface, pixels, width, height, bytes_per_row);

    MTLTextureDescriptor* desc =
        [MTLTextureDescriptor texture2DDescriptorWithPixelFormat:MTLPixelFormatBGRA8Unorm
                                                            width:width
                                                           height:height
                                                        mipmapped:NO];
    desc.usage = MTLTextureUsageShaderRead;
    desc.storageMode = MTLStorageModeShared;

    id<MTLTexture> texture = [h->device newTextureWithDescriptor:desc
                                                         iosurface:surface
                                                             plane:0];
    CFRelease(surface);

    if (!texture) {
        NSLog(@"[SyphonBridge] send_perframe: failed to wrap IOSurface as MTLTexture");
        return false;
    }

    // Publish is fire-and-forget: we commit and return immediately.
    // `addCompletedHandler` only logs a GPU-side error asynchronously —
    // callers see success as soon as the command buffer is queued, same
    // as the reference bridge. (`syphon_bridge_destroy` waits on
    // `h->lastCommandBuffer` before tearing down the server, so this stays
    // fire-and-forget from the caller's perspective.)
    id<MTLCommandBuffer> cmdBuf = [h->commandQueue commandBuffer];
    [h->server publishFrameTexture:texture
                    onCommandBuffer:cmdBuf
                        imageRegion:NSMakeRect(0, 0, width, height)
                            flipped:YES];
    commit_and_track(h, cmdBuf, "send_perframe");

    return true;
}

// Mode 1: reuse the cached IOSurface + MTLTexture, row-copy into it, publish.
static bool send_persistent(SyphonBridgeHandle* h, const uint8_t* pixels,
                             uint32_t width, uint32_t height, uint32_t bytes_per_row) {
    if (!ensure_cache(h, width, height)) {
        return false;
    }
    copy_rows(h->surface, pixels, width, height, bytes_per_row);
    return publish_cached(h, width, height);
}

extern "C" {

SyphonBridgeHandle* syphon_bridge_create(const char* server_name) {
    if (!server_name) {
        return nullptr;
    }
    @autoreleasepool {
        id<MTLDevice> device = MTLCreateSystemDefaultDevice();
        if (!device) {
            NSLog(@"[SyphonBridge] create: no default Metal device");
            return nullptr;
        }

        id<MTLCommandQueue> queue = [device newCommandQueue];
        if (!queue) {
            NSLog(@"[SyphonBridge] create: failed to create a command queue");
            return nullptr;
        }

        NSString* name = [NSString stringWithUTF8String:server_name];
        SyphonMetalServer* server = [[SyphonMetalServer alloc] initWithName:name
                                                                      device:device
                                                                     options:nil];
        if (!server) {
            NSLog(@"[SyphonBridge] create: failed to create SyphonMetalServer \"%@\"", name);
            return nullptr;
        }

        return new SyphonBridgeHandle{device, queue, server};
    }
}

bool syphon_bridge_send_rgba_mode(SyphonBridgeHandle* handle,
                                   const uint8_t* pixels,
                                   uint32_t width,
                                   uint32_t height,
                                   uint32_t bytes_per_row,
                                   uint32_t mode) {
    if (!handle || !pixels || width == 0 || height == 0) {
        return false;
    }
    // Defend against a caller-declared stride narrower than one packed row —
    // the row-wise copy below would read past the end of `pixels`.
    if ((uint64_t)bytes_per_row < (uint64_t)width * 4) {
        return false;
    }

    @autoreleasepool {
        switch (mode) {
            case 1:
                return send_persistent(handle, pixels, width, height, bytes_per_row);
            case 0:
            default:
                return send_perframe(handle, pixels, width, height, bytes_per_row);
        }
    }
}

bool syphon_bridge_send_rgba(SyphonBridgeHandle* handle,
                              const uint8_t* pixels,
                              uint32_t width,
                              uint32_t height,
                              uint32_t bytes_per_row) {
    // Production default: routed to the persistent cache (mode 1), which cut
    // CPU/frame ~4.5x at 1080p in the reference benchmark vs. per-frame
    // allocation (mode 0).
    return syphon_bridge_send_rgba_mode(handle, pixels, width, height, bytes_per_row, 1);
}

void syphon_bridge_destroy(SyphonBridgeHandle* handle) {
    if (!handle) {
        return;
    }
    @autoreleasepool {
        // Wait for the last publish's GPU work (and its completed handler)
        // to finish before touching `server`. Crash evidence (macOS .ips
        // reports, reproduced locally and on CI): EXC_BREAKPOINT SIGTRAP in
        // _dispatch_lane_class_dispose <- -[SyphonServerConnectionManager
        // .cxx_destruct] <- SyphonServer dealloc, always at a publisher-drop
        // boundary microseconds after the last publish commit. Syphon's
        // connection manager tears down a dispatch queue in dealloc;
        // destroying the server while the final publish's GPU work is still
        // in flight trips that dispose trap. `waitUntilCompleted` is
        // nil-safe, so this is a no-op if nothing was ever published.
        // Command buffers submitted to one MTLCommandQueue complete in
        // submission order, so waiting on only the last buffer also fences
        // every earlier buffer from `handle->commandQueue` — no need to
        // track more than one.
        [handle->lastCommandBuffer waitUntilCompleted];
        handle->lastCommandBuffer = nil;

        release_cache(handle);
        [handle->server stop];
        delete handle; // runs the ARC-synthesized destructor for device/commandQueue/server
    }
}

} // extern "C"
