use {
    super::*,
    crate::{
        os::windows::{
            named_pipe::{pipe_mode, PipeMode, WaitTimeout},
            path_conversion::*,
            SecurityDescriptor,
        },
        TryClone,
    },
    std::{borrow::Cow, num::NonZeroU8, os::windows::prelude::*},
    widestring::{u16cstr, U16CStr, U16CString},
    windows_sys::Win32::System::Pipes::{PIPE_NOWAIT, PIPE_TYPE_MESSAGE},
};

impl PipeListenerOptions<'_> {
    // TODO(2.3.0) detailed error information like with streams
    #[allow(clippy::unwrap_used, clippy::unwrap_in_result)]
    pub fn collect_from_handle(handle: BorrowedHandle<'_>) -> io::Result<Self> {
        let mut rslt = Self::default();

        let [mut flags, mut max_instances] = [0_u32; 2];
        c_wrappers::get_np_info(
            handle,
            Some(&mut flags),
            Some(&mut rslt.input_buffer_size_hint),
            Some(&mut rslt.output_buffer_size_hint),
            Some(&mut max_instances),
        )?;
        rslt.mode = PipeMode::try_from(flags & PIPE_TYPE_MESSAGE).unwrap();
        if max_instances == 255 {
            // 255 is sentinel for unlimited instances. We re-sentinel via NonZeroU8.
            max_instances = 0;
        }
        rslt.instance_limit = NonZeroU8::new(u8::try_from(max_instances).unwrap_or(0));

        // TODO(2.3.0) error out if PIPE_SERVER_END in flags, check for REJECT_REMOTE_CLIENTS (its presence
        // in the flags is not documented)

        let mode = c_wrappers::get_np_handle_mode(handle)?;
        rslt.nonblocking = mode & PIPE_NOWAIT != 0;

        let path = FileHandle::path(handle)?;
        rslt.path = Cow::Owned(unsafe {
            // SAFETY: Windows will never write interior nuls there.
            U16CString::from_vec_unchecked(path)
        });
        // TODO(2.3.0) security descriptor, inheritable

        Ok(rslt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // TODO(2.3.0)
    fn check_collect<Rm: PipeModeTag, Sm: PipeModeTag>(original: PipeListenerOptions<'_>) {
        let listener = original.create::<Rm, Sm>();
        let collected = PipeListenerOptions::collect_from_handle(todo!("as_handle"))
            .expect("failed to collect options");

        assert_eq!(collected.path, original.path);
        assert_eq!(collected.mode, original.mode);
        assert_eq!(collected.nonblocking, original.nonblocking);
        assert_eq!(collected.instance_limit, original.instance_limit);
        assert_eq!(collected.accept_remote, original.accept_remote);
        assert_eq!(collected.input_buffer_size_hint, original.input_buffer_size_hint);
        assert_eq!(collected.output_buffer_size_hint, original.output_buffer_size_hint);
        // FIXME(2.3.0) can't PartialEq security descriptors
        assert!(collected.security_descriptor.is_some());
        assert_eq!(collected.inheritable, original.inheritable);
    }

    #[test]
    fn collect_duplex_byte() {
        let opts = PipeListenerOptions {
            path: todo!(), // (2.3.0)
            mode: PipeMode::Bytes,
            nonblocking: true,
            instance_limit: NonZeroU8::new(250),
            write_through: true,
            accept_remote: false,
            input_buffer_size_hint: 420,
            output_buffer_size_hint: 228,
            wait_timeout: WaitTimeout::from_raw(1987),
            security_descriptor: todo!(), // (2.3.0)
            inheritable: true,
            ..Default::default()
        };
        check_collect::<pipe_mode::Bytes, pipe_mode::Bytes>(opts);
    }
}
