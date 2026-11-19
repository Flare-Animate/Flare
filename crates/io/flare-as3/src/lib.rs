//! ActionScript 3 in a Flash movie, read as data and patched in place (issue #230).
//!
//! A Flash 3D game (Alternativa3D, Flare3D, Away3D) is compiled ActionScript 3 in the movie's
//! DoABC blocks. Flare does not run it, but someone keeping an old game alive needs to see what
//! the code is and to change what it points at: the server it connects to, a URL, a message.
//! This crate does three things with a movie's scripts:
//!
//! - **outline**: every class with its package, base class, interfaces and members, with the
//!   types and default values the bytecode records (`outline`), and one class written out as an
//!   ActionScript declaration, its method bodies listed as instructions (`class_source`).
//! - **strings**: every string the scripts carry, with what uses it: code that pushes it as text,
//!   or a name the code links by (`strings`).
//! - **patch**: strings replaced by others (`patch_strings`). Only the string section of each
//!   block's constant pool is rewritten; every index, every other byte of the block and every
//!   other tag is kept, so the movie runs as before except for the text changed. A string the
//!   code also links by is refused unless asked for, as renaming it breaks the link.
//!
//! This is not a decompiler: method bodies are listed as instructions, not rebuilt into
//! statements, and nothing here compiles ActionScript. Everything read is untrusted; sizes and
//! counts are bounded and nothing panics on a crafted movie.
#![forbid(unsafe_code)]

mod disasm;
#[cfg(feature = "fixtures")]
pub mod fixtures;
mod movie;
mod names;
mod outline;
mod pool;
mod source;
mod strings;

pub use disasm::Instruction;
pub use movie::{BlockInfo, Movie};
pub use outline::{
    ClassOutline, Member, MemberKind, Outline, OutlineQuery, Param, Signature, outline,
};
pub use source::{ClassSource, SourceOptions, class_source};
pub use strings::{
    Patched, Replacement, ScriptString, StringChange, StringQuery, Strings, Uses, patch_strings,
    strings,
};

/// The largest movie file read, as on disk.
pub const MAX_MOVIE_BYTES: u64 = 64 * 1024 * 1024;

/// The most a movie may expand to once decompressed. The header claims the size, so the claim is
/// checked before anything is allocated for it.
pub const MAX_UNCOMPRESSED_BYTES: u32 = 256 * 1024 * 1024;

/// The most script blocks a movie may have. A real movie has one per frame that adds code, a few
/// at most; a crafted one could carry far more than anyone could read.
pub const MAX_BLOCKS: usize = 1024;

/// The most replacements one patch may make.
pub const MAX_REPLACEMENTS: usize = 1024;

/// The longest string a replacement may write, in bytes.
pub const MAX_REPLACEMENT_BYTES: usize = 64 * 1024;

/// Why a movie's scripts could not be read or patched.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ScriptError {
    #[error("the file is not a Flash movie: {0}")]
    NotSwf(String),
    #[error("the movie is larger than {limit} bytes {what}")]
    TooLarge { what: &'static str, limit: u64 },
    #[error("the movie has no ActionScript 3 (no DoABC blocks)")]
    NoScripts,
    #[error("the movie has more than {MAX_BLOCKS} script blocks")]
    TooManyBlocks,
    #[error("the movie's tag list is cut short at byte {0}")]
    Truncated(usize),
    #[error("script block {block} cannot be read: {reason}")]
    Unreadable { block: usize, reason: String },
    #[error("no class named {0} in the movie's scripts")]
    NoClass(String),
    #[error("class {class} has no member named {member}")]
    NoMember { class: String, member: String },
    #[error("a patch needs at least one replacement")]
    EmptyPatch,
    #[error("a patch may make at most {MAX_REPLACEMENTS} replacements")]
    TooManyReplacements,
    #[error("a replacement must name the string it replaces")]
    EmptyFrom,
    #[error("{0:?} is replaced twice in one patch")]
    Duplicate(String),
    #[error("the replacement for {0:?} is longer than {MAX_REPLACEMENT_BYTES} bytes")]
    ReplacementTooLong(String),
    #[error("the movie's scripts have no string {0:?}")]
    NotFound(String),
    #[error(
        "{0:?} is also a name the code links by (a class, package, property or method); \
         replacing it would break that link. Allow renaming names to replace it anyway"
    )]
    IsName(String),
}
