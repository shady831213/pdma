//! Permission projection across expanded registers and bank templates.
use hardware_interface_schema::Access;
use pdma_model_contract::dma::{DmaRegisters, DmaRegistersRegisterId};
use pdma_model_contract::view::RegisterView;

#[test]
fn host_bank_templates_and_expanded_leaves_exclude_administration() {
    // Given: multiple data and administration blocks with overlapping local offsets.
    let layout = DmaRegisters::new(3).unwrap();
    let full = layout.resolve().unwrap();
    // When: projecting the managed Host's register view.
    let host = full
        .project_access(|r| RegisterView::Host.dma_access(r.id(), r.access()))
        .unwrap();
    // Then: both representations retain all data registers and no admin registers.
    for bank in host.banks() {
        for register in bank.registers() {
            assert!(matches!(register.id(), DmaRegistersRegisterId::Channels));
        }
        if bank
            .registers()
            .first()
            .is_some_and(|r| r.id() == DmaRegistersRegisterId::Channels)
        {
            assert_eq!(bank.registers().len(), 16);
        }
    }
    assert_eq!(host.registers().len(), 49);
    assert_eq!(
        host.registers().first().unwrap().definition().access(),
        Access::ReadOnly
    );
}
