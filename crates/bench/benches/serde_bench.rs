use criterion::{black_box, Criterion};
use std::str::FromStr;
use paste::paste;
use std::path::{Path, PathBuf};
use std::io::Write;

use ::serde_json::{to_string, from_str};
use ::fixed_num::Dec19x19 as fixed_num;
use ::rust_decimal::Decimal as rust_decimal;
use ::bigdecimal::BigDecimal as bigdecimal;
use ::decimal_rs::Decimal as decimal_rs;
use validator::Series;

const WORKSPACE_ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn out_dir() -> PathBuf {
    Path::new(WORKSPACE_ROOT).join("target").join("criterion")
}

#[derive(Debug, Default)]
struct Buffer {
    ident: usize,
    str: String,
}

impl Buffer {
    fn line(&mut self, s: &str) {
        self.str.push_str(&"  ".repeat(self.ident));
        self.str.push_str(s);
        self.str.push('\n');
    }

    fn group_start(&mut self, s: &str) {
        self.line(s);
        self.ident += 1;
    }

    fn group_end(&mut self, s: &str) {
        self.ident -= 1;
        self.line(s);
    }
}

fn normalize_by(input: Vec<Option<f64>>, ix: usize) -> Vec<Option<f64>> {
    let base = input[ix].unwrap();
    input
        .into_iter()
        .map(|opt| opt.map(|val| base / val))
        .collect()
}

fn after_benchmarks(ops: &[&str], libs: &[&str]) {
    let out_dir = out_dir();
    let results = ops.iter().map(|op| {
        let results = libs.iter().map(|lib| {
            let path = out_dir.join(format!("{op} {lib}")).join("new").join("estimates.json");
            path.exists().then(|| {
                let content = std::fs::read_to_string(&path).unwrap();
                let json: serde_json::Value = from_str(&content).unwrap();
                json["median"]["point_estimate"].as_f64()
            }).flatten()
        }).collect::<Vec<_>>();
        normalize_by(results, 0)
    }).collect::<Vec<_>>();

    let mut out = Buffer::default();
    out.group_start("<table>");
    out.group_start("<thead>");
    out.group_start("<tr>");
    out.line("<th></th>");
    for lib in libs {
        out.line(&format!("<th>{lib}</th>"));
    }
    out.group_end("</tr>");
    out.group_end("</thead>");
    out.group_start("<tbody>");
    for (op, results) in ops.iter().zip(results) {
        out.group_start("<tr>");
        out.line(&format!("<td>{op}</td>"));
        let max = results
            .iter()
            .filter_map(|x| *x)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();
        for result in results {
            let norm = result.map(|x| x / max).unwrap_or(0.01);
            let coeff = ((1.0 + norm.log10()).max(0.0).min(1.0) * 100.0).round();
            let bg = format!("color-mix(in lch, #58760b {coeff}%, #c41c0d)");
            let fg_opacity = if norm == 1.0 || result.is_none() { 1.0 } else { 0.5 };
            let fg = format!("rgba(255, 255, 255, {fg_opacity})");
            let font = if norm == 1.0 { "bold" } else { "normal" };
            let style = format!(
                "style=\"color: {fg}; background-color: {bg}; font-weight: {font};\""
            ).replace("  ", " ");
            match result {
                Some(value) => out.line(&format!("<td {style}>{value:.2}</td>")),
                None => out.line(&format!("<td {style}>⚠️</td>")),
            }
        }
        out.group_end("</tr>");
    }
    out.group_end("</tbody>");
    out.group_end("</table>");

    let out_path = Path::new(WORKSPACE_ROOT).join("serde_results.html");
    let mut file = std::fs::File::create(&out_path).unwrap();
    file.write_all(out.str.as_bytes()).unwrap();
}

macro_rules! def_serde_bench {
    (serialize for [$($t:ty),* $(,)?]) => {
        paste! {
            $(
                #[allow(non_snake_case)]
                fn [<bench_serialize_ $t:snake>](c: &mut Criterion) {
                    let label = format!("serialize {}", stringify!($t));
                    let mut series = Series::new(0..=9, 0..=19);
                    series.seed = 7;
                    let s_series = validator::series_str::<fixed_num>(series);
                    let t_vec: Vec<$t> = s_series.iter().map(|s| black_box(<$t>::from_str(s).unwrap())).collect();
                    c.bench_function(&label, |bencher| {
                        bencher.iter(|| {
                            for t in &t_vec {
                                black_box(to_string(t).unwrap());
                            }
                        })
                    });
                }
            )*
        }
    };
    (deserialize for [$($t:ty),* $(,)?]) => {
        paste! {
            $(
                #[allow(non_snake_case)]
                fn [<bench_deserialize_ $t:snake>](c: &mut Criterion) {
                    let label = format!("deserialize {}", stringify!($t));
                    let mut series = Series::new(0..=9, 0..=19);
                    series.seed = 7;
                    let s_series = validator::series_str::<fixed_num>(series);
                    let json_vec: Vec<String> = s_series.iter().map(|s| {
                        format!(r#""{}""#, s)
                    }).collect();
                    c.bench_function(&label, |bencher| {
                        bencher.iter(|| {
                            for js in &json_vec {
                                black_box(from_str::<$t>(js).unwrap());
                            }
                        })
                    });
                }
            )*
        }
    };
}

def_serde_bench!(serialize for [fixed_num, rust_decimal, bigdecimal, decimal_rs]);
def_serde_bench!(deserialize for [fixed_num, rust_decimal, bigdecimal, decimal_rs]);

fn bench_serialize_f64(c: &mut Criterion) {
    let label = "serialize f64";
    let mut series = Series::new(0..=9, 0..=19);
    series.seed = 7;
    let s_series = validator::series_str::<fixed_num>(series);
    let t_vec: Vec<f64> = s_series.iter().map(|s| black_box(f64::from_str(s).unwrap())).collect();
    c.bench_function(&label, |bencher| {
        bencher.iter(|| {
            for t in &t_vec {
                black_box(to_string(t).unwrap());
            }
        })
    });
}

fn bench_deserialize_f64(c: &mut Criterion) {
    let label = "deserialize f64";
    let mut series = Series::new(0..=9, 0..=19);
    series.seed = 7;
    let s_series = validator::series_str::<fixed_num>(series);
    let json_vec: Vec<String> = s_series.iter().map(|s| format!("{}", s)).collect();
    c.bench_function(&label, |bencher| {
        bencher.iter(|| {
            for js in &json_vec {
                black_box(from_str::<f64>(js).unwrap());
            }
        })
    });
}

fn main() {
    let mut criterion = Criterion::default()
        .noise_threshold(1.0)
        .output_directory(&out_dir())
        .configure_from_args();

    // Serialize benchmarks
   // bench_serialize_fixed_num(&mut criterion);
   // bench_serialize_rust_decimal(&mut criterion);
   // bench_serialize_bigdecimal(&mut criterion);
   // bench_serialize_decimal_rs(&mut criterion);
   // bench_serialize_f64(&mut criterion);

    // Deserialize benchmarks
    bench_deserialize_fixed_num(&mut criterion);
    bench_deserialize_rust_decimal(&mut criterion);
   // bench_deserialize_bigdecimal(&mut criterion);
   // bench_deserialize_decimal_rs(&mut criterion);
    bench_deserialize_f64(&mut criterion);

    criterion.final_summary();

    let ops = &["serialize", "deserialize"];
    let libs = &["fixed_num", "rust_decimal", "bigdecimal", "decimal_rs", "f64"];
    after_benchmarks(ops, libs);
}