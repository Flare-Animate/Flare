//! The game record: the game description as CBOR. Reading it goes through the description's own
//! checks (ranges, requirements, limits), so a damaged file is refused instead of run.

use flare_game::GameSpec;

use crate::StorageError;
use crate::codec::{from_cbor, to_cbor};

/// The most a game record may be once decompressed: the description's limits (4096 entities of
/// at most nine components each) with room for names, far above what a real game holds.
pub(crate) const MAX_GAME_RECORD_BYTES: usize = 16 * 1024 * 1024;

pub(crate) fn frame(game: &GameSpec) -> Result<Vec<u8>, StorageError> {
    to_cbor(game)
}

pub(crate) fn unframe(raw: &[u8]) -> Result<GameSpec, StorageError> {
    from_cbor(raw)
}
