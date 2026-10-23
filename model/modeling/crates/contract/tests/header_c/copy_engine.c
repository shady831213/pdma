#include "pdma_host.h"
#include <stdio.h>
#include <string.h>

_Static_assert(C_COMMAND_SIZE == 112, "fixed command stride");
_Static_assert(C_COMMAND_RECORD_C_PAYLOAD_OFFSET == 16, "command payload");
_Static_assert(C_WORDTARGET_SIZE == 16, "word target");
_Static_assert(C_WORDWRITE_SIZE == 24, "write and fence payload");
_Static_assert(C_FILLCOMMAND_SIZE == 32, "fill payload");
_Static_assert(C_WAITCOMMAND_SIZE == 32, "wait payload");
_Static_assert(C_ATOMICCOMMAND_SIZE == 40, "atomic has no result destination");
_Static_assert(C_SEMAPHORECOMMAND_SIZE == 24, "semaphore payload");
_Static_assert(C_TIMESTAMPCOMMAND_SIZE == 16, "timestamp payload");
_Static_assert(C_EVENT_C_CODE_C_QUUNSUPPORTED == 6, "unsupported event");
_Static_assert(C_EVENT_C_CODE_C_QUINVALIDQUOPERATION == 7, "invalid operation event");
_Static_assert(C_EVENT_C_CODE_C_QUARITHMETICQUOVERFLOW == 8, "overflow event");

int main(void) {
    for (uint32_t kind = 1; kind <= 8; ++kind) {
        uint8_t command[C_COMMAND_SIZE] = {0};
        uint8_t *payload = command + C_COMMAND_RECORD_C_PAYLOAD_OFFSET;
        C_COMMAND_STORE_C_ID(command, UINT64_C(42));
        C_COMMAND_STORE_C_KIND(command, kind);
        if (kind != C_COMMAND_C_KIND_C_QUFILL && kind != C_COMMAND_C_KIND_C_QUTRAP && kind != C_COMMAND_C_KIND_C_QUTIMESTAMP) {
            C_WORDTARGET_STORE_C_ADDRESS(payload, UINT64_C(0x1238));
            C_WORDTARGET_STORE_C_CONTEXT(payload, UINT32_C(9));
            C_WORDTARGET_STORE_C_WIDTH(payload, C_WORDTARGET_C_WIDTH_C_QUBITS64);
        }
        switch (kind) {
        case C_COMMAND_C_KIND_C_QUWRITE:
        case C_COMMAND_C_KIND_C_QUFENCE:
            C_WORDWRITE_STORE_C_VALUE(payload, UINT64_C(0x5678));
            break;
        case C_COMMAND_C_KIND_C_QUFILL:
            C_FILLCOMMAND_STORE_C_ADDRESS(payload, UINT64_C(0x1238));
            C_FILLCOMMAND_STORE_C_LENGTH(payload, UINT64_C(19));
            C_FILLCOMMAND_STORE_C_PATTERN(payload, UINT64_C(0x5678));
            C_FILLCOMMAND_STORE_C_CONTEXT(payload, UINT32_C(9));
            C_FILLCOMMAND_STORE_C_WIDTH(payload, C_FILLCOMMAND_C_WIDTH_C_QUBITS32);
            break;
        case C_COMMAND_C_KIND_C_QUWAIT:
            C_WAITCOMMAND_STORE_C_VALUE(payload, UINT64_C(0x5678));
            C_WAITCOMMAND_STORE_C_COMPARISON(payload, C_WAITCOMMAND_C_COMPARISON_C_QUREACHED);
            break;
        case C_COMMAND_C_KIND_C_QUATOMIC:
            C_ATOMICCOMMAND_STORE_C_ORDER(payload, C_ATOMICCOMMAND_C_ORDER_C_QUACQQUIREQURELEASE);
            C_ATOMICCOMMAND_STORE_C_OPERATION(payload, C_ATOMICCOMMAND_C_OPERATION_C_QUCOMPAREQUEXCHANGE);
            C_ATOMICCOMMAND_STORE_C_WORD0(payload, UINT64_C(0x5678));
            C_ATOMICCOMMAND_STORE_C_WORD1(payload, UINT64_C(0x9abc));
            break;
        case C_COMMAND_C_KIND_C_QUSEMAPHORE:
            C_SEMAPHORECOMMAND_STORE_C_OPERATION(payload, C_SEMAPHORECOMMAND_C_OPERATION_C_QUWAIT);
            break;
        case C_COMMAND_C_KIND_C_QUTRAP:
            break;
        case C_COMMAND_C_KIND_C_QUTIMESTAMP:
            C_TIMESTAMPCOMMAND_STORE_C_ADDRESS(payload, UINT64_C(0x1238));
            C_TIMESTAMPCOMMAND_STORE_C_CONTEXT(payload, UINT32_C(9));
            break;
        default:
            return 1;
        }
        if (fwrite(command, 1, sizeof(command), stdout) != sizeof(command)) return 2;
    }
    return 0;
}
