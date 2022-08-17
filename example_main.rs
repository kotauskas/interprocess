#[allow(unused_macros)]
macro_rules! main {
    (@bmain) => {{
        use std::sync::mpsc;
        let (snd, rcv) = mpsc::channel();
        let srv = std::thread::spawn(move || server::main(snd));
        let _ = rcv.recv();
        if let Err(e) = client::main() {
            eprintln!("Client exited early with error: {:#}", e);
        }
        if let Err(e) = srv.join().expect("server thread panicked") {
            eprintln!("Server exited early with error: {:#}", e);
        }
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
        use tokio::sync::oneshot;

        let (snd, rcv) = oneshot::channel();
        let a = async {
            if let Err(e) = main_a(snd).await {
                eprintln!("Server exited early with error: {:#}", e);
            }
        };
        let b = async {
            if rcv.await.is_ok() {
                if let Err(e) = main_b().await {
                    eprintln!("Client exited early with error: {:#}", e);
                }
            }
        };
        tokio::join!(a, b);
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
