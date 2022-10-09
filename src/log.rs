use std::fmt::Display;

use tracing::error;

pub trait LogErr {
    fn log_err(self) -> Self;
}

impl<T, E> LogErr for Result<T, E>
where
    E: Display,
{
    fn log_err(self) -> Self {
        if let Err(err) = &self {
            error!("{:#}", err);
        }
        self
    }
}

pub trait WrapAndLogErr<T> {
    fn wrap_and_log_err<C>(self, context: C) -> anyhow::Result<T>
    where
        C: Display + Send + Sync + 'static;
}

impl<T, E> WrapAndLogErr<T> for Result<T, E>
where
    E: Into<anyhow::Error>,
{
    fn wrap_and_log_err<C>(self, context: C) -> anyhow::Result<T>
    where
        C: Display + Send + Sync + 'static,
    {
        match self {
            Ok(t) => Ok(t),
            Err(err) => {
                let err = err.into().context(context);
                error!("{:#}", err);
                Err(err)
            }
        }
    }
}
