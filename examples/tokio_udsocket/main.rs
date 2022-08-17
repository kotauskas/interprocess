include!("../../example_main.rs");

#[cfg(all(unix, feature = "tokio_support"))]
mod inner;

#[allow(dead_code)]
static A: &str = "side_a";
#[allow(dead_code)]
static B: &str = "side_b";

#[cfg(all(unix, feature = "tokio_support"))]
pub async fn main_a(notify: tokio::sync::oneshot::Sender<()>) -> std::io::Result<()> {
    inner::main(A, B, Some(notify)).await
}
#[cfg(all(unix, feature = "tokio_support"))]
pub async fn main_b() -> std::io::Result<()> {
    inner::main(B, A, None).await
}

tokio_main!(nomod unix, feature = "tokio_support");
