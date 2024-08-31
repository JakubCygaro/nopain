use std::path::PathBuf;


#[derive(Clone, Debug)]
pub struct ImportValidationError {
    pub path: PathBuf
}
impl std::error::Error for ImportValidationError {

}
impl std::fmt::Display for ImportValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the path `{:?}` is not a valid .jar file path", self.path)
    }
}

#[derive(Debug)]
pub struct BuildError {
    pub msg: String,
}

impl std::error::Error for BuildError {}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

#[derive(Debug, Clone)]
pub struct InitError {
    pub msg: String,
}

impl std::error::Error for InitError {}

impl std::fmt::Display for InitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.msg)
    }
}