#include "pdma_model_abi.h"
#include "pdma_model_abi.h"
#include <stdio.h>

_Static_assert(C_COMMAND_SIZE == 32, "command wire extent");
_Static_assert(C_RESPONSE_SIZE == 16, "response wire extent");
_Static_assert(C_PDMA_SIZE == 56, "device MMIO extent");
_Static_assert(C_COMMAND_ALIGNMENT == 8, "command alignment");
_Static_assert(C_RESPONSE_ALIGNMENT == 8, "response alignment");

int main(void) {
    uint8_t command[C_COMMAND_SIZE] = {0};
    uint8_t response[C_RESPONSE_SIZE] = {0};
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
    C_COMMAND_STORE_C_SOURCE(command, UINT64_C(0x100));
    C_COMMAND_STORE_C_DESTINATION(command, UINT64_C(0x200));
    C_COMMAND_STORE_C_LENGTH(command, UINT32_C(8));
    C_RESPONSE_STORE_C_ID(response, UINT64_C(7));
    C_RESPONSE_STORE_C_STATUS(response, UINT32_C(0));
    if (C_COMMAND_LOAD_C_ID(command) != UINT64_C(7)) return 1;
    if (C_COMMAND_LOAD_C_SOURCE(command) != UINT64_C(0x100)) return 2;
    if (C_COMMAND_LOAD_C_DESTINATION(command) != UINT64_C(0x200)) return 3;
    if (C_COMMAND_LOAD_C_LENGTH(command) != UINT32_C(8)) return 4;
    if (C_RESPONSE_LOAD_C_ID(response) != UINT64_C(7)) return 5;
    if (C_RESPONSE_LOAD_C_STATUS(response) != UINT32_C(0)) return 6;
    if (fwrite(command, 1, sizeof(command), stdout) != sizeof(command)) return 7;
    if (fwrite(response, 1, sizeof(response), stdout) != sizeof(response)) return 8;
    if (fwrite(offsets, 1, sizeof(offsets), stdout) != sizeof(offsets)) return 9;
    return 0;
}
