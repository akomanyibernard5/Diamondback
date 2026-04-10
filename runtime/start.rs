use std::env;

#[link(name = "our_code")]
extern "C" {
    #[link_name = "\x01our_code_starts_here"]
    fn our_code_starts_here(input: i64) -> i64;
}

#[export_name = "\x01snek_error"]
extern "C" fn snek_error(errcode: i64) {
    match errcode {
        1 => eprintln!("invalid argument"),
        2 => eprintln!("overflow"),
        _ => eprintln!("unknown error {errcode}"),
    }
    std::process::exit(1);
}

#[export_name = "\x01snek_print"]
extern "C" fn snek_print(val: i64) -> i64 {
    if val & 1 == 0 {
        println!("{}", val >> 1);
    } else if val == 3 {
        println!("true");
    } else if val == 1 {
        println!("false");
    }
    val
}

fn parse_input(s: &str) -> i64 {
    match s {
        "true"  => 3,
        "false" => 1,
        _ => {
            let n: i64 = s.parse().expect("Invalid input: not a number or boolean");
            if n < (i32::MIN as i64) || n > (i32::MAX as i64) {
                panic!("Invalid input: out of range");
            }
            n << 1
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let input = if args.len() >= 2 { parse_input(&args[1]) } else { 1 };

    let result: i64 = unsafe { our_code_starts_here(input) };

    if result & 1 == 0 {
        println!("{}", result >> 1);
    } else if result == 3 {
        println!("true");
    } else if result == 1 {
        println!("false");
    } else {
        println!("unknown value: {result}");
    }
}
