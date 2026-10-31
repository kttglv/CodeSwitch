#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <objbase.h>
#include <oleauto.h>
#include <UIAutomation.h>
#include <wrl/client.h>
#include "bridge.h"
#include <algorithm>
#include <atomic>
#include <chrono>
#include <cstring>
#include <cwchar>
#include <mutex>
#include <thread>
#include <vector>

using Microsoft::WRL::ComPtr;
namespace {
constexpr ULONG_PTR OWN = 0x43535748;
constexpr ULONG_PTR END = OWN + 1;
CsCallback callback = nullptr;
void* context = nullptr;
std::atomic<bool> running{false}, active{false};
std::atomic<bool> capture{false};
std::atomic<uint64_t> revision{0};
std::atomic<uint32_t> undoMods{3}, undoKey{8};
std::atomic<DWORD> hookThreadId{0};
std::mutex gateMutex;
bool gated = false, undoHeld = false;
std::chrono::steady_clock::time_point gateStarted;
std::vector<INPUT> queued;
std::thread hookThread;

void emit(uint32_t kind) {
    CsEvent event{};
    event.revision = kind == 5 ? revision.load() : ++revision;
    event.kind = kind;
    if (callback) callback(&event, context);
}

uint32_t layoutKind(HKL layout) {
    // Only standard US and Russian, not Dvorak, phonetic, or other variants.
    auto id = reinterpret_cast<uintptr_t>(layout);
    if ((id & 0xFFFFFFFF) == 0x04090409) return 1;
    if ((id & 0xFFFFFFFF) == 0x04190419) return 2;
    return 0;
}
uint32_t modifiers() {
    return ((GetAsyncKeyState(VK_CONTROL) & 0x8000) ? 1 : 0)
        | ((GetAsyncKeyState(VK_MENU) & 0x8000) ? 2 : 0)
        | ((GetAsyncKeyState(VK_SHIFT) & 0x8000) ? 4 : 0)
        | (((GetAsyncKeyState(VK_LWIN) | GetAsyncKeyState(VK_RWIN)) & 0x8000) ? 8 : 0);
}
bool modifier(DWORD key) {
    return key == VK_LSHIFT || key == VK_RSHIFT || key == VK_SHIFT
        || key == VK_LCONTROL || key == VK_RCONTROL || key == VK_CONTROL
        || key == VK_LMENU || key == VK_RMENU || key == VK_MENU
        || key == VK_LWIN || key == VK_RWIN || key == VK_CAPITAL;
}
bool enqueue(INPUT input) {
    std::lock_guard<std::mutex> lock(gateMutex);
    if (!gated) return false;
    if (queued.size() >= 128 || std::chrono::steady_clock::now() - gateStarted > std::chrono::milliseconds(200)) {
        queued.clear(); gated = false; emit(3); return false;
    }
    queued.push_back(input); return true;
}

LRESULT CALLBACK keyboardHook(int code, WPARAM message, LPARAM data) {
    if (code < 0) return CallNextHookEx(nullptr, code, message, data);
    const auto& event = *reinterpret_cast<KBDLLHOOKSTRUCT*>(data);
    if (event.dwExtraInfo == END) { emit(5); return CallNextHookEx(nullptr, code, message, data); }
    if (event.dwExtraInfo == OWN) return CallNextHookEx(nullptr, code, message, data);
    if (event.flags & LLKHF_INJECTED) { emit(3); return CallNextHookEx(nullptr, code, message, data); }
    bool up = (event.flags & LLKHF_UP) != 0;
    INPUT input{};
    input.type = INPUT_KEYBOARD;
    input.ki.wVk = (WORD)event.vkCode;
    input.ki.wScan = (WORD)event.scanCode;
    input.ki.dwFlags = (up ? KEYEVENTF_KEYUP : 0) | ((event.flags & LLKHF_EXTENDED) ? KEYEVENTF_EXTENDEDKEY : 0);
    input.ki.dwExtraInfo = OWN;
    if (enqueue(input)) return 1;
    if (!capture) return CallNextHookEx(nullptr, code, message, data);
    if (up) {
        if (undoHeld && event.vkCode == undoKey) { undoHeld = false; return 1; }
        return CallNextHookEx(nullptr, code, message, data);
    }
    const auto mods = modifiers();
    if (mods == undoMods && event.vkCode == undoKey) { undoHeld = true; emit(4); return 1; }
    if (modifier(event.vkCode)) {
        return CallNextHookEx(nullptr, code, message, data);
    }
    if (mods & (1 | 2 | 8)) { emit(3); return CallNextHookEx(nullptr, code, message, data); }
    if (event.vkCode == VK_BACK) emit(2);
    else if (event.vkCode == VK_RETURN || event.vkCode == VK_TAB || event.vkCode == VK_ESCAPE
        || event.vkCode == VK_DELETE || (event.vkCode >= VK_PRIOR && event.vkCode <= VK_DOWN)
        || (event.vkCode >= VK_F1 && event.vkCode <= VK_F24)) emit(3);
    else {
        // Do not turn password key events into strings. The worker obtains the
        // typed character only after UI Automation verifies a safe editable field.
        if (event.vkCode == VK_SPACE || (event.vkCode >= '0' && event.vkCode <= 'Z')
            || (event.vkCode >= VK_OEM_1 && event.vkCode <= VK_OEM_3)
            || (event.vkCode >= VK_OEM_4 && event.vkCode <= VK_OEM_8)) emit(1);
        else emit(3);
    }
    return CallNextHookEx(nullptr, code, message, data);
}

LRESULT CALLBACK mouseHook(int code, WPARAM message, LPARAM data) {
    if (code < 0) return CallNextHookEx(nullptr, code, message, data);
    const auto& event = *reinterpret_cast<MSLLHOOKSTRUCT*>(data);
    if (event.dwExtraInfo == OWN || (event.flags & LLMHF_INJECTED)) return CallNextHookEx(nullptr, code, message, data);
    DWORD flags = 0;
    switch (message) {
        case WM_LBUTTONDOWN: flags = MOUSEEVENTF_LEFTDOWN; break;
        case WM_LBUTTONUP: flags = MOUSEEVENTF_LEFTUP; break;
        case WM_RBUTTONDOWN: flags = MOUSEEVENTF_RIGHTDOWN; break;
        case WM_RBUTTONUP: flags = MOUSEEVENTF_RIGHTUP; break;
        case WM_MBUTTONDOWN: flags = MOUSEEVENTF_MIDDLEDOWN; break;
        case WM_MBUTTONUP: flags = MOUSEEVENTF_MIDDLEUP; break;
        case WM_MOUSEWHEEL: flags = MOUSEEVENTF_WHEEL; break;
        case WM_MOUSEHWHEEL: flags = MOUSEEVENTF_HWHEEL; break;
        case WM_XBUTTONDOWN: flags = MOUSEEVENTF_XDOWN; break;
        case WM_XBUTTONUP: flags = MOUSEEVENTF_XUP; break;
        default: return CallNextHookEx(nullptr, code, message, data);
    }
    INPUT input{}; input.type = INPUT_MOUSE;
    input.mi.dx = MulDiv(event.pt.x - GetSystemMetrics(SM_XVIRTUALSCREEN), 65535, std::max(1, GetSystemMetrics(SM_CXVIRTUALSCREEN) - 1));
    input.mi.dy = MulDiv(event.pt.y - GetSystemMetrics(SM_YVIRTUALSCREEN), 65535, std::max(1, GetSystemMetrics(SM_CYVIRTUALSCREEN) - 1));
    input.mi.dwFlags = flags | MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK;
    if (flags & (MOUSEEVENTF_WHEEL | MOUSEEVENTF_HWHEEL)) input.mi.mouseData = (DWORD)(SHORT)HIWORD(event.mouseData);
    else if (flags & (MOUSEEVENTF_XDOWN | MOUSEEVENTF_XUP)) input.mi.mouseData = HIWORD(event.mouseData);
    input.mi.dwExtraInfo = OWN;
    if (enqueue(input)) return 1;
    if (!capture) return CallNextHookEx(nullptr, code, message, data);
    if (message == WM_LBUTTONDOWN || message == WM_RBUTTONDOWN || message == WM_MBUTTONDOWN || message == WM_XBUTTONDOWN || message == WM_MOUSEWHEEL || message == WM_MOUSEHWHEEL) emit(3);
    return CallNextHookEx(nullptr, code, message, data);
}

struct Apartment {
    HRESULT initialized;
    ComPtr<IUIAutomation> automation;
    ComPtr<IUIAutomationElement> lastField;
    uint64_t identity = 0;
    Apartment() : initialized(CoInitializeEx(nullptr, COINIT_MULTITHREADED)) {
        if (SUCCEEDED(initialized)) {
            CoCreateInstance(__uuidof(CUIAutomation8), nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&automation));
            ComPtr<IUIAutomation2> bounded;
            if (automation && SUCCEEDED(automation.As(&bounded))) {
                bounded->put_ConnectionTimeout(25);
                bounded->put_TransactionTimeout(25);
            }
        }
    }
    ~Apartment() { lastField.Reset(); automation.Reset(); if (SUCCEEDED(initialized)) CoUninitialize(); }
};
thread_local Apartment apartment;
struct Guard {
    ComPtr<IUIAutomationElement> element;
    ComPtr<IUIAutomationTextRange> selection;
    HWND window;
    HWND inputWindow;
    DWORD thread;
};

DWORD integrity(DWORD pid) {
    HANDLE process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
    if (!process) return 0xFFFFFFFF;
    HANDLE token = nullptr;
    if (!OpenProcessToken(process, TOKEN_QUERY, &token)) { CloseHandle(process); return 0xFFFFFFFF; }
    DWORD size = 0;
    GetTokenInformation(token, TokenIntegrityLevel, nullptr, 0, &size);
    std::vector<BYTE> storage(size);
    DWORD level = 0xFFFFFFFF;
    if (size && GetTokenInformation(token, TokenIntegrityLevel, storage.data(), size, &size)) {
        auto label = reinterpret_cast<TOKEN_MANDATORY_LABEL*>(storage.data());
        level = *GetSidSubAuthority(label->Label.Sid, *GetSidSubAuthorityCount(label->Label.Sid) - 1);
    }
    CloseHandle(token); CloseHandle(process); return level;
}

bool snapshot(CsSnapshot* output) {
    auto automation = apartment.automation;
    if (!automation) return false;
    const auto epoch = revision.load();
    HWND window = GetForegroundWindow();
    DWORD pid = 0;
    DWORD thread = GetWindowThreadProcessId(window, &pid);
    if (!window || pid == GetCurrentProcessId()) return false;
    GUITHREADINFO inputInfo{}; inputInfo.cbSize = sizeof(inputInfo);
    if (!GetGUIThreadInfo(thread, &inputInfo) || !inputInfo.hwndFocus) return false;
    HWND inputWindow = inputInfo.hwndFocus;
    DWORD inputPid = 0;
    thread = GetWindowThreadProcessId(inputWindow, &inputPid);
    DWORD targetIntegrity = integrity(pid);
    if (targetIntegrity == 0xFFFFFFFF || targetIntegrity > SECURITY_MANDATORY_MEDIUM_RID || integrity(inputPid) > SECURITY_MANDATORY_MEDIUM_RID) return false;
    ComPtr<IUIAutomationElement> element;
    if (FAILED(automation->GetFocusedElement(&element)) || !element) return false;
    int elementPid = 0;
    if (FAILED(element->get_CurrentProcessId(&elementPid)) || ((DWORD)elementPid != pid && (DWORD)elementPid != inputPid)) return false;
    VARIANT password; VariantInit(&password);
    HRESULT passwordResult = element->GetCurrentPropertyValueEx(UIA_IsPasswordPropertyId, TRUE, &password);
    bool safe = SUCCEEDED(passwordResult) && password.vt == VT_BOOL && password.boolVal == VARIANT_FALSE;
    VariantClear(&password);
    BOOL enabled = FALSE, focus = FALSE;
    if (FAILED(element->get_CurrentIsEnabled(&enabled)) || !enabled
        || FAILED(element->get_CurrentHasKeyboardFocus(&focus)) || !focus) safe = false;
    ComPtr<IUIAutomationTextPattern> pattern;
    if (!safe || FAILED(element->GetCurrentPatternAs(UIA_TextPatternId, IID_PPV_ARGS(&pattern))) || !pattern) return false;
    ComPtr<IUIAutomationTextRangeArray> selection;
    if (FAILED(pattern->GetSelection(&selection)) || !selection) return false;
    int count = 0;
    if (FAILED(selection->get_Length(&count)) || count != 1) return false;
    ComPtr<IUIAutomationTextRange> caret;
    if (FAILED(selection->GetElement(0, &caret))) return false;
    int comparison = 1;
    if (FAILED(caret->CompareEndpoints(TextPatternRangeEndpoint_Start, caret.Get(), TextPatternRangeEndpoint_End, &comparison)) || comparison != 0) return false;
    VARIANT readonly; VariantInit(&readonly);
    HRESULT editableResult = caret->GetAttributeValue(UIA_IsReadOnlyAttributeId, &readonly);
    bool editable = SUCCEEDED(editableResult) && readonly.vt == VT_BOOL && readonly.boolVal == VARIANT_FALSE;
    VariantClear(&readonly);
    if (!editable) return false;
    ComPtr<IUIAutomationTextRange> start;
    caret->Clone(&start);
    int moved = 0;
    // Count character units through range movement, without retrieving document text.
    if (!start || FAILED(start->MoveEndpointByUnit(TextPatternRangeEndpoint_Start, TextUnit_Character, -1000000000, &moved))) return false;
    int64_t caretOffset = -(int64_t)moved;
    ComPtr<IUIAutomationTextRange> tail;
    caret->Clone(&tail);
    if (!tail || FAILED(tail->MoveEndpointByUnit(TextPatternRangeEndpoint_Start, TextUnit_Character, -192, &moved))) return false;
    BSTR text = nullptr;
    if (FAILED(tail->GetText(192, &text))) return false;
    auto length = text ? SysStringLen(text) : 0;
    uint32_t layout = layoutKind(GetKeyboardLayout(thread));
    if (!layout || length > 192 || revision != epoch || GetForegroundWindow() != window) { SysFreeString(text); return false; }
    BOOL equal = FALSE;
    if (apartment.lastField) automation->CompareElements(element.Get(), apartment.lastField.Get(), &equal);
    if (!equal) { apartment.lastField = element; ++apartment.identity; }
    memset(output, 0, sizeof(*output));
    output->field = apartment.identity;
    output->caret = caretOffset;
    output->revision = epoch;
    output->layout = layout;
    output->length = length;
    if (text) memcpy(output->suffix, text, length * sizeof(WCHAR));
    SysFreeString(text);
    HANDLE process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
    WCHAR path[32768]; DWORD size = 32768;
    bool identified = process && QueryFullProcessImageNameW(process, 0, path, &size);
    if (process) CloseHandle(process);
    if (!identified) return false;
    auto basename = wcsrchr(path, L'\\');
    basename = basename ? basename + 1 : path;
    if (!WideCharToMultiByte(CP_UTF8, 0, basename, -1, output->app, sizeof(output->app), nullptr, nullptr)) return false;
    auto guard = new Guard{element, caret, window, inputWindow, thread};
    output->handle = guard;
    return true;
}

INPUT keyboard(WORD key, DWORD flags, ULONG_PTR tag = OWN) {
    INPUT input{}; input.type = INPUT_KEYBOARD;
    input.ki.wVk = (flags & KEYEVENTF_UNICODE) ? 0 : key;
    input.ki.wScan = (flags & KEYEVENTF_UNICODE) ? key : 0;
    input.ki.dwFlags = flags; input.ki.dwExtraInfo = tag;
    return input;
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
        hookThreadId = GetCurrentThreadId();
        MSG message; PeekMessageW(&message, nullptr, 0, 0, PM_NOREMOVE);
        HHOOK keyboard = SetWindowsHookExW(WH_KEYBOARD_LL, keyboardHook, GetModuleHandleW(nullptr), 0);
        HHOOK mouse = SetWindowsHookExW(WH_MOUSE_LL, mouseHook, GetModuleHandleW(nullptr), 0);
        active = keyboard && mouse;
        if (active) { while (running && GetMessageW(&message, nullptr, 0, 0) > 0) { TranslateMessage(&message); DispatchMessageW(&message); } }
        if (keyboard) UnhookWindowsHookEx(keyboard);
        if (mouse) UnhookWindowsHookEx(mouse);
        active = false;
    });
    return 1;
}
void cs_stop() {
    running = false;
    if (hookThreadId) PostThreadMessageW(hookThreadId, WM_QUIT, 0, 0);
    if (hookThread.joinable()) hookThread.join();
    cs_finish(0);
}
uint32_t cs_permissions(int) { return 1 | (active ? 2 : 0); }
uint32_t cs_layouts() {
    HKL layouts[256]; uint32_t result = 0;
    int count = GetKeyboardLayoutList(256, layouts);
    for (int i = 0; i < count; ++i) { auto kind = layoutKind(layouts[i]); result |= kind == 1 ? 1 : kind == 2 ? 2 : 0; }
    return result;
}
uint64_t cs_revision() { return revision.load(); }
int cs_snapshot(CsSnapshot* output) { return snapshot(output) ? 1 : 0; }
void cs_release(void* handle) { delete static_cast<Guard*>(handle); }
void cs_shortcut(uint32_t modifiers, uint32_t key) { undoMods = modifiers; undoKey = key; }
void cs_capture(int enabled) { capture = enabled != 0; }

int cs_replace(const CsSnapshot* before, const uint16_t* expected, uint32_t expectedLen,
               const uint16_t* replacement, uint32_t replacementLen, uint32_t layout) {
    if (!before || !before->handle || !expectedLen || !replacementLen || replacementLen > 192 || !active) return 0;
    {
        std::lock_guard<std::mutex> lock(gateMutex);
        if (gated || revision != before->revision) return 0;
        gated = true; gateStarted = std::chrono::steady_clock::now();
    }
    CsSnapshot current{};
    bool valid = cs_snapshot(&current) && current.field == before->field && current.caret == before->caret
        && current.revision == before->revision && current.layout == before->layout
        && current.length >= expectedLen && memcmp(current.suffix + current.length - expectedLen, expected, expectedLen * 2) == 0;
    cs_release(current.handle);
    if (!valid) { cancelBeforeEditing(before); return 0; }
    auto guard = static_cast<Guard*>(before->handle);
    HKL target = nullptr; HKL installed[256]; int count = GetKeyboardLayoutList(256, installed);
    for (int i = 0; i < count; ++i) if (layoutKind(installed[i]) == layout) target = installed[i];
    if (!target || !PostMessageW(guard->inputWindow, WM_INPUTLANGCHANGEREQUEST, 0, reinterpret_cast<LPARAM>(target))) { cancelBeforeEditing(before); return 0; }
    for (int i = 0; i < 20 && GetKeyboardLayout(guard->thread) != target; ++i) std::this_thread::sleep_for(std::chrono::milliseconds(2));
    if (GetKeyboardLayout(guard->thread) != target || GetForegroundWindow() != guard->window) { cancelBeforeEditing(before); return 0; }
    CsSnapshot finalCheck{};
    bool unchanged = cs_snapshot(&finalCheck) && finalCheck.field == before->field && finalCheck.caret == before->caret
        && finalCheck.length >= expectedLen && memcmp(finalCheck.suffix + finalCheck.length - expectedLen, expected, expectedLen * 2) == 0;
    cs_release(finalCheck.handle);
    if (!unchanged) { cancelBeforeEditing(before); return 0; }
    std::vector<INPUT> events;
    std::vector<WORD> heldModifiers;
    for (WORD key : {VK_LCONTROL, VK_RCONTROL, VK_LMENU, VK_RMENU, VK_LSHIFT, VK_RSHIFT, VK_LWIN, VK_RWIN}) {
        if (GetAsyncKeyState(key) & 0x8000) {
            heldModifiers.push_back(key);
            events.push_back(keyboard(key, KEYEVENTF_KEYUP));
        }
    }
    for (uint32_t i = 0; i < expectedLen; ++i) { events.push_back(keyboard(VK_BACK, 0)); events.push_back(keyboard(VK_BACK, KEYEVENTF_KEYUP)); }
    for (uint32_t i = 0; i < replacementLen; ++i) {
        events.push_back(keyboard(replacement[i], KEYEVENTF_UNICODE));
        events.push_back(keyboard(replacement[i], KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
    }
    for (auto key : heldModifiers) events.push_back(keyboard(key, 0));
    events.back().ki.dwExtraInfo = END;
    auto inserted = SendInput((UINT)events.size(), events.data(), sizeof(INPUT));
    if (inserted != events.size()) { cs_finish(0); return -1; }
    return 1;
}
void cs_finish(int replay) {
    std::lock_guard<std::mutex> lock(gateMutex);
    if (replay && !queued.empty()) SendInput((UINT)queued.size(), queued.data(), sizeof(INPUT));
    queued.clear(); gated = false;
}
}
