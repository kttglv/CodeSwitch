// Include the adapter to exercise its private current-layout and selection paths.
// This test never starts an event tap, reads a field, or changes the input source.
#include "../macos.mm"
#include <cstdlib>

void checkLayoutCalls() {
    if (!onMainThread([] { return [NSThread isMainThread] == YES; })) std::abort();
    if (currentLayout() > 2 || cs_layouts() > 3) std::abort();
    // A deliberately unsupported ID exercises enumeration without selecting one.
    if (selectLayout(UINT32_MAX)) std::abort();
}

int main() {
    @autoreleasepool {
        checkLayoutCalls(); // main-thread fast path must not dispatch to itself
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
            @autoreleasepool {
                for (int i = 0; i < 32; ++i) checkLayoutCalls();
                std::exit(0);
            }
        });
        CFRunLoopRun(); // process main-queue work as the desktop app does
    }
    return 1;
}
