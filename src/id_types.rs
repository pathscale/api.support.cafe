use derive_more::{From, Into};
use psc_nanoid::{Nanoid, alphabet::Base62Alphabet, packed_nanoid_type};

/// Generic packed nanoid for ALL IDs in worktables (12 bytes packed from 16 chars)
pub type PackedNanoId = packed_nanoid_type!(16, Base62Alphabet);

/// Unpacked nanoid for API responses
pub type NanoId = Nanoid<16, Base62Alphabet>;

/// App public ID - use in services/handlers for type safety
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, From, Into)]
pub struct AppPublicId(pub PackedNanoId);

/// Session ID - use in services/handlers for type safety
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, From, Into)]
pub struct SessionId(pub PackedNanoId);

/// User public ID - use in services/handlers for type safety
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, From, Into)]
pub struct UserPubId(pub PackedNanoId);
