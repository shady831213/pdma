use pdma_model_contract::dma::{
    Command, Response, ResponseInfoValues, SignalCommand, SignalInfo, SignalOperation, SignalOrder,
    SignalTarget, WaitCommand, WaitCondition, WaitOrder,
};

/// Encodes signal fixtures and checks independently specified wire bytes.
///
/// # Errors
/// Returns codec errors or an invalid fixture offset.
///
/// # Panics
/// Panics if the generated encoding differs from the independent fixtures.
pub fn bytes() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let target = SignalTarget {
        address: 0x800,
        context: 3,
    };
    let signal = Command::signal(
        15,
        SignalCommand {
            target,
            operation: SignalOperation::CompareExchange,
            order: SignalOrder::AcquireRelease,
            value: u64::MAX,
            expected: 7,
        },
    );
    let wait = Command::wait(
        16,
        WaitCommand {
            target,
            condition: WaitCondition::LessThan,
            order: WaitOrder::Acquire,
            compare: u64::MAX,
        },
    );
    let response = Response {
        id: 15,
        status: pdma_model_contract::dma::ResponseStatus::SignalSuccess,
        info: ResponseInfoValues::SignalSuccess(SignalInfo { value: 7 }),
        ..Response::ZERO
    };
    let mut signal_bytes = [0; 112];
    for (index, value) in [
        (0, 15),
        (12, 1),
        (17, 8),
        (24, 3),
        (32, 8),
        (36, 3),
        (48, 7),
    ] {
        *signal_bytes.get_mut(index).ok_or("signal fixture offset")? = value;
    }
    signal_bytes[40..48].fill(255);
    let mut wait_bytes = [0; 112];
    for (index, value) in [(0, 16), (12, 2), (17, 8), (24, 3), (32, 2), (36, 1)] {
        *wait_bytes.get_mut(index).ok_or("wait fixture offset")? = value;
    }
    wait_bytes[40..48].fill(255);
    let mut response_bytes = [0; 24];
    response_bytes[0] = 15;
    response_bytes[8] = 7;
    response_bytes[16] = 7;
    assert_eq!(signal.to_bytes()?, signal_bytes);
    assert_eq!(wait.to_bytes()?, wait_bytes);
    assert_eq!(response.to_bytes()?, response_bytes);
    Ok([
        signal_bytes.as_slice(),
        wait_bytes.as_slice(),
        response_bytes.as_slice(),
    ]
    .concat())
}
