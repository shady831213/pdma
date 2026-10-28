use hw_model_schema::record_schema;

record_schema! {
    #[description = "Extensible Host address-space attributes. Default is distinct from PASID zero. PASID is at most twenty bits and must be zero for Default."]
    pub record ContextAttributes {
        alignment: 4, endian: Little,
        fields: {
            #[description = "Default uses no PASID and is distinct from the PASID-zero address space."]
            address_space: enum(u32) { variants: { Default = 0, Pasid = 1 } },
            #[description = "Twenty-bit PASID value; bits 20 through 31 must be zero. Default requires the complete value to be zero."]
            pasid: u32,
        },
    }
}

impl ContextAttributes {
    /// The Function's default address space, without a PASID.
    pub const DEFAULT: Self = Self {
        address_space: ContextAttributesAddressSpace::Default,
        pasid: 0,
    };

    /// Rejects attributes without defined address-space semantics.
    pub const fn validate(self) -> Result<(), InvalidContextAttributes> {
        let valid = match self.address_space {
            ContextAttributesAddressSpace::Default => self.pasid == 0,
            ContextAttributesAddressSpace::Pasid => self.pasid < (1 << 20),
        };
        if valid {
            Ok(())
        } else {
            Err(InvalidContextAttributes)
        }
    }
}

/// Invalid Default/PASID attribute combination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidContextAttributes;
impl core::fmt::Display for InvalidContextAttributes {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("invalid context attributes")
    }
}
impl core::error::Error for InvalidContextAttributes {}

/// Resolved fabric identity; backing selection remains platform-owned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryIdentity {
    /// Function authorized by the channel or fixed control PF.
    pub function: crate::memory::FunctionId,
    /// Address-space attributes of the immutable binding.
    pub attributes: ContextAttributes,
}

/// Platform-assigned identity, also used as the explicit target of trusted PF requests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct FunctionId(u32);

impl FunctionId {
    /// Constructs an identity in the platform's Function namespace.
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the register encoding.
    pub const fn raw(self) -> u32 {
        self.0
    }
}
