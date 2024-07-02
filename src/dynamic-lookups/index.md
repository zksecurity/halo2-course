# Dynamic Lookups

![](./top.webp)

*What if I don't know what I want to look up?*

## Dynamic Lookups

The previous section introduced the concept of lookups.
In that section, we created a lookup table that was hardcoded into the circuit.
This is fine if you know all the values you want to look up at compile time.
But what if you don't?

In fact, such dynamic lookups are widely used in machine emulation,
and in particular within zkEVM implementations.

In this section, we will introduce the concept of dynamic lookups,
and show how they can be implemented in Halo2.
We will do this by creating a very simple zkVM.