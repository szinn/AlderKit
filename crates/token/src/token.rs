use std::{fmt, hash::Hash, marker::PhantomData, str::FromStr};

use rand::RngExt;
use serde::{Deserialize, Serialize, de};
use thiserror::Error;

/// Build a reverse lookup table (ASCII byte → alphabet index, or `0xFF` for
/// invalid) for a given base-32 alphabet. Covers the full range `0..=b'Z'`
/// (91 entries).
#[expect(clippy::cast_possible_truncation, reason = "alphabet has 32 entries; i is 0..31, always fits u8")]
const fn build_decode_table(alphabet: &[u8; 32]) -> [u8; 91] {
    let mut table = [0xFF_u8; 91];
    let mut i = 0;
    while i < alphabet.len() {
        table[alphabet[i] as usize] = i as u8;
        i += 1;
    }
    table
}

/// Panics unless `alphabet` contains exactly 32 distinct ASCII bytes, each no
/// greater than `b'Z'` (so it fits within [`build_decode_table`]'s range).
/// Intended to be called from a `const` context, turning an invalid alphabet
/// into a compile error.
#[doc(hidden)]
pub const fn __assert_valid_alphabet(alphabet: &[u8; 32]) {
    let mut i = 0;
    while i < 32 {
        let byte = alphabet[i];
        assert!(byte.is_ascii(), "alphabet must contain only ASCII bytes");
        assert!(byte <= b'Z', "alphabet bytes must not exceed 'Z'");
        let mut j = i + 1;
        while j < 32 {
            assert!(byte != alphabet[j], "alphabet must contain 32 distinct bytes");
            j += 1;
        }
        i += 1;
    }
}

/// Trait that defines the base-32 alphabet used to encode and decode token
/// IDs.
///
/// [`DefaultAlphabet`] is the built-in alphabet and is used unless a [`Token`]
/// names a different one. Custom alphabets should be defined with the
/// [`define_alphabet!`] macro, which validates them at compile time.
pub trait Alphabet: fmt::Debug + Clone + PartialEq + Eq {
    /// 32 distinct ASCII bytes (each `<= b'Z'`) used as the encoding alphabet.
    const ALPHABET: &'static [u8; 32];

    /// Reverse lookup table derived from [`Self::ALPHABET`].
    const DECODE_TABLE: [u8; 91] = build_decode_table(Self::ALPHABET);
}

/// Base-32 alphabet excluding visually ambiguous characters (I, L, O, Q).
///
/// The default [`Alphabet`] used by [`Token`] when none is specified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DefaultAlphabet;

impl Alphabet for DefaultAlphabet {
    const ALPHABET: &'static [u8; 32] = b"Y4XK0N8AR3G6JM2VT9BS5WC1DPH7EUZF";
}

/// Trait that defines the prefix for a token kind.
pub trait TokenPrefix: fmt::Debug + Clone + PartialEq + Eq {
    const PREFIX: &'static str;
}

/// Trait that abstracts encode/decode over the numeric backing type.
pub trait TokenId: Copy + PartialEq + Eq + Hash + fmt::Debug {
    /// The zero value for this type.
    const ZERO: Self;

    /// Length of the encoded identifier portion (excluding the prefix).
    const ENCODED_LEN: usize;

    /// Encode this value into a base-32 string of [`Self::ENCODED_LEN`]
    /// characters, using alphabet `A`.
    fn encode<A: Alphabet>(self) -> String;

    /// Encode this value directly into a byte buffer, using alphabet `A`.
    /// The buffer must be exactly [`Self::ENCODED_LEN`] bytes. All bytes
    /// will be valid ASCII.
    fn encode_to_buf<A: Alphabet>(self, buf: &mut [u8]);

    /// Decode a base-32 string back into this numeric type, using alphabet
    /// `A`.
    fn decode<A: Alphabet>(s: &str) -> Result<Self, TokenError>;

    /// Generate a random value in `1..=max` where `max` is provided as a
    /// `u128` (from the const generic on [`Token`]).
    fn random_in_range(max: u128) -> Self;
}

impl TokenId for u64 {
    const ZERO: Self = 0;
    const ENCODED_LEN: usize = 13; // 32^13 > u64::MAX

    fn random_in_range(max: u128) -> Self {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "MAX const for u64 tokens is bounded by u64::MAX; caller guarantees max fits in u64"
        )]
        let max_u64 = max as Self;
        rand::rng().random_range(1..=max_u64)
    }

    fn encode<A: Alphabet>(self) -> String {
        let mut buf = [0u8; Self::ENCODED_LEN];
        self.encode_to_buf::<A>(&mut buf);
        String::from_utf8(buf.to_vec()).expect("alphabet is ASCII")
    }

    fn encode_to_buf<A: Alphabet>(self, buf: &mut [u8]) {
        let mut remaining = self;
        for i in (0..Self::ENCODED_LEN).rev() {
            buf[i] = A::ALPHABET[(remaining & 0x1F) as usize];
            remaining >>= 5;
        }
    }

    fn decode<A: Alphabet>(s: &str) -> Result<Self, TokenError> {
        let mut value: Self = 0;
        for ch in s.chars() {
            let byte = ch as usize;
            let idx = if byte < A::DECODE_TABLE.len() { A::DECODE_TABLE[byte] } else { 0xFF };
            if idx == 0xFF {
                return Err(TokenError::InvalidCharacter(ch));
            }
            value = value.checked_shl(5).and_then(|v| v.checked_add(Self::from(idx))).ok_or(TokenError::Overflow)?;
        }
        Ok(value)
    }
}

impl TokenId for u128 {
    const ZERO: Self = 0;
    const ENCODED_LEN: usize = 26; // 32^26 > u128::MAX

    fn random_in_range(max: u128) -> Self {
        rand::rng().random_range(1..=max)
    }

    fn encode<A: Alphabet>(self) -> String {
        let mut buf = [0u8; Self::ENCODED_LEN];
        self.encode_to_buf::<A>(&mut buf);
        String::from_utf8(buf.to_vec()).expect("alphabet is ASCII")
    }

    fn encode_to_buf<A: Alphabet>(self, buf: &mut [u8]) {
        let mut remaining = self;
        for i in (0..Self::ENCODED_LEN).rev() {
            buf[i] = A::ALPHABET[(remaining & 0x1F) as usize];
            remaining >>= 5;
        }
    }

    fn decode<A: Alphabet>(s: &str) -> Result<Self, TokenError> {
        let mut value: Self = 0;
        for ch in s.chars() {
            let byte = ch as usize;
            let idx = if byte < A::DECODE_TABLE.len() { A::DECODE_TABLE[byte] } else { 0xFF };
            if idx == 0xFF {
                return Err(TokenError::InvalidCharacter(ch));
            }
            value = value.checked_shl(5).and_then(|v| v.checked_add(Self::from(idx))).ok_or(TokenError::Overflow)?;
        }
        Ok(value)
    }
}

/// A typed, prefixed identifier for domain entities.
///
/// Stores only the numeric ID internally. The string representation (e.g.
/// `U_ABCD1234NRST0`) is computed on demand via [`fmt::Display`].
///
/// The `MAX` const generic controls the upper bound for random ID generation
/// via [`Token::generate`]. This allows token types to cap their range (e.g.
/// to `i64::MAX` for database-safe storage) without changing the backing type.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Token<P: TokenPrefix, I: TokenId = u64, A: Alphabet = DefaultAlphabet, const MAX: u128 = { u64::MAX as u128 }> {
    id: I,
    _marker: PhantomData<(P, A)>,
}

impl<P: TokenPrefix, I: TokenId, A: Alphabet, const MAX: u128> fmt::Debug for Token<P, I, A, MAX> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Token({self})")
    }
}

/// Errors that can occur when parsing a token from a string.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TokenError {
    #[error("invalid prefix: expected \"{expected}\", found \"{found}\"")]
    InvalidPrefix { expected: &'static str, found: String },

    #[error("invalid length: expected {expected}, found {found}")]
    InvalidLength { expected: usize, found: usize },

    #[error("invalid character: '{0}'")]
    InvalidCharacter(char),

    #[error("encoded value overflow")]
    Overflow,
}

impl<P: TokenPrefix, I: TokenId, A: Alphabet, const MAX: u128> Token<P, I, A, MAX> {
    /// Create a token from a numeric ID.
    pub fn new(id: I) -> Self {
        Self { id, _marker: PhantomData }
    }

    /// Generate a new token with a random ID in `1..=MAX`.
    #[must_use]
    pub fn generate() -> Self {
        Self::new(I::random_in_range(MAX))
    }

    /// Parse a token from its string representation (e.g. `"U_ABCD1234NRST0"`).
    pub fn parse(s: &str) -> Result<Self, TokenError> {
        let prefix = P::PREFIX;
        if !s.starts_with(prefix) {
            let found_len = s.len().min(prefix.len());
            return Err(TokenError::InvalidPrefix {
                expected: prefix,
                found: s[..found_len].to_string(),
            });
        }

        let encoded = &s[prefix.len()..];
        if encoded.len() != I::ENCODED_LEN {
            return Err(TokenError::InvalidLength {
                expected: prefix.len() + I::ENCODED_LEN,
                found: s.len(),
            });
        }

        let id = I::decode::<A>(encoded)?;
        Ok(Self::new(id))
    }

    /// Get the underlying numeric ID.
    pub fn id(&self) -> I {
        self.id
    }

    /// Returns the encoded portion of the token string without the prefix.
    ///
    /// This is the inverse of [`Token::from_encoded_id`].
    pub fn encoded_id(&self) -> String {
        self.id.encode::<A>()
    }

    /// Parse a token from the encoded portion alone (no prefix).
    ///
    /// Equivalent to parsing `"{PREFIX}{s}"` but without constructing the
    /// intermediate string. Returns an error if `s` has the wrong length or
    /// contains invalid characters.
    pub fn from_encoded_id(s: &str) -> Result<Self, TokenError> {
        if s.len() != I::ENCODED_LEN {
            return Err(TokenError::InvalidLength {
                expected: P::PREFIX.len() + I::ENCODED_LEN,
                found: P::PREFIX.len() + s.len(),
            });
        }
        let id = I::decode::<A>(s)?;
        Ok(Self::new(id))
    }

    /// Check if a string is a well-formed token of this type.
    #[must_use]
    pub fn is_valid(s: &str) -> bool {
        Self::parse(s).is_ok()
    }
}

impl<P: TokenPrefix, I: TokenId, A: Alphabet, const MAX: u128> fmt::Display for Token<P, I, A, MAX> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(P::PREFIX)?;
        let mut buf = [0u8; 26]; // max encoded length (u128)
        let buf = &mut buf[..I::ENCODED_LEN];
        self.id.encode_to_buf::<A>(buf);
        f.write_str(std::str::from_utf8(buf).expect("alphabet is ASCII"))
    }
}

impl<P: TokenPrefix, I: TokenId, A: Alphabet, const MAX: u128> FromStr for Token<P, I, A, MAX> {
    type Err = TokenError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl<P: TokenPrefix, I: TokenId, A: Alphabet, const MAX: u128> Serialize for Token<P, I, A, MAX> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de, P: TokenPrefix, I: TokenId, A: Alphabet, const MAX: u128> Deserialize<'de> for Token<P, I, A, MAX> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).map_err(de::Error::custom)
    }
}

/// Define a token prefix type and its associated `TokenPrefix` implementation.
///
/// # Example
///
/// ```
/// use alderkit_token::{define_token_prefix, token::Token};
///
/// define_token_prefix!(UserPrefix, "U_");
/// type UserId = u64;
/// type UserToken = Token<UserPrefix>;          // u64 (default), MAX = u64::MAX
///
/// define_token_prefix!(SessionPrefix, "S_");
/// type SessionId = u128;
/// type SessionToken = Token<SessionPrefix, SessionId>;
/// ```
#[macro_export]
macro_rules! define_token_prefix {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name;

        impl $crate::token::TokenPrefix for $name {
            const PREFIX: &'static str = $prefix;
        }
    };
}

/// Define a custom [`Alphabet`] type from a 32-byte base-32 alphabet.
///
/// The alphabet is validated at compile time: it must contain exactly 32
/// distinct ASCII bytes, each no greater than `b'Z'`. An invalid alphabet
/// fails the build rather than misbehaving at runtime.
///
/// # Example
///
/// ```
/// use alderkit_token::{define_alphabet, define_token_prefix, token::Token};
///
/// define_alphabet!(MyAlphabet, b"0123456789ABCDEFGHJKMNPQRSTVWXYZ");
/// define_token_prefix!(UserPrefix, "U_");
/// type UserToken = Token<UserPrefix, u64, MyAlphabet>;
/// ```
#[macro_export]
macro_rules! define_alphabet {
    ($name:ident, $alphabet:expr) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name;

        impl $crate::token::Alphabet for $name {
            const ALPHABET: &'static [u8; 32] = {
                $crate::token::__assert_valid_alphabet($alphabet);
                $alphabet
            };
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    define_token_prefix!(TestPrefix, "T_");
    type TestToken = Token<TestPrefix>;

    define_token_prefix!(UserPrefix, "U_");
    type UserId = u64;
    type UserToken = Token<UserPrefix, UserId>;

    // --- u64 tests (unchanged, use default type parameter) ---

    #[test]
    fn round_trip() {
        for id in [0, 1, 42, 1000, 123_456_789, u64::MAX] {
            let token = TestToken::new(id);
            let s = token.to_string();
            let parsed = TestToken::parse(&s).unwrap();
            assert_eq!(parsed.id(), id);
        }
    }

    #[test]
    fn zero_encodes_to_all_first_char() {
        let token = TestToken::new(0);
        assert_eq!(token.to_string(), "T_YYYYYYYYYYYYY");
    }

    #[test]
    fn u64_max_round_trips() {
        let token = TestToken::new(u64::MAX);
        let s = token.to_string();
        let parsed = TestToken::parse(&s).unwrap();
        assert_eq!(parsed.id(), u64::MAX);
    }

    #[test]
    fn known_value_encoding() {
        let token = TestToken::new(1);
        let s = token.to_string();
        assert_eq!(s, "T_YYYYYYYYYYYY4");
    }

    #[test]
    fn wrong_prefix_error() {
        let err = UserToken::parse("T_AAAAAAAAAAAAA").unwrap_err();
        assert_eq!(
            err,
            TokenError::InvalidPrefix {
                expected: "U_",
                found: "T_".to_string(),
            }
        );
    }

    #[test]
    fn invalid_character_error() {
        // 'I' is not in the alphabet
        let err = TestToken::parse("T_AAAAAAAAAAAIA").unwrap_err();
        assert_eq!(err, TokenError::InvalidCharacter('I'));
    }

    #[test]
    fn excluded_characters_rejected() {
        for ch in ['I', 'L', 'O', 'Q'] {
            let s = format!("T_AAAAAAAAAAAA{ch}");
            let err = TestToken::parse(&s).unwrap_err();
            assert_eq!(err, TokenError::InvalidCharacter(ch));
        }
    }

    #[test]
    fn wrong_length_error() {
        let err = TestToken::parse("T_AAAA").unwrap_err();
        assert_eq!(err, TokenError::InvalidLength { expected: 15, found: 6 });
    }

    #[test]
    fn is_valid_returns_true_for_valid() {
        let s = TestToken::new(42).to_string();
        assert!(TestToken::is_valid(&s));
    }

    #[test]
    fn is_valid_returns_false_for_invalid() {
        assert!(!TestToken::is_valid("INVALID"));
        assert!(!TestToken::is_valid("T_SHORT"));
        assert!(!TestToken::is_valid("X_AAAAAAAAAAAAA"));
    }

    #[test]
    fn from_str_works() {
        let s = TestToken::new(99).to_string();
        let parsed: TestToken = s.parse().unwrap();
        assert_eq!(parsed.id(), 99);
    }

    #[test]
    fn encoded_id_round_trips() {
        let token = TestToken::new(42);
        let enc = token.encoded_id();
        assert_eq!(enc.len(), 13);
        assert!(!enc.starts_with("T_"));
        let parsed = TestToken::from_encoded_id(&enc).unwrap();
        assert_eq!(parsed.id(), 42);
    }

    #[test]
    fn from_encoded_id_rejects_wrong_length() {
        let err = TestToken::from_encoded_id("SHORT").unwrap_err();
        assert!(matches!(err, TokenError::InvalidLength { .. }));
    }

    #[test]
    fn from_encoded_id_rejects_invalid_char() {
        let err = TestToken::from_encoded_id("AAAAAAAAAAAAI").unwrap_err();
        assert_eq!(err, TokenError::InvalidCharacter('I'));
    }

    #[test]
    fn encoded_id_does_not_include_prefix() {
        let token = TestToken::new(1);
        assert_eq!(token.encoded_id(), "YYYYYYYYYYYY4");
    }

    #[test]
    fn different_prefix_types_are_distinct() {
        let test_s = TestToken::new(42).to_string();
        let user_s = UserToken::new(42).to_string();
        assert_ne!(test_s, user_s);
        assert!(test_s.starts_with("T_"));
        assert!(user_s.starts_with("U_"));
    }

    #[test]
    fn serde_round_trip() {
        let token = TestToken::new(123_456);
        let json = serde_json::to_string(&token).unwrap();
        let parsed: TestToken = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id(), 123_456);
    }

    #[test]
    fn serde_serializes_as_string() {
        let token = TestToken::new(0);
        let json = serde_json::to_string(&token).unwrap();
        assert_eq!(json, r#""T_YYYYYYYYYYYYY""#);
    }

    #[test]
    fn serde_rejects_invalid_token() {
        let result = serde_json::from_str::<TestToken>(r#""INVALID""#);
        result.unwrap_err();
    }

    #[test]
    fn debug_format() {
        let token = TestToken::new(0);
        let debug = format!("{token:?}");
        assert_eq!(debug, "Token(T_YYYYYYYYYYYYY)");
    }

    // --- u128 tests ---

    define_token_prefix!(BigPrefix, "B_");
    type BigToken = Token<BigPrefix, u128>;

    #[test]
    fn u128_round_trip() {
        for id in [0u128, 1, u128::from(u64::MAX), u128::MAX] {
            let token = BigToken::new(id);
            let s = token.to_string();
            let parsed = BigToken::parse(&s).unwrap();
            assert_eq!(parsed.id(), id);
        }
    }

    #[test]
    fn u128_zero_encodes_to_26_as() {
        let token = BigToken::new(0);
        assert_eq!(token.to_string(), "B_YYYYYYYYYYYYYYYYYYYYYYYYYY");
    }

    #[test]
    fn u128_max_round_trips() {
        let token = BigToken::new(u128::MAX);
        let s = token.to_string();
        let parsed = BigToken::parse(&s).unwrap();
        assert_eq!(parsed.id(), u128::MAX);
    }

    #[test]
    fn u128_known_value_encoding() {
        let token = BigToken::new(1);
        let s = token.to_string();
        // 25 Y's + 4
        assert_eq!(s, "B_YYYYYYYYYYYYYYYYYYYYYYYYY4");
    }

    #[test]
    fn u128_wrong_length_error() {
        // prefix (2) + encoded (26) = 28
        let err = BigToken::parse("B_AAAA").unwrap_err();
        assert_eq!(err, TokenError::InvalidLength { expected: 28, found: 6 });
    }

    #[test]
    fn u128_serde_round_trip() {
        let token = BigToken::new(123_456);
        let json = serde_json::to_string(&token).unwrap();
        let parsed: BigToken = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id(), 123_456);
    }

    #[test]
    fn u128_debug_format() {
        let token = BigToken::new(0);
        let debug = format!("{token:?}");
        assert_eq!(debug, "Token(B_YYYYYYYYYYYYYYYYYYYYYYYYYY)");
    }

    // --- custom MAX tests ---

    define_token_prefix!(CappedPrefix, "C_");
    type CappedToken = Token<CappedPrefix, u64, DefaultAlphabet, { i64::MAX as u128 }>;

    #[test]
    fn capped_generate_respects_max() {
        for _ in 0..1000 {
            let token = CappedToken::generate();
            assert!(token.id() >= 1);
            i64::try_from(token.id()).unwrap();
        }
    }

    // --- custom alphabet tests ---

    // The default alphabet, reversed. Being a permutation of the default
    // guarantees 32 distinct ASCII bytes without duplicating validation logic.
    define_alphabet!(ReversedAlphabet, b"FZUE7HPD1CW5SB9TV2MJ6G3RA8N0KX4Y");

    define_token_prefix!(AlphaPrefix, "A_");
    type AlphaToken = Token<AlphaPrefix, u64, ReversedAlphabet>;

    #[test]
    fn custom_alphabet_round_trip() {
        for id in [0, 1, 42, 1000, 123_456_789, u64::MAX] {
            let token = AlphaToken::new(id);
            let s = token.to_string();
            let parsed = AlphaToken::parse(&s).unwrap();
            assert_eq!(parsed.id(), id);
        }
    }

    #[test]
    fn custom_alphabet_zero_encodes_to_its_first_char() {
        let token = AlphaToken::new(0);
        assert_eq!(token.to_string(), "A_FFFFFFFFFFFFF");
    }

    #[test]
    fn custom_alphabet_known_value_encoding() {
        let token = AlphaToken::new(1);
        let s = token.to_string();
        assert_eq!(s, "A_FFFFFFFFFFFFZ");
    }

    #[test]
    fn custom_alphabet_differs_from_default_encoding() {
        let default_token = TestToken::new(42);
        let custom_token = AlphaToken::new(42);
        assert_ne!(default_token.encoded_id(), custom_token.encoded_id());
    }

    #[test]
    fn custom_alphabet_rejects_default_alphabet_characters_not_in_it() {
        // 'Y' is the first char of the *default* alphabet but is present in
        // ReversedAlphabet too (it's a permutation), so instead check that a
        // character truly absent from the base-32 alphabet space is rejected.
        let err = AlphaToken::parse("A_FFFFFFFFFFFFI").unwrap_err();
        assert_eq!(err, TokenError::InvalidCharacter('I'));
    }

    #[test]
    fn default_alphabet_is_default_type_parameter() {
        // Token<P, I> (no third parameter) must be identical to
        // Token<P, I, DefaultAlphabet>.
        let a: Token<TestPrefix, u64> = Token::new(7);
        let b: Token<TestPrefix, u64, DefaultAlphabet> = Token::new(7);
        assert_eq!(a.to_string(), b.to_string());
    }

    #[test]
    fn assert_valid_alphabet_accepts_default_alphabet() {
        super::__assert_valid_alphabet(DefaultAlphabet::ALPHABET);
    }

    #[test]
    #[should_panic(expected = "distinct")]
    fn assert_valid_alphabet_rejects_duplicate_bytes() {
        super::__assert_valid_alphabet(b"YYXK0N8AR3G6JM2VT9BS5WC1DPH7EUZF");
    }

    #[test]
    #[should_panic(expected = "ASCII")]
    fn assert_valid_alphabet_rejects_non_ascii_byte() {
        let mut bad = *DefaultAlphabet::ALPHABET;
        bad[0] = 200;
        super::__assert_valid_alphabet(&bad);
    }

    #[test]
    #[should_panic(expected = "'Z'")]
    fn assert_valid_alphabet_rejects_byte_above_z() {
        let mut bad = *DefaultAlphabet::ALPHABET;
        bad[0] = b'a';
        super::__assert_valid_alphabet(&bad);
    }
}
