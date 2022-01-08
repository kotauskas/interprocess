#[allow(unused_macros)]
macro_rules! main {
    (@bmain) => {{
        std::thread::spawn(|| server::main().unwrap());
        client::main()?;
        Ok(())
    }};

    () => {
        use std::error::Error;

        mod client;
        mod server;

        fn main() -> Result<(), Box<dyn Error>> {
            main!(@bmain)
        }
    };

    ($($pred:tt)*) => {
        use std::error::Error;

        #[cfg(all($($pred)*))]
        mod client;
        #[cfg(all($($pred)*))]
        mod server;

        #[cfg(all($($pred)*))]
        fn main() -> Result<(), Box<dyn Error>> {
            main!(@bmain);
        }
        #[cfg(not(all($($pred)*)))]
        fn main() -> Result<(), Box<dyn Error>> {
            eprintln!("not supported on this platform");
            Ok(())
        }
    };
}
#[allow(unused_macros)]
macro_rules! tokio_main {
    (@bmain) => {{
        tokio::try_join!(client::main(), server::main())?;
        Ok(())
    }};
    () => {
        use std::error::Error;

        mod client;
        mod server;

        #[tokio::main]
        async fn main() -> Result<(), Box<dyn Error>> {
            tokio_main!(@bmain)
        }
    };
    ($($pred:tt)*) => {
        use std::error::Error;

        #[cfg(all($($pred)*))]
        mod client;
        #[cfg(all($($pred)*))]
        mod server;

        #[cfg(all($($pred)*))]
        #[tokio::main]
        async fn main() -> Result<(), Box<dyn Error>> {
            tokio_main!(@bmain)
        }
        #[cfg(not(all($($pred)*)))]
        #[tokio::main]
        async fn main() -> Result<(), Box<dyn Error>> {
            eprintln!("not supported on this platform");
            Ok(())
        }
    };
}
