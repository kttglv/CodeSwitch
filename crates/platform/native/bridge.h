#pragma once
#include <stdint.h>
#include <stddef.h>

// Only POD crosses the Rust/native boundary. NativeSnapshot owns its opaque handle.
// Never put document contents in diagnostics. Suffix is at most 192 UTF-16 units.
struct CsEvent {
    uint64_t revision;
    uint32_t kind; // 1 text, 2 backspace, 3 reset, 4 undo, 5 transaction complete
};
struct CsSnapshot {
    void* handle;
    uint64_t field;
    int64_t caret;
    uint64_t revision;
    uint32_t layout; // 1 US, 2 Russian; other layouts are unsupported
    uint16_t suffix[192];
    uint32_t length;
    char app[512];
};
typedef void (*CsCallback)(const CsEvent*, void*);

extern "C" {
int cs_start(CsCallback callback, void* context);
void cs_stop();
uint32_t cs_permissions(int request);
uint32_t cs_layouts();
uint64_t cs_revision();
int cs_snapshot(CsSnapshot* output);
void cs_release(void* handle);
// 1: transaction queued, 0: cancelled before editing, -1: injection failure.
int cs_replace(const CsSnapshot* before, const uint16_t* expected, uint32_t expected_len,
               const uint16_t* replacement, uint32_t replacement_len, uint32_t layout);
// Release queued hardware input only after the Rust worker verifies the result.
void cs_finish(int replay);
void cs_shortcut(uint32_t modifiers, uint32_t key); // Ctrl=1 Alt=2 Shift=4 Meta=8
void cs_capture(int enabled); // Paused hooks forward input without collecting events.
}
