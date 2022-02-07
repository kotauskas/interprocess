use super::*;

mod socket;
mod stream;

use {std::future::Future, tokio::runtime::Builder};
fn block_in_new_rt(f: impl Future<Output = ()>) {
    let rt = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime creation failed");
    rt.block_on(f);
}
