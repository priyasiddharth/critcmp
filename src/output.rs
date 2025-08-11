use std::collections::{BTreeMap, BTreeSet};
use std::iter;

use termcolor::{Color, ColorSpec, WriteColor};
use unicode_width::UnicodeWidthStr;

use statrs::distribution::{ContinuousCDF, StudentsT};

use crate::data;
use crate::Result;

#[derive(Clone, Debug)]
pub struct Comparison {
    name: String,
    benchmarks: Vec<Benchmark>,
    name_to_index: BTreeMap<String, usize>,
}

#[derive(Clone, Debug)]
pub struct Benchmark {
    name: String,
    nanoseconds: f64,
    stddev: Option<f64>,
    samples: Option<f64>,
    throughput: Option<data::Throughput>,
    /// Whether this is the best benchmark in a group. This is only populated
    /// when a `Comparison` is built.
    best: bool,
    /// The rank of this benchmark in a group. The best is always `1.0`. This
    /// is only populated when a `Comparison` is built.
    rank: f64,
}

impl Comparison {
    pub fn new(name: &str, benchmarks: Vec<Benchmark>) -> Comparison {
        let mut comp = Comparison {
            name: name.to_string(),
            benchmarks: benchmarks,
            name_to_index: BTreeMap::new(),
        };
        if comp.benchmarks.is_empty() {
            return comp;
        }

        comp.benchmarks.sort_by(|a, b| {
            a.nanoseconds.partial_cmp(&b.nanoseconds).unwrap()
        });
        comp.benchmarks[0].best = true;

        let top = comp.benchmarks[0].nanoseconds;
        for (i, b) in comp.benchmarks.iter_mut().enumerate() {
            comp.name_to_index.insert(b.name.to_string(), i);
            b.rank = b.nanoseconds / top;
        }
        comp
    }

    /// Return the biggest difference, percentage wise, between benchmarks
    /// in this comparison.
    ///
    /// If this comparison has fewer than two benchmarks, then 0 is returned.
    pub fn biggest_difference(&self) -> f64 {
        if self.benchmarks.len() < 2 {
            return 0.0;
        }
        let best = self.benchmarks[0].nanoseconds;
        let worst = self.benchmarks.last().unwrap().nanoseconds;
        ((worst - best) / best) * 100.0
    }

    fn get(&self, name: &str) -> Option<&Benchmark> {
        self.name_to_index.get(name).and_then(|&i| self.benchmarks.get(i))
    }

    pub fn welch_p_value(&self) -> Option<f64> {
        if self.benchmarks.len() != 2 {
            return None;
        }
        welch_p_value(&self.benchmarks[0], &self.benchmarks[1])
    }
}

impl Benchmark {
    pub fn from_data(b: &data::Benchmark) -> Benchmark {
        Benchmark {
            name: b.fullname().to_string(),
            nanoseconds: b.nanoseconds(),
            stddev: Some(b.stddev()),
            samples: b.sample_size(),
            throughput: b.throughput(),
            best: false,
            rank: 0.0,
        }
    }

    pub fn name(self, name: &str) -> Benchmark {
        Benchmark { name: name.to_string(), ..self }
    }
}

pub fn columns<W: WriteColor>(
    mut wtr: W,
    groups: &[Comparison],
) -> Result<()> {
    let mut columns = BTreeSet::new();
    for group in groups {
        for b in &group.benchmarks {
            columns.insert(b.name.to_string());
        }
    }
    let show_p = columns.len() == 2;

    write!(wtr, "group")?;
    for column in &columns {
        write!(wtr, "\t  {}", column)?;
    }
    if show_p {
        write!(wtr, "\t  p-value")?;
    }
    writeln!(wtr, "")?;

    write_divider(&mut wtr, '-', "group".width())?;
    for column in &columns {
        write!(wtr, "\t  ")?;
        write_divider(&mut wtr, '-', column.width())?;
    }
    if show_p {
        write!(wtr, "\t  ")?;
        write_divider(&mut wtr, '-', "p-value".width())?;
    }
    writeln!(wtr, "")?;

    for group in groups {
        if group.benchmarks.is_empty() {
            continue;
        }

        write!(wtr, "{}", group.name)?;
        for column_name in &columns {
            let b = match group.get(column_name) {
                Some(b) => b,
                None => {
                    write!(wtr, "\t")?;
                    continue;
                }
            };

            if b.best {
                let mut spec = ColorSpec::new();
                spec.set_fg(Some(Color::Green)).set_bold(true);
                wtr.set_color(&spec)?;
            }
            write!(
                wtr,
                "\t  {:<5.2} {:>14} {:>14}",
                b.rank,
                time(b.nanoseconds, b.stddev),
                throughput(b.throughput),
            )?;
            if b.best {
                wtr.reset()?;
            }
        }
        if show_p {
            if let Some(p) = group.welch_p_value() {
                write!(wtr, "\t  {:<5.3}", p)?;
            } else {
                write!(wtr, "\t")?;
            }
        }
        writeln!(wtr, "")?;
    }
    Ok(())
}

fn welch_p_value(a: &Benchmark, b: &Benchmark) -> Option<f64> {
    let (m1, m2) = (a.nanoseconds, b.nanoseconds);
    let (s1, s2) = (a.stddev?, b.stddev?);
    let (n1, n2) = (a.samples?, b.samples?);
    if n1 <= 1.0 || n2 <= 1.0 {
        return None;
    }
    let s1_sq = s1.powi(2);
    let s2_sq = s2.powi(2);
    let t_denom = (s1_sq / n1 + s2_sq / n2).sqrt();
    if t_denom == 0.0 {
        return None;
    }
    let t = (m1 - m2) / t_denom;
    let df_num = (s1_sq / n1 + s2_sq / n2).powi(2);
    let df_den = (s1_sq.powi(2) / ((n1 - 1.0) * n1.powi(2)))
        + (s2_sq.powi(2) / ((n2 - 1.0) * n2.powi(2)));
    if df_den == 0.0 {
        return None;
    }
    let df = df_num / df_den;
    let dist = StudentsT::new(0.0, 1.0, df).ok()?;
    Some(2.0 * (1.0 - dist.cdf(t.abs())))
}

pub fn rows<W: WriteColor>(mut wtr: W, groups: &[Comparison]) -> Result<()> {
    for (i, group) in groups.iter().enumerate() {
        if i > 0 {
            writeln!(wtr, "")?;
        }
        rows_one(&mut wtr, group)?;
    }
    Ok(())
}

fn rows_one<W: WriteColor>(mut wtr: W, group: &Comparison) -> Result<()> {
    writeln!(wtr, "{}", group.name)?;
    write_divider(&mut wtr, '-', group.name.width())?;
    writeln!(wtr, "")?;

    if group.benchmarks.is_empty() {
        writeln!(wtr, "NOTHING TO SHOW")?;
        return Ok(());
    }

    for b in &group.benchmarks {
        writeln!(
            wtr,
            "{}\t{:>7.2}\t{:>15}\t{:>12}",
            b.name,
            b.rank,
            time(b.nanoseconds, b.stddev),
            throughput(b.throughput),
        )?;
    }
    if let Some(p) = group.welch_p_value() {
        writeln!(wtr, "Welch p-value:\t{:.3}", p)?;
    }
    Ok(())
}

fn write_divider<W: WriteColor>(
    mut wtr: W,
    divider: char,
    width: usize,
) -> Result<()> {
    let div: String = iter::repeat(divider).take(width).collect();
    write!(wtr, "{}", div)?;
    Ok(())
}

fn time(nanos: f64, stddev: Option<f64>) -> String {
    const MIN_MICRO: f64 = 2_000.0;
    const MIN_MILLI: f64 = 2_000_000.0;
    const MIN_SEC: f64 = 2_000_000_000.0;

    let (div, label) = if nanos < MIN_MICRO {
        (1.0, "ns")
    } else if nanos < MIN_MILLI {
        (1_000.0, "µs")
    } else if nanos < MIN_SEC {
        (1_000_000.0, "ms")
    } else {
        (1_000_000_000.0, "s")
    };
    if let Some(stddev) = stddev {
        format!("{:.1}±{:.2}{}", nanos / div, stddev / div, label)
    } else {
        format!("{:.1}{}", nanos / div, label)
    }
}

fn throughput(throughput: Option<data::Throughput>) -> String {
    use data::Throughput::*;
    match throughput {
        Some(Bytes(num)) => throughput_per(num, "B"),
        Some(Elements(num)) => throughput_per(num, "Elem"),
        _ => "? ?/sec".to_string(),
    }
}

fn throughput_per(per: f64, unit: &str) -> String {
    const MIN_K: f64 = (2 * (1 << 10) as u64) as f64;
    const MIN_M: f64 = (2 * (1 << 20) as u64) as f64;
    const MIN_G: f64 = (2 * (1 << 30) as u64) as f64;

    if per < MIN_K {
        format!("{} {}/sec", per as u64, unit)
    } else if per < MIN_M {
        format!("{:.1} K{}/sec", (per / (1 << 10) as f64), unit)
    } else if per < MIN_G {
        format!("{:.1} M{}/sec", (per / (1 << 20) as f64), unit)
    } else {
        format!("{:.1} G{}/sec", (per / (1 << 30) as f64), unit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bm(mean: f64, stddev: f64, samples: f64) -> Benchmark {
        Benchmark {
            name: String::new(),
            nanoseconds: mean,
            stddev: Some(stddev),
            samples: Some(samples),
            throughput: None,
            best: false,
            rank: 0.0,
        }
    }

    #[test]
    fn welch_p_value_known_case() {
        let a = bm(10.0, 1.0, 10.0);
        let b = bm(12.0, 1.0, 10.0);
        let p = super::welch_p_value(&a, &b).unwrap();
        let expected = 0.000294564155366661f64;
        assert!((p - expected).abs() < 1e-12);
    }
}
