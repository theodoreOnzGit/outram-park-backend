//! MPI datatypes for the primitive Rust numeric types.
//!
//! The MPI standard describes every message by the triple `(buffer, count,
//! datatype)`. Because messages of different element types share one untyped
//! mailbox in the [transport](crate::transport), this crate carries each message
//! as a byte buffer tagged with a [`Datatype`] and an element `count`; the
//! receiver decodes those bytes back into a typed slice, checking the tag so a
//! type mismatch is an error rather than silent reinterpretation (mirroring
//! `MPI_ERR_TYPE`).
//!
//! # Enum dispatch, not `dyn`
//!
//! Per the workspace rules the set of built-in datatypes is a closed [`Datatype`]
//! enum, matched exhaustively. The [`MpiPrimitive`] trait is a *compiler-checked
//! contract* on each concrete primitive (it maps the Rust type to its `Datatype`
//! tag and byte codec); it is never used as a trait object.
//!
//! # Provenance
//!
//! The built-in datatype set corresponds to the MPI-3.1 standard named predefined
//! datatypes (`MPI_INT32_T`, `MPI_DOUBLE`, …). Byte encoding is native-endian,
//! valid because this shared-memory transport never crosses machine boundaries; a
//! future TCP transport would negotiate/convert endianness.

/// A built-in MPI datatype tag — one per supported primitive Rust type.
///
/// The set is closed and matched exhaustively (no `dyn`). Each variant names the
/// element type carried in a message's byte buffer and its fixed size in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Datatype {
    /// `i8` — MPI `MPI_INT8_T`.
    I8,
    /// `i16` — MPI `MPI_INT16_T`.
    I16,
    /// `i32` — MPI `MPI_INT32_T`.
    I32,
    /// `i64` — MPI `MPI_INT64_T`.
    I64,
    /// `u8` — MPI `MPI_UINT8_T`.
    U8,
    /// `u16` — MPI `MPI_UINT16_T`.
    U16,
    /// `u32` — MPI `MPI_UINT32_T`.
    U32,
    /// `u64` — MPI `MPI_UINT64_T`.
    U64,
    /// `f32` — MPI `MPI_FLOAT`.
    F32,
    /// `f64` — MPI `MPI_DOUBLE`.
    F64,
}

impl Datatype {
    /// Size of one element of this datatype, in bytes.
    pub fn size(self) -> usize {
        match self {
            Datatype::I8 | Datatype::U8 => 1,
            Datatype::I16 | Datatype::U16 => 2,
            Datatype::I32 | Datatype::U32 | Datatype::F32 => 4,
            Datatype::I64 | Datatype::U64 | Datatype::F64 => 8,
        }
    }
}

/// Compiler-checked contract mapping a primitive Rust type to its [`Datatype`]
/// tag and a native-endian byte codec.
///
/// Implemented for the ten built-in numeric primitives. This is a bound on
/// generic message operations (`comm.send::<f64>(…)`), never a `dyn` object.
///
/// # Safety of the byte codec
///
/// [`MpiPrimitive::encode`]/[`MpiPrimitive::decode`] round-trip a slice through a
/// native-endian byte buffer. They are pure-safe Rust (`to_ne_bytes` /
/// `from_ne_bytes`), so there is no `unsafe`, no alignment hazard, and no
/// undefined behaviour on malformed input (a short trailing chunk is rejected).
pub trait MpiPrimitive: Copy + Send + Sync + 'static {
    /// The [`Datatype`] tag for this Rust type.
    const DATATYPE: Datatype;

    /// Append the native-endian bytes of one value to `out`.
    fn push_bytes(self, out: &mut Vec<u8>);

    /// Read one value from exactly [`Datatype::size`] native-endian bytes.
    fn from_bytes(chunk: &[u8]) -> Self;

    /// Encode a typed slice into a native-endian byte buffer.
    fn encode(data: &[Self]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len() * Self::DATATYPE.size());
        for &v in data {
            v.push_bytes(&mut out);
        }
        out
    }

    /// Decode a native-endian byte buffer back into a typed `Vec`.
    ///
    /// The buffer length must be an exact multiple of the element size; a partial
    /// trailing element is dropped (it can only arise from a corrupt transport,
    /// which the shared-memory backend never produces).
    fn decode(bytes: &[u8]) -> Vec<Self> {
        let sz = Self::DATATYPE.size();
        bytes.chunks_exact(sz).map(Self::from_bytes).collect()
    }
}

impl MpiPrimitive for i8 {
    const DATATYPE: Datatype = Datatype::I8;

    fn push_bytes(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_ne_bytes());
    }

    fn from_bytes(chunk: &[u8]) -> Self {
        let mut buf = [0u8; core::mem::size_of::<i8>()];
        buf.copy_from_slice(&chunk[..core::mem::size_of::<i8>()]);
        i8::from_ne_bytes(buf)
    }
}

impl MpiPrimitive for i16 {
    const DATATYPE: Datatype = Datatype::I16;

    fn push_bytes(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_ne_bytes());
    }

    fn from_bytes(chunk: &[u8]) -> Self {
        let mut buf = [0u8; core::mem::size_of::<i16>()];
        buf.copy_from_slice(&chunk[..core::mem::size_of::<i16>()]);
        i16::from_ne_bytes(buf)
    }
}

impl MpiPrimitive for i32 {
    const DATATYPE: Datatype = Datatype::I32;

    fn push_bytes(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_ne_bytes());
    }

    fn from_bytes(chunk: &[u8]) -> Self {
        let mut buf = [0u8; core::mem::size_of::<i32>()];
        buf.copy_from_slice(&chunk[..core::mem::size_of::<i32>()]);
        i32::from_ne_bytes(buf)
    }
}

impl MpiPrimitive for i64 {
    const DATATYPE: Datatype = Datatype::I64;

    fn push_bytes(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_ne_bytes());
    }

    fn from_bytes(chunk: &[u8]) -> Self {
        let mut buf = [0u8; core::mem::size_of::<i64>()];
        buf.copy_from_slice(&chunk[..core::mem::size_of::<i64>()]);
        i64::from_ne_bytes(buf)
    }
}

impl MpiPrimitive for u8 {
    const DATATYPE: Datatype = Datatype::U8;

    fn push_bytes(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_ne_bytes());
    }

    fn from_bytes(chunk: &[u8]) -> Self {
        let mut buf = [0u8; core::mem::size_of::<u8>()];
        buf.copy_from_slice(&chunk[..core::mem::size_of::<u8>()]);
        u8::from_ne_bytes(buf)
    }
}

impl MpiPrimitive for u16 {
    const DATATYPE: Datatype = Datatype::U16;

    fn push_bytes(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_ne_bytes());
    }

    fn from_bytes(chunk: &[u8]) -> Self {
        let mut buf = [0u8; core::mem::size_of::<u16>()];
        buf.copy_from_slice(&chunk[..core::mem::size_of::<u16>()]);
        u16::from_ne_bytes(buf)
    }
}

impl MpiPrimitive for u32 {
    const DATATYPE: Datatype = Datatype::U32;

    fn push_bytes(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_ne_bytes());
    }

    fn from_bytes(chunk: &[u8]) -> Self {
        let mut buf = [0u8; core::mem::size_of::<u32>()];
        buf.copy_from_slice(&chunk[..core::mem::size_of::<u32>()]);
        u32::from_ne_bytes(buf)
    }
}

impl MpiPrimitive for u64 {
    const DATATYPE: Datatype = Datatype::U64;

    fn push_bytes(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_ne_bytes());
    }

    fn from_bytes(chunk: &[u8]) -> Self {
        let mut buf = [0u8; core::mem::size_of::<u64>()];
        buf.copy_from_slice(&chunk[..core::mem::size_of::<u64>()]);
        u64::from_ne_bytes(buf)
    }
}

impl MpiPrimitive for f32 {
    const DATATYPE: Datatype = Datatype::F32;

    fn push_bytes(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_ne_bytes());
    }

    fn from_bytes(chunk: &[u8]) -> Self {
        let mut buf = [0u8; core::mem::size_of::<f32>()];
        buf.copy_from_slice(&chunk[..core::mem::size_of::<f32>()]);
        f32::from_ne_bytes(buf)
    }
}

impl MpiPrimitive for f64 {
    const DATATYPE: Datatype = Datatype::F64;

    fn push_bytes(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_ne_bytes());
    }

    fn from_bytes(chunk: &[u8]) -> Self {
        let mut buf = [0u8; core::mem::size_of::<f64>()];
        buf.copy_from_slice(&chunk[..core::mem::size_of::<f64>()]);
        f64::from_ne_bytes(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datatype_sizes_match_rust() {
        assert_eq!(Datatype::I8.size(), 1);
        assert_eq!(Datatype::F32.size(), 4);
        assert_eq!(Datatype::F64.size(), 8);
        assert_eq!(Datatype::U64.size(), 8);
    }

    #[test]
    fn encode_decode_roundtrips_f64() {
        let data = [1.5_f64, -2.0, 3.25, 0.0, 1e300];
        let bytes = f64::encode(&data);
        assert_eq!(bytes.len(), data.len() * 8);
        let back = f64::decode(&bytes);
        assert_eq!(back, data);
    }

    #[test]
    fn encode_decode_roundtrips_i32() {
        let data = [0_i32, -1, 2, i32::MAX, i32::MIN];
        let back = i32::decode(&i32::encode(&data));
        assert_eq!(back, data);
    }

    #[test]
    fn datatype_tag_is_correct() {
        assert_eq!(<f64 as MpiPrimitive>::DATATYPE, Datatype::F64);
        assert_eq!(<i32 as MpiPrimitive>::DATATYPE, Datatype::I32);
        assert_eq!(<u8 as MpiPrimitive>::DATATYPE, Datatype::U8);
    }

    #[test]
    fn empty_slice_roundtrips() {
        let data: [f32; 0] = [];
        assert!(f32::encode(&data).is_empty());
        assert!(f32::decode(&[]).is_empty());
    }
}
