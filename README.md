# AlderKit

A collection of Rust utility crates.

## Crates

| Crate                            | Description                                                           |
| -------------------------------- | --------------------------------------------------------------------- |
| [`alderkit-token`](crates/token) | Typed, prefixed, base-32 encoded identifiers (e.g. `U_ABCD1234NRST0`) |

### `alderkit-token`

A typed identifier built from a prefix and a base-32 encoded numeric ID, in the
style of Stripe-style resource IDs.

```rust
use alderkit_token::{define_token_prefix, token::Token};

define_token_prefix!(UserPrefix, "U_");
type UserToken = Token<UserPrefix>;

let token = UserToken::generate();
let s = token.to_string(); // e.g. "U_YXK08AR3G6JM2"

let parsed: UserToken = s.parse().unwrap();
assert_eq!(parsed, token);
```

Backing type (`u64`/`u128`), the maximum value used for random generation, and
the base-32 alphabet are all configurable per token type.

## License

MIT
