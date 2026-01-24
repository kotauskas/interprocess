//{
// TODO(2.3.0)..?
#[cfg(not(all(windows, feature = "tokio")))]
fn main() {
    #[rustfmt::skip] eprintln!("\
This example is not available on platforms other than Windows or when the \
Tokio feature is disabled.");
}
#[cfg(all(windows, feature = "tokio"))]
fn main() -> std::io::Result<()> {
    //}
    //{
    Ok(())
} //}
