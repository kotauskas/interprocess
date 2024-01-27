use super::Xorshift32;
use std::sync::Arc;

#[derive(Copy, Clone, Debug)]
pub struct NameGen {
    rng: Xorshift32,
    namespaced: bool,
}
impl NameGen {
    pub fn new(id: &'static str, namespaced: bool) -> Self {
        Self { rng: Xorshift32::from_id(id), namespaced }
    }
    fn next_path(&mut self) -> Arc<str> {
        let rn = self.rng.next();
        if cfg!(windows) {
            format!(r"\\.\pipe\interprocess-test-{rn:08x}.sock")
        } else if cfg!(unix) {
            format!("/tmp/interprocess-test-{rn:08x}.sock")
        } else {
            unreachable!()
        }
        .into()
    }
    fn next_namespaced(&mut self) -> Arc<str> {
        format!("@interprocess-test-{:08x}.sock", self.rng.next()).into()
    }
}
impl Iterator for NameGen {
    type Item = Arc<str>;
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
