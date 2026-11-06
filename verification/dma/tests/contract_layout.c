#include "pdma_common.h"

/* The initial RTL decoder slices this wire layout. Fail compilation if the
 * independently generated software contract moves any consumed field. */
_Static_assert(C_COPYCOMMAND_SIZE == 96, "command extent");
_Static_assert(C_COPYCOMMAND_C_HEADERQX2EOPCODE_OFFSET == 0, "opcode offset");
_Static_assert(C_COPYCOMMAND_C_HEADERQX2EOPCODE_C_QUCOPY == 0, "copy opcode");
_Static_assert(C_COPYCOMMAND_C_OPTIONS_OFFSET == 1, "endpoint selectors");
_Static_assert(C_COPYCOMMAND_C_OPTIONS_C_SOURCEQSMODE_C_QULINEAR == 0, "linear source");
_Static_assert(C_COPYCOMMAND_C_OPTIONS_C_DESTINATIONQSMODE_C_QULINEAR == 0, "linear destination");
_Static_assert(C_COPYCOMMAND_C_ID_OFFSET == 8, "command identity");
_Static_assert(C_COPYCOMMAND_C_COPYQSLENGTH_OFFSET == 16, "copy length");
_Static_assert(C_COPYCOMMAND_C_SOURCEQSCONTEXT_OFFSET == 24, "source selector");
_Static_assert(C_COPYCOMMAND_C_DESTINATIONQSCONTEXT_OFFSET == 28, "destination selector");
_Static_assert(C_COPYCOMMAND_RECORD_C_SOURCE_OFFSET == 32, "source endpoint");
_Static_assert(C_COPYCOMMAND_RECORD_C_DESTINATION_OFFSET == 64, "destination endpoint");
_Static_assert(C_LINEARENDPOINT_C_ADDRESS_OFFSET == 0, "linear address");
_Static_assert(C_LINEARENDPOINT_C_OFFSET_OFFSET == 8, "linear offset");
_Static_assert(C_EVENT_SIZE == 24, "event extent");
_Static_assert(C_EVENT_C_ID_OFFSET == 0, "event identity");
_Static_assert(C_EVENT_C_CODE_OFFSET == 8, "event code");
_Static_assert(C_EVENT_RECORD_C_INFO_OFFSET == 16, "event progress");
_Static_assert(C_FENCECOMMAND_C_HEADERQX2EOPCODE_OFFSET == 0, "Fence opcode");
_Static_assert(C_FENCECOMMAND_C_ID_OFFSET == 8, "Fence identity");
_Static_assert(C_FENCECOMMAND_C_WORDQX2ETARGETQX2EADDRESS_OFFSET == 16, "Fence address");
_Static_assert(C_FENCECOMMAND_C_WORDQX2ETARGETQX2ECONTEXT_OFFSET == 24, "Fence context");
_Static_assert(C_FENCECOMMAND_C_WORDQX2ETARGETQX2EWIDTH_OFFSET == 28, "Fence width");
_Static_assert(C_FENCECOMMAND_C_WORDQX2EVALUE_OFFSET == 32, "Fence value");
