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
};

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

bool syphon_bridge_send_rgba(SyphonBridgeHandle* handle,
                              const uint8_t* pixels,
                              uint32_t width,
                              uint32_t height,
                              uint32_t bytes_per_row) {
    if (!handle || !pixels || width == 0 || height == 0) {
        return false;
    }
    // Defend against a caller-declared stride narrower than one packed row —
    // the row-wise copy below would read past the end of `pixels`.
    if ((uint64_t)bytes_per_row < (uint64_t)width * 4) {
        return false;
    }

    @autoreleasepool {
        // Metal requires an IOSurface-backed texture's stride to satisfy the
        // platform row-alignment (16 bytes for BGRA8); the caller's stride is
        // the tightly-packed source layout, which a cropped width can leave
        // unaligned.
        size_t surfaceBytesPerRow =
            IOSurfaceAlignProperty(kIOSurfaceBytesPerRow, (size_t)width * 4);

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
            NSLog(@"[SyphonBridge] send_rgba: IOSurfaceCreate failed (%ux%u)", width, height);
            return false;
        }

        IOSurfaceLock(surface, 0, nullptr);
        uint8_t* dstBase = static_cast<uint8_t*>(IOSurfaceGetBaseAddress(surface));
        size_t dstBytesPerRow = IOSurfaceGetBytesPerRow(surface);
        size_t copyWidth = (size_t)width * 4;

        // Row-wise copy: the IOSurface's actual stride (page/tile aligned by
        // the system) can differ from the caller's `bytes_per_row`, so a
        // single memcpy over the whole buffer would misalign every row past
        // the first.
        const uint8_t* srcRow = pixels;
        uint8_t* dstRow = dstBase;
        for (uint32_t row = 0; row < height; row++) {
            memcpy(dstRow, srcRow, copyWidth);
            srcRow += bytes_per_row;
            dstRow += dstBytesPerRow;
        }
        IOSurfaceUnlock(surface, 0, nullptr);

        MTLTextureDescriptor* desc =
            [MTLTextureDescriptor texture2DDescriptorWithPixelFormat:MTLPixelFormatBGRA8Unorm
                                                                width:width
                                                               height:height
                                                            mipmapped:NO];
        desc.usage = MTLTextureUsageShaderRead;
        desc.storageMode = MTLStorageModeShared;

        id<MTLTexture> texture = [handle->device newTextureWithDescriptor:desc
                                                                 iosurface:surface
                                                                     plane:0];
        CFRelease(surface);

        if (!texture) {
            NSLog(@"[SyphonBridge] send_rgba: failed to wrap IOSurface as MTLTexture");
            return false;
        }

        // Publish is fire-and-forget: we commit and return immediately.
        // `addCompletedHandler` only logs a GPU-side error asynchronously —
        // callers see success as soon as the command buffer is queued, same
        // as the reference bridge.
        id<MTLCommandBuffer> cmdBuf = [handle->commandQueue commandBuffer];
        [handle->server publishFrameTexture:texture
                            onCommandBuffer:cmdBuf
                                imageRegion:NSMakeRect(0, 0, width, height)
                                    flipped:YES];
        [cmdBuf addCompletedHandler:^(id<MTLCommandBuffer> completed) {
            if (completed.error) {
                NSLog(@"[SyphonBridge] send_rgba: publish command buffer error: %@", completed.error);
            }
        }];
        [cmdBuf commit];

        return true;
    }
}

void syphon_bridge_destroy(SyphonBridgeHandle* handle) {
    if (!handle) {
        return;
    }
    @autoreleasepool {
        [handle->server stop];
        delete handle; // runs the ARC-synthesized destructor for device/commandQueue/server
    }
}

} // extern "C"
