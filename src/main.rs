use sexp::*;
use sexp::Atom::*;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use im::HashMap;

// Tagged values: LSB distinguishes type
//   Numbers: value << 1 (LSB = 0)
//   Booleans: true = 3 (0b11), false = 1 (0b01)
const TRUE:  i64 = 3;
const FALSE: i64 = 1;

const TAGGED_MIN: i64 = (i32::MIN as i64) << 1;
const TAGGED_MAX: i64 = (i32::MAX as i64) << 1;

enum Op1 { Add1, Sub1, Negate, IsNum, IsBool, Print }
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
    Call(String, Vec<Expr>),
}

struct Definition {
    name: String,
    params: Vec<String>,
    body: Expr,
}

struct Program {
    defns: Vec<Definition>,
    main: Expr,
}

const RESERVED: &[&str] = &[
    "let", "add1", "sub1", "negate", "isnum", "isbool", "print",
    "if", "block", "loop", "break", "set!", "true", "false", "input", "fun",
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
            [Sexp::Atom(S(op)), e] if op == "print"  => Expr::UnOp(Op1::Print,  Box::new(parse_expr(e))),

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

            // Function call: (name arg*)
            [Sexp::Atom(S(name)), args @ ..] if !RESERVED.contains(&name.as_str()) =>
                Expr::Call(name.to_string(), args.iter().map(parse_expr).collect()),

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

fn parse_defn(s: &Sexp) -> Option<Definition> {
    match s {
        Sexp::List(vec) => match &vec[..] {
            [Sexp::Atom(S(kw)), Sexp::List(sig), body] if kw == "fun" => {
                match &sig[..] {
                    [Sexp::Atom(S(name)), params @ ..] => {
                        let param_names: Vec<String> = params.iter().map(|p| match p {
                            Sexp::Atom(S(n)) => {
                                if RESERVED.contains(&n.as_str()) { panic!("Invalid") }
                                n.clone()
                            }
                            _ => panic!("Invalid"),
                        }).collect();
                        // Check duplicate params
                        let mut seen = std::collections::HashSet::new();
                        for p in &param_names {
                            if !seen.insert(p.clone()) { panic!("Duplicate binding") }
                        }
                        Some(Definition { name: name.clone(), params: param_names, body: parse_expr(body) })
                    }
                    _ => panic!("Invalid"),
                }
            }
            _ => None,
        },
        _ => None,
    }
}

fn parse_program(sexps: &[Sexp]) -> Program {
    let mut defns = vec![];
    let mut main_expr = None;
    for s in sexps {
        match parse_defn(s) {
            Some(d) => {
                if main_expr.is_some() { panic!("Invalid") }
                defns.push(d);
            }
            None => {
                if main_expr.is_some() { panic!("Invalid") }
                main_expr = Some(parse_expr(s));
            }
        }
    }
    Program { defns, main: main_expr.expect("Invalid") }
}

fn new_label(lc: &mut i32, name: &str) -> String {
    *lc += 1;
    format!("{}_{}", name, lc)
}

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

fn assert_both_num(instrs: &mut Vec<String>, lc: &mut i32, rbp_off: i32) {
    assert_num_rax(instrs, lc);
    instrs.push(format!("mov rbx, [rbp{}]", fmt_off(rbp_off)));
    assert_num_rbx(instrs, lc);
}

fn assert_same_type(instrs: &mut Vec<String>, lc: &mut i32, rbp_off: i32) {
    let ok = new_label(lc, "ok_same_type");
    instrs.push("mov rbx, rax".into());
    instrs.push(format!("xor rbx, [rbp{}]", fmt_off(rbp_off)));
    instrs.push("test rbx, 1".into());
    instrs.push(format!("jz {ok}"));
    instrs.push("mov rdi, 1".into());
    instrs.push("jmp snek_error".into());
    instrs.push(format!("{ok}:"));
}

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

// Format a signed offset for memory addressing: +N, -N, or empty for 0
fn fmt_off(off: i32) -> String {
    if off > 0 { format!("+{off}") }
    else if off < 0 { format!("{off}") }
    else { String::new() }
}

// si: next free local slot index (1-based); locals live at [rbp - si*8]
// env maps variable names to their rbp-relative byte offsets (negative = local, positive = param)
// brk: innermost loop end label
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
        // input is stored at [rbp-8] (slot 1) in main
        Expr::Input => instrs.push("mov rax, [rbp-8]".into()),

        Expr::Id(name) => match env.get(name) {
            Some(off) => instrs.push(format!("mov rax, [rbp{}]", fmt_off(*off))),
            None => panic!("Unbound variable identifier {name}"),
        },

        Expr::UnOp(op, e) => {
            compile(e, si, env, lc, brk, instrs);
            match op {
                Op1::Add1 => {
                    assert_num_rax(instrs, lc);
                    instrs.push("add rax, 2".into());
                    assert_no_overflow(instrs, lc);
                }
                Op1::Sub1 => {
                    assert_num_rax(instrs, lc);
                    instrs.push("sub rax, 2".into());
                    assert_no_overflow(instrs, lc);
                }
                Op1::Negate => {
                    assert_num_rax(instrs, lc);
                    instrs.push("neg rax".into());
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
                Op1::Print => {
                    // Align stack to 16 bytes before call: rsp must be 16-byte aligned at call site.
                    // We save rax, align, call snek_print(rax), restore.
                    instrs.push("mov rdi, rax".into());
                    // sub rsp by 8 to align (rbp frame already pushed rbp so rsp is 8-misaligned here)
                    instrs.push("sub rsp, 8".into());
                    instrs.push("call snek_print".into());
                    instrs.push("add rsp, 8".into());
                }
            }
        }

        Expr::BinOp(op, e1, e2) => {
            let off = -(8 * si); // rbp-relative offset for spill slot

            compile(e1, si, env, lc, brk, instrs);
            instrs.push(format!("mov [rbp{}], rax", fmt_off(off)));
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
                    instrs.push(format!("add rax, [rbp{}]", fmt_off(off)));
                    assert_no_overflow(instrs, lc);
                }
                Op2::Minus => {
                    instrs.push("mov rbx, rax".into());
                    instrs.push(format!("mov rax, [rbp{}]", fmt_off(off)));
                    instrs.push("sub rax, rbx".into());
                    assert_no_overflow(instrs, lc);
                }
                Op2::Times => {
                    instrs.push("sar rax, 1".into());
                    instrs.push(format!("imul rax, [rbp{}]", fmt_off(off)));
                    assert_no_overflow(instrs, lc);
                }
                Op2::Less | Op2::Greater | Op2::LessEq | Op2::GreaterEq | Op2::Equal => {
                    instrs.push(format!("mov rbx, [rbp{}]", fmt_off(off)));
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
                instrs.push(format!("mov [rbp{}], rax", fmt_off(*off)));
            }
            None => panic!("Unbound variable identifier {name}"),
        },

        Expr::Let(bindings, body) => {
            let mut new_env = env.clone();
            let mut curr_si = si;
            let mut seen = std::collections::HashSet::new();

            for (name, expr) in bindings {
                if !seen.insert(name.clone()) { panic!("Duplicate binding") }
                let off = -(8 * curr_si);
                compile(expr, curr_si, &new_env, lc, brk, instrs);
                instrs.push(format!("mov [rbp{}], rax", fmt_off(off)));
                new_env = new_env.update(name.clone(), off);
                curr_si += 1;
            }

            compile(body, curr_si, &new_env, lc, brk, instrs);
        }

        Expr::Call(name, args) => {
            // Validate arity against known definitions is done at compile time in compile_program.
            // Push args right-to-left, then call, then clean up.
            // Each arg is compiled with increasing si to avoid clobbering spill slots.
            let n = args.len();

            // Align: after push rbp + mov rbp,rsp the stack is 16-byte aligned at function entry.
            // Each push rax is 8 bytes. We need rsp 16-byte aligned before the call instruction.
            // Number of pushes = n args. If n is odd, add one extra 8-byte pad.
            let pad = if n % 2 == 1 { 8usize } else { 0usize };
            if pad > 0 {
                instrs.push(format!("sub rsp, {pad}"));
            }

            for (i, arg) in args.iter().enumerate().rev() {
                compile(arg, si + i as i32, env, lc, brk, instrs);
                instrs.push("push rax".into());
            }

            instrs.push(format!("call fun_{name}"));

            let cleanup = n * 8 + pad;
            if cleanup > 0 {
                instrs.push(format!("add rsp, {cleanup}"));
            }
        }
    }
}

// Compute the maximum stack index (si) needed by an expression.
// This determines how much space to reserve with sub rsp.
fn max_si(e: &Expr, si: i32) -> i32 {
    match e {
        Expr::Num(_) | Expr::Bool(_) | Expr::Input | Expr::Id(_) => si,
        Expr::UnOp(_, e) => max_si(e, si),
        Expr::BinOp(_, e1, e2) => max_si(e1, si).max(max_si(e2, si + 1)).max(si + 1),
        Expr::If(c, t, f) => max_si(c, si).max(max_si(t, si)).max(max_si(f, si)),
        Expr::Block(es) => es.iter().map(|e| max_si(e, si)).max().unwrap_or(si),
        Expr::Loop(e) => max_si(e, si),
        Expr::Break(e) => max_si(e, si),
        Expr::Set(_, e) => max_si(e, si),
        Expr::Let(binds, body) => {
            let mut depth = si;
            let mut curr = si;
            for (_, e) in binds {
                depth = depth.max(max_si(e, curr));
                curr += 1;
            }
            depth.max(max_si(body, curr))
        }
        Expr::Call(_, args) => args.iter().enumerate()
            .map(|(i, a)| max_si(a, si + i as i32))
            .max().unwrap_or(si),
    }
}

fn compile_defn(defn: &Definition, lc: &mut i32) -> String {
    let mut instrs = vec![];

    // Build env: params at [rbp+16], [rbp+24], ...
    let mut env: HashMap<String, i32> = HashMap::new();
    for (i, param) in defn.params.iter().enumerate() {
        env = env.update(param.clone(), 16 + i as i32 * 8);
    }

    // Locals start at si=1 ([rbp-8]); compute max depth to allocate
    let depth = max_si(&defn.body, 1);
    // Round up to 16-byte alignment so rsp stays 16-byte aligned after sub
    let local_bytes = ((depth * 8) + 15) & !15;

    compile(&defn.body, 1, &env, lc, &None, &mut instrs);

    let body = instrs.join("\n  ");
    format!(
"fun_{}:
  push rbp
  mov rbp, rsp
  sub rsp, {local_bytes}
  {body}
  add rsp, {local_bytes}
  pop rbp
  ret",
        defn.name
    )
}

fn compile_program(prog: &Program) -> String {
    // Validate calls: check arity and that called functions exist
    let func_arities: std::collections::HashMap<String, usize> = prog.defns.iter()
        .map(|d| (d.name.clone(), d.params.len()))
        .collect();
    // Check for duplicate function names
    {
        let mut seen = std::collections::HashSet::new();
        for d in &prog.defns {
            if !seen.insert(d.name.clone()) { panic!("Duplicate binding") }
        }
    }

    let mut lc = 0;
    let mut parts = vec![];

    for defn in &prog.defns {
        validate_calls(&defn.body, &func_arities);
        parts.push(compile_defn(defn, &mut lc));
    }

    validate_calls(&prog.main, &func_arities);

    // Main entry: slot 1 ([rbp-8]) holds input, locals start at si=2
    // slot 1 = input, locals start at 2; ensure at least 2 slots allocated
    let main_depth = max_si(&prog.main, 2).max(2);
    let main_bytes = ((main_depth * 8) + 15) & !15;
    let mut main_instrs = vec![];
    compile(&prog.main, 2, &HashMap::new(), &mut lc, &None, &mut main_instrs);
    let main_body = main_instrs.join("\n  ");

    parts.push(format!(
"our_code_starts_here:
  push rbp
  mov rbp, rsp
  sub rsp, {main_bytes}
  mov [rbp-8], rdi
  {main_body}
  add rsp, {main_bytes}
  pop rbp
  ret"
    ));

    format!(
"section .text
extern snek_error
extern snek_print
global our_code_starts_here
{}
",
        parts.join("\n\n")
    )
}

fn validate_calls(e: &Expr, funcs: &std::collections::HashMap<String, usize>) {
    match e {
        Expr::Call(name, args) => {
            match funcs.get(name) {
                None => panic!("Unbound variable identifier {name}"),
                Some(&arity) if arity != args.len() =>
                    panic!("Invalid: wrong number of arguments for {name}"),
                _ => {}
            }
            for a in args { validate_calls(a, funcs); }
        }
        Expr::UnOp(_, e) => validate_calls(e, funcs),
        Expr::BinOp(_, e1, e2) => { validate_calls(e1, funcs); validate_calls(e2, funcs); }
        Expr::If(c, t, f) => { validate_calls(c, funcs); validate_calls(t, funcs); validate_calls(f, funcs); }
        Expr::Block(es) => { for e in es { validate_calls(e, funcs); } }
        Expr::Loop(e) => validate_calls(e, funcs),
        Expr::Break(e) => validate_calls(e, funcs),
        Expr::Set(_, e) => validate_calls(e, funcs),
        Expr::Let(binds, body) => {
            for (_, e) in binds { validate_calls(e, funcs); }
            validate_calls(body, funcs);
        }
        _ => {}
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

    // Parse as a sequence of top-level s-expressions
    let wrapped = format!("({})", src.trim());
    let top = parse(&wrapped).expect("Invalid");
    let sexps = match &top {
        Sexp::List(v) => v.as_slice(),
        _ => panic!("Invalid"),
    };

    let prog = parse_program(sexps);
    let asm = compile_program(&prog);

    File::create(&args[2])?.write_all(asm.as_bytes())?;
    Ok(())
}
