use serde;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ConfigFile {
    pub package: Package,
    pub import: Option<Vec<Import>>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Import {
    pub path: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub compiler: String,
    pub java: String,
    pub main: Option<String>,
    pub jar: String,
}
