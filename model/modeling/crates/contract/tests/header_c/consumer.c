#include "pdma_model_abi.h"
#include "pdma_model_abi.h"
#include <stdio.h>

_Static_assert(C_COMMAND_SIZE == 64, "command wire extent");
_Static_assert(C_RESPONSE_SIZE == 24, "response wire extent");
_Static_assert(C_ENDPOINT_SIZE == 24, "endpoint wire extent");
_Static_assert(C_SEGMENT_SIZE == 16, "segment wire extent");
_Static_assert(C_PDMA_SIZE == 228, "device MMIO extent");
_Static_assert(PDMA_WINDOW_CONTROL_BASE == UINT32_C(0xffffff00), "firmware control address");
_Static_assert(PDMA_WINDOW_STATUS_BASE == UINT32_C(0xffffff08), "host readiness address");
_Static_assert(PDMA_FIRMWARE_STATUS_BASE == UINT32_C(0xffffff10), "firmware boot report address");
_Static_assert(C_FIRMWARE_SIZE == 4, "atomic firmware boot report extent");
_Static_assert(C_WINDOWCONTROL_SIZE == 4, "control map extent");
_Static_assert(C_WINDOWSTATUS_SIZE == 4, "status map extent");
_Static_assert(C_WINDOWCONTROL_C_QUOQUPQUEQUN_OFFSET == 0, "control map relative offset");
_Static_assert(C_WINDOWSTATUS_C_QURQUEQUAQUDQUY_OFFSET == 0, "status map relative offset");
_Static_assert(C_WINDOWCONTROL_C_QUOQUPQUEQUN_ACCESS == C_WINDOWCONTROL_ACCESS_READ_WRITE, "firmware can configure access");
_Static_assert(C_WINDOWCONTROL_C_QUOQUPQUEQUN_WRITABLE_MASK == 1, "only access-enable bit is writable");
_Static_assert(C_WINDOWCONTROL_C_QUOQUPQUEQUN_RESET_VALUE == 0, "window starts closed");
_Static_assert(C_WINDOWSTATUS_C_QURQUEQUAQUDQUY_ACCESS == C_WINDOWSTATUS_ACCESS_READ_ONLY, "readiness is read-only");
_Static_assert(C_WINDOWSTATUS_C_QURQUEQUAQUDQUY_WRITABLE_MASK == 0, "readiness has no writable fields");
_Static_assert(C_COMMAND_C_FLAGS_OFFSET == 56, "command IOC offset");
_Static_assert(C_RESPONSE_C_FLAGS_OFFSET == 12, "response IOC offset");
_Static_assert(C_COMMAND_ALIGNMENT == 8, "command alignment");
_Static_assert(C_RESPONSE_ALIGNMENT == 8, "response alignment");
_Static_assert(C_ENDPOINT_ALIGNMENT == 8, "endpoint alignment");
_Static_assert(C_SEGMENT_ALIGNMENT == 8, "segment alignment");
_Static_assert(C_ENDPOINT_C_MODE_C_QULINEAR == 0, "linear mode");
_Static_assert(C_ENDPOINT_C_MODE_C_QUSCATTERQUGATHER == 1, "scatter gather mode");
_Static_assert(C_PDMA_C_QUPQURQUOQUDQUUQUCQUEQUR__2_OFFSET == 52, "first channel response producer");
_Static_assert(C_PDMA_C_QUPQURQUOQUDQUUQUCQUEQUR__4_OFFSET == 124, "second channel response producer");
_Static_assert(C_PDMA_C_QUPQURQUOQUDQUUQUCQUEQUR__6_OFFSET == 196, "third channel response producer");
_Static_assert(C_PDMA_C_QUCQUHQUAQUNQUNQUEQULQSQUNQUUQUM_RESET_VALUE == 0, "channel count populated by hardware");
_Static_assert(C_PDMA_C_QUCQUHQUAQUNQUNQUEQULQSQUNQUUQUM_RESET_KIND == C_PDMA_RESET_POST_RESET, "instance discovery initialized after reset");
_Static_assert(C_PDMA_C_QUCQUHQUAQUNQUNQUEQULQSQUNQUUQUM_ACCESS == C_PDMA_ACCESS_READ_ONLY, "channel count is read-only");
_Static_assert(C_PDMA_C_QUFQUAQUUQULQUT__2_C_CODE_C_QUNONE == 0, "no channel fault code");
_Static_assert(C_PDMA_C_QUFQUAQUUQULQUT__2_C_CODE_C_QUCOMMANDQUREAD == 1, "command fault code");
_Static_assert(C_PDMA_C_QUFQUAQUUQULQUT__2_C_CODE_C_QURESPONSEQUWRITE == 2, "response fault code");
_Static_assert(C_PDMA_C_QUFQUAQUUQULQUT__1_C_CODE_C_QUINTERNAL == 1, "device internal fault code");
_Static_assert(C_PDMA_C_QUIQURQUQQSQUCQUAQUUQUSQUE__1_ACCESS == C_PDMA_ACCESS_READ_ONLY, "causes are read-only");
_Static_assert(C_PDMA_C_QUIQURQUQQSQUMQUAQUSQUK__1_WRITABLE_MASK == 3, "two notification mask bits");
_Static_assert(C_PDMA_C_QURQUEQUSQUEQUT__2_ACCESS == C_PDMA_ACCESS_READ_WRITE, "channel reset exposes busy state");
_Static_assert(C_PDMA_C_QURQUEQUSQUEQUT__1_ACCESS == C_PDMA_ACCESS_READ_WRITE, "device reset exposes busy state");
_Static_assert(C_RESPONSE_C_STATUS_C_QUSUCCESS == 0, "success status code");
_Static_assert(C_RESPONSE_C_STATUS_C_QUINVALIDQULENGTH == 1, "invalid length status code");
_Static_assert(C_RESPONSE_C_STATUS_C_QUREADQUFAULT == 2, "read fault status code");
_Static_assert(C_RESPONSE_C_STATUS_C_QUWRITEQUFAULT == 3, "write fault status code");
_Static_assert(C_RESPONSE_C_STATUS_C_QULENGTHQUMISMATCH == 4, "length mismatch status code");
_Static_assert(C_RESPONSE_C_STATUS_C_QULISTQUREADQUFAULT == 5, "list read fault status code");
_Static_assert(PDMA_CONTROL_CHANNEL_BASE == UINT32_C(0xfffffe00), "independent control channel address");
_Static_assert(C_CONTROL_SIZE == 48, "control channel extent");
_Static_assert(C_CONTROL_C_QUBQUAQUSQUE__1_OFFSET == 0, "command ring address");
_Static_assert(C_CONTROL_C_QUBQUAQUSQUE__2_OFFSET == 24, "response ring address");
_Static_assert(C_CONTROL_C_QUEQUNQUAQUBQULQUE_OFFSET == 44, "activation address");
_Static_assert(C_CONTROLREQQUEST_SIZE == 16, "control request extent");
_Static_assert(C_CONTROLRESPONSE_SIZE == 16, "control response extent");
_Static_assert(C_CONTROLREQQUEST_ALIGNMENT == 8, "control request alignment");
_Static_assert(C_CONTROLRESPONSE_ALIGNMENT == 8, "control response alignment");
_Static_assert(C_CONTROLREQQUEST_C_ID_OFFSET == 0, "control request id offset");
_Static_assert(C_CONTROLREQQUEST_C_OPCODE_OFFSET == 8, "control request opcode offset");
_Static_assert(C_CONTROLREQQUEST_C_CHANNEL_OFFSET == 12, "control request channel offset");
_Static_assert(C_CONTROLRESPONSE_C_ID_OFFSET == 0, "control response id offset");
_Static_assert(C_CONTROLRESPONSE_C_STATUS_OFFSET == 8, "control response status offset");
_Static_assert(C_CONTROLRESPONSE_C_CHANNEL_OFFSET == 12, "control response channel offset");
_Static_assert(C_CONTROLREQQUEST_C_OPCODE_C_QUACQQUIREQUCHANNEL == 0, "acquire opcode");
_Static_assert(C_CONTROLREQQUEST_C_OPCODE_C_QURELEASEQUCHANNEL == 1, "release opcode");
_Static_assert(C_CONTROLRESPONSE_C_STATUS_C_QUSUCCESS == 0, "control success status");
_Static_assert(C_CONTROLRESPONSE_C_STATUS_C_QUNOQURESOURCES == 1, "no resources status");
_Static_assert(C_CONTROLRESPONSE_C_STATUS_C_QUINVALIDQUCHANNEL == 2, "invalid channel status");
_Static_assert(C_CONTROLRESPONSE_C_STATUS_C_QUNOTQUALLOCATED == 3, "not allocated status");

int main(void) {
    uint8_t command[C_COMMAND_SIZE] = {0};
    uint8_t response[C_RESPONSE_SIZE] = {0};
    uint8_t segment[C_SEGMENT_SIZE] = {0};
    uint8_t release[C_CONTROLREQQUEST_SIZE] = {0};
    uint8_t acquire[C_CONTROLREQQUEST_SIZE] = {0};
    uint8_t granted[C_CONTROLRESPONSE_SIZE] = {0};
    uint8_t rejected[C_CONTROLRESPONSE_SIZE] = {0};
    const uint8_t control_offsets[] = {
        C_CONTROL_C_QUBQUAQUSQUE__1_OFFSET,
        C_CONTROL_C_QUCQUAQUPQUAQUCQUIQUTQUY__1_OFFSET,
        C_CONTROL_C_QUPQURQUOQUDQUUQUCQUEQUR__1_OFFSET,
        C_CONTROL_C_QUCQUOQUNQUSQUUQUMQUEQUR__1_OFFSET,
        C_CONTROL_C_QUBQUAQUSQUE__2_OFFSET,
        C_CONTROL_C_QUCQUAQUPQUAQUCQUIQUTQUY__2_OFFSET,
        C_CONTROL_C_QUPQURQUOQUDQUUQUCQUEQUR__2_OFFSET,
        C_CONTROL_C_QUCQUOQUNQUSQUUQUMQUEQUR__2_OFFSET,
        C_CONTROL_C_QUEQUNQUAQUBQULQUE_OFFSET,
    };
    const uint8_t offsets[] = {
        C_PDMA_C_QUCQUHQUAQUNQUNQUEQULQSQUNQUUQUM_OFFSET,
        C_PDMA_C_QUIQURQUQQSQUEQUNQUAQUBQULQUE__1_OFFSET,
        C_PDMA_C_QUFQUAQUUQULQUT__1_OFFSET,
        C_PDMA_C_QURQUEQUSQUEQUT__1_OFFSET,
        C_PDMA_C_QUBQUAQUSQUE__1_OFFSET,
        C_PDMA_C_QUCQUAQUPQUAQUCQUIQUTQUY__1_OFFSET,
        C_PDMA_C_QUPQURQUOQUDQUUQUCQUEQUR__1_OFFSET,
        C_PDMA_C_QUCQUOQUNQUSQUUQUMQUEQUR__1_OFFSET,
        C_PDMA_C_QUBQUAQUSQUE__2_OFFSET,
        C_PDMA_C_QUCQUAQUPQUAQUCQUIQUTQUY__2_OFFSET,
        C_PDMA_C_QUPQURQUOQUDQUUQUCQUEQUR__2_OFFSET,
        C_PDMA_C_QUCQUOQUNQUSQUUQUMQUEQUR__2_OFFSET,
        C_PDMA_C_QUEQUNQUAQUBQULQUE__1_OFFSET,
        C_PDMA_C_QUIQURQUQQSQUEQUNQUAQUBQULQUE__2_OFFSET,
        C_PDMA_C_QUFQUAQUUQULQUT__2_OFFSET,
        C_PDMA_C_QUIQURQUQQSQUCQUAQUUQUSQUE__1_OFFSET,
        C_PDMA_C_QUIQURQUQQSQUMQUAQUSQUK__1_OFFSET,
        C_PDMA_C_QURQUEQUSQUEQUT__2_OFFSET,
        C_PDMA_C_QUBQUAQUSQUE__3_OFFSET,
        C_PDMA_C_QUCQUAQUPQUAQUCQUIQUTQUY__3_OFFSET,
        C_PDMA_C_QUPQURQUOQUDQUUQUCQUEQUR__3_OFFSET,
        C_PDMA_C_QUCQUOQUNQUSQUUQUMQUEQUR__3_OFFSET,
        C_PDMA_C_QUBQUAQUSQUE__4_OFFSET,
        C_PDMA_C_QUCQUAQUPQUAQUCQUIQUTQUY__4_OFFSET,
        C_PDMA_C_QUPQURQUOQUDQUUQUCQUEQUR__4_OFFSET,
        C_PDMA_C_QUCQUOQUNQUSQUUQUMQUEQUR__4_OFFSET,
        C_PDMA_C_QUEQUNQUAQUBQULQUE__2_OFFSET,
        C_PDMA_C_QUIQURQUQQSQUEQUNQUAQUBQULQUE__3_OFFSET,
        C_PDMA_C_QUFQUAQUUQULQUT__3_OFFSET,
        C_PDMA_C_QUIQURQUQQSQUCQUAQUUQUSQUE__2_OFFSET,
        C_PDMA_C_QUIQURQUQQSQUMQUAQUSQUK__2_OFFSET,
        C_PDMA_C_QURQUEQUSQUEQUT__3_OFFSET,
        C_PDMA_C_QUBQUAQUSQUE__5_OFFSET,
        C_PDMA_C_QUCQUAQUPQUAQUCQUIQUTQUY__5_OFFSET,
        C_PDMA_C_QUPQURQUOQUDQUUQUCQUEQUR__5_OFFSET,
        C_PDMA_C_QUCQUOQUNQUSQUUQUMQUEQUR__5_OFFSET,
        C_PDMA_C_QUBQUAQUSQUE__6_OFFSET,
        C_PDMA_C_QUCQUAQUPQUAQUCQUIQUTQUY__6_OFFSET,
        C_PDMA_C_QUPQURQUOQUDQUUQUCQUEQUR__6_OFFSET,
        C_PDMA_C_QUCQUOQUNQUSQUUQUMQUEQUR__6_OFFSET,
        C_PDMA_C_QUEQUNQUAQUBQULQUE__3_OFFSET,
        C_PDMA_C_QUIQURQUQQSQUEQUNQUAQUBQULQUE__4_OFFSET,
        C_PDMA_C_QUFQUAQUUQULQUT__4_OFFSET,
        C_PDMA_C_QUIQURQUQQSQUCQUAQUUQUSQUE__3_OFFSET,
        C_PDMA_C_QUIQURQUQQSQUMQUAQUSQUK__3_OFFSET,
        C_PDMA_C_QURQUEQUSQUEQUT__4_OFFSET,
    };
    C_COMMAND_STORE_C_ID(command, UINT64_C(7));
    C_COMMAND_STORE_C_SOURCEQX2EMODE(command, C_COMMAND_C_SOURCEQX2EMODE_C_QULINEAR);
    C_COMMAND_STORE_C_SOURCEQX2EWORD0(command, UINT64_C(0x100));
    C_COMMAND_STORE_C_SOURCEQX2EWORD1(command, UINT64_C(8));
    C_COMMAND_STORE_C_DESTINATIONQX2EMODE(command, C_COMMAND_C_DESTINATIONQX2EMODE_C_QUSCATTERQUGATHER);
    C_COMMAND_STORE_C_DESTINATIONQX2EWORD0(command, UINT64_C(0x200));
    C_COMMAND_STORE_C_DESTINATIONQX2EWORD1(command, UINT64_C(2));
    C_COMMAND_STORE_C_FLAGS(command, UINT32_C(1));
    C_RESPONSE_STORE_C_ID(response, UINT64_C(7));
    C_RESPONSE_STORE_C_STATUS(response, C_RESPONSE_C_STATUS_C_QUSUCCESS);
    C_RESPONSE_STORE_C_FLAGS(response, UINT32_C(1));
    C_RESPONSE_STORE_C_BYTESQSCOMPLETED(response, UINT64_C(8));
    C_SEGMENT_STORE_C_ADDRESS(segment, UINT64_C(0x300));
    C_SEGMENT_STORE_C_LENGTH(segment, UINT64_C(8));
    C_CONTROLREQQUEST_STORE_C_ID(release, UINT64_C(0x0807060504030201));
    C_CONTROLREQQUEST_STORE_C_OPCODE(release, C_CONTROLREQQUEST_C_OPCODE_C_QURELEASEQUCHANNEL);
    C_CONTROLREQQUEST_STORE_C_CHANNEL(release, UINT32_C(0x10203040));
    C_CONTROLREQQUEST_STORE_C_ID(acquire, UINT64_C(9));
    C_CONTROLREQQUEST_STORE_C_OPCODE(acquire, C_CONTROLREQQUEST_C_OPCODE_C_QUACQQUIREQUCHANNEL);
    C_CONTROLREQQUEST_STORE_C_CHANNEL(acquire, UINT32_C(0));
    C_CONTROLRESPONSE_STORE_C_ID(granted, UINT64_C(9));
    C_CONTROLRESPONSE_STORE_C_STATUS(granted, C_CONTROLRESPONSE_C_STATUS_C_QUSUCCESS);
    C_CONTROLRESPONSE_STORE_C_CHANNEL(granted, UINT32_C(2));
    C_CONTROLRESPONSE_STORE_C_ID(rejected, UINT64_C(0x0807060504030201));
    C_CONTROLRESPONSE_STORE_C_STATUS(rejected, C_CONTROLRESPONSE_C_STATUS_C_QUNOTQUALLOCATED);
    C_CONTROLRESPONSE_STORE_C_CHANNEL(rejected, UINT32_C(0x10203040));
    if (C_COMMAND_LOAD_C_ID(command) != UINT64_C(7)) return 1;
    if (C_COMMAND_LOAD_C_SOURCEQX2EMODE(command) != UINT32_C(0)) return 2;
    if (C_COMMAND_LOAD_C_SOURCEQX2EWORD0(command) != UINT64_C(0x100)) return 3;
    if (C_COMMAND_LOAD_C_SOURCEQX2EWORD1(command) != UINT64_C(8)) return 4;
    if (C_COMMAND_LOAD_C_DESTINATIONQX2EMODE(command) != UINT32_C(1)) return 10;
    if (C_COMMAND_LOAD_C_DESTINATIONQX2EWORD0(command) != UINT64_C(0x200)) return 11;
    if (C_COMMAND_LOAD_C_DESTINATIONQX2EWORD1(command) != UINT64_C(2)) return 12;
    if (C_COMMAND_LOAD_C_FLAGS(command) != UINT32_C(1)) return 17;
    if (C_RESPONSE_LOAD_C_ID(response) != UINT64_C(7)) return 5;
    if (C_RESPONSE_LOAD_C_STATUS(response) != UINT32_C(0)) return 6;
    if (C_RESPONSE_LOAD_C_FLAGS(response) != UINT32_C(1)) return 18;
    if (C_RESPONSE_LOAD_C_BYTESQSCOMPLETED(response) != UINT64_C(8)) return 13;
    if (C_SEGMENT_LOAD_C_ADDRESS(segment) != UINT64_C(0x300)) return 14;
    if (C_SEGMENT_LOAD_C_LENGTH(segment) != UINT64_C(8)) return 15;
    if (fwrite(command, 1, sizeof(command), stdout) != sizeof(command)) return 7;
    if (fwrite(response, 1, sizeof(response), stdout) != sizeof(response)) return 8;
    if (fwrite(segment, 1, sizeof(segment), stdout) != sizeof(segment)) return 16;
    if (fwrite(offsets, 1, sizeof(offsets), stdout) != sizeof(offsets)) return 9;
    if (fwrite(release, 1, sizeof(release), stdout) != sizeof(release)) return 19;
    if (fwrite(acquire, 1, sizeof(acquire), stdout) != sizeof(acquire)) return 20;
    if (fwrite(granted, 1, sizeof(granted), stdout) != sizeof(granted)) return 21;
    if (fwrite(rejected, 1, sizeof(rejected), stdout) != sizeof(rejected)) return 22;
    if (fwrite(control_offsets, 1, sizeof(control_offsets), stdout) != sizeof(control_offsets)) return 23;
    return 0;
}
