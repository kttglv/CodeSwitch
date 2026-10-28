#import <Cocoa/Cocoa.h>
#import <ApplicationServices/ApplicationServices.h>
#import <Carbon/Carbon.h>
#include "bridge.h"
#include <atomic>
#include <chrono>
#include <mutex>
#include <thread>
#include <vector>
#include <cstring>

namespace {
constexpr int64_t OWN = 0x43535749544348;
constexpr int64_t END = OWN + 1;
CsCallback callback = nullptr;
void* context = nullptr;
std::atomic<bool> running{false}, active{false};
std::atomic<bool> capture{false};
std::atomic<uint64_t> revision{0};
std::atomic<uint32_t> undoMods{10}, undoKey{8};
std::mutex gateMutex;
bool gated = false;
std::chrono::steady_clock::time_point gateStarted;
std::vector<CGEventRef> queued;
CFRunLoopRef hookLoop = nullptr;
CFMachPortRef eventTap = nullptr;
std::thread hookThread;
AXUIElementRef lastField = nullptr;
uint64_t fieldIdentity = 0;
bool undoHeld = false;

void emit(uint32_t kind) {
    CsEvent message{};
    message.kind = kind;
    message.revision = kind == 5 ? revision.load() : ++revision;
    if (callback) callback(&message, context);
}

uint32_t mods(CGEventFlags flags) {
    return ((flags & kCGEventFlagMaskControl) ? 1 : 0)
        | ((flags & kCGEventFlagMaskAlternate) ? 2 : 0)
        | ((flags & kCGEventFlagMaskShift) ? 4 : 0)
        | ((flags & kCGEventFlagMaskCommand) ? 8 : 0);
}

uint32_t key(uint64_t native) {
    switch (native) {
        case kVK_Delete: return 8;
        case kVK_F8: return 119;
        case kVK_F9: return 120;
        case kVK_F10: return 121;
        default: return 0;
    }
}

CGEventRef tapCallback(CGEventTapProxy, CGEventType type, CGEventRef event, void*) {
    if (type == kCGEventTapDisabledByTimeout || type == kCGEventTapDisabledByUserInput) {
        active = false;
        emit(3);
        if (eventTap && AXIsProcessTrusted()) { CGEventTapEnable(eventTap, true); active = true; }
        return event;
    }
    auto tag = CGEventGetIntegerValueField(event, kCGEventSourceUserData);
    if (tag == OWN) return event;
    if (tag == END) { emit(5); return event; }
    {
        std::lock_guard<std::mutex> lock(gateMutex);
        if (gated) {
            if (queued.size() < 128 && std::chrono::steady_clock::now() - gateStarted < std::chrono::milliseconds(200)) {
                queued.push_back(CGEventCreateCopy(event));
                return nullptr;
            }
            // Never let a stalled transaction freeze the keyboard indefinitely.
            for (auto item : queued) CFRelease(item);
            queued.clear(); gated = false; emit(3);
        }
    }
    if (!capture) return event;
    if (type == kCGEventLeftMouseDown || type == kCGEventRightMouseDown || type == kCGEventOtherMouseDown || type == kCGEventScrollWheel) {
        emit(3); return event;
    }
    if (type == kCGEventFlagsChanged) {
        // Preserve immediate undo while its modifier keys are being pressed.
        // The following non-modifier key determines whether this is undo or reset.
        return event;
    }
    auto code = CGEventGetIntegerValueField(event, kCGKeyboardEventKeycode);
    if (type == kCGEventKeyUp) {
        if (undoHeld && key(code) == undoKey.load()) { undoHeld = false; return nullptr; }
        return event;
    }
    if (type != kCGEventKeyDown) return event;
    auto modifiers = mods(CGEventGetFlags(event));
    if (modifiers == undoMods && key(code) == undoKey) {
        undoHeld = true; emit(4); return nullptr;
    }
    if (modifiers & (1 | 2 | 8)) { emit(3); return event; }
    if (code == kVK_Delete) { emit(2); return event; }
    if (code == kVK_Return || code == kVK_Tab || code == kVK_Escape || code == kVK_ForwardDelete
        || code == kVK_LeftArrow || code == kVK_RightArrow || code == kVK_UpArrow || code == kVK_DownArrow
        || code == kVK_Home || code == kVK_End || code == kVK_PageUp || code == kVK_PageDown) {
        emit(3); return event;
    }
    // Send only a physical-key notification. Text is obtained later from a
    // verified editable accessibility range, never from password key events.
    if (code <= 50) emit(1);
    else emit(3);
    return event;
}

uint32_t sourceLayout(TISInputSourceRef source) {
    if (!source) return 0;
    auto id = (CFStringRef)TISGetInputSourceProperty(source, kTISPropertyInputSourceID);
    if (!id) return 0;
    if (CFEqual(id, CFSTR("com.apple.keylayout.US")) || CFEqual(id, CFSTR("com.apple.keylayout.ABC"))) return 1;
    // Russian — PC matches standard Windows ЙЦУКЕН punctuation/ё positions.
    // Apple's different "Russian" mapping is deliberately not treated as identical.
    if (CFEqual(id, CFSTR("com.apple.keylayout.RussianWin"))) return 2;
    return 0;
}

uint32_t currentLayout() {
    auto source = TISCopyCurrentKeyboardInputSource();
    auto result = sourceLayout(source);
    if (source) CFRelease(source);
    return result;
}

bool selectLayout(uint32_t layout) {
    auto sources = TISCreateInputSourceList(nullptr, false);
    if (!sources) return false;
    bool selected = false;
    for (CFIndex i = 0; i < CFArrayGetCount(sources); ++i) {
        auto source = (TISInputSourceRef)CFArrayGetValueAtIndex(sources, i);
        auto enabled = (CFBooleanRef)TISGetInputSourceProperty(source, kTISPropertyInputSourceIsEnabled);
        if (sourceLayout(source) == layout && enabled && CFBooleanGetValue(enabled)) {
            selected = TISSelectInputSource(source) == noErr;
            break;
        }
    }
    CFRelease(sources);
    return selected && currentLayout() == layout;
}

struct Guard {
    AXUIElementRef element;
    CFRange selection;
    Guard(AXUIElementRef element, CFRange selection) : element(element), selection(selection) { CFRetain(element); }
    ~Guard() { CFRelease(element); }
};

bool attribute(AXUIElementRef element, CFStringRef name, CFTypeRef* value) {
    return AXUIElementCopyAttributeValue(element, name, value) == kAXErrorSuccess && *value;
}

bool snapshot(CsSnapshot* output) {
    if (!AXIsProcessTrusted() || IsSecureEventInputEnabled()) return false;
    const auto epoch = revision.load();
    auto system = AXUIElementCreateSystemWide();
    CFTypeRef rawElement = nullptr;
    if (!attribute(system, kAXFocusedUIElementAttribute, &rawElement)) { CFRelease(system); return false; }
    CFRelease(system);
    auto element = (AXUIElementRef)rawElement;
    AXUIElementSetMessagingTimeout(element, 0.025);
    CFTypeRef role = nullptr, subrole = nullptr, enabled = nullptr, selected = nullptr;
    bool safe = attribute(element, kAXRoleAttribute, &role)
        && (CFEqual(role, kAXTextFieldRole) || CFEqual(role, kAXTextAreaRole) || CFEqual(role, kAXComboBoxRole));
    if (attribute(element, kAXSubroleAttribute, &subrole) && CFEqual(subrole, kAXSecureTextFieldSubrole)) safe = false;
    if (!attribute(element, kAXEnabledAttribute, &enabled) || !CFEqual(enabled, kCFBooleanTrue)) safe = false;
    Boolean editable = false;
    if (AXUIElementIsAttributeSettable(element, kAXSelectedTextAttribute, &editable) != kAXErrorSuccess || !editable) safe = false;
    CFRange selection{};
    if (!attribute(element, kAXSelectedTextRangeAttribute, &selected)
        || CFGetTypeID(selected) != AXValueGetTypeID()
        || !AXValueGetValue((AXValueRef)selected, kAXValueTypeCFRange, &selection)
        || selection.length != 0 || selection.location < 0) safe = false;
    if (role) CFRelease(role);
    if (subrole) CFRelease(subrole);
    if (enabled) CFRelease(enabled);
    if (selected) CFRelease(selected);
    pid_t pid = 0;
    AXUIElementGetPid(element, &pid);
    if (pid == getpid()) safe = false; // the settings sandbox never watches itself
    auto layout = currentLayout();
    if (!layout) safe = false;
    if (!safe) { CFRelease(element); return false; }
    CFRange range{std::max((CFIndex)0, selection.location - 192), std::min((CFIndex)192, selection.location)};
    auto parameter = AXValueCreate(kAXValueTypeCFRange, &range);
    CFTypeRef text = nullptr;
    bool readable = AXUIElementCopyParameterizedAttributeValue(element, kAXStringForRangeParameterizedAttribute, parameter, &text) == kAXErrorSuccess
        && text && CFGetTypeID(text) == CFStringGetTypeID();
    CFRelease(parameter);
    if (!readable || revision != epoch) {
        if (text) CFRelease(text);
        CFRelease(element); return false;
    }
    memset(output, 0, sizeof(*output));
    if (!lastField || !CFEqual(element, lastField)) {
        if (lastField) CFRelease(lastField);
        lastField = (AXUIElementRef)CFRetain(element);
        ++fieldIdentity;
    }
    output->field = fieldIdentity;
    output->caret = selection.location;
    output->revision = epoch;
    output->layout = layout;
    output->length = (uint32_t)CFStringGetLength((CFStringRef)text);
    if (output->length > 192) { CFRelease(text); CFRelease(element); return false; }
    CFStringGetCharacters((CFStringRef)text, CFRangeMake(0, output->length), output->suffix);
    CFRelease(text);
    @autoreleasepool {
        auto app = [NSRunningApplication runningApplicationWithProcessIdentifier:pid];
        NSString* identifier = app.bundleIdentifier ?: app.executableURL.lastPathComponent;
        if (!identifier) { CFRelease(element); return false; }
        strncpy(output->app, identifier.UTF8String, sizeof(output->app) - 1);
    }
    output->handle = new Guard(element, selection);
    CFRelease(element);
    return true;
}

void postKey(CGKeyCode keycode, bool down, const uint16_t* character = nullptr, int64_t tag = OWN) {
    auto event = CGEventCreateKeyboardEvent(nullptr, keycode, down);
    CGEventSetFlags(event, 0);
    if (character) CGEventKeyboardSetUnicodeString(event, 1, character);
    CGEventSetIntegerValueField(event, kCGEventSourceUserData, tag);
    CGEventPost(kCGHIDEventTap, event);
    CFRelease(event);
}

void cancelBeforeEditing(const CsSnapshot* before) {
    CsSnapshot current{};
    bool unchanged = cs_snapshot(&current) && current.field == before->field
        && current.caret == before->caret && current.length == before->length
        && memcmp(current.suffix, before->suffix, before->length * sizeof(uint16_t)) == 0;
    cs_release(current.handle);
    cs_finish(unchanged ? 1 : 0);
}
}

extern "C" {
int cs_start(CsCallback fn, void* ctx) {
    if (running.exchange(true)) return 0;
    callback = fn; context = ctx;
    hookThread = std::thread([] {
        hookLoop = CFRunLoopGetCurrent();
        CFRetain(hookLoop);
        while (running) {
            if (!eventTap && AXIsProcessTrusted()) {
                CGEventMask mask = CGEventMaskBit(kCGEventKeyDown) | CGEventMaskBit(kCGEventKeyUp)
                    | CGEventMaskBit(kCGEventFlagsChanged) | CGEventMaskBit(kCGEventLeftMouseDown)
                    | CGEventMaskBit(kCGEventLeftMouseUp) | CGEventMaskBit(kCGEventRightMouseDown)
                    | CGEventMaskBit(kCGEventRightMouseUp) | CGEventMaskBit(kCGEventOtherMouseDown)
                    | CGEventMaskBit(kCGEventOtherMouseUp) | CGEventMaskBit(kCGEventScrollWheel);
                eventTap = CGEventTapCreate(kCGSessionEventTap, kCGHeadInsertEventTap, kCGEventTapOptionDefault, mask, tapCallback, nullptr);
                if (eventTap) {
                    auto source = CFMachPortCreateRunLoopSource(nullptr, eventTap, 0);
                    CFRunLoopAddSource(hookLoop, source, kCFRunLoopCommonModes);
                    CFRelease(source);
                    CGEventTapEnable(eventTap, true); active = true;
                }
            }
            if (!AXIsProcessTrusted() && active) { active = false; emit(3); }
            if (eventTap && !active && AXIsProcessTrusted()) { CGEventTapEnable(eventTap, true); active = true; }
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.2, false);
        }
        if (eventTap) { CFMachPortInvalidate(eventTap); CFRelease(eventTap); eventTap = nullptr; }
        active = false; CFRelease(hookLoop); hookLoop = nullptr;
    });
    return 1;
}
void cs_stop() {
    running = false;
    if (hookLoop) CFRunLoopStop(hookLoop);
    if (hookThread.joinable()) hookThread.join();
    cs_finish(0);
}
uint32_t cs_permissions(int request) {
    bool trusted = AXIsProcessTrusted();
    if (request && !trusted) {
        const void* keys[] = {kAXTrustedCheckOptionPrompt};
        const void* values[] = {kCFBooleanTrue};
        auto options = CFDictionaryCreate(nullptr, keys, values, 1, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
        trusted = AXIsProcessTrustedWithOptions(options);
        CFRelease(options);
    }
    return (trusted ? 1 : 0) | (active ? 2 : 0);
}
uint32_t cs_layouts() {
    uint32_t flags = 0;
    auto sources = TISCreateInputSourceList(nullptr, false);
    if (!sources) return 0;
    for (CFIndex i = 0; i < CFArrayGetCount(sources); ++i) {
        auto source = (TISInputSourceRef)CFArrayGetValueAtIndex(sources, i);
        auto enabled = (CFBooleanRef)TISGetInputSourceProperty(source, kTISPropertyInputSourceIsEnabled);
        if (enabled && CFBooleanGetValue(enabled)) {
            auto layout = sourceLayout(source);
            flags |= layout == 1 ? 1 : layout == 2 ? 2 : 0;
        }
    }
    CFRelease(sources); return flags;
}
uint64_t cs_revision() { return revision.load(); }
int cs_snapshot(CsSnapshot* output) { @autoreleasepool { return snapshot(output) ? 1 : 0; } }
void cs_release(void* handle) { delete (Guard*)handle; }
void cs_shortcut(uint32_t modifiers, uint32_t shortcutKey) { undoMods = modifiers; undoKey = shortcutKey; }
void cs_capture(int enabled) { capture = enabled != 0; }

int cs_replace(const CsSnapshot* before, const uint16_t* expected, uint32_t expectedLen,
               const uint16_t* replacement, uint32_t replacementLen, uint32_t layout) {
    if (!before || !before->handle || !expectedLen || replacementLen > 192 || !active) return 0;
    {
        std::lock_guard<std::mutex> lock(gateMutex);
        if (gated || revision != before->revision) return 0;
        gated = true; gateStarted = std::chrono::steady_clock::now();
    }
    CsSnapshot current{};
    bool valid = cs_snapshot(&current) && current.field == before->field && current.caret == before->caret
        && current.revision == before->revision && current.layout == before->layout
        && current.length >= expectedLen
        && memcmp(current.suffix + current.length - expectedLen, expected, expectedLen * sizeof(uint16_t)) == 0;
    cs_release(current.handle);
    if (!valid || !selectLayout(layout)) { cancelBeforeEditing(before); return 0; }
    // The event tap holds newly arriving hardware events until the final key-up is
    // delivered and the worker verifies the edited suffix. No clipboard is involved.
    for (uint32_t i = 0; i < expectedLen; ++i) { postKey(kVK_Delete, true); postKey(kVK_Delete, false); }
    for (uint32_t i = 0; i < replacementLen; ++i) {
        postKey(0, true, replacement + i);
        postKey(0, false, replacement + i, i + 1 == replacementLen ? END : OWN);
    }
    return 1;
}

void cs_finish(int replay) {
    std::lock_guard<std::mutex> lock(gateMutex);
    for (auto event : queued) {
        if (replay) {
            CGEventSetIntegerValueField(event, kCGEventSourceUserData, OWN);
            CGEventPost(kCGHIDEventTap, event);
        }
        CFRelease(event);
    }
    queued.clear(); gated = false;
}
}
