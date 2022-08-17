#[allow(unused_macros)]
macro_rules! main {
    (@bmain) => {{
        std::thread::spawn(|| server::main().unwrap());
        client::main()?;
        Ok(())
    }};

    () => {
        mod client;
        mod server;

        fn main() -> anyhow::Result<()> {
            main!(@bmain)
        }
    };

    ($($pred:tt)*) => {
        #[cfg(all($($pred)*))]
        mod client;
        #[cfg(all($($pred)*))]
        mod server;

        #[cfg(all($($pred)*))]
        fn main() -> anyhow::Result<()> {
            main!(@bmain);
        }
        #[cfg(not(all($($pred)*)))]
        fn main() -> anyhow::Result<()> {
            eprintln!("not supported on this platform");
            Ok(())
        }
    };
}
#[allow(unused_macros)]
macro_rules! tokio_main {
    (@bmain) => {{
        tokio::try_join!(main_a(), main_b())?;
        Ok(())
    }};
    () => {
        mod client;
        mod server;

        #[tokio::main(flavor = "current_thread")]
        async fn main() -> anyhow::Result<()> {
            tokio_main!(@bmain)
        }
    };
    (nomod $($pred:tt)*) => {
        #[cfg(all($($pred)*))]
        #[tokio::main(flavor = "current_thread")]
        async fn main() -> anyhow::Result<()> {
            tokio_main!(@bmain)
        }
        #[cfg(not(all($($pred)*)))]
        #[tokio::main(flavor = "current_thread")]
        async fn main() -> anyhow::Result<()> {
            eprintln!("not supported on this platform or feature set");
            Ok(())
        }
    };
    ($($pred:tt)*) => {
        #[cfg(all($($pred)*))]
        mod client;
        #[cfg(all($($pred)*))]
        mod server;
        #[cfg(all($($pred)*))]
        use {server::main as main_a, client::main as main_b};

        tokio_main!(nomod $($pred)*);
    };
}
