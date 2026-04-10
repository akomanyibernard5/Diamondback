# Diamondback Compiler

A compiler for the Diamondback language — extends Cobra with **function definitions**, **function calls**, and a **print** built-in — built for a compilers course.

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
cargo run -- test/fun_factorial.snek test/fun_factorial.s
make test/fun_factorial.run
./test/fun_factorial.run false
```

Run all tests:

```bash
make test
```

## Language

### Grammar

```
<prog> := <defn>* <expr>

<defn> := (fun (<name> <name>*) <expr>)

<expr> :=
  | <number> | true | false | input
  | <identifier>
  | (let ((<identifier> <expr>)+) <expr>)
  | (add1 <expr>) | (sub1 <expr>) | (negate <expr>)
  | (isnum <expr>) | (isbool <expr>) | (print <expr>)
  | (+ <expr> <expr>) | (- <expr> <expr>) | (* <expr> <expr>)
  | (< <expr> <expr>) | (> <expr> <expr>)
  | (<= <expr> <expr>) | (>= <expr> <expr>) | (= <expr> <expr>)
  | (if <expr> <expr> <expr>)
  | (block <expr>+)
  | (loop <expr>)
  | (break <expr>)
  | (set! <identifier> <expr>)
  | (<name> <expr>*)        ; function call
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
- **print**: Prints the value to stdout and returns it.
- **if**: If condition is `false` (0b01), take else branch; otherwise take then branch.
- **block**: Evaluate expressions in order, return last value.
- **loop**: Repeat body forever; exit with `break`.
- **break**: Exit innermost loop with given value.
- **set!**: Mutate an existing variable binding.
- **fun**: Define a named function with zero or more parameters.
- **call**: Apply a function to arguments; arity is checked at compile time.

### Examples

| Program | Result |
|---------|--------|
| `(fun (double x) (+ x x))` / `(double 5)` | 10 |
| `(fun (factorial n) (if (= n 1) 1 (* n (factorial (- n 1)))))` / `(factorial 5)` | 120 |
| `(fun (add3 x y z) (+ (+ x y) z))` / `(add3 1 2 3)` | 6 |
| `(print 42)` | prints `42`, returns `42` |

## Calling Convention

### Stack Frame Layout

```
Higher addresses
---------------
rbp+32  | arg 3       |  (if 3+ args)
---------------
rbp+24  | arg 2       |  (if 2+ args)
---------------
rbp+16  | arg 1       |  (if 1+ args)
---------------
rbp+8   | return addr |
---------------
rbp     | saved rbp   |  <- rbp points here
---------------
rbp-8   | local 1     |
---------------
rbp-16  | local 2     |
---------------
Lower addresses
```

### Caller responsibilities

1. If the number of arguments is odd, pad `rsp` by 8 to maintain 16-byte alignment.
2. Push arguments onto the stack **right-to-left**.
3. `call fun_<name>` — pushes return address and transfers control.
4. After return: `add rsp, <args * 8 + pad>` to clean up.

### Callee responsibilities

1. `push rbp` — save caller's frame pointer.
2. `mov rbp, rsp` — establish own frame pointer.
3. `sub rsp, <locals * 8>` — allocate space for local variables.
4. Execute body — result in `rax`.
5. `add rsp, <locals * 8>` — release locals.
6. `pop rbp` — restore caller's frame pointer.
7. `ret` — return to caller.

### Variable access

- **Parameters**: `[rbp + 16]`, `[rbp + 24]`, `[rbp + 32]`, … (positive offsets)
- **Locals / spill slots**: `[rbp - 8]`, `[rbp - 16]`, … (negative offsets)
- All addressing is **rbp-relative** throughout — `rsp` is never used for variable lookup.

### 16-byte alignment

x86-64 System V ABI requires `rsp` to be 16-byte aligned at every `call` instruction. After `push rbp` / `mov rbp, rsp` the frame is aligned. Each argument push is 8 bytes, so an odd number of arguments requires an extra 8-byte pad before the first push.

## Error Handling

| Condition | Message | When |
|-----------|---------|------|
| Duplicate binding in same `let` | `Duplicate binding` | Compile time |
| Duplicate function name | `Duplicate binding` | Compile time |
| Duplicate parameter name | `Duplicate binding` | Compile time |
| Unbound variable | `Unbound variable identifier <name>` | Compile time |
| Undefined function | `Unbound variable identifier <name>` | Compile time |
| Wrong number of arguments | `Invalid` | Compile time |
| `break` outside loop | `break outside of loop` | Compile time |
| Invalid syntax / out-of-range number | `Invalid` | Compile time |
| Type mismatch in arithmetic/comparison | `invalid argument` | Runtime |
| Integer overflow | `overflow` | Runtime |

## Implementation Notes

- Tagged value representation: numbers shifted left by 1, booleans use LSB=1.
- Runtime type checks emit `test rax, 1` before arithmetic operations.
- Overflow detection uses `jo` (jump on overflow) after arithmetic, plus explicit range checks for i32 bounds.
- Comparisons use `cmp` + conditional jump to produce tagged boolean results.
- Loops use label pairs (start/end); `break` jumps to the end label.
- `input` is passed via `rdi` (System V calling convention) and stored at `[rbp-8]` in main's frame.
- `snek_error` handles error codes: 1 = invalid argument, 2 = overflow.
- `snek_print` prints a tagged value and returns it unchanged.
- Stack space for locals is explicitly allocated with `sub rsp` in each function prologue.

## Project Structure

```
src/main.rs          — compiler (parser + code generator)
runtime/start.rs     — runtime: entry point, snek_error, snek_print
test/*.snek          — test programs
test/error/*.snek    — compile-time and runtime error test programs
Makefile             — build rules
```
