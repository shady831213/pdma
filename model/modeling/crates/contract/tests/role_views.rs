//! Role projections are complete contract inputs, independent of output format.
use hw_model_schema::Access;
use pdma_model_contract::{
    dma::{DmaRegisters, DmaRegistersRegisterId},
    hw_components::{
        context::ContextTableRegisters,
        control::{ControlNotificationRegisters, ControlRegisters},
        windows::{ChannelWindowRegisters, WindowControl},
    },
    view::RegisterView,
};

#[test]
fn role_views_preserve_layout_and_apply_permissions() -> Result<(), Box<dyn std::error::Error>> {
    // Given: independently sized channels and context entries, spanning bitmap words.
    let channel_count = 33;
    let layout = DmaRegisters::new(channel_count)?;
    let windows = ChannelWindowRegisters::new(channel_count)?;
    let contexts = ContextTableRegisters::for_channels(5, channel_count)?;
    // When: both software roles request fully projected register blocks.
    for role in [RegisterView::Host, RegisterView::Firmware] {
        let dma = role.dma(&layout)?;
        let boot = role.boot()?;
        let control = role.control()?;
        let window = role.windows(&windows)?;
        let context = role.context_table(&contexts)?;
        // Then: permissions change while physical geometry and instance identity remain.
        assert_eq!(dma.size(), layout.extent());
        assert_eq!(window.size(), windows.extent());
        let banks = window.banks();
        let bank = banks.first().ok_or("missing window bank")?;
        assert_eq!(bank.count(), channel_count);
        assert_eq!(bank.stride(), windows.windows().stride());
        let event_error = ControlRegisters::NOTIFICATION.offset()
            + ControlNotificationRegisters::EVENT_ERROR.offset();
        let status = control
            .registers()
            .iter()
            .find(|r| r.offset() == event_error)
            .ok_or("missing event status")?;
        match role {
            RegisterView::Host => {
                assert!(context.is_none());
                assert!(
                    dma.registers()
                        .iter()
                        .all(|r| r.id() != DmaRegistersRegisterId::Admin)
                );
                assert!(
                    boot.registers()
                        .iter()
                        .all(|r| r.definition().access() == Access::ReadOnly)
                );
                assert_eq!(status.definition().access(), Access::ReadOnly);
                assert_eq!(window.registers().len(), channel_count);
                for (index, register) in window.registers().iter().enumerate() {
                    assert_eq!(register.offset(), windows.windows().offset(index).ok_or("window offset")?
                        + pdma_model_contract::hw_components::windows::ChannelWindowBlock::STATUS.offset());
                    assert_eq!(register.definition().access(), Access::ReadOnly);
                }
            }
            RegisterView::Firmware => {
                assert_eq!(
                    context.ok_or("missing firmware contexts")?.size(),
                    contexts.extent()
                );
                assert!(
                    dma.registers()
                        .iter()
                        .any(|r| r.id() == DmaRegistersRegisterId::Admin)
                );
                assert!(
                    boot.registers()
                        .iter()
                        .all(|r| r.definition().access() == Access::ReadWrite)
                );
                assert_eq!(status.definition().access(), Access::ReadWrite);
                assert_eq!(
                    window.registers().len(),
                    channel_count * WindowControl::resolve()?.registers().len()
                );
                assert!(
                    window
                        .registers()
                        .iter()
                        .all(|r| r.definition().access() == Access::ReadWrite)
                );
            }
        }
    }
    Ok(())
}
