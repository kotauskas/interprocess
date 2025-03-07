//{
// TODO(2.3.0)..?
#[cfg(not(all(windows, feature = "tokio")))]
fn main() {
    eprintln!("This example is not available when the Tokio feature is disabled.");
}
#[cfg(all(windows, feature = "tokio"))]
fn main() -> std::io::Result<()> {
    //}
    //{
    Ok(())
} //}
