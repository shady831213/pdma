#include "pdma_firmware.h"
#include "pdma_firmware.h"
#ifdef C_FCTL_C_QURQUEQUQQUUQUEQUSQUT_OFFSET
#error Firmware must not expose the Host request doorbell
#endif
#ifdef PDMA_FIRMWARE_WINDOW_STATUS_BASE
#error Firmware management bus cannot reach Host readiness
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
_Static_assert(C_FWIN_C_QUOQUPQUEQUN_ACCESS == C_FWIN_ACCESS_READ_WRITE, "firmware window ownership");
_Static_assert(PDMA_FIRMWARE_WINDOW_CONTROL_BASE == UINT32_C(0xffffff00), "window physical base");
_Static_assert(PDMA_FIRMWARE_CONTROL_BASE == UINT32_C(0xfffffe00), "control physical base");
_Static_assert(PDMA_FIRMWARE_BOOT_BASE == UINT32_C(0xffffff10), "boot physical base");
int main(void) {
    uint8_t bytes[C_CONTROLRESPONSE_SIZE] = {0};
    C_CONTROLRESPONSE_STORE_C_ID(bytes, UINT64_C(0x12345678));
    return C_CONTROLRESPONSE_LOAD_C_ID(bytes) == UINT64_C(0x12345678) ? 0 : 1;
}
