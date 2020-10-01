use std::borrow::Cow;
use std::fs::File;
use std::io::prelude::*;
use std::slice;
use std::mem::{transmute, size_of, forget};
use std::io::{Write, Result, Error, ErrorKind};

/// object used to extend functionality of File
/// used for reading and writing byte vectors to files
pub trait VecIO {
    fn write_vec_to_file<T>(&mut self, data: &Vec<T>) -> Result<()>;
    fn read_vec_from_file<T>(&mut self) -> Result<Vec<T>>;
}

impl VecIO for File {
    /// Writes a vector of type T to file as bytes
    fn write_vec_to_file<T>(&mut self, data: &Vec<T>) -> Result<()> {
        unsafe {
            self.write_all(slice::from_raw_parts(transmute::<*const T, *const u8>(data.as_ptr()), data.len() * size_of::<T>()))?;
        }
        Ok(())
    }
    /// Reads a Vector of type T from file
    fn read_vec_from_file<T>(&mut self) -> Result<Vec<T>> {
        let mut buffer: Vec<T> = Vec::new();
        let length = buffer.len();
        let capacity = buffer.capacity();
        unsafe {
            let mut converted = Vec::<u8>::from_raw_parts(buffer.as_mut_ptr() as *mut u8, length * size_of::<T>(), capacity * size_of::<T>());
            match self.read_to_end(&mut converted) {
                Ok(size) => {
                    if converted.len() % size_of::<T>() != 0 {
                        converted.truncate(length * size_of::<T>());
                        forget(converted);
                        return Err(Error::new(
                            ErrorKind::UnexpectedEof,
                            format!("read_file() returned a number of bytes ({}) which is not a multiple of size ({})", size, size_of::<T>())
                        ));
                    }
                },
                Err(e) => {
                    converted.truncate(length * size_of::<T>());
                    forget(converted);
                    return Err(e);
                }
            }
            let new_length = converted.len() / size_of::<T>();
            let new_capacity = converted.len() / size_of::<T>();
            buffer = Vec::from_raw_parts(converted.as_mut_ptr() as *mut T, new_length, new_capacity);
            forget(converted);
            Ok(buffer)
        }
    }
}

/// Helper function used to unpack vectors from RustEmbed Assets
///
/// This is data that is embeded in the binary
///
/// [Rust Embed]: https://crates.io/crates/rust-embed
pub fn unpack_vec_from_asset<T>(asset: Option<Cow<'static, [u8]>>) -> Result<Vec<T>> {
    let buffer: Vec<T>;
    match asset {
        Some(data) => {
            unsafe {
                let mut bytes = data.into_owned();
                if bytes.len() % size_of::<T>() != 0 {
                    forget(bytes);
                    return Err(Error::new(
                        ErrorKind::UnexpectedEof,
                        format!("read_asset() returned a number of bytes which is not a multiple of size ({})", size_of::<T>())
                    ));
                }
                let length = bytes.len() / size_of::<T>();
                let capacity = bytes.len() / size_of::<T>();
                buffer = Vec::from_raw_parts(bytes.as_mut_ptr() as *mut T, length, capacity);
                forget(bytes);
            }
        },
        None => {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("unable to read asset, file not found")
            ));
        }
    }
    Ok(buffer)
}
