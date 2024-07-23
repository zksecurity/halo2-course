# Static Lookups

![](./top.webp)

*Let's write some ~circuits~ weird machines*


In this installment we will introduce the concept of *lookups*.

Rather than trotting through yet another example with range checks,
we will explore *lookups* by creating a
"circuit" that can (very efficiently) match regular expressions.
Regular expression matching has been used in projects such as [zk-email](https://blog.aayushg.com/zkemail/), although the approach outlined here is substantially more efficient than that of [zk-email](https://blog.aayushg.com/zkemail/#regex-with-deterministic-finite-automata).

You might already be thinking that regular expressions are not very efficient to check using a circuit over a finite field,
and you would be right.
Fortunately, lookups enable us to implement this kind of very non-field-friendly functionality in an efficient way.
It might also sound complicated, but in fact the whole circuit fits in just over 200 lines of code.

## Brief Detour: Regular Expressions

```admonish cite
Some people, when confronted with a problem, think "I know, I'll use regular expressions.". \
Now they have two problems.

-- Jamie Zawinski
```

The particular regular expression we will match is `a+b+c` meaning:

- One or more `a`s.
- Followed by one or more `b`s.
- Followed by a single `c`.

For example, it would match:

- `aaabbc`
- `abc`

But not:

- `aaabbb`
- `bbbc`
- `aaac`

For us, the convient way to view a regular expression is
as a "Non-Deterministic Finite Automaton" (NFA).
You can visualize this as a directed graph where each node is a state
and every edge is labeled with a character.
You are allowed to move between states if the next charecter in the string matches the edge label.
For instance, for our regular expression `a+b+c` an NFA would look like this:

![Regex](./nfa.svg)

Meaning:

- You start in a state `ST_A`.
- In this state `ST_A`, you can either:
  - Move to `ST_A` if the next character is `a`.
  - Move to `ST_B` if the next character is `a`.
- In state `ST_B`, you can either:
  - Move to `ST_B` if the next character is `b`.
  - Move to `ST_C` if the next character is `b`.
- In state `ST_C`, you can:
  - Only move to `ST_DONE` if the next character is `c`.

For instance, if the string we are matching is `aaabbc`, we would move through the states like this:

```
ST_A -> ST_A -> ST_A -> ST_B -> ST_B -> ST_C -> ST_DONE
```

Conversely, if you have the string `aaabbb`, you would get stuck in state `ST_B` after the third `b` and not be able to move to `ST_C`.
We can represent this "NFA" as a table of valid state transitions:

| State | Character | Next State |
|-------|-----------|------------|
| ST_A  | a         | ST_A       |
| ST_A  | a         | ST_B       |
| ST_B  | b         | ST_B       |
| ST_B  | b         | ST_C       |
| ST_C  | c         | ST_DONE    |

In Rust we could encode this table as follows:

```rust,noplaypen
{{#include ../../halo-hero/examples/regex.rs:regex}}
```

The "values" of the states (e.g. `const ST_A: usize = 1`) are just arbitrary distinct integers.
We introduce a special `EOF` character which we will use to "pad" the end of the string: our circuit has a fixed sized, but we want it to accommodate strings of different lengths.
We also add another transition `ST_DONE -> ST_DONE` for the `EOF` character: you can stay in the `ST_DONE` state forever by consuming the special `EOF` padding character.

If you are still confused, it simply means that we are now matching strings like:

```
aaabbc<EOF><EOF><EOF>...<EOF>
```

Of some fixed length, where `<EOF>` is a special character.

```admonish note
In this example we will just match the regular expression,
but in general you want to do something more interesting with the result or restrict the string being matched.

For instance, in [zk-email](https://blog.aayushg.com/zkemail/#regex-with-deterministic-finite-automata) it is used to extract the senders address from an email after *verifying a DKIM signature* on the email:
in this case the string being matched comes with a digital signature (an RSA signature).

In our case, the string ends up in a column and it is trivial to use it for further processing, like hashing it to check a digital signature on it.
```

## Configuration

In order for us to match against the regular expression we will have the prover supply a table of transitions, we will then look up each transition in a fixed table of valid state transitions.
We need two gates:

- A gate to force the current state to be a fixed value: this is used at the start and end, to ensure that the prover starts in the `ST_A` state and ends in the `ST_DONE` state.

- A gate to check the transition is valid: this is used to check each transition in the table.

Our configuration step looks as follows:

```rust,noplaypen
{{#include ../../halo-hero/examples/regex.rs:columns}}
```

The new thing to notice is the `meta.lookup_table_column()` function calls:
these introduce a new column on which we can lookup rows.
In our case the lookup table will have three columns:

1. The current state (e.g. `ST_B`).
1. The next state (e.g. `ST_C`).
1. The character being consumed (e.g. `b`).

### Gate: Fix

Let us start with the gate that can force the current state to be a fixed value,
to the reader, there should be no surprises here:

```rust,noplaypen
{{#include ../../halo-hero/examples/regex.rs:fix}}
```

It simply:

- Reads the current state, `st`.
- Reads the fixed state, `fix_st`.
- Forces `st = fix_st` if `q_match = 1`.

### Gate: Transition

The new magic is happening in the "transition" gate:

```rust,noplaypen
{{#include ../../halo-hero/examples/regex.rs:lookup}}
```

Let's break it down:

- It read the current state, `st_cur`, from the `st` (state column).
- It reads the next state, `st_nxt`, from the `st` (state column).
- It reads the next character, `ch`, from the `ch` (character column).

The layout looks like this:

![Regex](./regex.svg)

It then multiplies them all by the selector `q_regex`, meaning:

- If `q_regex = 0`, then the lookup checks:
  - `(0, 0, 0)` in `(tbl_st_cur, st_nxt, tbl_ch)`.
- If `q_regex = 1`, then the lookup checks:
  - `(st_cur, st_nxt, ch)` in `(tbl_st_cur, st_nxt, tbl_ch)`.

This 3-tuple is looked up in our table
of valid state transitions.

## Synthesize

With our gates and lookup table in place, we can now synthesize the circuit.

### Assign Table

Let us now take a look at how we populate the table of valid state transitions:

```rust,noplaypen
{{#include ../../halo-hero/examples/regex.rs:assign_table}}
```

Take a second to look at this code.
It's mostly self-explanatory, but let's go through it:

1. We use `layouter.assign_table` to get a closure with a `Table` argument.
2. We convert the `REGEX` table into tuples of field elements:
  - We encode the state as a field element.
  - We encode each (e.g. ASCII) character as a field element in the obvious way.
  - We encode `EOF` as some arbitrary value outside the range of valid characters.
3. This is then stored `transitions` as a vector of such tuples.
4. Finally we assign the vector of tuples to the table using `table.assign_cell`.

### Start Contraint

At the first row of our region, we will need to force the state to be `ST_A` (aka. `ST_START`).
This is done by assigning the fixed column `fix_st` to be `ST_A` and turning on the `q_match` selector:

```rust,noplaypen
{{#include ../../halo-hero/examples/regex.rs:region_start}}
```

Nothing new here, we saw similar code in the section of `Column<Fixed>`.

### Assign Transitions

The meet of the circuit is in the region where we assign the sequence of states we are transitioning through, as well as the sequence of characters we are consuming:

```rust,noplaypen
{{#include ../../halo-hero/examples/regex.rs:region_steps}}
```

### End Constraint

At the end, similar to the start, we need to ensure that the state is `ST_DONE`:

```rust,noplaypen
{{#include ../../halo-hero/examples/regex.rs:region_end}}
```

### Exercises

The full code is at the end.

```admonish exercise
Modify the regular expression to be `a*b+c`, i.e. the change of `a+` to `a*`.
```

```admonish hint
You do not need to change the circuit, only the `REGEX` constant.

However, this exercise is slightly more complicated than it seems at first:
we must somehow be able to transition from matching `a`s without consuming an `a`
(in case there are zero `a`).
```

```admonish exercise
Create a circuit which computes a single round of [AES](https://en.wikipedia.org/wiki/Advanced_Encryption_Standard) encryption.

You may ignore the key schedule.
```

```admonish hint
Use four separate lookup tables to represent:

- The S-box. A table with 256 entries.
- The GF(2^8) addition (XOR). A table with 256x256 entries.
- A "multiplication by 2" in GF(2^8) table. A table with 256 entries.
- A "multiplication by 3" in GF(2^8) table. A table with 256 entries.
```

```admonish hint
The above is dominated by the XOR table, which is a 256x256 table.
To avoid this massive table we can use the fact that XOR acts component-wise on the bits of the input:

\\[
  (l_1, l_2) \oplus (r_1, r_2) = (l_1 \oplus r_1, l_2 \oplus r_2)
\\]

Therefore, we can replace a single lookup in a 256x256 table with two lookups in a 16x16 tables.
```

```admonish exercise
Create a circuit which takes an AES ciphertext as public input (instance) and a key as private input (witness) and checks that the ciphertext decrypts to a known plaintext.
```

```rust,noplaypen
{{#include ../../halo-hero/examples/regex.rs}}
```
