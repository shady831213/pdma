#include "pdma_host.h"
#include "pdma_host.h"
#ifdef C_HCTL_C_QURQUEQUSQUPQUOQUNQUSQUE_OFFSET
#error Host must not expose the firmware response doorbell
#endif
#if defined(PDMA_HOST_WINDOW_CONTROL_BASE) || defined(PDMA_HOST_WINDOW_STATUS_BASE)
#error Host window view must use one grouped bank base
#endif
#if defined(C_HWIN_C_QUOQUPQUEQUN__1_OFFSET) || defined(C_HWIN_C_QUOQUPQUEQUN__2_OFFSET) || defined(C_HWIN_C_QUOQUPQUEQUN__3_OFFSET)
#error Host must not expose firmware window control leaves
#endif
_Static_assert(C_HBOOT_C_QUBQUOQUOQUT_ACCESS == C_HBOOT_ACCESS_READ_ONLY, "Host boot is read-only");
_Static_assert(C_HBOOT_C_QUBQUOQUOQUT_WRITABLE_MASK == 0, "Host boot is not writable");
_Static_assert(C_HCTL_C_QUBQUOQUOQUTQUSQUTQURQUAQUP_ACCESS == C_HCTL_ACCESS_READ_WRITE, "Host bootstrap ownership");
_Static_assert(C_HCTL_C_QUEQUNQUAQUBQULQUE_ACCESS == C_HCTL_ACCESS_READ_WRITE, "Host enable ownership");
_Static_assert(C_HCTL_C_QURQUEQUQQUUQUEQUSQUT_ACCESS == C_HCTL_ACCESS_WRITE_ONLY, "Host request pulse");
_Static_assert(C_HCTL_C_QURQUEQUQQUUQUEQUSQUT_OFFSET == 12, "request physical offset");
_Static_assert(C_HCTL_SIZE == 20, "hidden response retains extent");
_Static_assert(C_HWIN_C_QURQUEQUAQUDQUY__1_ACCESS == C_HWIN_ACCESS_READ_ONLY, "Host readiness");
_Static_assert(PDMA_HOST_WINDOW_BASE == UINT32_C(0xff000000), "window bank physical base");
_Static_assert(C_HWIN_SIZE == 48, "Host view retains the whole window bank");
_Static_assert(C_HWIN_C_QURQUEQUAQUDQUY__1_OFFSET == 8, "hidden control retains readiness offset");
_Static_assert(PDMA_HOST_WINDOW_BASE + C_HWIN_C_QURQUEQUAQUDQUY__1_OFFSET == UINT32_C(0xff000008), "status physical address");
_Static_assert(PDMA_HOST_CONTROL_BASE == UINT32_C(0xfffffe00), "control physical base");
_Static_assert(PDMA_HOST_BOOT_BASE == UINT32_C(0xffffff10), "boot physical base");
int main(void) {
    uint8_t bytes[C_CONTROLREQQUEST_SIZE] = {0};
    C_CONTROLREQQUEST_STORE_C_ID(bytes, UINT64_C(0x12345678));
    return C_CONTROLREQQUEST_LOAD_C_ID(bytes) == UINT64_C(0x12345678) ? 0 : 1;
}
