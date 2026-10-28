#include "bridge.h"
extern "C" {
int cs_start(CsCallback, void*) { return 0; }
void cs_stop() {}
uint32_t cs_permissions(int) { return 0; }
uint32_t cs_layouts() { return 0; }
uint64_t cs_revision() { return 0; }
int cs_snapshot(CsSnapshot*) { return 0; }
void cs_release(void*) {}
int cs_replace(const CsSnapshot*, const uint16_t*, uint32_t, const uint16_t*, uint32_t, uint32_t) { return 0; }
void cs_finish(int) {}
void cs_shortcut(uint32_t, uint32_t) {}
void cs_capture(int) {}
}
