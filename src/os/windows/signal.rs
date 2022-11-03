//! C signal support on Windows.
//!
//! A big difference from POSIX platforms is the amount of signals that Windows supports. Signals on Windows are therefore much less useful than POSIX ones.
//!
//! # Signal safe C functions
//! The C standard specifies that calling any functions *other than those ones* from a signal hook **results in undefined behavior**:
//! - `abort`
//! - `_Exit`
//! - `quick_exit`
//! - `signal`, but only if it is used for setting a handler for the same signal as the one being currently handled
//! - atomic C functions, but only the ones which are lock-free (practically never used in Rust since it has its own atomics which use compiler intrinsics)
//! - `atomic_is_lock_free`

use super::imports::*;
use std::{
    convert::{TryFrom, TryInto},
    error::Error,
    fmt::{self, Formatter},
    panic, process,
};

/// Installs the specified handler for the specified signal.
///
/// # Example
/// ```no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # #[cfg(all(windows, feature = "signals"))] {
/// use interprocess::os::windows::signal::{self, SignalType, SignalHandler};
///
/// let handler = unsafe {
///     // Since signal handlers are restricted to a specific set of C functions, creating a
///     // handler from an arbitrary function is unsafe because it might call a function
///     // outside the list, and there's no real way to know that at compile time with the
///     // current version of Rust. Since we're only using the write() system call here, this
///     // is safe.
///     SignalHandler::from_fn(|| {
///         println!("You pressed Ctrl-C!");
///     })
/// };
///
/// // Install our handler for the KeyboardInterrupt signal type.
/// signal::set_handler(SignalType::KeyboardInterrupt, handler)?;
/// # }
/// # Ok(()) }
/// ```
pub fn set_handler(signal_type: SignalType, handler: SignalHandler) -> Result<(), SetHandlerError> {
    if signal_type.is_unsafe() {
        return Err(SetHandlerError::UnsafeSignal);
    }

    unsafe { set_unsafe_handler(signal_type, handler) }
}
/// Installs the specified handler for the specified unsafe signal.
///
/// # Safety
/// The handler and all code that may or may not execute afterwards must be prepared for the aftermath of what might've caused the signal.
///
/// [`SegmentationFault`] or [`IllegalInstruction`] are most likely caused by undefined behavior invoked from Rust (the former is caused by dereferencing invalid memory, the latter is caused by dereferencing an incorrectly aligned pointer on ISAs like ARM which do not tolerate misaligned pointers), which means that the program is unsound and the only meaningful thing to do is to capture as much information as possible in a safe way – preferably using OS services to create a dump, rather than trying to read the program's global state, which might be irreversibly corrupted – and write the crash dump to some on-disk location.
///
/// # Example
/// ```no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # #[cfg(all(windows, feature = "signals"))] {
/// use interprocess::os::windows::signal::{self, SignalType, SignalHandler};
///
/// let handler = unsafe {
///     // Since signal handlers are restricted to a specific set of C functions, creating a
///     // handler from an arbitrary function is unsafe because it might call a function
///     // outside the list, and there's no real way to know that at compile time with the
///     // current version of Rust. Since we're only using the write() system call here, this
///     // is safe.
///     SignalHandler::from_fn(|| {
///         println!("Oh no, we're running on an i386!");
///         std::process::abort();
///     })
/// };
///
/// unsafe {
///     // Install our handler for the IllegalInstruction signal type.
///     signal::set_unsafe_handler(SignalType::IllegalInstruction, handler)?;
/// }
/// # }
/// # Ok(()) }
/// ```
///
/// [`SegmentationFault`]: enum.SignalType.html#variant.SegmentationFault " "
/// [`IllegalInstruction`]: enum.SignalType.html#variant.IllegalInstruction " "
pub unsafe fn set_unsafe_handler(
    signal_type: SignalType,
    handler: SignalHandler,
) -> Result<(), SetHandlerError> {
    let signal_type = signal_type as u64;
    let handlers = HANDLERS.read();
    let new_signal = handlers.get(signal_type).is_none();
    drop(handlers);
    if new_signal {
        let mut handlers = HANDLERS.write();
        handlers.remove(signal_type);
        handlers.insert(signal_type, handler);
        drop(handlers);

        let hook_val = match handler {
            SignalHandler::Default => SIG_DFL,
            _ => signal_receiver as usize,
        };
        unsafe {
            // SAFETY: we're using a correct value for the hook
            install_hook(signal_type as i32, hook_val)
                .map_err(|_| SetHandlerError::UnexpectedLibcCallFailure)?
        }
    }
    Ok(())
}

#[cfg(all(windows, feature = "signals"))]
static HANDLERS: Lazy<RwLock<IntMap<SignalHandler>>> = Lazy::new(|| RwLock::new(IntMap::new()));

unsafe fn install_hook(signum: i32, hook: usize) -> Result<(), ()> {
    let success = unsafe {
        // SAFETY: hook validity is required via safety contract
        libc::signal(signum, hook)
    } != libc::SIG_ERR as _;
    if success {
        Ok(())
    } else {
        Err(())
    }
}

/// The actual hook which is passed to `sigaction` which dispatches signals according to the global handler map (the `HANDLERS` static).
extern "C" fn signal_receiver(signum: i32) {
    let catched = panic::catch_unwind(|| {
        let handler = {
            let handlers = HANDLERS.read();
            let val = handlers
                .get(signum as u64)
                .expect("unregistered signal passed by the OS to the shared receiver");
            *val
        };
        match handler {
            SignalHandler::Ignore => {}
            SignalHandler::Hook(hook) => hook.inner()(),
            SignalHandler::NoReturnHook(hook) => hook.inner()(),
            SignalHandler::Default => unreachable!(
                "signal receiver was unregistered but has been called by the OS anyway"
            ),
        }
    });
    // The panic hook already ran, so we only have to abort the process
    catched.unwrap_or_else(|_| process::abort());
}

/// The error produced when setting a signal handler fails.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(all(windows, feature = "signals"), derive(Error))]
pub enum SetHandlerError {
    /// An unsafe signal was attempted to be handled using `set_handler` instead of `set_unsafe_handler`.
    #[cfg_attr(
        all(windows, feature = "signals"),
        error(
            "\
an unsafe signal was attempted to be handled using `set_handler` instead of `set_unsafe_handler`"
        )
    )]
    UnsafeSignal,
    /// the C library call unexpectedly failed without error information.
    #[cfg_attr(
        all(windows, feature = "signals"),
        error("the C library call unexpectedly failed without error information")
    )]
    UnexpectedLibcCallFailure,
}

/// A signal handling method.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SignalHandler {
    /// Use the default behavior specified by the C standard.
    Default,
    /// Ignore the signal whenever it is received.
    Ignore,
    /// Call a function whenever the signal is received. The function can return, execution will continue.
    Hook(SignalHook),
    /// Call a function whenever the signal is received. The function must not return.
    NoReturnHook(NoReturnSignalHook),
}
impl SignalHandler {
    /// Returns `true` for the [`Default`] variant, `false` otherwise.
    ///
    /// [`Default`]: #variant.Default.html " "
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Default)
    }
    /// Returns `true` for the [`Ignore`] variant, `false` otherwise.
    ///
    /// [`Ignore`]: #variant.Ignore.html " "
    pub const fn is_ignore(self) -> bool {
        matches!(self, Self::Ignore)
    }
    /// Returns `true` for the [`Hook`] and [`NoReturnHook`] variants, `false` otherwise.
    ///
    /// [`Hook`]: #variant.Hook.html " "
    /// [`NoReturnHook`]: #variant.NoReturnHook.html " "
    pub const fn is_hook(self) -> bool {
        matches!(self, Self::Hook(..))
    }
    /// Creates a handler which calls the specified function.
    ///
    /// # Safety
    /// The function must not call any C functions which are not considered signal-safe. See the [module-level section on signal-safe C functions] for more.
    ///
    /// [module-level section on signal-safe C functions]: index.html#signal-safe-c-functions " "
    pub unsafe fn from_fn(function: fn()) -> Self {
        let hook = unsafe {
            // SAFETY: hook validity required by safety contract
            SignalHook::from_fn(function)
        };
        Self::Hook(hook)
    }
    /// Creates a handler which calls the specified function and is known to never return.
    ///
    /// # Safety
    /// The function must not call any C functions which are not considered signal-safe. See the [module-level section on signal-safe C functions] for more.
    ///
    /// [module-level section on signal-safe C functions]: index.html#signal-safe-c-functions " "
    pub unsafe fn from_fn_noreturn(function: fn() -> !) -> Self {
        let hook = unsafe {
            // SAFETY: hook validity required by safety contract
            NoReturnSignalHook::from_fn(function)
        };
        Self::NoReturnHook(hook)
    }
}
impl Default for SignalHandler {
    /// Returns [`SignalHandler::Default`].
    ///
    /// [`SignalHandler::Default`]: #variant.Default " "
    fn default() -> Self {
        Self::Default
    }
}

/// A function which can be used as a signal handler.
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SignalHook(fn());
impl SignalHook {
    /// Creates a hook which calls the specified function.
    ///
    /// # Safety
    /// The function must not call any C functions which are not considered signal-safe. See the [module-level section on signal-safe functions] for more.
    ///
    /// [module-level section on signal-safe functions]: index.html#signal-safe-functions " "
    pub unsafe fn from_fn(function: fn()) -> Self {
        Self(function)
    }
    /// Returns the wrapped function.
    pub fn inner(self) -> fn() {
        self.0
    }
}
impl From<SignalHook> for fn() {
    fn from(op: SignalHook) -> Self {
        op.0
    }
}

/// A function which can be used as a signal handler, but one which also never returns.
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct NoReturnSignalHook(fn() -> !);
impl NoReturnSignalHook {
    /// Creates a hook which calls the specified function.
    ///
    /// # Safety
    /// Same as for the normal [`SignalHook`].
    ///
    /// [`SignalHook`]: struct.SignalHook.html " "
    pub unsafe fn from_fn(function: fn() -> !) -> Self {
        Self(function)
    }
    /// Returns the wrapped function.
    pub fn inner(self) -> fn() -> ! {
        self.0
    }
}
impl From<NoReturnSignalHook> for fn() -> ! {
    fn from(op: NoReturnSignalHook) -> Self {
        op.0
    }
}

/// All standard signal types as defined in the C standard.
///
/// The values can be safely and quickly converted to [`i32`]/[`u32`]. The reverse process involves safety checks, making sure that unknown signal values are never stored.
///
/// [`i32`]: https://doc.rust-lang.org/std/primitive.i32.html " "
/// [`u32`]: https://doc.rust-lang.org/std/primitive.u32.html " "
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
#[non_exhaustive]
pub enum SignalType {
    /// `SIGINT` – keyboard interrupt, usually sent by pressing `Ctrl`+`C` by the terminal. This signal is typically set to be ignored if the program runs an interactive interface: GUI/TUI, interactive shell (the Python shell, for example) or any other kind of interface which runs in a loop, as opposed to a command-line invocation of the program which reads its standard input or command-line arguments, performs a task and exits. If the interactive interface is running a lengthy operation, a good idea is to temporarily re-enable the signal and abort the lengthy operation if the signal is received, then disable it again.
    ///
    /// *Default handler: process termination.*
    KeyboardInterrupt = SIGINT,
    /// `SIGILL` – illegal or malformed instruction exception, generated by the CPU whenever such an instruction is executed. This signal normally should not be overriden or masked out, since it likely means that the executable file or the memory of the process has been corrupted and further execution is a risk of invoking negative consequences.
    ///
    /// For reasons described above, **this signal is considered unsafe** – handling it requires using `set_unsafe_handler`. **Signal hooks for this signal are also required to never return** – those must be wrapped into a `NoReturnSignalHook`.
    ///
    /// *Default handler: process termination with a core dump.*
    IllegalInstruction = SIGILL,
    /// `SIGABRT` – abnormal termination requested. This signal is typically invoked by the program itself, using [`std::process::abort`] or the equivalent C function; still, like any other signal, it can be sent from outside the process.
    ///
    /// *Default handler: process termination with a core dump.*
    ///
    /// [`std::process::abort`]: https://doc.rust-lang.org/std/process/fn.abort.html " "
    Abort = SIGABRT,
    /// `SIGFPE` – mathematical exception. This signal is generated whenever an undefined mathematical operation is performed – mainly integer division by zero.
    ///
    /// **Signal hooks for this signal are required to never return** – those must be wrapped into a `NoReturnSignalHook`.
    ///
    /// *Default handler: process termination with a core dump.*
    MathException = SIGFPE,
    /// `SIGSEGV` – invaid memory access. This signal is issued by the OS whenever the program tries to access an invalid memory location, such as the `NULL` pointer or simply an address outside the user-mode address space as established by the OS. The only case when this signal can be received by a Rust program is if memory unsafety occurs due to misuse of unsafe code. As such, it should normally not be masked out or handled, as it likely indicates a critical bug (soundness hole), executable file corruption or process memory corruption.
    ///
    /// For reasons described above, **this signal is considered unsafe** – handling it requires using `set_unsafe_handler`. **Signal hooks for this signal are also required to never return** – those must be wrapped into a `NoReturnSignalHook`.
    ///
    /// *Default handler: process termination with a core dump.*
    SegmentationFault = SIGSEGV,
    /// `SIGTERM` – request for termination. This signal can only be sent using the usual signal sending procedures. Unlike [`KeyboardInterrupt`], this signal is not a request to break out of a lengthy operation, but rather to close the program as a whole. Signal handlers for this signal are expected to perform minimal cleanup and quick state save procedures and then exit.
    ///
    /// *Default handler: process termination.*
    ///
    /// [`KeyboardInterrupt`]: #variant.KeyboardInterrupt " "
    Termination = SIGTERM,
}
impl SignalType {
    /// Returns `true` if the value is a signal which requires its custom handler functions to never return, `false` otherwise.
    pub const fn requires_diverging_hook(self) -> bool {
        matches!(
            self,
            Self::SegmentationFault | Self::IllegalInstruction | Self::MathException
        )
    }
    /// Returns `true` if the value is an unsafe signal which requires unsafe code when setting a handling method, `false` otherwise.
    pub const fn is_unsafe(self) -> bool {
        matches!(self, Self::SegmentationFault | Self::IllegalInstruction)
    }
}
impl From<SignalType> for i32 {
    fn from(op: SignalType) -> Self {
        op as i32
    }
}
impl From<SignalType> for u32 {
    fn from(op: SignalType) -> Self {
        op as u32
    }
}
impl TryFrom<i32> for SignalType {
    type Error = UnknownSignalError;
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            SIGINT => Ok(Self::KeyboardInterrupt),
            SIGILL => Ok(Self::IllegalInstruction),
            SIGABRT => Ok(Self::Abort),
            SIGFPE => Ok(Self::MathException),
            SIGSEGV => Ok(Self::SegmentationFault),
            SIGTERM => Ok(Self::Termination),
            _ => Err(UnknownSignalError { value }),
        }
    }
}
impl TryFrom<u32> for SignalType {
    type Error = UnknownSignalError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        value.try_into()
    }
}
/// Error type returned when a conversion from [`i32`]/[`u32`] to [`SignalType`] fails.
///
/// [`i32`]: https://doc.rust-lang.org/std/primitive.i32.html " "
/// [`u32`]: https://doc.rust-lang.org/std/primitive.u32.html " "
/// [`SignalType`]: enum.SignalType.html " "
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct UnknownSignalError {
    /// The unknown signal value which was encountered.
    pub value: i32,
}
impl fmt::Display for UnknownSignalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "unknown signal value {}", self.value)
    }
}
impl fmt::Binary for UnknownSignalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "unknown signal value {:b}", self.value)
    }
}
impl fmt::LowerHex for UnknownSignalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "unknown signal value {:x}", self.value)
    }
}
impl fmt::UpperExp for UnknownSignalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "unknown signal value {:X}", self.value)
    }
}
impl fmt::Octal for UnknownSignalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "unknown signal value {:o}", self.value)
    }
}
impl Error for UnknownSignalError {}
