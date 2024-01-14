use std::sync::Mutex;

pub type TestResult<T = ()> = color_eyre::eyre::Result<T>;

static COLOR_EYRE_INSTALLED: Mutex<bool> = Mutex::new(false);
pub(super) fn install() {
    let mut lock = COLOR_EYRE_INSTALLED.lock().unwrap();
    if !*lock {
        let _ = color_eyre::install();
        *lock = true;
    }
}

macro_rules! ensure_eq {
    ($left:expr, $right:expr $(,)?) => {
        match (&$left, &$right) {
            (left_val, right_val) => {
                ::color_eyre::eyre::ensure!((left_val == right_val), r#"assertion failed: `(left == right)`
  left: `{:?}`,
 right: `{:?}`"#, left_val, right_val);
            }
        }
    };
    ($left:expr, $right:expr, $($arg:tt)+) => {
        match (&$left, &$right) {
            (left_val, right_val) => {
                ::color_eyre::eyre::ensure!((left_val == right_val), r#"assertion failed: `(left == right)`
  left: `{:?}`,
 right: `{:?}`: {}"#, left_val, right_val, ::core::format_args!($($arg)+));
            }
        }
    };
}
