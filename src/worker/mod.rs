pub mod download;
pub mod launch;
pub mod meta;
pub mod modrinth;
pub mod natives;
pub mod fabric;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModLoader {
    Vanilla,
    Forge,
    NeoForge,
    Fabric,
    Quilt,
}
impl ModLoader {
    pub fn title(self) -> &'static str {
        match self {
            ModLoader::Vanilla => "原版",
            ModLoader::Forge => "Forge",
            ModLoader::NeoForge => "NeoForge",
            ModLoader::Fabric => "Fabric",
            ModLoader::Quilt => "Quilt",
        }
    }
    pub fn all() -> [ModLoader; 5] {
        [
            ModLoader::Vanilla,
            ModLoader::Fabric,
            ModLoader::Forge,
            ModLoader::NeoForge,
            ModLoader::Quilt,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Relevance,
    Downloads,
    Follows,
    Newest,
}