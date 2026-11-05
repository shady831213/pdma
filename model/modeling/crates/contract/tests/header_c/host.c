#include "pdma_host.h"
#include "pdma_host.h"
#ifdef C_HCTL_C_NOTIFICATION_C_RESPONSE_OFFSET
#error Host must not expose the firmware response doorbell
#endif
#if defined(PDMA_HOST_WINDOW_CONTROL_BASE) || defined(PDMA_HOST_WINDOW_STATUS_BASE)
#error Host window view must use one grouped bank base
#endif
#if defined(C_HWIN_C_WINDOWS_0_C_CONTROL_C_OPEN_OFFSET) || defined(C_HWIN_C_WINDOWS_1_C_CONTROL_C_OPEN_OFFSET) || defined(C_HWIN_C_WINDOWS_2_C_CONTROL_C_OPEN_OFFSET)
#error Host must not expose firmware window control leaves
#endif
_Static_assert(C_HBOOT_C_BOOT_ACCESS == C_HBOOT_ACCESS_READ_ONLY, "Host boot is read-only");
_Static_assert(C_HBOOT_C_BOOT_WRITABLE_MASK == 0, "Host boot is not writable");
_Static_assert(C_HCTL_C_CONFIG_C_BOOTSTRAP_ACCESS == C_HCTL_ACCESS_READ_WRITE, "Host bootstrap ownership");
_Static_assert(C_HCTL_C_CONFIG_C_ENABLE_ACCESS == C_HCTL_ACCESS_READ_WRITE, "Host enable ownership");
_Static_assert(C_HCTL_C_REQQUEST_ACCESS == C_HCTL_ACCESS_WRITE_ONLY, "Host request pulse");
_Static_assert(C_HCTL_C_REQQUEST_OFFSET == 20, "request physical offset");
_Static_assert(C_HCTL_SIZE == 36, "hidden response retains extent");
_Static_assert(C_HCTL_C_NOTIFICATION_C_EVENTQSERROR_ACCESS == C_HCTL_ACCESS_READ_ONLY, "PF reads event error");
_Static_assert(C_HCTL_C_NOTIFICATION_C_EVENTQSERROR_WRITABLE_MASK == 0, "PF cannot clear event error");
#ifdef C_HCTL_C_NOTIFICATION_C_EVENT_OFFSET
#error Host must not expose the firmware event doorbell
#endif
_Static_assert(C_HWIN_C_WINDOWS_0_C_STATUS_C_READY_ACCESS == C_HWIN_ACCESS_READ_ONLY, "Host readiness");
_Static_assert(C_HWIN_SIZE == 36, "Host view retains the whole window bank");
_Static_assert(C_HWIN_C_WINDOWS_0_C_STATUS_C_READY_OFFSET == 8, "hidden control retains readiness offset");
#if defined(PDMA_HOST_CONTROL_STRIDE) || defined(PDMA_FIRMWARE_CONTROL_STRIDE)
#error Management is a single PF control block, not a Function bank
#endif
#if defined(PDMA_HOST_WINDOW_BASE) || defined(PDMA_HOST_CONTROL_BASE) || defined(PDMA_HOST_BOOT_BASE)
#error Platform base addresses must not be fixed by the contract
#endif
#ifdef C_HDMA_C_ADMIN_C_CONTROL_C_RESET_OFFSET
#error Managed Host must not expose DMA reset
#endif
#ifdef C_HDMA_C_ADMIN_C_CHANNELS_0_C_RESET_OFFSET
#error Managed Host must not expose channel reset
#endif
int main(void) {
    uint8_t bytes[C_CONTROLREQQUEST_SIZE] = {0};
    C_CONTROLREQQUEST_STORE_C_ID(bytes, UINT64_C(0x12345678));
    return C_CONTROLREQQUEST_LOAD_C_ID(bytes) == UINT64_C(0x12345678) ? 0 : 1;
}
