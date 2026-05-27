use psc_nanoid::{alphabet::Base62Alphabet, packed_nanoid_type};

/// Packed version of UserPublicId for efficient database storage
/// 16-char Base62 nanoid packs into 12 bytes
pub type PackedUserPubId = packed_nanoid_type!(16, Base62Alphabet);