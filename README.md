# Cobra Compiler

A compiler for the Cobra language — **C**onditionals, **O**perators, **B**ooleans, **R**untime checks, **A**nd loops — built for a compilers course. Extends Boa with booleans, conditionals, loops, mutation, and runtime type checking.

## Dependencies

- Rust (with Cargo)
- NASM (Netwide Assembler)
- `rustc` with `x86_64-apple-darwin` target (for linking the runtime)

## Building

```bash
cargo build
```

## Running

```bash
cargo run -- test/loop_count.snek test/loop_count.s
make test/loop_count.run
./test/loop_count.run false
```

Or in one step:

```bash
make test/loop_count.run && ./test/loop_count.run false
```

Run all tests:

```bash
make test
```

## Language

### Grammar

```
<expr> :=
  | <number>
  | true | false
  | input
  | <identifier>
  | (let ((<identifier> <expr>)+) <expr>)
  | (add1 <expr>) | (sub1 <expr>) | (negate <expr>)
  | (+ <expr> <expr>) | (- <expr> <expr>) | (* <expr> <expr>)
  | (< <expr> <expr>) | (> <expr> <expr>)
  | (<= <expr> <expr>) | (>= <expr> <expr>) | (= <expr> <expr>)
  | (isnum <expr>) | (isbool <expr>)
  | (if <expr> <expr> <expr>)
  | (block <expr>+)
  | (loop <expr>)
  | (break <expr>)
  | (set! <identifier> <expr>)
```

### Tagged Value Representation

Values use the least significant bit to distinguish types:
- **Numbers**: shifted left by 1 (LSB = 0). Value `5` → `10` (0b1010)
- **Booleans**: LSB = 1. `true` → `3` (0b11), `false` → `1` (0b01)

### Semantics

- **Numbers**: 32-bit signed integers, tagged by left-shift.
- **Booleans**: `true` and `false` are literals.
- **input**: Command-line argument passed to the program.
- **add1 / sub1 / negate**: Arithmetic on numbers; runtime error if given a boolean.
- **Binary arithmetic** (`+`, `-`, `*`): Both operands must be numbers.
- **Comparisons** (`<`, `>`, `<=`, `>=`): Both operands must be numbers; returns boolean.
- **Equality** (`=`): Both operands must have the same type; returns boolean.
- **isnum / isbool**: Type predicates returning booleans.
- **if**: If condition is `false` (0b01), take else branch; otherwise take then branch.
- **block**: Evaluate expressions in order, return last value.
- **loop**: Repeat body forever; exit with `break`.
- **break**: Exit innermost loop with given value.
- **set!**: Mutate an existing variable binding.

### Examples

| Program | Result |
|---------|--------|
| `true` | true |
| `(if true 5 10)` | 5 |
| `(< 3 5)` | true |
| `(= 42 42)` | true |
| `(isnum 5)` | true |
| `(isbool true)` | true |
| `(negate 5)` | -5 |
| `(block 1 2 3)` | 3 |
| `(let ((x 5)) (block (set! x 10) x))` | 10 |
| `(let ((x 0)) (loop (if (= x 10) (break x) (set! x (+ x 1)))))` | 10 |

## Error Handling

| Condition | Message | When |
|-----------|---------|------|
| Duplicate binding in same `let` | `Duplicate binding` | Compile time |
| Unbound variable | `Unbound variable identifier <name>` | Compile time |
| `break` outside loop | `break outside of loop` | Compile time |
| Invalid syntax / out-of-range number | `Invalid` | Compile time |
| Type mismatch in arithmetic/comparison | `invalid argument` | Runtime |
| Integer overflow | `overflow` | Runtime |

## Implementation Notes

- Tagged value representation: numbers shifted left by 1, booleans use LSB=1.
- Runtime type checks emit `test rax, 1` before arithmetic operations.
- Overflow detection uses `jo` (jump on overflow) after arithmetic.
- Comparisons use `cmp` + conditional jump to produce tagged boolean results.
- Loops use label pairs (start/end); `break` jumps to the end label.
- `input` is passed via `rdi` (System V calling convention) and stored at `[rsp-8]`.
- The `snek_error` function in the runtime handles error codes 1 (invalid argument) and 2 (overflow).

## Project Structure

```
src/main.rs          — compiler (parser + code generator)
runtime/start.rs     — runtime: calls our_code_starts_here, prints result, handles errors
test/*.snek          — test programs
test/error/*.snek    — error test programs
Makefile             — build rules
```
