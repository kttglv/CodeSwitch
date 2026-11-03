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
// Zero disables the convert shortcut.
std::atomic<uint64_t> convertShortcut{0};
std::atomic<bool> available{false};
// Armed while the worker has verified a supported, allowed field.
std::atomic<bool> convertAvailable{false};
std::atomic<uint32_t> heldKey{0};
}

extern "C" {
void cs_shortcut(uint32_t modifiers, uint32_t key) {
    available = false;
    shortcut = (uint64_t(modifiers) << 32) | key;
}

void cs_convert_shortcut(uint32_t modifiers, uint32_t key) {
    convertShortcut = key ? (uint64_t(modifiers) << 32) | key : 0;
}

void cs_undo_arm(int enabled) { available = enabled != 0; }
void cs_convert_arm(int enabled) { convertAvailable = enabled != 0; }
void cs_undo_clear() { available = false; convertAvailable = false; heldKey = 0; }

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
    if (key == uint32_t(configured) && modifiers == uint32_t(configured >> 32)
        && available.exchange(false)) {
        heldKey = key;
        return 2;
    }
    // Convert is a deliberate request, so it needs no armed correction record.
    const auto convert = convertShortcut.load();
    if (convert && convertAvailable && key == uint32_t(convert) && modifiers == uint32_t(convert >> 32)) {
        heldKey = key;
        return 3;
    }
    return 0;
}
}
