#include "pdma_host.h"
#include "pdma_firmware.h"
#include <stdio.h>

_Static_assert(C_COMMAND_SIZE == 104, "command wire extent");
_Static_assert(C_RESPONSE_SIZE == 24, "response wire extent");
_Static_assert(C_HDMA_C_QUFQUAQUUQULQUTQSQUSQUTQUAQUTQUUQUS__1_ACCESS == C_HDMA_ACCESS_READ_ONLY, "fault status is observable");
_Static_assert(C_HDMA_C_QURQUEQUSQUUQUMQUE__1_ACCESS == C_HDMA_ACCESS_WRITE_ONLY, "owner resume is a request");
_Static_assert(C_HDMA_C_QURQUEQUSQUUQUMQUE__1_WIDTH == 4, "resume is a word action");
_Static_assert(C_ENDPOINT_SIZE == 40, "endpoint wire extent");
_Static_assert(C_SEGMENT_SIZE == 16, "segment wire extent");
_Static_assert(C_FDMA_SIZE == 16384, "device MMIO extent");
_Static_assert(C_FBOOT_SIZE == 4, "atomic firmware boot report extent");
_Static_assert(C_FWIN_SIZE == 36, "firmware view retains full bank extent");
_Static_assert(C_HWIN_SIZE == 36, "Host view retains full bank extent");
_Static_assert(PDMA_HOST_WINDOW_COUNT == 3, "Host channel permission count");
_Static_assert(PDMA_FIRMWARE_WINDOW_COUNT == 3, "firmware channel permission count");
_Static_assert(PDMA_HOST_WINDOW_STRIDE == 12, "Host permission stride");
_Static_assert(PDMA_FIRMWARE_WINDOW_STRIDE == 12, "firmware permission stride");
_Static_assert(C_FWIN_C_QUOQUPQUEQUN__2_OFFSET == 12, "second firmware gate");
_Static_assert(C_FWIN_C_QUOQUPQUEQUN__3_OFFSET == 24, "third firmware gate");
_Static_assert(C_HWIN_C_QURQUEQUAQUDQUY__2_OFFSET == 20, "second Host status");
_Static_assert(C_HWIN_C_QURQUEQUAQUDQUY__3_OFFSET == 32, "third Host status");
_Static_assert(C_HWIN_C_QURQUEQUAQUDQUY__3_ACCESS == C_HWIN_ACCESS_READ_ONLY, "last Host status permission");
_Static_assert(C_FWIN_C_QUOQUPQUEQUN__3_ACCESS == C_FWIN_ACCESS_READ_WRITE, "last firmware control permission");
_Static_assert(C_FWIN_C_QUOQUPQUEQUN__1_OFFSET == 0, "control map relative offset");
_Static_assert(C_HWIN_C_QURQUEQUAQUDQUY__1_OFFSET == 8, "status bank relative offset");
_Static_assert(C_FWIN_C_QUOQUPQUEQUN__1_ACCESS == C_FWIN_ACCESS_READ_WRITE, "firmware can configure access");
_Static_assert(C_FWIN_C_QUOQUPQUEQUN__1_WRITABLE_MASK == 1, "only access-enable bit is writable");
_Static_assert(C_FWIN_C_QUOQUPQUEQUN__1_RESET_VALUE == 0, "window starts closed");
_Static_assert(C_HWIN_C_QURQUEQUAQUDQUY__1_ACCESS == C_HWIN_ACCESS_READ_ONLY, "readiness is read-only");
_Static_assert(C_HWIN_C_QURQUEQUAQUDQUY__1_WRITABLE_MASK == 0, "readiness has no writable fields");
_Static_assert(C_COMMAND_C_FLAGS_OFFSET == 96, "command IOC offset");
_Static_assert(C_RESPONSE_C_FLAGS_OFFSET == 12, "response IOC offset");
_Static_assert(C_COMMAND_ALIGNMENT == 8, "command alignment");
_Static_assert(C_RESPONSE_ALIGNMENT == 8, "response alignment");
_Static_assert(C_ENDPOINT_ALIGNMENT == 8, "endpoint alignment");
_Static_assert(C_SEGMENT_ALIGNMENT == 8, "segment alignment");
_Static_assert(C_ENDPOINT_C_MODE_C_QULINEAR == 0, "linear mode");
_Static_assert(C_ENDPOINT_C_MODE_C_QUSCATTERQUGATHER == 1, "scatter gather mode");
_Static_assert(C_FDMA_C_QUPQURQUOQUDQUUQUCQUEQUR__2_OFFSET == 4132, "first channel response producer");
_Static_assert(C_FDMA_C_QUPQURQUOQUDQUUQUCQUEQUR__4_OFFSET == 8228, "second channel response producer");
_Static_assert(C_FDMA_C_QUPQURQUOQUDQUUQUCQUEQUR__6_OFFSET == 12324, "third channel response producer");
_Static_assert(C_FDMA_C_QUCQUHQUAQUNQUNQUEQULQSQUNQUUQUM_RESET_VALUE == 0, "channel count populated by hardware");
_Static_assert(C_FDMA_C_QUCQUHQUAQUNQUNQUEQULQSQUNQUUQUM_RESET_KIND == C_FDMA_RESET_POST_RESET, "instance discovery initialized after reset");
_Static_assert(C_FDMA_C_QUCQUHQUAQUNQUNQUEQULQSQUNQUUQUM_ACCESS == C_FDMA_ACCESS_READ_ONLY, "channel count is read-only");
_Static_assert(C_FDMA_C_QUFQUAQUTQUAQUL__2_C_CODE_C_QUNONE == 0, "no channel fatal code");
_Static_assert(C_FDMA_C_QUFQUAQUTQUAQUL__2_C_CODE_C_QUCOMMANDQUREAD == 1, "command fatal code");
_Static_assert(C_FDMA_C_QUFQUAQUTQUAQUL__2_C_CODE_C_QURESPONSEQUWRITE == 2, "response fatal code");
_Static_assert(C_FDMA_C_QUFQUAQUTQUAQUL__1_C_CODE_C_QUINTERNAL == 1, "device internal fatal code");
_Static_assert(C_FDMA_C_QUIQURQUQQSQUCQUAQUUQUSQUE__1_ACCESS == C_FDMA_ACCESS_READ_ONLY, "causes are read-only");
_Static_assert(C_FDMA_C_QUIQURQUQQSQUMQUAQUSQUK__1_WRITABLE_MASK == 3, "two notification mask bits");
_Static_assert(C_FDMA_C_QURQUEQUSQUEQUT__2_ACCESS == C_FDMA_ACCESS_READ_WRITE, "channel reset exposes busy state");
_Static_assert(C_FDMA_C_QURQUEQUSQUEQUT__1_ACCESS == C_FDMA_ACCESS_READ_WRITE, "device reset exposes busy state");
_Static_assert(C_RESPONSE_C_STATUS_C_QUSUCCESS == 0, "success status code");
_Static_assert(C_RESPONSE_C_STATUS_C_QUINVALIDQULENGTH == 1, "invalid length status code");
_Static_assert(C_RESPONSE_C_STATUS_C_QUREADQUERROR == 2, "read error status code");
_Static_assert(C_RESPONSE_C_STATUS_C_QUWRITEQUERROR == 3, "write error status code");
_Static_assert(C_RESPONSE_C_STATUS_C_QULENGTHQUMISMATCH == 4, "length mismatch status code");
_Static_assert(C_RESPONSE_C_STATUS_C_QULISTQUREADQUERROR == 5, "list read error status code");
_Static_assert(C_HCTL_SIZE == 36, "control channel extent");
_Static_assert(C_HCTL_C_QUBQUOQUOQUTQUSQUTQURQUAQUP_OFFSET == 0, "bootstrap address");
_Static_assert(C_HCTL_C_QUEQUNQUAQUBQULQUE_OFFSET == 8, "activation address");
_Static_assert(C_HCTL_C_QURQUEQUQQUUQUEQUSQUT_OFFSET == 20, "request doorbell address");
_Static_assert(C_FCTL_C_QURQUEQUSQUPQUOQUNQUSQUE_OFFSET == 24, "response doorbell address");
_Static_assert(C_HCTL_C_QURQUEQUQQUUQUEQUSQUT_ACCESS == C_HCTL_ACCESS_WRITE_ONLY, "request doorbell is write-only");
_Static_assert(C_FCTL_C_QURQUEQUSQUPQUOQUNQUSQUE_WRITABLE_MASK == 1, "response doorbell only accepts pulse bit");
_Static_assert(C_CONTROLRINGSTATE_SIZE == 24, "ring state extent");
_Static_assert(C_CONTROLQQUEUE_SIZE == 72, "queue extent");
_Static_assert(C_CONTROLQQUEUE_ALIGNMENT == 8, "queue alignment");
_Static_assert(C_CONTROLQQUEUE_C_COMMANDQX2EBASE_OFFSET == 0, "command base offset");
_Static_assert(C_CONTROLQQUEUE_C_COMMANDQX2EPRODUCER_OFFSET == 12, "command producer offset");
_Static_assert(C_CONTROLQQUEUE_C_COMMANDQX2ECONSUMER_OFFSET == 16, "command consumer offset");
_Static_assert(C_CONTROLQQUEUE_C_RESPONSEQX2EBASE_OFFSET == 24, "response base offset");
_Static_assert(C_CONTROLQQUEUE_C_RESPONSEQX2EPRODUCER_OFFSET == 36, "response producer offset");
_Static_assert(C_CONTROLQQUEUE_C_RESPONSEQX2ECONSUMER_OFFSET == 40, "response consumer offset");
_Static_assert(C_CONTROLREQQUEST_SIZE == 32, "control request extent");
_Static_assert(C_CONTROLRESPONSE_SIZE == 24, "control response extent");
_Static_assert(C_CONTROLREQQUEST_ALIGNMENT == 8, "control request alignment");
_Static_assert(C_CONTROLRESPONSE_ALIGNMENT == 8, "control response alignment");
_Static_assert(C_CONTROLREQQUEST_C_FUNCTION_OFFSET == 12, "target Function offset");
_Static_assert(C_CONTROLREQQUEST_C_ID_OFFSET == 0, "control request id offset");
_Static_assert(C_CONTROLREQQUEST_C_OPCODE_OFFSET == 8, "control request opcode offset");
_Static_assert(C_CONTROLREQQUEST_C_CHANNEL_OFFSET == 16, "control request channel offset");
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
    uint8_t queue[C_CONTROLQQUEUE_SIZE] = {0};
    const uint8_t control_offsets[] = {
        C_HCTL_C_QUBQUOQUOQUTQUSQUTQURQUAQUP_OFFSET,
        C_HCTL_C_QUEQUNQUAQUBQULQUE_OFFSET,
        C_HCTL_C_QUAQUDQUDQURQUEQUSQUSQSQUSQUPQUAQUCQUE_OFFSET,
        C_HCTL_C_QUPQUAQUSQUIQUD_OFFSET,
        C_HCTL_C_QURQUEQUQQUUQUEQUSQUT_OFFSET,
        C_FCTL_C_QURQUEQUSQUPQUOQUNQUSQUE_OFFSET,
        C_FCTL_C_QUEQUVQUEQUNQUT_OFFSET,
        C_FCTL_C_QUEQUVQUEQUNQUTQSQUEQURQURQUOQUR_OFFSET,
    };
    const uint32_t offsets[] = {
        C_FDMA_C_QUCQUHQUAQUNQUNQUEQULQSQUNQUUQUM_OFFSET,
        C_FDMA_C_QUIQURQUQQSQUEQUNQUAQUBQULQUE__1_OFFSET,
        C_FDMA_C_QUFQUAQUTQUAQUL__1_OFFSET,
        C_FDMA_C_QURQUEQUSQUEQUT__1_OFFSET,
        C_FDMA_C_QURQUEQUSQUEQUT__2_OFFSET,
        C_FDMA_C_QURQUEQUSQUEQUT__3_OFFSET,
        C_FDMA_C_QURQUEQUSQUEQUT__4_OFFSET,
        C_FDMA_C_QUBQUAQUSQUE__1_OFFSET,
        C_FDMA_C_QUCQUAQUPQUAQUCQUIQUTQUY__1_OFFSET,
        C_FDMA_C_QUPQURQUOQUDQUUQUCQUEQUR__1_OFFSET,
        C_FDMA_C_QUCQUOQUNQUSQUUQUMQUEQUR__1_OFFSET,
        C_FDMA_C_QUBQUAQUSQUE__2_OFFSET,
        C_FDMA_C_QUCQUAQUPQUAQUCQUIQUTQUY__2_OFFSET,
        C_FDMA_C_QUPQURQUOQUDQUUQUCQUEQUR__2_OFFSET,
        C_FDMA_C_QUCQUOQUNQUSQUUQUMQUEQUR__2_OFFSET,
        C_FDMA_C_QUEQUNQUAQUBQULQUE__1_OFFSET,
        C_FDMA_C_QUIQURQUQQSQUEQUNQUAQUBQULQUE__2_OFFSET,
        C_FDMA_C_QUFQUAQUTQUAQUL__2_OFFSET,
        C_FDMA_C_QUIQURQUQQSQUCQUAQUUQUSQUE__1_OFFSET,
        C_FDMA_C_QUIQURQUQQSQUMQUAQUSQUK__1_OFFSET,
        C_FDMA_C_QURQUIQUNQUGQSQUCQUOQUNQUTQUEQUXQUT__1_OFFSET,
        C_FDMA_C_QUFQUAQUUQULQUTQSQUSQUTQUAQUTQUUQUS__1_OFFSET,
        C_FDMA_C_QURQUEQUSQUUQUMQUE__1_OFFSET,
        C_FDMA_C_QUBQUAQUSQUE__3_OFFSET,
        C_FDMA_C_QUCQUAQUPQUAQUCQUIQUTQUY__3_OFFSET,
        C_FDMA_C_QUPQURQUOQUDQUUQUCQUEQUR__3_OFFSET,
        C_FDMA_C_QUCQUOQUNQUSQUUQUMQUEQUR__3_OFFSET,
        C_FDMA_C_QUBQUAQUSQUE__4_OFFSET,
        C_FDMA_C_QUCQUAQUPQUAQUCQUIQUTQUY__4_OFFSET,
        C_FDMA_C_QUPQURQUOQUDQUUQUCQUEQUR__4_OFFSET,
        C_FDMA_C_QUCQUOQUNQUSQUUQUMQUEQUR__4_OFFSET,
        C_FDMA_C_QUEQUNQUAQUBQULQUE__2_OFFSET,
        C_FDMA_C_QUIQURQUQQSQUEQUNQUAQUBQULQUE__3_OFFSET,
        C_FDMA_C_QUFQUAQUTQUAQUL__3_OFFSET,
        C_FDMA_C_QUIQURQUQQSQUCQUAQUUQUSQUE__2_OFFSET,
        C_FDMA_C_QUIQURQUQQSQUMQUAQUSQUK__2_OFFSET,
        C_FDMA_C_QURQUIQUNQUGQSQUCQUOQUNQUTQUEQUXQUT__2_OFFSET,
        C_FDMA_C_QUFQUAQUUQULQUTQSQUSQUTQUAQUTQUUQUS__2_OFFSET,
        C_FDMA_C_QURQUEQUSQUUQUMQUE__2_OFFSET,
        C_FDMA_C_QUBQUAQUSQUE__5_OFFSET,
        C_FDMA_C_QUCQUAQUPQUAQUCQUIQUTQUY__5_OFFSET,
        C_FDMA_C_QUPQURQUOQUDQUUQUCQUEQUR__5_OFFSET,
        C_FDMA_C_QUCQUOQUNQUSQUUQUMQUEQUR__5_OFFSET,
        C_FDMA_C_QUBQUAQUSQUE__6_OFFSET,
        C_FDMA_C_QUCQUAQUPQUAQUCQUIQUTQUY__6_OFFSET,
        C_FDMA_C_QUPQURQUOQUDQUUQUCQUEQUR__6_OFFSET,
        C_FDMA_C_QUCQUOQUNQUSQUUQUMQUEQUR__6_OFFSET,
        C_FDMA_C_QUEQUNQUAQUBQULQUE__3_OFFSET,
        C_FDMA_C_QUIQURQUQQSQUEQUNQUAQUBQULQUE__4_OFFSET,
        C_FDMA_C_QUFQUAQUTQUAQUL__4_OFFSET,
        C_FDMA_C_QUIQURQUQQSQUCQUAQUUQUSQUE__3_OFFSET,
        C_FDMA_C_QUIQURQUQQSQUMQUAQUSQUK__3_OFFSET,
        C_FDMA_C_QURQUIQUNQUGQSQUCQUOQUNQUTQUEQUXQUT__3_OFFSET,
        C_FDMA_C_QUFQUAQUUQULQUTQSQUSQUTQUAQUTQUUQUS__3_OFFSET,
        C_FDMA_C_QURQUEQUSQUUQUMQUE__3_OFFSET,
    };
    C_COMMAND_STORE_C_ID(command, UINT64_C(7));
    C_COMMAND_STORE_C_SOURCEQX2ECONTEXT(command, UINT32_C(11));
    C_COMMAND_STORE_C_DESTINATIONQX2ECONTEXT(command, UINT32_C(22));
    C_COMMAND_STORE_C_SGQSCONTEXT(command, UINT32_C(33));
    C_COMMAND_STORE_C_SOURCEQX2EMODE(command, C_COMMAND_C_SOURCEQX2EMODE_C_QULINEAR);
    C_COMMAND_STORE_C_SOURCEQX2EWORD0(command, UINT64_C(0x100));
    C_COMMAND_STORE_C_COPYQSLENGTH(command, UINT64_C(8));
    C_COMMAND_STORE_C_SOURCEQX2EWORD1(command, UINT64_C(7));
    C_COMMAND_STORE_C_DESTINATIONQX2EWORD2(command, UINT64_C(1));
    C_COMMAND_STORE_C_DESTINATIONQX2EWORD3(command, UINT64_C(3));
    C_COMMAND_STORE_C_DESTINATIONQX2EMODE(command, C_COMMAND_C_DESTINATIONQX2EMODE_C_QUSCATTERQUGATHER);
    C_COMMAND_STORE_C_DESTINATIONQX2EWORD0(command, UINT64_C(0x200));
    C_COMMAND_STORE_C_DESTINATIONQX2EWORD1(command, UINT64_C(2));
    C_COMMAND_STORE_C_FLAGS(command, UINT32_C(1));
    C_RESPONSE_STORE_C_ID(response, UINT64_C(7));
    C_RESPONSE_STORE_C_STATUS(response, C_RESPONSE_C_STATUS_C_QUWRITEQUERROR);
    uint8_t *info = response + C_RESPONSE_RECORD_C_INFO_OFFSET;
    C_RESPONSE_STORE_C_FLAGS(response, UINT32_C(1));
    C_COMPLETIONINFO_STORE_C_BYTESQSCOMPLETED(info, UINT64_C(8));
    C_SEGMENT_STORE_C_ADDRESS(segment, UINT64_C(0x300));
    C_SEGMENT_STORE_C_LENGTH(segment, UINT64_C(8));
    C_CONTROLREQQUEST_STORE_C_ID(release, UINT64_C(0x0807060504030201));
    C_CONTROLREQQUEST_STORE_C_OPCODE(release, C_CONTROLREQQUEST_C_OPCODE_C_QURELEASEQUCHANNEL);
    C_CONTROLREQQUEST_STORE_C_FUNCTION(release, UINT32_C(7));
    C_CONTROLREQQUEST_STORE_C_CHANNEL(release, UINT32_C(0x10203040));
    C_CONTROLREQQUEST_STORE_C_ID(acquire, UINT64_C(9));
    C_CONTROLREQQUEST_STORE_C_OPCODE(acquire, C_CONTROLREQQUEST_C_OPCODE_C_QUACQQUIREQUCHANNEL);
    C_CONTROLREQQUEST_STORE_C_FUNCTION(acquire, UINT32_C(2));
    C_CONTROLREQQUEST_STORE_C_CHANNEL(acquire, UINT32_C(0));
    C_CONTROLRESPONSE_STORE_C_ID(granted, UINT64_C(9));
    C_CONTROLRESPONSE_STORE_C_STATUS(granted, C_CONTROLRESPONSE_C_STATUS_C_QUSUCCESS);
    C_CONTROLRESPONSE_STORE_C_CHANNEL(granted, UINT32_C(2));
    C_CONTROLRESPONSE_STORE_C_ID(rejected, UINT64_C(0x0807060504030201));
    C_CONTROLRESPONSE_STORE_C_STATUS(rejected, C_CONTROLRESPONSE_C_STATUS_C_QUNOTQUALLOCATED);
    C_CONTROLRESPONSE_STORE_C_CHANNEL(rejected, UINT32_C(0x10203040));
    C_CONTROLQQUEUE_STORE_C_COMMANDQX2EBASE(queue, UINT64_C(0x1000));
    C_CONTROLQQUEUE_STORE_C_COMMANDQX2ECAPACITY(queue, UINT32_C(8));
    C_CONTROLQQUEUE_STORE_C_COMMANDQX2EPRODUCER(queue, UINT32_C(3));
    C_CONTROLQQUEUE_STORE_C_COMMANDQX2ECONSUMER(queue, UINT32_C(1));
    C_CONTROLQQUEUE_STORE_C_RESPONSEQX2EBASE(queue, UINT64_C(0x2000));
    C_CONTROLQQUEUE_STORE_C_RESPONSEQX2ECAPACITY(queue, UINT32_C(4));
    C_CONTROLQQUEUE_STORE_C_RESPONSEQX2EPRODUCER(queue, UINT32_C(2));
    C_CONTROLQQUEUE_STORE_C_RESPONSEQX2ECONSUMER(queue, UINT32_C(1));
    C_CONTROLQQUEUE_STORE_C_EVENTQX2EBASE(queue, UINT64_C(0x3000));
    C_CONTROLQQUEUE_STORE_C_EVENTQX2ECAPACITY(queue, UINT32_C(8));
    C_CONTROLQQUEUE_STORE_C_EVENTQX2EPRODUCER(queue, UINT32_C(7));
    C_CONTROLQQUEUE_STORE_C_EVENTQX2ECONSUMER(queue, UINT32_C(5));
    if (C_CONTROLQQUEUE_LOAD_C_COMMANDQX2EBASE(queue) != UINT64_C(0x1000)) return 24;
    if (C_CONTROLQQUEUE_LOAD_C_COMMANDQX2ECAPACITY(queue) != UINT32_C(8)) return 25;
    if (C_CONTROLQQUEUE_LOAD_C_COMMANDQX2EPRODUCER(queue) != UINT32_C(3)) return 26;
    if (C_CONTROLQQUEUE_LOAD_C_COMMANDQX2ECONSUMER(queue) != UINT32_C(1)) return 27;
    if (C_CONTROLQQUEUE_LOAD_C_RESPONSEQX2EBASE(queue) != UINT64_C(0x2000)) return 28;
    if (C_CONTROLQQUEUE_LOAD_C_RESPONSEQX2ECAPACITY(queue) != UINT32_C(4)) return 29;
    if (C_CONTROLQQUEUE_LOAD_C_RESPONSEQX2EPRODUCER(queue) != UINT32_C(2)) return 30;
    if (C_CONTROLQQUEUE_LOAD_C_RESPONSEQX2ECONSUMER(queue) != UINT32_C(1)) return 31;
    if (C_COMMAND_LOAD_C_ID(command) != UINT64_C(7)) return 1;
    if (C_COMMAND_LOAD_C_SOURCEQX2EMODE(command) != UINT32_C(0)) return 2;
    if (C_COMMAND_LOAD_C_SOURCEQX2EWORD0(command) != UINT64_C(0x100)) return 3;
    if (C_COMMAND_LOAD_C_COPYQSLENGTH(command) != UINT64_C(8)) return 4;
    if (C_COMMAND_LOAD_C_DESTINATIONQX2EMODE(command) != UINT32_C(1)) return 10;
    if (C_COMMAND_LOAD_C_DESTINATIONQX2EWORD0(command) != UINT64_C(0x200)) return 11;
    if (C_COMMAND_LOAD_C_DESTINATIONQX2EWORD1(command) != UINT64_C(2)) return 12;
    if (C_COMMAND_LOAD_C_FLAGS(command) != UINT32_C(1)) return 17;
    if (C_RESPONSE_LOAD_C_ID(response) != UINT64_C(7)) return 5;
    if (C_RESPONSE_LOAD_C_STATUS(response) != UINT32_C(3)) return 6;
    if (C_RESPONSE_LOAD_C_FLAGS(response) != UINT32_C(1)) return 18;
    if (C_COMPLETIONINFO_LOAD_C_BYTESQSCOMPLETED(info) != UINT64_C(8)) return 13;
    if (C_SEGMENT_LOAD_C_ADDRESS(segment) != UINT64_C(0x300)) return 14;
    if (C_SEGMENT_LOAD_C_LENGTH(segment) != UINT64_C(8)) return 15;
    if (fwrite(command, 1, sizeof(command), stdout) != sizeof(command)) return 7;
    if (fwrite(response, 1, sizeof(response), stdout) != sizeof(response)) return 8;
    if (fwrite(segment, 1, sizeof(segment), stdout) != sizeof(segment)) return 16;
    for (size_t i = 0; i < sizeof(offsets) / sizeof(offsets[0]); ++i) {
        const uint8_t bytes[4] = {
            (uint8_t)offsets[i], (uint8_t)(offsets[i] >> 8),
            (uint8_t)(offsets[i] >> 16), (uint8_t)(offsets[i] >> 24)
        };
        if (fwrite(bytes, 1, sizeof(bytes), stdout) != sizeof(bytes)) return 9;
    }
    if (fwrite(release, 1, sizeof(release), stdout) != sizeof(release)) return 19;
    if (fwrite(acquire, 1, sizeof(acquire), stdout) != sizeof(acquire)) return 20;
    if (fwrite(granted, 1, sizeof(granted), stdout) != sizeof(granted)) return 21;
    if (fwrite(rejected, 1, sizeof(rejected), stdout) != sizeof(rejected)) return 22;
    if (fwrite(control_offsets, 1, sizeof(control_offsets), stdout) != sizeof(control_offsets)) return 23;
    if (fwrite(queue, 1, sizeof(queue), stdout) != sizeof(queue)) return 32;
    return 0;
}
