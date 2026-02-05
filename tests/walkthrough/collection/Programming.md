---
name = "Programming"
---

Q: What does the `?` operator do in Rust?
A: The `?` operator propagates errors. If the value is `Ok(v)`, it unwraps to `v`. If the value is `Err(e)`, it returns early from the function with that error.

Q: What is the difference between `&str` and `String` in Rust?
A: `&str` is an immutable reference to a string slice (borrowed data), while `String` is an owned, heap-allocated, growable string type.

C: In Rust, the [borrow checker] enforces memory safety at compile time without a garbage collector.

C: The `Option<T>` type in Rust represents a value that is either [Some(T)] or [None].
