use sexp::*;
use sexp::Atom::*;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use im::HashMap; // persistent map — O(1) clone for scoped environments

// Tagged values: LSB distinguishes type
//   Numbers: value << 1 (LSB = 0)
//   Booleans: true = 3 (0b11), false = 1 (0b01)
const TRUE:  i64 = 3;
const FALSE: i64 = 1;

// 64-bit arithmetic won't overflow for i32 inputs, so we range-check instead
const TAGGED_MIN: i64 = (i32::MIN as i64) << 1;
const TAGGED_MAX: i64 = (i32::MAX as i64) << 1;

enum Op1 { Add1, Sub1, Negate, IsNum, IsBool }
enum Op2 { Plus, Minus, Times, Less, Greater, LessEq, GreaterEq, Equal }

enum Expr {
    Num(i32),
    Bool(bool),
    Input,
    Id(String),
    Let(Vec<(String, Expr)>, Box<Expr>),
    UnOp(Op1, Box<Expr>),
    BinOp(Op2, Box<Expr>, Box<Expr>),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    Block(Vec<Expr>),
    Loop(Box<Expr>),
    Break(Box<Expr>),
    Set(String, Box<Expr>),
}

const RESERVED: &[&str] = &[
    "let", "add1", "sub1", "negate", "isnum", "isbool",
    "if", "block", "loop", "break", "set!", "true", "false", "input",
];

fn parse_expr(s: &Sexp) -> Expr {
    match s {
        Sexp::Atom(I(n)) => {
            if *n < i32::MIN as i64 || *n > i32::MAX as i64 { panic!("Invalid") }
            Expr::Num(*n as i32)
        }
        Sexp::Atom(S(name)) if name == "true"  => Expr::Bool(true),
        Sexp::Atom(S(name)) if name == "false" => Expr::Bool(false),
        Sexp::Atom(S(name)) if name == "input" => Expr::Input,
        Sexp::Atom(S(name)) => {
            if RESERVED.contains(&name.as_str()) { panic!("Invalid") }
            Expr::Id(name.to_string())
        }
        Sexp::List(vec) => match &vec[..] {
            [Sexp::Atom(S(op)), e] if op == "add1"   => Expr::UnOp(Op1::Add1,   Box::new(parse_expr(e))),
            [Sexp::Atom(S(op)), e] if op == "sub1"   => Expr::UnOp(Op1::Sub1,   Box::new(parse_expr(e))),
            [Sexp::Atom(S(op)), e] if op == "negate" => Expr::UnOp(Op1::Negate, Box::new(parse_expr(e))),
            [Sexp::Atom(S(op)), e] if op == "isnum"  => Expr::UnOp(Op1::IsNum,  Box::new(parse_expr(e))),
            [Sexp::Atom(S(op)), e] if op == "isbool" => Expr::UnOp(Op1::IsBool, Box::new(parse_expr(e))),

            [Sexp::Atom(S(op)), e1, e2] if op == "+"  => Expr::BinOp(Op2::Plus,      Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),
            [Sexp::Atom(S(op)), e1, e2] if op == "-"  => Expr::BinOp(Op2::Minus,     Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),
            [Sexp::Atom(S(op)), e1, e2] if op == "*"  => Expr::BinOp(Op2::Times,     Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),
            [Sexp::Atom(S(op)), e1, e2] if op == "<"  => Expr::BinOp(Op2::Less,      Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),
            [Sexp::Atom(S(op)), e1, e2] if op == ">"  => Expr::BinOp(Op2::Greater,   Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),
            [Sexp::Atom(S(op)), e1, e2] if op == "<=" => Expr::BinOp(Op2::LessEq,    Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),
            [Sexp::Atom(S(op)), e1, e2] if op == ">=" => Expr::BinOp(Op2::GreaterEq, Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),
            [Sexp::Atom(S(op)), e1, e2] if op == "="  => Expr::BinOp(Op2::Equal,     Box::new(parse_expr(e1)), Box::new(parse_expr(e2))),

            [Sexp::Atom(S(op)), cond, then_e, else_e] if op == "if" =>
                Expr::If(Box::new(parse_expr(cond)), Box::new(parse_expr(then_e)), Box::new(parse_expr(else_e))),

            [Sexp::Atom(S(op)), Sexp::List(bindings), body] if op == "let" => {
                if bindings.is_empty() { panic!("Invalid") }
                Expr::Let(bindings.iter().map(parse_bind).collect(), Box::new(parse_expr(body)))
            }

            [Sexp::Atom(S(op)), Sexp::Atom(S(name)), e] if op == "set!" => {
                if RESERVED.contains(&name.as_str()) { panic!("Invalid") }
                Expr::Set(name.to_string(), Box::new(parse_expr(e)))
            }

            [Sexp::Atom(S(op)), body] if op == "loop"  => Expr::Loop(Box::new(parse_expr(body))),
            [Sexp::Atom(S(op)), e]    if op == "break" => Expr::Break(Box::new(parse_expr(e))),

            [Sexp::Atom(S(op)), rest @ ..] if op == "block" => {
                if rest.is_empty() { panic!("Invalid") }
                Expr::Block(rest.iter().map(parse_expr).collect())
            }

            _ => panic!("Invalid"),
        },
        _ => panic!("Invalid"),
    }
}

fn parse_bind(s: &Sexp) -> (String, Expr) {
    match s {
        Sexp::List(vec) => match &vec[..] {
            [Sexp::Atom(S(name)), expr] if !RESERVED.contains(&name.as_str()) =>
                (name.to_string(), parse_expr(expr)),
            _ => panic!("Invalid"),
        },
        _ => panic!("Invalid"),
    }
}

fn new_label(lc: &mut i32, name: &str) -> String {
    *lc += 1;
    format!("{}_{}", name, lc)
}

// Error helpers — each emits: skip over error on success, error block, ok label

fn assert_num_rax(instrs: &mut Vec<String>, lc: &mut i32) {
    let ok = new_label(lc, "ok_num");
    instrs.push("test rax, 1".into());
    instrs.push(format!("jz {ok}"));
    instrs.push("mov rdi, 1".into());
    instrs.push("jmp snek_error".into());
    instrs.push(format!("{ok}:"));
}

fn assert_num_rbx(instrs: &mut Vec<String>, lc: &mut i32) {
    let ok = new_label(lc, "ok_num");
    instrs.push("test rbx, 1".into());
    instrs.push(format!("jz {ok}"));
    instrs.push("mov rdi, 1".into());
    instrs.push("jmp snek_error".into());
    instrs.push(format!("{ok}:"));
}

fn assert_both_num(instrs: &mut Vec<String>, lc: &mut i32, stack_off: i32) {
    assert_num_rax(instrs, lc);
    instrs.push(format!("mov rbx, [rsp{}]", stack_off));
    assert_num_rbx(instrs, lc);
}

// For (=): XOR detects mixed tags — if bit 0 differs, types don't match
fn assert_same_type(instrs: &mut Vec<String>, lc: &mut i32, stack_off: i32) {
    let ok = new_label(lc, "ok_same_type");
    instrs.push("mov rbx, rax".into());
    instrs.push(format!("xor rbx, [rsp{}]", stack_off));
    instrs.push("test rbx, 1".into());
    instrs.push(format!("jz {ok}"));
    instrs.push("mov rdi, 1".into());
    instrs.push("jmp snek_error".into());
    instrs.push(format!("{ok}:"));
}

// jo catches 64-bit overflow; range check catches i32 overflow that fits in 64 bits
fn assert_no_overflow(instrs: &mut Vec<String>, lc: &mut i32) {
    let ok_64 = new_label(lc, "ok_no_ov64");
    instrs.push(format!("jno {ok_64}"));
    instrs.push("mov rdi, 2".into());
    instrs.push("jmp snek_error".into());
    instrs.push(format!("{ok_64}:"));

    let ok_hi = new_label(lc, "ok_no_ov_hi");
    instrs.push(format!("mov rbx, {}", TAGGED_MAX));
    instrs.push("cmp rax, rbx".into());
    instrs.push(format!("jle {ok_hi}"));
    instrs.push("mov rdi, 2".into());
    instrs.push("jmp snek_error".into());
    instrs.push(format!("{ok_hi}:"));

    let ok_lo = new_label(lc, "ok_no_ov_lo");
    instrs.push(format!("mov rbx, {}", TAGGED_MIN));
    instrs.push("cmp rax, rbx".into());
    instrs.push(format!("jge {ok_lo}"));
    instrs.push("mov rdi, 2".into());
    instrs.push("jmp snek_error".into());
    instrs.push(format!("{ok_lo}:"));
}

// si: next free stack slot (slot 1 = [rsp-8] reserved for input, so si starts at 2)
// brk: break target label — replaced per loop so break hits the innermost one
fn compile(
    e: &Expr,
    si: i32,
    env: &HashMap<String, i32>,
    lc: &mut i32,
    brk: &Option<String>,
    instrs: &mut Vec<String>,
) {
    match e {
        Expr::Num(n) => instrs.push(format!("mov rax, {}", (*n as i64) << 1)),
        Expr::Bool(true)  => instrs.push(format!("mov rax, {TRUE}")),
        Expr::Bool(false) => instrs.push(format!("mov rax, {FALSE}")),
        Expr::Input => instrs.push("mov rax, [rsp-8]".into()),

        Expr::Id(name) => match env.get(name) {
            Some(off) => instrs.push(format!("mov rax, [rsp{}]", off)),
            None => panic!("Unbound variable identifier {name}"),
        },

        Expr::UnOp(op, e) => {
            compile(e, si, env, lc, brk, instrs);
            match op {
                Op1::Add1 => {
                    assert_num_rax(instrs, lc);
                    instrs.push("add rax, 2".into()); // +1 in value = +2 in tagged
                    assert_no_overflow(instrs, lc);
                }
                Op1::Sub1 => {
                    assert_num_rax(instrs, lc);
                    instrs.push("sub rax, 2".into());
                    assert_no_overflow(instrs, lc);
                }
                Op1::Negate => {
                    assert_num_rax(instrs, lc);
                    instrs.push("neg rax".into()); // neg(n<<1) = (-n)<<1 in two's complement
                    assert_no_overflow(instrs, lc);
                }
                Op1::IsNum => {
                    instrs.push("and rax, 1".into());
                    instrs.push("cmp rax, 0".into());
                    let end = new_label(lc, "isnum_end");
                    instrs.push(format!("mov rax, {TRUE}"));
                    instrs.push(format!("je {end}"));
                    instrs.push(format!("mov rax, {FALSE}"));
                    instrs.push(format!("{end}:"));
                }
                Op1::IsBool => {
                    instrs.push("and rax, 1".into());
                    instrs.push("cmp rax, 1".into());
                    let end = new_label(lc, "isbool_end");
                    instrs.push(format!("mov rax, {TRUE}"));
                    instrs.push(format!("je {end}"));
                    instrs.push(format!("mov rax, {FALSE}"));
                    instrs.push(format!("{end}:"));
                }
            }
        }

        Expr::BinOp(op, e1, e2) => {
            let off = -8 * si;

            // Spill left operand so right can use rax
            compile(e1, si, env, lc, brk, instrs);
            instrs.push(format!("mov [rsp{}], rax", off));
            compile(e2, si + 1, env, lc, brk, instrs);

            match op {
                Op2::Plus | Op2::Minus | Op2::Times |
                Op2::Less | Op2::Greater | Op2::LessEq | Op2::GreaterEq => {
                    assert_both_num(instrs, lc, off);
                }
                Op2::Equal => {
                    assert_same_type(instrs, lc, off);
                }
            }

            match op {
                Op2::Plus => {
                    instrs.push(format!("add rax, [rsp{}]", off));
                    assert_no_overflow(instrs, lc);
                }
                Op2::Minus => {
                    // Need left - right; left is on stack, right is in rax
                    instrs.push("mov rbx, rax".into());
                    instrs.push(format!("mov rax, [rsp{}]", off));
                    instrs.push("sub rax, rbx".into());
                    assert_no_overflow(instrs, lc);
                }
                Op2::Times => {
                    // Untag one operand to avoid double-shift: b * (a<<1) = (a*b)<<1
                    instrs.push("sar rax, 1".into());
                    instrs.push(format!("imul rax, [rsp{}]", off));
                    assert_no_overflow(instrs, lc);
                }
                Op2::Less | Op2::Greater | Op2::LessEq | Op2::GreaterEq | Op2::Equal => {
                    // Tagged cmp preserves ordering since both operands share the same tag
                    instrs.push(format!("mov rbx, [rsp{}]", off));
                    instrs.push("cmp rbx, rax".into());
                    let jmp = match op {
                        Op2::Less      => "jl",
                        Op2::Greater   => "jg",
                        Op2::LessEq    => "jle",
                        Op2::GreaterEq => "jge",
                        Op2::Equal     => "je",
                        _ => unreachable!(),
                    };
                    let end = new_label(lc, "cmp_end");
                    instrs.push(format!("mov rax, {TRUE}"));
                    instrs.push(format!("{jmp} {end}"));
                    instrs.push(format!("mov rax, {FALSE}"));
                    instrs.push(format!("{end}:"));
                }
            }
        }

        // Only false (tagged 1) takes the else branch; everything else is truthy
        Expr::If(cond, then_e, else_e) => {
            let else_lbl = new_label(lc, "if_else");
            let end_lbl  = new_label(lc, "if_end");
            compile(cond, si, env, lc, brk, instrs);
            instrs.push(format!("cmp rax, {FALSE}"));
            instrs.push(format!("je {else_lbl}"));
            compile(then_e, si, env, lc, brk, instrs);
            instrs.push(format!("jmp {end_lbl}"));
            instrs.push(format!("{else_lbl}:"));
            compile(else_e, si, env, lc, brk, instrs);
            instrs.push(format!("{end_lbl}:"));
        }

        Expr::Block(exprs) => {
            for expr in exprs {
                compile(expr, si, env, lc, brk, instrs);
            }
        }

        // Each loop gets its own end label, replacing any outer brk target
        Expr::Loop(body) => {
            let start = new_label(lc, "loop_start");
            let end   = new_label(lc, "loop_end");
            instrs.push(format!("{start}:"));
            compile(body, si, env, lc, &Some(end.clone()), instrs);
            instrs.push(format!("jmp {start}"));
            instrs.push(format!("{end}:"));
        }

        Expr::Break(e) => match brk {
            Some(label) => {
                compile(e, si, env, lc, brk, instrs);
                instrs.push(format!("jmp {label}"));
            }
            None => panic!("break outside of loop"),
        },

        Expr::Set(name, e) => match env.get(name) {
            Some(off) => {
                compile(e, si, env, lc, brk, instrs);
                instrs.push(format!("mov [rsp{}], rax", off));
            }
            None => panic!("Unbound variable identifier {name}"),
        },

        Expr::Let(bindings, body) => {
            let mut new_env = env.clone();
            let mut curr_si = si;
            let mut seen = std::collections::HashSet::new();

            for (name, expr) in bindings {
                if !seen.insert(name.clone()) {
                    panic!("Duplicate binding");
                }
                let off = -8 * curr_si;
                compile(expr, curr_si, &new_env, lc, brk, instrs);
                instrs.push(format!("mov [rsp{}], rax", off));
                new_env = new_env.update(name.clone(), off);
                curr_si += 1;
            }

            compile(body, curr_si, &new_env, lc, brk, instrs);
        }
    }
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <input.snek> <output.s>", args[0]);
        std::process::exit(1);
    }

    let mut src = String::new();
    File::open(&args[1])?.read_to_string(&mut src)?;

    let expr = parse_expr(&parse(&src).unwrap());
    let mut instrs = Vec::new();
    let mut lc = 0;
    compile(&expr, 2, &HashMap::new(), &mut lc, &None, &mut instrs);
    let body = instrs.join("\n  ");

    let asm = format!(
"section .text
extern snek_error
global our_code_starts_here
our_code_starts_here:
  mov [rsp-8], rdi
  {body}
  ret
"
    );

    File::create(&args[2])?.write_all(asm.as_bytes())?;
    Ok(())
}
