use {super::Xorshift32, interprocess::local_socket::NameTypeSupport};
#[derive(Copy, Clone, Debug)]
pub struct NameGen {
    rng: Xorshift32,
    namespaced: bool,
}
impl NameGen {
    pub fn new(id: &'static str, namespaced: bool) -> Self {
        Self {
            rng: Xorshift32::from_id(id),
            namespaced,
        }
    }
    /// Automatically chooses name type based on OS support and preference.
    pub fn new_auto(id: &'static str, prefer_namespaced: bool) -> Self {
        let namespaced = {
            use NameTypeSupport::*;
            let nts = NameTypeSupport::query();
            match (nts, prefer_namespaced) {
                (OnlyPaths, _) | (Both, false) => false,
                (OnlyNamespaced, _) | (Both, true) => true,
            }
        };
        Self::new(id, namespaced)
    }
    fn next_path(&mut self) -> String {
        format!("/tmp/interprocess-test-{:08x}.sock", self.rng.next())
    }
    fn next_namespaced(&mut self) -> String {
        format!("@interprocess-test-{:08x}.sock", self.rng.next())
    }
}
impl Iterator for NameGen {
    type Item = String;
    fn next(&mut self) -> Option<Self::Item> {
        let name = match self.namespaced {
            false => self.next_path(),
            true => self.next_namespaced(),
        };
        Some(name)
    }
}

macro_rules! make_id {
    () => {
        concat!(file!(), line!(), column!())
    };
}
