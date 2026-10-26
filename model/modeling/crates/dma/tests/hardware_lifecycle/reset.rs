use super::*;

#[test]
fn reset_retires_an_accepted_invocation_when_its_stalled_stage_observes_reset() -> TestResult {
    // Given: accepted work whose execution graph stalls until reset.
    let (mut dma, memory) = setup(0x1100, 2)?;
    submit(&mut dma, &memory, command())?;
    let cleanup = std::rc::Rc::new(std::cell::Cell::new(0));
    let mut hardware = dma.hardware_with(Stalled(None, std::rc::Rc::clone(&cleanup)));
    let mut driver = HardwareDriver::new();
    for _ in 0..64 {
        assert!(matches!(
            driver.step(&mut hardware, &mut dma)?,
            StepOutcome::InProgress
        ));
        if cleanup.get() == 1 {
            break;
        }
    }
    assert_eq!(read_word(&mut dma, CONSUMER)?, 1);
    assert_eq!(cleanup.get(), 1);
    // When: reset is requested while accepted ownership remains.
    dma.write(RESET, &1_u32.to_le_bytes())?;
    assert_eq!(read_word(&mut dma, RESET)?, 1);
    assert!(matches!(
        driver.step(&mut hardware, &mut dma)?,
        StepOutcome::InProgress
    ));
    assert_eq!(cleanup.get(), 2);
    assert_eq!(read_word(&mut dma, RESET)?, 1);
    for _ in 0..64 {
        assert!(matches!(
            driver.step(&mut hardware, &mut dma)?,
            StepOutcome::InProgress
        ));
        if read_word(&mut dma, RESET)? == 0 {
            break;
        }
    }
    // Then: the stage's own terminal step acknowledges reset without completing data.
    assert_eq!(read_word(&mut dma, RESET)?, 0);
    assert_eq!(cleanup.get(), 3);
    assert!(!driver.is_complete());
    assert_eq!(read_word(&mut dma, PRODUCER)?, 0);
    Ok(())
}
