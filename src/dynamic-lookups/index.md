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

We will do this by creating a very simple zkVM:
we will verify the execution of (a subset of) the AVR instruction set by Atmel.
These microcontrollers are most commonly known from the Arduino series of development boards

```admonish note
We will create a zkArduino
```

https://ww1.microchip.com/downloads/en/devicedoc/atmel-0856-avr-instruction-set-manual.pdf


## A Logical Division

### State Proof

Which manages the state of the virtual machine:
loading/storing registers, loading/storing instructions and memory, setting flags, etc.

### Execution Proof

Which manages the execution of the virtual machine:
enforces the constraints of the current instruction, and updates the state accordingly.