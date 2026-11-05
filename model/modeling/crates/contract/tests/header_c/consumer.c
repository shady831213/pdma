#include "pdma_host.h"
#include "pdma_firmware.h"
#include <stdio.h>

_Static_assert(C_COMMAND_SIZE == 96, "command wire extent");
_Static_assert(C_EVENT_SIZE == 24, "response wire extent");
_Static_assert(C_HDMA_C_CHANNELS_0_C__4A203A109E0E1F3F_ACCESS == C_HDMA_ACCESS_READ_ONLY, "fault status is observable");
_Static_assert(C_HDMA_C_CHANNELS_0_C__E868AC5DE18BE710_ACCESS == C_HDMA_ACCESS_WRITE_ONLY, "owner resume is a request");
_Static_assert(C_HDMA_C_CHANNELS_0_C__E868AC5DE18BE710_WIDTH == 4, "resume is a word action");
_Static_assert(C_LINEARENDPOINT_SIZE == 16, "endpoint wire extent");
_Static_assert(C_SEGMENT_SIZE == 16, "segment wire extent");
_Static_assert(C_FDMA_SIZE == 16384, "device MMIO extent");
_Static_assert(C_FBOOT_SIZE == 4, "atomic firmware boot report extent");
_Static_assert(C_FWIN_SIZE == 36, "firmware view retains full bank extent");
_Static_assert(C_HWIN_SIZE == 36, "Host view retains full bank extent");
_Static_assert(PDMA_HOST_WINDOW_COUNT == 3, "Host channel permission count");
_Static_assert(PDMA_FIRMWARE_WINDOW_COUNT == 3, "firmware channel permission count");
_Static_assert(PDMA_HOST_WINDOW_STRIDE == 12, "Host permission stride");
_Static_assert(PDMA_FIRMWARE_WINDOW_STRIDE == 12, "firmware permission stride");
_Static_assert(C_FWIN_C_WINDOWS_1_C_CONTROL_C_OPEN_OFFSET == 12, "second firmware gate");
_Static_assert(C_FWIN_C_WINDOWS_2_C_CONTROL_C_OPEN_OFFSET == 24, "third firmware gate");
_Static_assert(C_HWIN_C_WINDOWS_1_C_STATUS_C_READY_OFFSET == 20, "second Host status");
_Static_assert(C_HWIN_C_WINDOWS_2_C_STATUS_C_READY_OFFSET == 32, "third Host status");
_Static_assert(C_HWIN_C_WINDOWS_2_C_STATUS_C_READY_ACCESS == C_HWIN_ACCESS_READ_ONLY, "last Host status permission");
_Static_assert(C_FWIN_C_WINDOWS_2_C_CONTROL_C_OPEN_ACCESS == C_FWIN_ACCESS_READ_WRITE, "last firmware control permission");
_Static_assert(C_FWIN_C_WINDOWS_0_C_CONTROL_C_OPEN_OFFSET == 0, "control map relative offset");
_Static_assert(C_HWIN_C_WINDOWS_0_C_STATUS_C_READY_OFFSET == 8, "status bank relative offset");
_Static_assert(C_FWIN_C_WINDOWS_0_C_CONTROL_C_OPEN_ACCESS == C_FWIN_ACCESS_READ_WRITE, "firmware can configure access");
_Static_assert(C_FWIN_C_WINDOWS_0_C_CONTROL_C_OPEN_WRITABLE_MASK == 1, "only access-enable bit is writable");
_Static_assert(C_FWIN_C_WINDOWS_0_C_CONTROL_C_OPEN_RESET_VALUE == 0, "window starts closed");
_Static_assert(C_HWIN_C_WINDOWS_0_C_STATUS_C_READY_ACCESS == C_HWIN_ACCESS_READ_ONLY, "readiness is read-only");
_Static_assert(C_HWIN_C_WINDOWS_0_C_STATUS_C_READY_WRITABLE_MASK == 0, "readiness has no writable fields");
_Static_assert(C_COPYCOMMAND_RECORD_C_SOURCE_OFFSET == 32, "tagged command payload offset");
_Static_assert(C_EVENT_C_CODE_OFFSET == 8, "event code offset");
_Static_assert(C_HDMA_C_DISCOVERY_C_CAPABILITIES_ACCESS == C_HDMA_ACCESS_READ_ONLY, "capabilities read only");
_Static_assert(C_HDMA_C_CHANNELS_0_C__3803902AD0228AE3_ACCESS == C_HDMA_ACCESS_WRITE_ONLY, "Trap acknowledgment write only");
_Static_assert(C_COMMAND_ALIGNMENT == 8, "command alignment");
_Static_assert(C_EVENT_ALIGNMENT == 8, "response alignment");
_Static_assert(C_LINEARENDPOINT_ALIGNMENT == 8, "endpoint alignment");
_Static_assert(C_SEGMENT_ALIGNMENT == 8, "segment alignment");
_Static_assert(C_COPYCOMMAND_C_OPTIONS_C_SOURCEQSMODE_C_QULINEAR == 0, "linear mode");
_Static_assert(C_COPYCOMMAND_C_OPTIONS_C_SOURCEQSMODE_C_QUSCATTERQUGATHER == 1, "scatter gather mode");
_Static_assert(C_FDMA_C_CHANNELS_0_C_EVENT_C_PRODUCER_OFFSET == 4132, "first channel response producer");
_Static_assert(C_FDMA_C_CHANNELS_1_C_EVENT_C_PRODUCER_OFFSET == 8228, "second channel response producer");
_Static_assert(C_FDMA_C_CHANNELS_2_C_EVENT_C_PRODUCER_OFFSET == 12324, "third channel response producer");
_Static_assert(C_FDMA_C_DISCOVERY_C_CHANNELQSNUM_RESET_VALUE == 0, "channel count populated by hardware");
_Static_assert(C_FDMA_C_DISCOVERY_C_CHANNELQSNUM_RESET_KIND == C_FDMA_RESET_POST_RESET, "instance discovery initialized after reset");
_Static_assert(C_FDMA_C_DISCOVERY_C_CHANNELQSNUM_ACCESS == C_FDMA_ACCESS_READ_ONLY, "channel count is read-only");
_Static_assert(C_FDMA_C_CHANNELS_0_C__840E2344D2276CAF_C_CODE_C_QUNONE == 0, "no channel fatal code");
_Static_assert(C_FDMA_C_CHANNELS_0_C__840E2344D2276CAF_C_CODE_C_QUCOMMANDQUREAD == 1, "command fatal code");
_Static_assert(C_FDMA_C_CHANNELS_0_C__840E2344D2276CAF_C_CODE_C_QUEVENTQUWRITE == 2, "response fatal code");
_Static_assert(C_FDMA_C_ADMIN_C_CONTROL_C_FATAL_C_CODE_C_QUINTERNAL == 1, "device internal fatal code");
_Static_assert(C_FDMA_C_CHANNELS_0_C__B5F9B0737D60E9E0_ACCESS == C_FDMA_ACCESS_READ_ONLY, "causes are read-only");
_Static_assert(C_FDMA_C_CHANNELS_0_C__5BA471A5B97D7D09_WRITABLE_MASK == 3, "two notification mask bits");
_Static_assert(C_FDMA_C_ADMIN_C_CHANNELS_0_C_RESET_ACCESS == C_FDMA_ACCESS_READ_WRITE, "channel reset exposes busy state");
_Static_assert(C_FDMA_C_ADMIN_C_CONTROL_C_RESET_ACCESS == C_FDMA_ACCESS_READ_WRITE, "device reset exposes busy state");
_Static_assert(C_EVENT_C_CODE_C_QUINVALIDQULENGTH == 1, "invalid length status code");
_Static_assert(C_EVENT_C_CODE_C_QUREADQUERROR == 2, "read error status code");
_Static_assert(C_EVENT_C_CODE_C_QUWRITEQUERROR == 3, "write error status code");
_Static_assert(C_EVENT_C_CODE_C_QULENGTHQUMISMATCH == 4, "length mismatch status code");
_Static_assert(C_EVENT_C_CODE_C_QULISTQUREADQUERROR == 5, "list read error status code");
_Static_assert(C_HCTL_SIZE == 36, "control channel extent");
_Static_assert(C_HCTL_C_CONFIG_C_BOOTSTRAP_OFFSET == 0, "bootstrap address");
_Static_assert(C_HCTL_C_CONFIG_C_ENABLE_OFFSET == 16, "activation address");
_Static_assert(C_HCTL_C_REQQUEST_OFFSET == 20, "request doorbell address");
_Static_assert(C_FCTL_C_NOTIFICATION_C_RESPONSE_OFFSET == 24, "response doorbell address");
_Static_assert(C_HCTL_C_REQQUEST_ACCESS == C_HCTL_ACCESS_WRITE_ONLY, "request doorbell is write-only");
_Static_assert(C_FCTL_C_NOTIFICATION_C_RESPONSE_WRITABLE_MASK == 1, "response doorbell only accepts pulse bit");
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
    uint8_t response[C_EVENT_SIZE] = {0};
    uint8_t segment[C_SEGMENT_SIZE] = {0};
    uint8_t release[C_CONTROLREQQUEST_SIZE] = {0};
    uint8_t acquire[C_CONTROLREQQUEST_SIZE] = {0};
    uint8_t granted[C_CONTROLRESPONSE_SIZE] = {0};
    uint8_t rejected[C_CONTROLRESPONSE_SIZE] = {0};
    uint8_t queue[C_CONTROLQQUEUE_SIZE] = {0};
    const uint8_t control_offsets[] = {
        C_HCTL_C_CONFIG_C_BOOTSTRAP_OFFSET,
        C_HCTL_C_CONFIG_C_ATTR_AB444C4A4C16BF72_OFFSET,
        C_HCTL_C_CONFIG_C_ATTRIBUTES_C_PASID_OFFSET,
        C_HCTL_C_CONFIG_C_ENABLE_OFFSET,
        C_HCTL_C_REQQUEST_OFFSET,
        C_FCTL_C_NOTIFICATION_C_RESPONSE_OFFSET,
        C_FCTL_C_NOTIFICATION_C_EVENT_OFFSET,
        C_FCTL_C_NOTIFICATION_C_EVENTQSERROR_OFFSET,
    };
    const uint32_t offsets[] = {
        C_FDMA_C_DISCOVERY_C_CHANNELQSNUM_OFFSET,
        C_FDMA_C_DISCOVERY_C_CAPABILITIES_OFFSET,
        C_FDMA_C_ADMIN_C_CONTROL_C_IRQQQSENABLE_OFFSET,
        C_FDMA_C_ADMIN_C_CONTROL_C_FATAL_OFFSET,
        C_FDMA_C_ADMIN_C_CONTROL_C_RESET_OFFSET,
        C_FDMA_C_ADMIN_C_CHANNELS_0_C_RESET_OFFSET,
        C_FDMA_C_ADMIN_C_CHANNELS_1_C_RESET_OFFSET,
        C_FDMA_C_ADMIN_C_CHANNELS_2_C_RESET_OFFSET,
        C_FDMA_C_CHANNELS_0_C_COMMAND_C_BASE_OFFSET,
        C_FDMA_C_CHANNELS_0_C__49E6655CF24A87C7_OFFSET,
        C_FDMA_C_CHANNELS_0_C__3AE495DDCCAB77ED_OFFSET,
        C_FDMA_C_CHANNELS_0_C__12A3A66B35836683_OFFSET,
        C_FDMA_C_CHANNELS_0_C_EVENT_C_BASE_OFFSET,
        C_FDMA_C_CHANNELS_0_C_EVENT_C_CAPACITY_OFFSET,
        C_FDMA_C_CHANNELS_0_C_EVENT_C_PRODUCER_OFFSET,
        C_FDMA_C_CHANNELS_0_C_EVENT_C_CONSUMER_OFFSET,
        C_FDMA_C_CHANNELS_0_C__340CEF41970AA9BF_OFFSET,
        C_FDMA_C_CHANNELS_0_C__511E55BB3A85FCB1_OFFSET,
        C_FDMA_C_CHANNELS_0_C__840E2344D2276CAF_OFFSET,
        C_FDMA_C_CHANNELS_0_C__4A203A109E0E1F3F_OFFSET,
        C_FDMA_C_CHANNELS_0_C__E868AC5DE18BE710_OFFSET,
        C_FDMA_C_CHANNELS_0_C__FC6B8C6067290F8E_OFFSET,
        C_FDMA_C_CHANNELS_0_C__B5F9B0737D60E9E0_OFFSET,
        C_FDMA_C_CHANNELS_0_C__5BA471A5B97D7D09_OFFSET,
        C_FDMA_C_CHANNELS_0_C__3803902AD0228AE3_OFFSET,
        C_FDMA_C_CHANNELS_1_C_COMMAND_C_BASE_OFFSET,
        C_FDMA_C_CHANNELS_1_C__28CDE102B609692E_OFFSET,
        C_FDMA_C_CHANNELS_1_C__FA2D1B39B2827F34_OFFSET,
        C_FDMA_C_CHANNELS_1_C__58E091BB136F424A_OFFSET,
        C_FDMA_C_CHANNELS_1_C_EVENT_C_BASE_OFFSET,
        C_FDMA_C_CHANNELS_1_C_EVENT_C_CAPACITY_OFFSET,
        C_FDMA_C_CHANNELS_1_C_EVENT_C_PRODUCER_OFFSET,
        C_FDMA_C_CHANNELS_1_C_EVENT_C_CONSUMER_OFFSET,
        C_FDMA_C_CHANNELS_1_C__9E23A59CA038DD50_OFFSET,
        C_FDMA_C_CHANNELS_1_C__8F21236301AB9928_OFFSET,
        C_FDMA_C_CHANNELS_1_C__DE19C80C2F1FD8EE_OFFSET,
        C_FDMA_C_CHANNELS_1_C__C47B86CFB0B21FC2_OFFSET,
        C_FDMA_C_CHANNELS_1_C__D1758CE5B5D98387_OFFSET,
        C_FDMA_C_CHANNELS_1_C__458C380942F60DE9_OFFSET,
        C_FDMA_C_CHANNELS_1_C__F33A610409871909_OFFSET,
        C_FDMA_C_CHANNELS_1_C__F3507E69BFDCFE02_OFFSET,
        C_FDMA_C_CHANNELS_1_C__466513D14983256E_OFFSET,
        C_FDMA_C_CHANNELS_2_C_COMMAND_C_BASE_OFFSET,
        C_FDMA_C_CHANNELS_2_C__47CF63D379D52125_OFFSET,
        C_FDMA_C_CHANNELS_2_C__3D1411EF4B37C3EB_OFFSET,
        C_FDMA_C_CHANNELS_2_C__BCFE28D281EB5DF5_OFFSET,
        C_FDMA_C_CHANNELS_2_C_EVENT_C_BASE_OFFSET,
        C_FDMA_C_CHANNELS_2_C_EVENT_C_CAPACITY_OFFSET,
        C_FDMA_C_CHANNELS_2_C_EVENT_C_PRODUCER_OFFSET,
        C_FDMA_C_CHANNELS_2_C_EVENT_C_CONSUMER_OFFSET,
        C_FDMA_C_CHANNELS_2_C__B17DA5818BACC321_OFFSET,
        C_FDMA_C_CHANNELS_2_C__3B5C0C1F2524852B_OFFSET,
        C_FDMA_C_CHANNELS_2_C__D21C30EE6BB190E1_OFFSET,
        C_FDMA_C_CHANNELS_2_C__32B8EEFA9FBE55F5_OFFSET,
        C_FDMA_C_CHANNELS_2_C__AB66AC3DEBE0EC42_OFFSET,
        C_FDMA_C_CHANNELS_2_C__471BFA51E5572058_OFFSET,
        C_FDMA_C_CHANNELS_2_C__4BBD9C61497943D6_OFFSET,
        C_FDMA_C_CHANNELS_2_C__1858B3CEFDB7A8CB_OFFSET,
        C_FDMA_C_CHANNELS_2_C__8EB6C4A065CBA189_OFFSET,
    };
    uint8_t *copy = command;
    C_COMMANDHEADER_STORE_C_OPCODE(command, C_COMMANDHEADER_C_OPCODE_C_QUCOPY);
    C_COPYCOMMAND_STORE_C_ID(command, UINT64_C(7));
    C_COPYCOMMAND_STORE_C_SOURCEQSCONTEXT(copy, UINT32_C(11));
    C_COPYCOMMAND_STORE_C_DESTINATIONQSCONTEXT(copy, UINT32_C(22));
    C_COPYCOMMAND_STORE_C_SGQSCONTEXT(copy, UINT32_C(33));
    C_COPYCOMMAND_STORE_C_OPTIONS(copy, UINT8_C(2));
    C_LINEARENDPOINT_STORE_C_ADDRESS(copy + C_COPYCOMMAND_RECORD_C_SOURCE_OFFSET, UINT64_C(0x100));
    C_COPYCOMMAND_STORE_C_COPYQSLENGTH(copy, UINT64_C(8));
    C_LINEARENDPOINT_STORE_C_OFFSET(copy + C_COPYCOMMAND_RECORD_C_SOURCE_OFFSET, UINT64_C(7));
    C_SCATTERGATHERENDPOINT_STORE_C_INDEX(copy + C_COPYCOMMAND_RECORD_C_DESTINATION_OFFSET, UINT64_C(1));
    C_SCATTERGATHERENDPOINT_STORE_C_OFFSET(copy + C_COPYCOMMAND_RECORD_C_DESTINATION_OFFSET, UINT64_C(3));
    C_SCATTERGATHERENDPOINT_STORE_C_LISTQSADDRESS(copy + C_COPYCOMMAND_RECORD_C_DESTINATION_OFFSET, UINT64_C(0x200));
    C_SCATTERGATHERENDPOINT_STORE_C_COUNT(copy + C_COPYCOMMAND_RECORD_C_DESTINATION_OFFSET, UINT64_C(2));
    C_EVENT_STORE_C_ID(response, UINT64_C(7));
    C_EVENT_STORE_C_CODE(response, C_EVENT_C_CODE_C_QUWRITEQUERROR);
    uint8_t *info = response + C_EVENT_RECORD_C_INFO_OFFSET;
    C_ERRORINFO_STORE_C_BYTESQSCOMPLETED(info, UINT64_C(8));
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
    if (C_COPYCOMMAND_LOAD_C_ID(command) != UINT64_C(7)) return 1;
    if ((C_COPYCOMMAND_LOAD_C_OPTIONS(copy) & 1) != 0) return 2;
    if (C_LINEARENDPOINT_LOAD_C_ADDRESS(copy + C_COPYCOMMAND_RECORD_C_SOURCE_OFFSET) != UINT64_C(0x100)) return 3;
    if (C_COPYCOMMAND_LOAD_C_COPYQSLENGTH(copy) != UINT64_C(8)) return 4;
    if ((C_COPYCOMMAND_LOAD_C_OPTIONS(copy) >> 1) != 1) return 10;
    if (C_SCATTERGATHERENDPOINT_LOAD_C_LISTQSADDRESS(copy + C_COPYCOMMAND_RECORD_C_DESTINATION_OFFSET) != UINT64_C(0x200)) return 11;
    if (C_SCATTERGATHERENDPOINT_LOAD_C_COUNT(copy + C_COPYCOMMAND_RECORD_C_DESTINATION_OFFSET) != UINT64_C(2)) return 12;
    if (C_EVENT_LOAD_C_ID(response) != UINT64_C(7)) return 5;
    if (C_EVENT_LOAD_C_CODE(response) != UINT32_C(3)) return 6;
    if (C_ERRORINFO_LOAD_C_BYTESQSCOMPLETED(info) != UINT64_C(8)) return 13;
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
