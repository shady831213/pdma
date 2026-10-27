#![allow(clippy::expect_used, reason = "checked test fixture operations")]
#![allow(missing_docs, reason = "integration test crate")]

use hardware_interface_io::register::{
    AddressedRegisterAccess, RegisterAccess, RegisterBusAddress, RegisterBusBase, RegisterBusError,
    RegisterBusOffset, RegisterBusWidth,
};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::sync::Mutex;

#[derive(Debug, PartialEq, Eq)]
struct EndpointError;

impl Display for EndpointError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("endpoint error")
    }
}

impl Error for EndpointError {}

#[derive(Default)]
struct Endpoint {
    accesses: Mutex<Vec<(u32, usize)>>,
}

impl RegisterAccess for Endpoint {
    type Error = EndpointError;

    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        response.fill(0x5a);
        self.accesses
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((offset, response.len()));
        Ok(())
    }

    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        self.accesses
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((offset, request.len()));
        Ok(())
    }
}

#[test]
fn nonzero_base_translates_the_full_address_without_truncating_width() {
    // Given one endpoint and a base that is not representable as the raw u32 offset.
    let endpoint = Endpoint::default();
    let base = RegisterBusBase::new(0x1_0000_0000);
    let address = RegisterBusAddress::new(0x1_0000_0218);
    let mut response = [0; 8];

    // When the full address is accessed through the additive addressed surface.
    let result = {
        let future = endpoint.read_addressed(base, address, &mut response);
        let mut future = std::pin::pin!(future);
        std::future::Future::poll(
            future.as_mut(),
            &mut std::task::Context::from_waker(std::task::Waker::noop()),
        )
    };
    assert!(matches!(result, std::task::Poll::Ready(Ok(()))));

    // Then the endpoint receives the exact offset and caller-owned width.
    assert_eq!(response, [0x5a; 8]);
    assert_eq!(
        *endpoint
            .accesses
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        vec![(0x218, 8)]
    );
    println!("base=0x1_0000_0000 address=0x1_0000_0218 offset=0x218 width=8");
}

#[test]
fn zero_base_accepts_the_first_and_last_representable_offset() {
    // Given the complete u32 offset range rooted at zero.
    let base = RegisterBusBase::new(0);
    let width = RegisterBusWidth::new(8);

    // When its first and last bytes are translated.
    let first = RegisterBusOffset::from_address(base, RegisterBusAddress::new(0), width)
        .expect("zero offset is representable");
    let last =
        RegisterBusOffset::from_address(base, RegisterBusAddress::new(u64::from(u32::MAX)), width)
            .expect("u32 maximum offset is representable");

    // Then both canonical offsets are preserved exactly.
    assert_eq!(first.raw(), 0);
    assert_eq!(last.raw(), u32::MAX);
}

#[test]
fn adverse_reports_original_operands_without_wrapping_or_truncation() {
    // Given full-width inputs that cannot produce a canonical offset.
    let below_base = RegisterBusBase::new(0x1_0000_0000);
    let below_address = RegisterBusAddress::new(0xffff_ffff);
    let below_width = RegisterBusWidth::new(9);
    let overflow_base = RegisterBusBase::new(0);
    let overflow_address = RegisterBusAddress::new(0x1_0000_0000);
    let overflow_width = RegisterBusWidth::new(usize::MAX);

    // When subtraction underflows or the full delta cannot narrow to u32.
    let below = RegisterBusOffset::from_address(below_base, below_address, below_width)
        .expect_err("address below base rejects before narrowing");
    let overflow = RegisterBusOffset::from_address(overflow_base, overflow_address, overflow_width)
        .expect_err("full delta outside u32 rejects");
    let maximum = RegisterBusOffset::from_address(
        RegisterBusBase::new(0),
        RegisterBusAddress::new(u64::MAX),
        RegisterBusWidth::new(usize::MAX),
    )
    .expect_err("maximum full address does not truncate");

    // Then all diagnostic operands remain full-width and exact.
    assert!(matches!(
        below,
        RegisterBusError::AddressBelowBase {
            base,
            address,
            width,
        } if base == below_base && address == below_address && width == below_width
    ));
    assert!(matches!(
        overflow,
        RegisterBusError::OffsetOutOfRange {
            base,
            address,
            width,
            delta,
        } if base == overflow_base
            && address == overflow_address
            && width == overflow_width
            && delta == 0x1_0000_0000
    ));
    assert!(matches!(
        maximum,
        RegisterBusError::OffsetOutOfRange {
            base,
            address,
            width,
            delta,
        } if base.raw() == 0
            && address.raw() == u64::MAX
            && width.raw() == usize::MAX
            && delta == u64::MAX
    ));
    println!(
        "below-base base=0x1_0000_0000 address=0xffff_ffff width=9; unrepresentable base=0x0 address=0x1_0000_0000 width={}",
        usize::MAX
    );
}
