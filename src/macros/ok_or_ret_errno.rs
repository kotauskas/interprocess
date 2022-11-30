macro_rules! ok_or_ret_errno {
    ($success:expr => $($scb:tt)+) => {
        if $success {
            Ok($($scb)+)
        } else {
            Err(::std::io::Error::last_os_error())
        }
    };
}
