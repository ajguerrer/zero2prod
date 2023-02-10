use anyhow::bail;

#[derive(Debug)]
pub struct IdempotencyKey(String);

impl TryFrom<String> for IdempotencyKey {
    type Error = anyhow::Error;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        if s.is_empty() {
            bail!("The idempotency key cannot be empty");
        }

        const MAX_LENGTH: usize = 50;
        if s.len() >= MAX_LENGTH {
            bail!("The idempotency key must be shorter than {MAX_LENGTH} characters");
        }

        Ok(Self(s))
    }
}

impl From<IdempotencyKey> for String {
    fn from(k: IdempotencyKey) -> Self {
        k.0
    }
}

impl AsRef<str> for IdempotencyKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
