#include "pdma_model_abi.h"
#include "pdma_model_abi.h"
#include <stdio.h>

_Static_assert(C_COMMAND_SIZE == 56, "command wire extent");
_Static_assert(C_RESPONSE_SIZE == 24, "response wire extent");
_Static_assert(C_ENDPOINT_SIZE == 24, "endpoint wire extent");
_Static_assert(C_SEGMENT_SIZE == 16, "segment wire extent");
_Static_assert(C_PDMA_SIZE == 56, "device MMIO extent");
_Static_assert(C_COMMAND_ALIGNMENT == 8, "command alignment");
_Static_assert(C_RESPONSE_ALIGNMENT == 8, "response alignment");
_Static_assert(C_ENDPOINT_ALIGNMENT == 8, "endpoint alignment");
_Static_assert(C_SEGMENT_ALIGNMENT == 8, "segment alignment");
_Static_assert(C_ENDPOINT_C_MODE_C_QULINEAR == 0, "linear mode");
_Static_assert(C_ENDPOINT_C_MODE_C_QUSCATTERQUGATHER == 1, "scatter gather mode");
_Static_assert(C_PDMA_C_QUFQUAQUUQULQUT_C_CODE_C_QUNONE == 0, "no fault code");
_Static_assert(C_PDMA_C_QUFQUAQUUQULQUT_C_CODE_C_QUCOMMANDQUREAD == 1, "command fault code");
_Static_assert(C_PDMA_C_QUFQUAQUUQULQUT_C_CODE_C_QURESPONSEQUWRITE == 2, "response fault code");
_Static_assert(C_RESPONSE_C_STATUS_C_QUSUCCESS == 0, "success status code");
_Static_assert(C_RESPONSE_C_STATUS_C_QUINVALIDQULENGTH == 1, "invalid length status code");
_Static_assert(C_RESPONSE_C_STATUS_C_QUREADQUFAULT == 2, "read fault status code");
_Static_assert(C_RESPONSE_C_STATUS_C_QUWRITEQUFAULT == 3, "write fault status code");
_Static_assert(C_RESPONSE_C_STATUS_C_QULENGTHQUMISMATCH == 4, "length mismatch status code");
_Static_assert(C_RESPONSE_C_STATUS_C_QULISTQUREADQUFAULT == 5, "list read fault status code");

int main(void) {
    uint8_t command[C_COMMAND_SIZE] = {0};
    uint8_t response[C_RESPONSE_SIZE] = {0};
    uint8_t segment[C_SEGMENT_SIZE] = {0};
    const uint8_t offsets[] = {
        C_PDMA_C_QUBQUAQUSQUE__1_OFFSET,
        C_PDMA_C_QUCQUAQUPQUAQUCQUIQUTQUY__1_OFFSET,
        C_PDMA_C_QUPQURQUOQUDQUUQUCQUEQUR__1_OFFSET,
        C_PDMA_C_QUCQUOQUNQUSQUUQUMQUEQUR__1_OFFSET,
        C_PDMA_C_QUBQUAQUSQUE__2_OFFSET,
        C_PDMA_C_QUCQUAQUPQUAQUCQUIQUTQUY__2_OFFSET,
        C_PDMA_C_QUPQURQUOQUDQUUQUCQUEQUR__2_OFFSET,
        C_PDMA_C_QUCQUOQUNQUSQUUQUMQUEQUR__2_OFFSET,
        C_PDMA_C_QUEQUNQUAQUBQULQUE_OFFSET,
        C_PDMA_C_QUIQURQUQQSQUEQUNQUAQUBQULQUE_OFFSET,
        C_PDMA_C_QUFQUAQUUQULQUT_OFFSET,
    };
    C_COMMAND_STORE_C_ID(command, UINT64_C(7));
    C_COMMAND_STORE_C_SOURCEQX2EMODE(command, C_COMMAND_C_SOURCEQX2EMODE_C_QULINEAR);
    C_COMMAND_STORE_C_SOURCEQX2EWORD0(command, UINT64_C(0x100));
    C_COMMAND_STORE_C_SOURCEQX2EWORD1(command, UINT64_C(8));
    C_COMMAND_STORE_C_DESTINATIONQX2EMODE(command, C_COMMAND_C_DESTINATIONQX2EMODE_C_QUSCATTERQUGATHER);
    C_COMMAND_STORE_C_DESTINATIONQX2EWORD0(command, UINT64_C(0x200));
    C_COMMAND_STORE_C_DESTINATIONQX2EWORD1(command, UINT64_C(2));
    C_RESPONSE_STORE_C_ID(response, UINT64_C(7));
    C_RESPONSE_STORE_C_STATUS(response, C_RESPONSE_C_STATUS_C_QUSUCCESS);
    C_RESPONSE_STORE_C_BYTESQSCOMPLETED(response, UINT64_C(8));
    C_SEGMENT_STORE_C_ADDRESS(segment, UINT64_C(0x300));
    C_SEGMENT_STORE_C_LENGTH(segment, UINT64_C(8));
    if (C_COMMAND_LOAD_C_ID(command) != UINT64_C(7)) return 1;
    if (C_COMMAND_LOAD_C_SOURCEQX2EMODE(command) != UINT32_C(0)) return 2;
    if (C_COMMAND_LOAD_C_SOURCEQX2EWORD0(command) != UINT64_C(0x100)) return 3;
    if (C_COMMAND_LOAD_C_SOURCEQX2EWORD1(command) != UINT64_C(8)) return 4;
    if (C_COMMAND_LOAD_C_DESTINATIONQX2EMODE(command) != UINT32_C(1)) return 10;
    if (C_COMMAND_LOAD_C_DESTINATIONQX2EWORD0(command) != UINT64_C(0x200)) return 11;
    if (C_COMMAND_LOAD_C_DESTINATIONQX2EWORD1(command) != UINT64_C(2)) return 12;
    if (C_RESPONSE_LOAD_C_ID(response) != UINT64_C(7)) return 5;
    if (C_RESPONSE_LOAD_C_STATUS(response) != UINT32_C(0)) return 6;
    if (C_RESPONSE_LOAD_C_BYTESQSCOMPLETED(response) != UINT64_C(8)) return 13;
    if (C_SEGMENT_LOAD_C_ADDRESS(segment) != UINT64_C(0x300)) return 14;
    if (C_SEGMENT_LOAD_C_LENGTH(segment) != UINT64_C(8)) return 15;
    if (fwrite(command, 1, sizeof(command), stdout) != sizeof(command)) return 7;
    if (fwrite(response, 1, sizeof(response), stdout) != sizeof(response)) return 8;
    if (fwrite(segment, 1, sizeof(segment), stdout) != sizeof(segment)) return 16;
    if (fwrite(offsets, 1, sizeof(offsets), stdout) != sizeof(offsets)) return 9;
    return 0;
}
