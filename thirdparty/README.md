# Wait a minute... this isn't a C project! Why is there a thirdparty directory?

In short, there's a type inference problem in sqlx which is fixed by [#4285](https://github.com/transact-rs/sqlx/pull/4285)
but has been left open for like, a half year now? I also fixed an annoying compilation warning.

`make thirdparty/sqlx` is your friend.
