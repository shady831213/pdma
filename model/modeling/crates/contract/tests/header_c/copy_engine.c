#include "pdma_host.h"
#include <stdio.h>
#include <string.h>

_Static_assert(C_COMMAND_SIZE == 96, "fixed command stride");
_Static_assert(C_TRAPCOMMAND_C_ID_OFFSET == 8, "common identifier");
_Static_assert(C_WORDTARGET_SIZE == 16, "word target");
_Static_assert(C_WORDWRITE_SIZE == 24, "write and fence payload");
_Static_assert(C_FILLCOMMAND_SIZE == 48, "fill payload");
_Static_assert(C_WAITCOMMAND_SIZE == 48, "wait payload");
_Static_assert(C_ATOMICCOMMAND_SIZE == 56, "atomic has no result destination");
_Static_assert(C_SEMAPHORECOMMAND_SIZE == 40, "semaphore payload");
_Static_assert(C_TIMESTAMPCOMMAND_SIZE == 32, "timestamp payload");
_Static_assert(C_EVENT_C_CODE_C_QUUNSUPPORTED == 6, "unsupported event");
_Static_assert(C_EVENT_C_CODE_C_QUINVALIDQUOPERATION == 7, "invalid operation event");
_Static_assert(C_EVENT_C_CODE_C_QUARITHMETICQUOVERFLOW == 8, "overflow event");

int main(void) {
    for (uint32_t kind = 1; kind <= 8; ++kind) {
        uint8_t command[C_COMMAND_SIZE] = {0};
        uint8_t *payload = command + 16;
        C_TRAPCOMMAND_STORE_C_ID(command, UINT64_C(42));
        C_COMMANDHEADER_STORE_C_OPCODE(command, kind);
        if (kind != C_COMMANDHEADER_C_OPCODE_C_QUFILL && kind != C_COMMANDHEADER_C_OPCODE_C_QUTRAP && kind != C_COMMANDHEADER_C_OPCODE_C_QUTIMESTAMP) {
            C_WORDTARGET_STORE_C_ADDRESS(payload, UINT64_C(0x1238));
            C_WORDTARGET_STORE_C_CONTEXT(payload, UINT32_C(9));
            C_WORDTARGET_STORE_C_WIDTH(payload, C_WORDTARGET_C_WIDTH_C_QUBITS64);
        }
        switch (kind) {
        case C_COMMANDHEADER_C_OPCODE_C_QUWRITE:
        case C_COMMANDHEADER_C_OPCODE_C_QUFENCE:
            C_WORDWRITE_STORE_C_VALUE(payload, UINT64_C(0x5678));
            break;
        case C_COMMANDHEADER_C_OPCODE_C_QUFILL:
            C_FILLCOMMAND_STORE_C_ADDRESS(command, UINT64_C(0x1238));
            C_FILLCOMMAND_STORE_C_LENGTH(command, UINT64_C(19));
            C_FILLCOMMAND_STORE_C_PATTERN(command, UINT64_C(0x5678));
            C_FILLCOMMAND_STORE_C_CONTEXT(command, UINT32_C(9));
            C_FILLCOMMAND_STORE_C_WIDTH(command, C_FILLCOMMAND_C_WIDTH_C_QUBITS32);
            break;
        case C_COMMANDHEADER_C_OPCODE_C_QUWAIT:
            C_WAITCOMMAND_STORE_C_VALUE(command, UINT64_C(0x5678));
            C_WAITCOMMAND_STORE_C_COMPARISON(command, C_WAITCOMMAND_C_COMPARISON_C_QUREACHED);
            break;
        case C_COMMANDHEADER_C_OPCODE_C_QUATOMIC:
            C_ATOMICCOMMAND_STORE_C_ORDER(command, C_ATOMICCOMMAND_C_ORDER_C_QUACQQUIREQURELEASE);
            C_ATOMICCOMMAND_STORE_C_OPERATION(command, C_ATOMICCOMMAND_C_OPERATION_C_QUCOMPAREQUEXCHANGE);
            C_ATOMICCOMMAND_STORE_C_WORD0(command, UINT64_C(0x5678));
            C_ATOMICCOMMAND_STORE_C_WORD1(command, UINT64_C(0x9abc));
            break;
        case C_COMMANDHEADER_C_OPCODE_C_QUSEMAPHORE:
            C_SEMAPHORECOMMAND_STORE_C_OPERATION(command, C_SEMAPHORECOMMAND_C_OPERATION_C_QUWAIT);
            break;
        case C_COMMANDHEADER_C_OPCODE_C_QUTRAP:
            break;
        case C_COMMANDHEADER_C_OPCODE_C_QUTIMESTAMP:
            C_TIMESTAMPCOMMAND_STORE_C_ADDRESS(command, UINT64_C(0x1238));
            C_TIMESTAMPCOMMAND_STORE_C_CONTEXT(command, UINT32_C(9));
            break;
        default:
            return 1;
        }
        if (fwrite(command, 1, sizeof(command), stdout) != sizeof(command)) return 2;
    }
    for (uint8_t mode = 0; mode < 4; ++mode) {
        uint8_t command[C_COMMAND_SIZE] = {0};
        C_COMMANDHEADER_STORE_C_OPCODE(command, C_COMMANDHEADER_C_OPCODE_C_QUCOPY);
        C_COPYCOMMAND_STORE_C_OPTIONS(command, mode);
        C_COPYCOMMAND_STORE_C_ID(command, UINT64_MAX);
        C_COPYCOMMAND_STORE_C_COPYQSLENGTH(command, UINT64_MAX);
        C_COPYCOMMAND_STORE_C_SGQSCONTEXT(command, UINT32_MAX);
        C_COPYCOMMAND_STORE_C_SOURCEQSCONTEXT(command, UINT32_MAX);
        C_COPYCOMMAND_STORE_C_DESTINATIONQSCONTEXT(command, UINT32_MAX);
        for (uint8_t side = 0; side < 2; ++side) {
            uint8_t *endpoint = command + (side == 0 ? C_COPYCOMMAND_RECORD_C_SOURCE_OFFSET : C_COPYCOMMAND_RECORD_C_DESTINATION_OFFSET);
            if ((mode & (1 << side)) != 0) {
                C_SCATTERGATHERENDPOINT_STORE_C_LISTQSADDRESS(endpoint, UINT64_MAX);
                C_SCATTERGATHERENDPOINT_STORE_C_COUNT(endpoint, UINT64_MAX);
                C_SCATTERGATHERENDPOINT_STORE_C_INDEX(endpoint, UINT64_MAX);
                C_SCATTERGATHERENDPOINT_STORE_C_OFFSET(endpoint, UINT64_MAX);
            } else {
                C_LINEARENDPOINT_STORE_C_ADDRESS(endpoint, UINT64_MAX);
                C_LINEARENDPOINT_STORE_C_OFFSET(endpoint, UINT64_MAX);
            }
        }
        if (fwrite(command, 1, sizeof(command), stdout) != sizeof(command)) return 3;
    }
    return 0;
}
