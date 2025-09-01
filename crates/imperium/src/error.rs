use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    DatabaseError(#[from] sqlx::Error),
}
