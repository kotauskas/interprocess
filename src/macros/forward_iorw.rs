macro_rules! forward_sync_read {
    ($ty:ident) => {
        impl ::std::io::Read for $ty {
            #[inline(always)]
            fn read(&mut self, buf: &mut [u8]) -> ::std::io::Result<usize> {
                self.0.read(buf)
            }
            #[inline(always)]
            fn read_vectored(&mut self, bufs: &mut [::std::io::IoSliceMut<'_>]) -> ::std::io::Result<usize> {
                self.0.read_vectored(bufs)
            }
            // read_to_end isn't here because this macro isn't supposed to be used on Chain-like
            // adapters
            // FUTURE is_read_vectored
        }
    };
}
macro_rules! forward_sync_write {
    ($ty:ident) => {
        impl ::std::io::Write for $ty {
            #[inline(always)]
            fn write(&mut self, buf: &[u8]) -> ::std::io::Result<usize> {
                self.0.write(buf)
            }
            #[inline(always)]
            fn flush(&mut self) -> ::std::io::Result<()> {
                self.0.flush()
            }
            #[inline(always)]
            fn write_vectored(&mut self, bufs: &[::std::io::IoSlice<'_>]) -> ::std::io::Result<usize> {
                self.0.write_vectored(bufs)
            }
            // FUTURE is_write_vectored
        }
    };
}
macro_rules! forward_sync_rw {
    ($ty:ident) => {
        forward_sync_read!($ty);
        forward_sync_write!($ty);
    };
}
