use fixed_num::*;
use std::str::FromStr;
use std::time::Instant;

fn main() {
    let val = Dec19x19::from_str("1.5").unwrap();

    let time = Instant::now();
    let fixed = val.format_prec(2);
    let elapsed = time.elapsed().as_nanos();
    println!("format_prec(2): {} ns", elapsed);
    assert_eq!(&*fixed, "1.50");
}