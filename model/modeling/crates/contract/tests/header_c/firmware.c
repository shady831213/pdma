#include "pdma_firmware.h"
#include "pdma_firmware.h"
#ifdef C_FCTL_C_QURQUEQUQQUUQUEQUSQUT_OFFSET
#error Firmware must not expose the Host request doorbell
#endif
#if defined(PDMA_FIRMWARE_WINDOW_CONTROL_BASE) || defined(PDMA_FIRMWARE_WINDOW_STATUS_BASE)
#error Firmware window view must use one grouped bank base
#endif
#if defined(C_FWIN_C_QURQUEQUAQUDQUY__1_OFFSET) || defined(C_FWIN_C_QURQUEQUAQUDQUY__2_OFFSET) || defined(C_FWIN_C_QURQUEQUAQUDQUY__3_OFFSET)
#error Firmware management bus cannot expose Host readiness leaves
#endif
_Static_assert(C_FBOOT_C_QUBQUOQUOQUT_ACCESS == C_FBOOT_ACCESS_READ_WRITE, "firmware boot ownership");
_Static_assert(C_FBOOT_C_QUBQUOQUOQUT_WRITABLE_MASK == UINT64_C(0x10003), "boot mask unchanged");
_Static_assert(C_FCTL_C_QUBQUOQUOQUTQUSQUTQURQUAQUP_ACCESS == C_FCTL_ACCESS_READ_ONLY, "firmware bootstrap read-only");
_Static_assert(C_FCTL_C_QUBQUOQUOQUTQUSQUTQURQUAQUP_WRITABLE_MASK == 0, "firmware bootstrap is not writable");
_Static_assert(C_FCTL_C_QUEQUNQUAQUBQULQUE_ACCESS == C_FCTL_ACCESS_READ_ONLY, "firmware enable read-only");
_Static_assert(C_FCTL_C_QUEQUNQUAQUBQULQUE_WRITABLE_MASK == 0, "firmware enable is not writable");
_Static_assert(C_FCTL_C_QURQUEQUSQUPQUOQUNQUSQUE_ACCESS == C_FCTL_ACCESS_WRITE_ONLY, "firmware response pulse");
_Static_assert(C_FCTL_C_QURQUEQUSQUPQUOQUNQUSQUE_OFFSET == 16, "hidden request retains response offset");
_Static_assert(C_FCTL_SIZE == 20, "firmware control physical extent");
_Static_assert(C_FWIN_C_QUOQUPQUEQUN__1_ACCESS == C_FWIN_ACCESS_READ_WRITE, "firmware window ownership");
_Static_assert(PDMA_FIRMWARE_WINDOW_BASE == UINT32_C(0xff000000), "window bank physical base");
_Static_assert(C_FWIN_SIZE == 48, "firmware view retains the whole window bank");
_Static_assert(C_FWIN_C_QUOQUPQUEQUN__1_OFFSET == 0, "control bank relative offset");
_Static_assert(PDMA_FIRMWARE_WINDOW_BASE + C_FWIN_C_QUOQUPQUEQUN__1_OFFSET == UINT32_C(0xff000000), "control physical address");
_Static_assert(PDMA_FIRMWARE_CONTROL_BASE == UINT32_C(0xfffffe00), "control physical base");
_Static_assert(PDMA_FIRMWARE_BOOT_BASE == UINT32_C(0xffffff10), "boot physical base");
int main(void) {
    uint8_t bytes[C_CONTROLRESPONSE_SIZE] = {0};
    C_CONTROLRESPONSE_STORE_C_ID(bytes, UINT64_C(0x12345678));
    return C_CONTROLRESPONSE_LOAD_C_ID(bytes) == UINT64_C(0x12345678) ? 0 : 1;
}
