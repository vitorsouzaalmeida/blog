pub const TITLE: &str = "vitor s. almeida";
pub const WEBSITE: &str = "https://vitorsalmeida.com";
pub const DESCRIPTION: &str =
    "A dedicated space to share part of me. You will find some articles, essays and some links";
pub const AUTHOR: &str = "vitor s. almeida";
pub const BIRTH_YEAR: i32 = 2004;

#[derive(Clone, Copy, Debug)]
pub struct Ctx {
    pub year: i32,
    pub live_reload: bool,
    pub og_images: bool,
}

impl Ctx {
    pub fn prod(year: i32) -> Self {
        Ctx {
            year,
            live_reload: false,
            og_images: true,
        }
    }

    pub fn dev(year: i32) -> Self {
        Ctx {
            year,
            live_reload: true,
            og_images: false,
        }
    }
}
