# Halo Hero

This book is a course on [Halo2](https://github.com/zcash/halo2) development and PlonKish arithmetization in general.
The intended audience is Rust developers who have a vague understanding of succinct proofs in general,
but no specific knowledge of Halo2, PlonK or developing circuits for zkSNARKs.

The book was created as a collaboration between [ZKSecurity](https://zksecurity.xyz) and
the [Zircuit](https://www.zircuit.com/) development team.
All material, full code examples and the book's source, are available on [GitHub](https://github.com/zksecurity/halo2-course).

![zks](src/intro/zks-scale.png)

![zircuit](src/intro/zircuit.png)

## Running The Book

The book is build using `mdbook` and `mdbook-admonish`:

```
cargo install mdbook
cargo install mdbook-admonish
mdbook serve
```
