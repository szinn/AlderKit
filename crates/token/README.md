# alderkit-token

Typed, prefixed, base-32 encoded identifiers, in the style of Stripe-style resource IDs
(e.g. `U_ABCD1234NRST0`).

A `Token<Prefix>` wraps a numeric ID (`u64` by default) and renders as a short,
URL-safe string: a fixed prefix followed by a base-32 encoding of the ID. Only the
numeric ID is stored; the string form is computed on demand.

## Usage

```rust
use alderkit_token::{define_token_prefix, token::Token};

define_token_prefix!(UserPrefix, "U_");
type UserToken = Token<UserPrefix>;

let token = UserToken::generate();
let s = token.to_string(); // e.g. "U_YXK08AR3G6JM2"

let parsed: UserToken = s.parse().unwrap();
assert_eq!(parsed, token);
```

Each prefix defined with `define_token_prefix!` produces a distinct type, so tokens
for different entities can't be mixed up at compile time even though they share the
same backing representation.

## Configuration

`Token<Prefix, Id, Alphabet, MAX>` has three optional type parameters beyond the
prefix:

- **`Id`** — the backing integer type, `u64` (default) or `u128`. `u128` encodes to a
  longer string but covers a larger ID space.
- **`Alphabet`** — the base-32 alphabet used for encoding. Defaults to
  `DefaultAlphabet`, which excludes the visually ambiguous characters `I`, `L`, `O`,
  `Q`. Define a custom alphabet with `define_alphabet!`, which validates it at
  compile time (exactly 32 distinct ASCII bytes, each `<= b'Z'`).
- **`MAX`** — the upper bound (inclusive) used by `Token::generate` when picking a
  random ID. Defaults to the backing type's max value; lower it to keep generated IDs
  within a smaller range (e.g. `i64::MAX` for database columns that must fit a signed
  64-bit integer).

```rust
use alderkit_token::{define_alphabet, define_token_prefix, token::Token};

// u128 backing type, for a larger ID space.
define_token_prefix!(SessionPrefix, "S_");
type SessionToken = Token<SessionPrefix, u128>;

// Custom alphabet.
define_alphabet!(MyAlphabet, b"0123456789ABCDEFGHJKMNPQRSTVWXYZ");
define_token_prefix!(OrderPrefix, "O_");
type OrderToken = Token<OrderPrefix, u64, MyAlphabet>;

// Capped random-generation range (e.g. to stay within i64::MAX).
define_token_prefix!(InvoicePrefix, "I_");
type InvoiceToken = Token<InvoicePrefix, u64, alderkit_token::token::DefaultAlphabet, { i64::MAX as u128 }>;
```

## API

- `Token::generate()` — create a token with a random ID in `1..=MAX`.
- `Token::new(id)` — create a token from an explicit numeric ID.
- `Token::parse(s)` / `s.parse()` (via `FromStr`) — parse a token from its string
  form, validating the prefix, length, and alphabet.
- `Token::is_valid(s)` — check whether a string is a well-formed token, without
  keeping the parsed value.
- `Token::id()` — get the underlying numeric ID.
- `Token::encoded_id()` / `Token::from_encoded_id(s)` — convert to/from the encoded
  portion alone, without the prefix.

`Token` implements `Display`, `FromStr`, `Debug`, and (via the `serde` dependency)
`Serialize`/`Deserialize` as its string form.

## Errors

Parsing failures return a `TokenError`:

- `InvalidPrefix` — the string doesn't start with the expected prefix.
- `InvalidLength` — the encoded portion isn't the expected length for the backing type.
- `InvalidCharacter` — a character isn't in the token's alphabet.
- `Overflow` — decoding produced a value that doesn't fit the backing type.

## License

MIT
