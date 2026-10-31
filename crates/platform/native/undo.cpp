#include "bridge.h"
#include <atomic>

namespace {
// A single atomic value keeps a shortcut's modifiers and key consistent while
// the settings thread updates it and the hook thread reads it.
#ifdef __APPLE__
constexpr uint32_t defaultModifiers = 10;
#else
constexpr uint32_t defaultModifiers = 3;
#endif
std::atomic<uint64_t> shortcut{(uint64_t(defaultModifiers) << 32) | 8};
std::atomic<bool> available{false};
std::atomic<uint32_t> heldKey{0};
}

extern "C" {
void cs_shortcut(uint32_t modifiers, uint32_t key) {
    available = false;
    shortcut = (uint64_t(modifiers) << 32) | key;
}

void cs_undo_arm(int enabled) { available = enabled != 0; }
void cs_undo_clear() { available = false; heldKey = 0; }

int cs_undo_input(uint32_t modifiers, uint32_t key, int down) {
    if (!key) return 0;
    if (!down) {
        uint32_t expected = key;
        return heldKey.compare_exchange_strong(expected, 0) ? 1 : 0;
    }
    // Swallow repeats of a shortcut whose initial keydown was consumed, even
    // when the user has since released its modifier keys or changed settings.
    if (heldKey == key) return 1;
    const auto configured = shortcut.load();
    if (key != uint32_t(configured) || modifiers != uint32_t(configured >> 32)) return 0;
    if (!available.exchange(false)) return 0;
    heldKey = key;
    return 2;
}
}
