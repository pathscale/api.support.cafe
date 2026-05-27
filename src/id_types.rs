use derive_more::{From, Into};
use psc_nanoid::{Nanoid, alphabet::Base62Alphabet, packed_nanoid_type};

/// Generic packed nanoid for ALL IDs in worktables (12 bytes packed from 16 chars)
pub type PackedNanoId = packed_nanoid_type!(16, Base62Alphabet);

/// Unpacked nanoid for API responses
pub type NanoId = Nanoid<16, Base62Alphabet>;

macro_rules! define_id_type {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, From, Into)]
        pub struct $name(NanoId);

        impl $name {
            pub fn pack(&self) -> eyre::Result<PackedNanoId> {
                PackedNanoId::pack(&self.0)
                    .map_err(|e| eyre::eyre!("Failed to pack {}: {}", stringify!($name), e))
            }

            pub fn from_packed(packed: PackedNanoId) -> eyre::Result<Self> {
                packed.unpack()
                    .map($name)
                    .map_err(|e| eyre::eyre!("Failed to unpack {}: {}", stringify!($name), e))
            }
        }
    };
}

define_id_type!(AppPublicId);
define_id_type!(SessionId);
define_id_type!(UserPubId);