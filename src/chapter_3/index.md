# Many Regions

![](./top.webp)


Okay new gate time:

```rust
{{#include ../../halo-hero/examples/regions.rs:configure}}
```

<details>
<summary><b>Question: What does this gate do?</b></summary>

W
</details>



```rust
{{#include ../../halo-hero/examples/regions.rs:mul_region}}
```

This function takes two assigned cells and returns a cell that is assigned the product of the two inputs. 

```admonish warning
This code is not safe (yet). We will get to that.
```

In order to use this function, we will also need some way to *create* assigned cells.
To do this we create a function which allocates a small (1 row) region, enables no gates and simply returns the cell:

```rust
{{#include ../../halo-hero/examples/regions.rs:wit_region}}
```