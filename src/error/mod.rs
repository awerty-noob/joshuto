mod error_kind;
mod error_type;

pub use self::error_kind::JoshutoErrorKind;
pub use self::error_type::JoshutoError;

pub type AppResult<T = ()> = Result<T, JoshutoError>;
