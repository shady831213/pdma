//! Strongly typed identifiers used at software/hardware boundaries.

macro_rules! id_type {
    ($name:ident) => {
        #[doc = concat!("Typed PDMA identifier for ", stringify!($name), ".")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(usize);

        impl $name {
            #[doc = concat!("Creates a ", stringify!($name), " from its raw value.")]
            pub const fn new(raw: usize) -> Self {
                Self(raw)
            }

            /// Returns the raw integer value.
            pub const fn raw(self) -> usize {
                self.0
            }
        }
    };
}

id_type!(QueueId);
id_type!(CommandId);
id_type!(BufferId);
id_type!(CompletionRingId);
