use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid event kind")]
    InvalidEvent,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Nix(#[from] nix::Error),
}
