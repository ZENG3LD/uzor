/// Icon identifier - string-based reference to an icon asset
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IconId(pub String);

impl IconId {
    pub fn new(name: &str) -> Self {
        Self(name.to_string())
    }

    pub fn name(&self) -> &str {
        &self.0
    }
}

impl From<&str> for IconId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl std::fmt::Display for IconId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
