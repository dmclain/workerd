// Copyright (c) 2026 Cloudflare, Inc.
// Licensed under the Apache 2.0 license found in the LICENSE file or at:
//     https://opensource.org/licenses/Apache-2.0

//! The `TextEncoder` interface of the WHATWG Encoding Standard:
//! <https://encoding.spec.whatwg.org/#interface-textencoder>

#![allow(
    unsafe_code,
    reason = "TextEncoder writes UTF-8 directly into the bytes of a JavaScript Uint8Array, calls \
              simdutf, and hands C++ its template through a cxx bridge"
)]

use std::mem::MaybeUninit;

use jsg::MemberRegistry;
use jsg::PropertyPlacement;
use jsg::feature_flags::compatibility_flags;
use jsg::v8;
use jsg_macros::jsg_constructor;
use jsg_macros::jsg_method;
use jsg_macros::jsg_property;
use jsg_macros::jsg_resource;
use jsg_macros::jsg_struct;

#[cxx::bridge(namespace = "workerd::rust::encoding")]
mod ffi {
    #[namespace = "workerd::rust::jsg"]
    unsafe extern "C++" {
        include!("workerd/rust/jsg/ffi.h");
        include!("workerd/rust/jsg/v8.rs.h");

        type Isolate = jsg::v8::ffi::Isolate;
        type Local = jsg::v8::ffi::Local;
    }
    unsafe extern "C++" {
        include!("workerd/rust/encoding/text_encoder.h");

        unsafe fn simdutf_ascii_prefix_length(input: *const u8, length: usize) -> usize;
        unsafe fn simdutf_utf8_length_from_latin1(input: *const u8, length: usize) -> usize;
        unsafe fn simdutf_convert_latin1_to_utf8(
            input: *const u8,
            length: usize,
            utf8_output: *mut u8,
        ) -> usize;
        unsafe fn simdutf_convert_latin1_to_utf8_safe(
            input: *const u8,
            length: usize,
            utf8_output: *mut u8,
            utf8_length: usize,
            input_count: &mut usize,
        ) -> usize;
        unsafe fn simdutf_utf8_length_from_utf16(input: *const u16, length: usize) -> usize;
        unsafe fn simdutf_utf8_length_from_utf16_with_replacement(
            input: *const u16,
            length: usize,
            has_surrogates: &mut bool,
        ) -> usize;
        unsafe fn simdutf_to_well_formed_utf16(input: *const u16, length: usize, output: *mut u16);
        unsafe fn simdutf_convert_utf16_to_utf8(
            input: *const u16,
            length: usize,
            utf8_output: *mut u8,
        ) -> usize;
        unsafe fn simdutf_convert_utf16_to_utf8_with_errors(
            input: *const u16,
            length: usize,
            utf8_output: *mut u8,
            ok: &mut bool,
        ) -> usize;
        unsafe fn simdutf_convert_utf16_to_utf8_safe(
            input: *const u16,
            length: usize,
            utf8_output: *mut u8,
            utf8_length: usize,
            input_count: &mut usize,
            output_count: &mut usize,
        ) -> u8;
        unsafe fn simdutf_convert_utf16_to_utf8_with_replacement_safe(
            input: *const u16,
            length: usize,
            utf8_output: *mut u8,
            utf8_length: usize,
            input_count: &mut usize,
        ) -> usize;
    }
    extern "Rust" {
        /// Returns the `TextEncoder` constructor's `v8::Local<v8::FunctionTemplate>`, for C++
        /// to expose on the global scope with `JSG_NESTED_TEMPLATE`. `isolate` must be locked,
        /// have a Rust `Realm`, and have a `HandleScope` open until C++ consumes the result.
        unsafe fn text_encoder_template(isolate: *mut Isolate) -> Local;
    }
}

/// The outcomes of `simdutf_convert_utf16_to_utf8_safe`, numbered as in `text_encoder.h`.
const SIMDUTF_OUTPUT_BUFFER_TOO_SMALL: u8 = 1;
const SIMDUTF_SURROGATE: u8 = 2;

/// The result of `TextEncoder.encodeInto()`: UTF-16 code units read from the input and
/// UTF-8 bytes written to the destination.
#[jsg_struct(name = "TextEncoderEncodeIntoResult")]
pub struct EncodeIntoResult {
    pub read: u32,
    pub written: u32,
}

#[jsg_resource]
pub struct TextEncoder;

#[jsg_resource(custom_members)]
impl TextEncoder {
    #[jsg_constructor]
    fn constructor() -> Self {
        Self
    }

    /// Encodes `input` (default `""`), converted with `ToString`, as UTF-8, replacing lone
    /// surrogates with U+FFFD.
    ///
    /// # Errors
    ///
    /// Fails only if `input` cannot be converted to a string (e.g. a `Symbol`).
    #[jsg_method]
    pub fn encode<'s>(
        &self,
        lock: &mut jsg::Lock<'s>,
        input: Option<jsg::JsString<'s>>,
    ) -> jsg::Result<v8::Local<'s, v8::Uint8Array>> {
        let input = input.map_or_else(|| v8::String::empty(lock), Into::into);
        encode_utf8(lock, input)
    }

    /// Encodes as much of `input`, converted with `ToString`, as fits in `dest`, never splitting
    /// a code point.
    ///
    /// # Errors
    ///
    /// Fails only if `input` cannot be converted to a string (e.g. a `Symbol`).
    #[jsg_method]
    pub fn encode_into<'s>(
        &self,
        lock: &mut jsg::Lock<'s>,
        input: v8::Local<'s, v8::Value>,
        dest: v8::Local<'s, v8::Uint8Array>,
    ) -> jsg::Result<EncodeIntoResult> {
        let input = input.to_js_string(lock)?;
        let mut dest: v8::Local<'_, v8::ArrayBufferView> = dest.into();
        let (read, written) = encode_utf8_prefix(lock, input, &mut dest);
        // V8 strings are shorter than 2^30 code units, and each one encodes to at most three
        // bytes, so both counts fit in a u32.
        Ok(EncodeIntoResult {
            read: u32::try_from(read).unwrap_or(u32::MAX),
            written: u32::try_from(written).unwrap_or(u32::MAX),
        })
    }

    /// Always `"utf-8"`, the only encoding the standard allows.
    #[jsg_property(readonly)]
    pub fn get_encoding(&self) -> String {
        "utf-8".to_owned()
    }
}

impl jsg::Resource for TextEncoder {
    fn members(registry: &mut impl MemberRegistry, flags: compatibility_flags::Reader<'_>) {
        let placement = if flags.get_jsg_property_on_prototype_template() {
            PropertyPlacement::Prototype
        } else {
            PropertyPlacement::Instance
        };
        registry.add(Self::constructor_member());
        registry.add(Self::encode_member());
        registry.add(Self::encode_into_member());
        registry.add(Self::get_encoding_member(placement));
    }
}

/// The longest scratch space kept on the stack rather than the heap, in elements, as in C++.
const MAX_STACK_UNITS: usize = 4096;

/// Calls `f` with `length` zeroed elements of scratch space.
fn with_scratch<T: Copy + Default, R>(length: usize, f: impl FnOnce(&mut [T]) -> R) -> R {
    if length > MAX_STACK_UNITS {
        return f(&mut vec![T::default(); length]);
    }
    let mut stack = [MaybeUninit::<T>::uninit(); MAX_STACK_UNITS];
    let scratch = &mut stack[..length];
    for unit in &mut *scratch {
        unit.write(T::default());
    }
    // SAFETY: each of the `length` elements was just initialized.
    f(unsafe { std::slice::from_raw_parts_mut(scratch.as_mut_ptr().cast::<T>(), length) })
}

/// Encodes `input` as UTF-8, replacing lone surrogates with U+FFFD, as
/// `encodeUtf8WithSimdutf()` in `api/encoding.c++` does, except that it reads a one-byte string
/// in place twice, to measure it and then to encode it, where C++ encodes a copy.
fn encode_utf8<'s>(
    lock: &mut jsg::Lock<'s>,
    input: v8::Local<'s, v8::String>,
) -> jsg::Result<v8::Local<'s, v8::Uint8Array>> {
    let mut stack = [MaybeUninit::<u16>::uninit(); MAX_STACK_UNITS];
    let mut heap = Vec::new();
    // simdutf's bounded UTF-16 conversion is slower than copying the input to convert it
    // unbounded, so a two-byte string is copied, into `stack` if it fits.
    let first = input.with_value_view(lock, |view| match view {
        v8::StringView::OneByte(latin1) => {
            // SAFETY: `latin1` is valid for its length.
            let length =
                unsafe { ffi::simdutf_utf8_length_from_latin1(latin1.as_ptr(), latin1.len()) };
            Read::Latin1 {
                utf8_length: length,
            }
        }
        v8::StringView::TwoByte(utf16) => {
            if utf16.len() > MAX_STACK_UNITS {
                heap.extend_from_slice(utf16);
            } else {
                // SAFETY: `stack` has room for `utf16`, and they do not overlap.
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        utf16.as_ptr(),
                        stack.as_mut_ptr().cast::<u16>(),
                        utf16.len(),
                    );
                }
            }
            Read::Utf16 {
                length: utf16.len(),
            }
        }
    });
    match first {
        Read::Latin1 { utf8_length } => encode_latin1_in_place(lock, input, utf8_length),
        Read::Utf16 { length } => {
            let utf16 = if length > MAX_STACK_UNITS {
                &mut heap[..]
            } else {
                // SAFETY: the first `length` elements of `stack` were just copied to.
                unsafe { std::slice::from_raw_parts_mut(stack.as_mut_ptr().cast::<u16>(), length) }
            };
            encode_utf16_copy(lock, utf16)
        }
    }
}

/// What [`encode_utf8`] learns from its first read of its input.
enum Read {
    /// The input is one-byte, with this UTF-8 length.
    Latin1 { utf8_length: usize },
    /// The input is two-byte, and its `length` code units were copied.
    Utf16 { length: usize },
}

/// Returns a new zeroed `ArrayBuffer` of `length` bytes, which JavaScript cannot reach yet.
fn new_output_buffer<'s>(
    lock: &mut jsg::Lock<'s>,
    length: usize,
) -> jsg::Result<v8::Local<'s, v8::ArrayBuffer>> {
    v8::ArrayBuffer::new_with_mode(
        lock,
        length,
        v8::ffi::BackingStoreInitializationMode::ZeroInitialized,
    )
    .ok_or_else(|| jsg::Error::new_range_error("Failed to allocate memory for Uint8Array"))
}

/// Encodes the one-byte string `input`, measured at `utf8_length` bytes, reading it in place.
fn encode_latin1_in_place<'s>(
    lock: &mut jsg::Lock<'s>,
    input: v8::Local<'s, v8::String>,
    utf8_length: usize,
) -> jsg::Result<v8::Local<'s, v8::Uint8Array>> {
    let mut buffer = new_output_buffer(lock, utf8_length)?;
    // SAFETY: `buffer` was just created, and JavaScript cannot reach it before this function
    // returns, so no other reference into it exists.
    let output = unsafe { buffer.as_mut_slice(lock) };
    let (data, capacity) = (output.as_mut_ptr(), output.len());
    input.with_value_view(lock, |view| {
        // SAFETY: as above. `output` is not used again, and `with_value_view` borrows `lock`
        // only to keep JavaScript from running.
        let output = unsafe { std::slice::from_raw_parts_mut(data, capacity) };
        // The string is the one just measured unless it was corrupted in the V8 sandbox, so the
        // conversion is bounded by `output` rather than trusting the measurement. A two-byte
        // string, which only corruption could make it, is left as zeros.
        match view {
            v8::StringView::OneByte(latin1) if latin1.len() == capacity => {
                // ASCII, which is the same in Latin-1 and UTF-8.
                output.copy_from_slice(latin1);
            }
            v8::StringView::OneByte(latin1) => {
                let mut read = 0;
                // SAFETY: `latin1` and `output` are valid for their lengths, and the conversion
                // writes no more than `capacity` bytes.
                unsafe {
                    ffi::simdutf_convert_latin1_to_utf8_safe(
                        latin1.as_ptr(),
                        latin1.len(),
                        data,
                        capacity,
                        &mut read,
                    );
                }
            }
            v8::StringView::TwoByte(_) => {}
        }
    });
    v8::Uint8Array::from_buffer(lock, &buffer, 0, utf8_length)
}

/// Encodes `utf16`, a copy of a two-byte string, which it may overwrite.
fn encode_utf16_copy<'s>(
    lock: &mut jsg::Lock<'s>,
    utf16: &mut [u16],
) -> jsg::Result<v8::Local<'s, v8::Uint8Array>> {
    let mut has_surrogates = false;
    // SAFETY: `utf16` is valid for its length.
    let utf8_length = unsafe {
        ffi::simdutf_utf8_length_from_utf16_with_replacement(
            utf16.as_ptr(),
            utf16.len(),
            &mut has_surrogates,
        )
    };
    if has_surrogates {
        // SAFETY: `utf16` is valid for its length, and simdutf allows the conversion in place.
        unsafe {
            ffi::simdutf_to_well_formed_utf16(utf16.as_ptr(), utf16.len(), utf16.as_mut_ptr());
        };
    }
    let mut buffer = new_output_buffer(lock, utf8_length)?;
    // SAFETY: as in `encode_latin1_in_place`.
    let output = unsafe { buffer.as_mut_slice(lock) };
    // SAFETY: `utf16` is now well-formed, and `output` has room for its UTF-8 encoding, measured
    // with each lone surrogate as U+FFFD. The copy is not in the V8 sandbox, so it cannot change.
    unsafe { ffi::simdutf_convert_utf16_to_utf8(utf16.as_ptr(), utf16.len(), output.as_mut_ptr()) };
    v8::Uint8Array::from_buffer(lock, &buffer, 0, utf8_length)
}

/// Encodes as much of `input` as fits in `dest`, as `TextEncoder::encodeInto()` in
/// `api/encoding.c++` does. Returns the code units read and bytes written.
fn encode_utf8_prefix<'s>(
    lock: &mut jsg::Lock<'s>,
    input: v8::Local<'s, v8::String>,
    dest: &mut v8::Local<'s, v8::ArrayBufferView>,
) -> (usize, usize) {
    // SAFETY: no other reference into `dest`'s buffer exists while `output` is live: the input
    // is a string, and no JavaScript runs until this function returns.
    let output = unsafe { dest.as_mut_slice(lock) };
    let (data, capacity) = (output.as_mut_ptr(), output.len());
    input.with_value_view(lock, |view| {
        // SAFETY: as above. `output` is not used again, and `with_value_view` borrows `lock`
        // only to keep JavaScript from running.
        let output = unsafe { std::slice::from_raw_parts_mut(data, capacity) };
        match view {
            v8::StringView::OneByte(input) => encode_latin1_prefix(input, output),
            v8::StringView::TwoByte(input) => encode_utf16_prefix(input, output),
        }
    })
}

/// Encodes as much of the Latin-1 `input` as fits in `output`, as `encodeLatin1Prefix()` in
/// `api/encoding.c++` does. Returns the code units read and bytes written.
fn encode_latin1_prefix(input: &[u8], output: &mut [u8]) -> (usize, usize) {
    // SAFETY: `input` is valid for at least this many bytes.
    let ascii =
        unsafe { ffi::simdutf_ascii_prefix_length(input.as_ptr(), input.len().min(output.len())) };
    output[..ascii].copy_from_slice(&input[..ascii]);
    let rest = &input[ascii..];
    let output = &mut output[ascii..];
    if rest.is_empty() || output.is_empty() {
        return (ascii, ascii);
    }
    if output.len() / 2 >= rest.len() {
        // SAFETY: a Latin-1 character takes at most two bytes, so `output` has room for `rest`.
        let written = unsafe {
            ffi::simdutf_convert_latin1_to_utf8(rest.as_ptr(), rest.len(), output.as_mut_ptr())
        };
        return (input.len(), ascii + written);
    }
    let mut read = 0;
    // SAFETY: `rest` and `output` are valid for their lengths, and the conversion writes no more
    // than `output.len()` bytes.
    let written = unsafe {
        ffi::simdutf_convert_latin1_to_utf8_safe(
            rest.as_ptr(),
            rest.len(),
            output.as_mut_ptr(),
            output.len(),
            &mut read,
        )
    };
    (ascii + read, ascii + written)
}

/// Encodes as much of the UTF-16 `input` as fits in `output`, replacing lone surrogates with
/// U+FFFD, as `encodeUtf16Prefix()` in `api/encoding.c++` does. Returns the code units read and
/// bytes written.
fn encode_utf16_prefix(input: &[u16], output: &mut [u8]) -> (usize, usize) {
    let (read, written) = if output.len() / 3 >= input.len() {
        let mut ok = false;
        // SAFETY: a code unit takes at most three bytes, so `output` has room for `input`.
        let count = unsafe {
            ffi::simdutf_convert_utf16_to_utf8_with_errors(
                input.as_ptr(),
                input.len(),
                output.as_mut_ptr(),
                &mut ok,
            )
        };
        if ok {
            return (input.len(), count);
        }
        // The output before the error is complete, which simdutf's bounded conversions also
        // rely on.
        // SAFETY: `count`, the position of the error, is within `input`.
        (count, unsafe {
            ffi::simdutf_utf8_length_from_utf16(input.as_ptr(), count)
        })
    } else {
        let (mut read, mut written) = (0, 0);
        // SAFETY: `input` and `output` are valid for their lengths, and the conversion writes no
        // more than `output.len()` bytes.
        let error = unsafe {
            ffi::simdutf_convert_utf16_to_utf8_safe(
                input.as_ptr(),
                input.len(),
                output.as_mut_ptr(),
                output.len(),
                &mut read,
                &mut written,
            )
        };
        if error == SIMDUTF_OUTPUT_BUFFER_TOO_SMALL {
            // simdutf reports a lone lead surrogate with three bytes left as too small rather
            // than as a surrogate, which would drop a U+FFFD that fits.
            let rest_in = &input[read..];
            let rest_out = &mut output[written..];
            let mut tail_read = 0;
            // SAFETY: as above, for the rest of `input` and `output`.
            let tail_written = unsafe {
                ffi::simdutf_convert_utf16_to_utf8_with_replacement_safe(
                    rest_in.as_ptr(),
                    rest_in.len(),
                    rest_out.as_mut_ptr(),
                    rest_out.len(),
                    &mut tail_read,
                )
            };
            return (read + tail_read, written + tail_written);
        }
        if error != SIMDUTF_SURROGATE {
            return (read, written);
        }
        (read, written)
    };
    // A lone surrogate becomes U+FFFD in place, so a well-formed copy reads and writes the same
    // counts. simdutf's replacing conversions restart at every lone surrogate, which is far
    // slower when they are dense. Each code unit takes at least one byte, so at most
    // `rest_out.len()` of them are read, and one more keeps a pair on that boundary whole.
    let rest_out = &mut output[written..];
    let length = (input.len() - read).min(rest_out.len() + 1);
    let rest_in = &input[read..read + length];
    with_scratch(length, |well_formed: &mut [u16]| {
        // SAFETY: `rest_in` and `well_formed` are valid for `length` code units.
        unsafe {
            ffi::simdutf_to_well_formed_utf16(rest_in.as_ptr(), length, well_formed.as_mut_ptr());
        }
        if rest_out.len() / 3 >= length {
            // SAFETY: `well_formed` is well-formed, and `rest_out` has room for three bytes per
            // code unit.
            let tail_written = unsafe {
                ffi::simdutf_convert_utf16_to_utf8(
                    well_formed.as_ptr(),
                    length,
                    rest_out.as_mut_ptr(),
                )
            };
            return (read + length, written + tail_written);
        }
        let (mut tail_read, mut tail_written) = (0, 0);
        // SAFETY: as above; the conversion writes no more than `rest_out.len()` bytes.
        unsafe {
            ffi::simdutf_convert_utf16_to_utf8_safe(
                well_formed.as_ptr(),
                length,
                rest_out.as_mut_ptr(),
                rest_out.len(),
                &mut tail_read,
                &mut tail_written,
            );
        }
        (read + tail_read, written + tail_written)
    })
}

/// Returns the `TextEncoder` constructor's `v8::Local<v8::FunctionTemplate>`.
///
/// # Safety
///
/// `isolate` must be valid, locked, and have a Rust `Realm`. Its innermost `HandleScope`
/// must stay open and innermost until C++ consumes the returned handle.
pub unsafe fn text_encoder_template(isolate: *mut ffi::Isolate) -> ffi::Local {
    // SAFETY: the caller guarantees `isolate` is valid and locked.
    let js = unsafe { jsg::Isolate::from_raw(isolate) };
    let template = jsg::resource::template_of::<TextEncoder>(js);
    // SAFETY: the template belongs to this isolate, and C++ keeps its HandleScope open.
    unsafe { v8::ffi::global_to_local(isolate, &template.as_ffi()) }
}

#[cfg(test)]
#[path = "text_encoder-test.rs"]
mod tests;
