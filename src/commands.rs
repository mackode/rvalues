use std::collections::{HashMap, HashSet};
use std::fs;
use crate::expr::{self, Expr};
use crate::io::IOManager;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::io::Read;

// Shared argument parsing helpers
pub fn parse_indices(s: &str) -> Result<Vec<usize>, String> {
    let mut idx = Vec::new();
    if s.is_empty() {
        return Ok(idx);
    }
    for cle in s.split(',') {
        if cle.contains(':') {
            let fl: Vec<&str> = cle.split(':').collect();
            if fl.len() != 2 {
                return Err(format!("Invalid field: {}", cle));
            }
            let n1 = fl[0].parse::<i32>().map_err(|_| format!("Invalid range: {}", cle))?;
            let n2 = fl[1].parse::<i32>().map_err(|_| format!("Invalid range: {}", cle))?;
            if n1 < 1 || n2 < 1 {
                return Err(format!("Invalid range: {}", cle));
            }
            if n1 < n2 {
                let mut current = n1;
                while current <= n2 {
                    idx.push((current - 1) as usize);
                    current += 1;
                }
            } else {
                let mut current = n1;
                while current >= n2 {
                    idx.push((current - 1) as usize);
                    current -= 1;
                }
            }
        } else {
            let n = cle.parse::<i32>().map_err(|_| format!("Need integer, not '{}'", cle))?;
            if n < 1 {
                return Err(format!("Index must be greater than zero, not '{}'", cle));
            }
            idx.push((n - 1) as usize);
        }
    }
    Ok(idx)
}

pub fn should_skip_or_pass(
    row: &[String],
    line_no: usize,
    file_name: &str,
    skip_expr: Option<&Expr>,
    pass_expr: Option<&Expr>,
) -> (bool, bool) {
    let mut vars = HashMap::new();
    vars.insert("line".to_string(), line_no.to_string());
    vars.insert("file".to_string(), file_name.to_string());
    vars.insert("fields".to_string(), row.len().to_string());

    let skip = if let Some(e) = skip_expr {
        e.eval(row, &vars, None).map(|v| expr::to_bool(&v)).unwrap_or(false)
    } else {
        false
    };

    let pass = if let Some(e) = pass_expr {
        e.eval(row, &vars, None).map(|v| expr::to_bool(&v)).unwrap_or(false)
    } else {
        false
    };

    (skip, pass)
}

// 1. Echo Command
pub fn run_echo(
    mut io: IOManager,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }
        io.write_row(&row)?;
    }
    Ok(())
}

// 2. Head Command
pub fn run_head(
    mut io: IOManager,
    n: usize,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let mut row = Vec::new();
    let mut count = 0;
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }
        if count >= n {
            break;
        }
        io.write_row(&row)?;
        count += 1;
    }
    Ok(())
}

// 3. Tail Command
pub fn run_tail(
    mut io: IOManager,
    n: usize,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let mut row = Vec::new();
    let mut buffer = std::collections::VecDeque::new();
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }
        buffer.push_back(row.clone());
        if buffer.len() > n {
            buffer.pop_front();
        }
    }
    for r in buffer {
        io.write_row(&r)?;
    }
    Ok(())
}

// 4. Case Commands (upper, lower, mixed)
pub enum CaseType {
    Upper,
    Lower,
    Mixed,
}

pub fn run_case(
    mut io: IOManager,
    fields: Option<String>,
    case_type: CaseType,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let cols = fields.map(|s| parse_indices(&s)).transpose()?;
    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }
        if !pass {
            for i in 0..row.len() {
                if cols.is_none() || cols.as_ref().unwrap().contains(&i) {
                    row[i] = match case_type {
                        CaseType::Upper => row[i].to_uppercase(),
                        CaseType::Lower => row[i].to_lowercase(),
                        CaseType::Mixed => {
                            // Title Case (capitalize first char of each word)
                            let lower = row[i].to_lowercase();
                            let mut c = true;
                            lower.chars().map(|ch| {
                                if ch.is_whitespace() {
                                    c = true;
                                    ch
                                } else if c {
                                    c = false;
                                    ch.to_ascii_uppercase()
                                } else {
                                    ch
                                }
                            }).collect()
                        }
                    };
                }
            }
        }
        io.write_row(&row)?;
    }
    Ok(())
}

// 5. Exclude Command
pub fn run_exclude(
    mut io: IOManager,
    fields: String,
    reverse: bool,
    if_expr: Option<Expr>,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let excl_cols = parse_indices(&fields)?;
    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }

        let apply_exclude = if !pass {
            if let Some(ref e) = if_expr {
                let mut vars = HashMap::new();
                vars.insert("line".to_string(), io.current_line().to_string());
                vars.insert("file".to_string(), io.current_file_name().to_string());
                vars.insert("fields".to_string(), row.len().to_string());
                e.eval(&row, &vars, None).map(|v| expr::to_bool(&v)).unwrap_or(true)
            } else {
                true
            }
        } else {
            false
        };

        if apply_exclude {
            let mut new_row = Vec::new();
            let mut current = row.clone();
            if reverse {
                current.reverse();
            }
            for (idx, val) in current.iter().enumerate() {
                if !excl_cols.contains(&idx) {
                    new_row.push(val.clone());
                }
            }
            if reverse {
                new_row.reverse();
            }
            row = new_row;
        }

        io.write_row(&row)?;
    }
    Ok(())
}

// 6. Number formatting command
pub fn run_number(
    mut io: IOManager,
    fields: Option<String>,
    fmt: String,
    err_str: Option<String>,
    err_code: bool,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let cols = fields.map(|s| parse_indices(&s)).transpose()?;
    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }
        if !pass {
            for i in 0..row.len() {
                if cols.is_none() || cols.as_ref().unwrap().contains(&i) {
                    let ts = if fmt == "EU" { '.' } else { ',' };
                    let dp = if fmt == "EU" { ',' } else { '.' };

                    // Parse number
                    let mut s = String::new();
                    let mut havedp = false;
                    for c in row[i].chars() {
                        if c == dp {
                            havedp = true;
                            s.push('.');
                        } else if c == ts && !havedp {
                            continue;
                        } else {
                            s.push(c);
                        }
                    }

                    if s.parse::<f64>().is_ok() {
                        row[i] = s;
                    } else {
                        if err_code {
                            return Err(format!("Invalid number: {}", row[i]));
                        } else if let Some(ref estr) = err_str {
                            row[i] = estr.clone();
                        }
                    }
                }
            }
        }
        io.write_row(&row)?;
    }
    Ok(())
}

// 7. Sequence Command
pub fn run_sequence(
    mut io: IOManager,
    start: i32,
    inc: i32,
    pad: usize,
    col: usize,
    mask: Option<String>,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let mut seq = start;
    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }
        if !pass {
            let mut sn = if seq >= 0 && pad > 0 {
                format!("{:0width$}", seq, width = pad)
            } else {
                seq.to_string()
            };

            if let Some(ref m) = mask {
                if let Some(pos) = m.find('@') {
                    sn = format!("{}{}{}", &m[..pos], sn, &m[pos + 1..]);
                }
            }

            if col >= row.len() {
                row.push(sn);
            } else {
                row.insert(col, sn);
            }
            seq += inc;
        }
        io.write_row(&row)?;
    }
    Ok(())
}

// 8. Unique Command
pub fn run_unique(
    mut io: IOManager,
    fields: Option<String>,
    show_dupes: bool,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let cols = fields.map(|s| parse_indices(&s)).transpose()?;
    let mut row = Vec::new();
    
    struct RowInfo {
        first: Vec<String>,
        count: usize,
    }
    let mut map: HashMap<String, RowInfo> = HashMap::new();

    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }

        // Make key
        let mut key = String::new();
        if let Some(ref cs) = cols {
            for &c in cs {
                if c >= row.len() {
                    key.push('\0');
                } else {
                    key.push_str(&row[c]);
                    key.push('\0');
                }
            }
        } else {
            for val in &row {
                key.push_str(val);
                key.push('\0');
            }
        }

        if show_dupes {
            if let Some(info) = map.get_mut(&key) {
                if info.count == 1 {
                    io.write_row(&info.first)?;
                }
                info.count += 1;
                io.write_row(&row)?;
            } else {
                map.insert(key, RowInfo { first: row.clone(), count: 1 });
            }
        } else {
            if !map.contains_key(&key) {
                map.insert(key, RowInfo { first: row.clone(), count: 1 });
                io.write_row(&row)?;
            }
        }
    }
    Ok(())
}

// 9. Shuffle Command
pub fn run_shuffle(
    mut io: IOManager,
    seed: Option<u64>,
    fields: Option<String>,
    count: Option<usize>,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let seed_val = seed.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    });
    let mut rng = ChaCha8Rng::seed_from_u64(seed_val);

    let mut row = Vec::new();
    if let Some(ref fs) = fields {
        let cols = parse_indices(fs)?;
        while io.read_csv(&mut row)? {
            let (skip, pass) = should_skip_or_pass(
                &row,
                io.current_line(),
                io.current_file_name(),
                skip_expr.as_ref(),
                pass_expr.as_ref(),
            );
            if skip {
                continue;
            }
            if !pass {
                let mut vals = Vec::new();
                for &c in &cols {
                    if c < row.len() {
                        vals.push(row[c].clone());
                    }
                }
                vals.shuffle(&mut rng);
                for (idx, &c) in cols.iter().enumerate() {
                    if c < row.len() {
                        row[c] = vals[idx].clone();
                    }
                }
            }
            io.write_row(&row)?;
        }
    } else {
        let mut rows = Vec::new();
        while io.read_csv(&mut row)? {
            let (skip, pass) = should_skip_or_pass(
                &row,
                io.current_line(),
                io.current_file_name(),
                skip_expr.as_ref(),
                pass_expr.as_ref(),
            );
            if skip {
                continue;
            }
            rows.push(row.clone());
        }

        rows.shuffle(&mut rng);
        let limit = count.unwrap_or(rows.len()).min(rows.len());
        for r in &rows[..limit] {
            io.write_row(r)?;
        }
    }
    Ok(())
}

// 10. Sort Command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortCmpType {
    Alpha,
    Numeric,
    NoCase,
}

#[derive(Debug, Clone)]
pub struct SortField {
    pub index: usize,
    pub direction: SortDirection,
    pub cmp_type: SortCmpType,
}

fn parse_sort_specs(s: &str) -> Result<Vec<SortField>, String> {
    let mut fields = Vec::new();
    for spec in s.split(',') {
        if spec.is_empty() {
            continue;
        }
        let parts: Vec<&str> = spec.split(':').collect();
        if parts.len() < 1 || parts.len() > 2 {
            return Err(format!("Invalid field specification: {}", spec));
        }
        let idx_str = parts[0];
        let idx = idx_str.parse::<usize>().map_err(|_| format!("Index in field specification must be integer: {}", spec))?;
        if idx == 0 {
            return Err(format!("Index must be non-zero in field specification: {}", spec));
        }
        
        let flags = if parts.len() == 2 { parts[1] } else { "AS" };
        
        // Validate flags
        if flags.len() > 2 || flags.chars().any(|c| ! "ADNSI".contains(c)) {
            return Err(format!("Invalid field parameters: {}", flags));
        }
        
        let ok_combinations = [
            "A", "D", "I", "S", "N",
            "AI", "AS", "AN", "IA", "SA", "NA",
            "DI", "DS", "DN", "ID", "SD", "ND"
        ];
        if !ok_combinations.contains(&flags) {
            return Err(format!("Invalid field parameter combination: {}", flags));
        }
        
        let direction = if flags.contains('D') {
            SortDirection::Desc
        } else {
            SortDirection::Asc
        };
        
        let cmp_type = if flags.contains('N') {
            SortCmpType::Numeric
        } else if flags.contains('I') {
            SortCmpType::NoCase
        } else {
            SortCmpType::Alpha
        };
        
        fields.push(SortField {
            index: idx - 1,
            direction,
            cmp_type,
        });
    }
    Ok(fields)
}

pub fn run_sort(
    mut io: IOManager,
    fields: Option<String>,
    rh: bool,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let sort_fields = if let Some(ref f) = fields {
        parse_sort_specs(f)?
    } else {
        vec![SortField {
            index: 0,
            direction: SortDirection::Asc,
            cmp_type: SortCmpType::Alpha,
        }]
    };

    let mut row = Vec::new();
    let mut rows = Vec::new();
    let mut header = Vec::new();

    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }
        if !pass {
            if rh && header.is_empty() {
                header = row.clone();
                continue;
            }
        }
        rows.push(row.clone());
    }

    // Validate that numeric fields are valid floats
    for r in &rows {
        for f in &sort_fields {
            if f.cmp_type == SortCmpType::Numeric && f.index < r.len() {
                let s = &r[f.index];
                if s.trim_start().parse::<f64>().is_err() {
                    return Err(format!("Invalid real value '{}'", s));
                }
            }
        }
    }

    rows.sort_by(|a, b| {
        let nc = std::cmp::min(a.len(), b.len());
        for f in &sort_fields {
            if f.index >= nc {
                continue;
            }
            let va = &a[f.index];
            let vb = &b[f.index];
            if va == vb {
                continue;
            }
            let cmp = match f.cmp_type {
                SortCmpType::Alpha => va.cmp(vb),
                SortCmpType::Numeric => {
                    let da = va.trim_start().parse::<f64>().unwrap_or(0.0);
                    let db = vb.trim_start().parse::<f64>().unwrap_or(0.0);
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                }
                SortCmpType::NoCase => {
                    va.to_lowercase().cmp(&vb.to_lowercase())
                }
            };
            if cmp != std::cmp::Ordering::Equal {
                return match f.direction {
                    SortDirection::Asc => cmp,
                    SortDirection::Desc => cmp.reverse(),
                };
            }
        }
        std::cmp::Ordering::Equal
    });

    if rh && !header.is_empty() {
        let mut line = String::new();
        for (i, val) in header.iter().enumerate() {
            line.push_str(val);
            if i != header.len() - 1 {
                line.push(',');
            }
        }
        writeln!(io.output_writer, "{}", line).map_err(|e| e.to_string())?;
    }

    for r in rows {
        io.write_row(&r)?;
    }
    Ok(())
}

// 11. Trim Command
pub fn run_trim(
    mut io: IOManager,
    fields: Option<String>,
    left: bool,
    right: bool,
    widths: Option<String>,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let cols = fields.map(|s| parse_indices(&s)).transpose()?;
    
    let mut width_vals = Vec::new();
    if let Some(ref ws) = widths {
        for w in ws.split(',') {
            let n = w.parse::<i32>().map_err(|_| format!("Invalid width {}", w))?;
            width_vals.push(n);
        }
    }

    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }
        if !pass {
            for i in 0..row.len() {
                if cols.is_none() || cols.as_ref().unwrap().contains(&i) {
                    row[i] = if left && right {
                        row[i].trim().to_string()
                    } else if left {
                        row[i].trim_start().to_string()
                    } else if right {
                        row[i].trim_end().to_string()
                    } else {
                        row[i].trim().to_string()
                    };
                }

                if !width_vals.is_empty() {
                    let should_chop = if cols.is_none() {
                        i < width_vals.len()
                    } else {
                        cols.as_ref().unwrap().iter().position(|&x| x == i)
                            .map(|idx| idx < width_vals.len())
                            .unwrap_or(false)
                    };

                    if should_chop {
                        let idx = if cols.is_none() {
                            i
                        } else {
                            cols.as_ref().unwrap().iter().position(|&x| x == i).unwrap()
                        };
                        let w = width_vals[idx];
                        if w >= 0 {
                            let limit = w as usize;
                            row[i] = row[i].chars().take(limit).collect::<String>();
                        }
                    }
                }
            }
        }
        io.write_row(&row)?;
    }
    Ok(())
}

// 12. Truncate Command
pub fn run_truncate(
    mut io: IOManager,
    count: usize,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }
        if !pass {
            if count < row.len() {
                row.truncate(count);
            }
        }
        io.write_row(&row)?;
    }
    Ok(())
}

// 13. Pad Command
pub fn run_pad(
    mut io: IOManager,
    count: Option<usize>,
    pad_vals: Option<String>,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let pad_list: Vec<String> = if let Some(ref pv) = pad_vals {
        pv.split(',').map(|s| s.to_string()).collect()
    } else {
        Vec::new()
    };
    
    let ncolspec = count.is_some();
    let ncols = count.unwrap_or(0);
    
    if !ncolspec && pad_list.is_empty() {
        return Err("Need -n flag to specify field count".to_string());
    }

    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }
        if !pass {
            let target_cols = if ncolspec { ncols } else { row.len() + pad_list.len() };
            let sz = row.len();
            for i in sz..target_cols {
                if pad_list.is_empty() {
                    row.push(String::new());
                } else {
                    let ci = i - sz;
                    if ci >= pad_list.len() {
                        row.push(pad_list[pad_list.len() - 1].clone());
                    } else {
                        row.push(pad_list[ci].clone());
                    }
                }
            }
        }
        io.write_row(&row)?;
    }
    Ok(())
}

// 14. Escape Command
pub fn run_escape(
    mut io: IOManager,
    fields: Option<String>,
    chars_str: Option<String>,
    esc: String,
    sql_mode: bool,
    escape_off: bool,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let cols = fields.map(|s| parse_indices(&s)).transpose()?;
    let mut special = String::new();
    let escape_str;

    if sql_mode {
        if chars_str.is_some() || esc != "\\" {
            return Err("Cannot specify -sql with -s or -e".to_string());
        }
        escape_str = "'".to_string();
        special = "'".to_string();
    } else {
        special = chars_str.ok_or("escape needs characters to escape (-s)")?;
        escape_str = esc;
        if escape_str.chars().count() == 1 {
            special.push(escape_str.chars().next().unwrap());
        }
    }

    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }
        if !pass {
            for i in 0..row.len() {
                if cols.is_none() || cols.as_ref().unwrap().contains(&i) {
                    if sql_mode {
                        row[i] = sql_quote(&row[i]);
                    } else if row[i].chars().any(|c| special.contains(c)) {
                        let mut s = String::new();
                        for c in row[i].chars() {
                            if special.contains(c) {
                                s.push_str(&escape_str);
                            }
                            s.push(c);
                        }
                        row[i] = s;
                    }
                }
            }
        }
        if escape_off {
            let line = row.join(&io.output_sep.to_string());
            writeln!(io.output_writer, "{}", line).map_err(|e| e.to_string())?;
        } else {
            io.write_row(&row)?;
        }
    }
    Ok(())
}

// 15. Template Command
pub fn run_template(
    mut io: IOManager,
    tpl_file: String,
    fn_tpl: Option<String>,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let tpl_content = fs::read_to_string(&tpl_file)
        .map_err(|e| format!("Cannot open file {} for input: {}", tpl_file, e))?;

    let replace_cols = |tplate: &str, row: &[String]| -> Result<String, String> {
        let mut out = String::new();
        let mut chars = tplate.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some(escaped) => out.push(escaped),
                    None => return Err("Invalid escape at end of line".to_string()),
                }
            } else if c == '{' {
                let mut placeholder = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some('\n') | Some('\r') | None => return Err("Missing closing brace in template".to_string()),
                        Some(pc) => placeholder.push(pc),
                    }
                }

                if placeholder.starts_with('@') {
                    // Evaluate expression
                    let expr_str = &placeholder[1..];
                    let exprs = expr::parse(expr_str)?;
                    if let Some(expr) = exprs.last() {
                        let mut vars = HashMap::new();
                        vars.insert("fields".to_string(), row.len().to_string());
                        let val = expr.eval(row, &vars, None)?;
                        out.push_str(&val);
                    }
                } else if let Ok(idx) = placeholder.parse::<usize>() {
                    if idx > 0 && idx - 1 < row.len() {
                        out.push_str(&row[idx - 1]);
                    }
                } else {
                    return Err(format!("Invalid placeholder: {{{}}}", placeholder));
                }
            } else {
                out.push(c);
            }
        }
        Ok(out)
    };

    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }

        let formatted = replace_cols(&tpl_content, &row)?;
        if let Some(ref fn_template) = fn_tpl {
            let out_fname = replace_cols(fn_template, &row)?;
            fs::write(&out_fname, formatted)
                .map_err(|e| format!("Cannot open file {} for output: {}", out_fname, e))?;
        } else {
            write!(io.output_writer, "{}", formatted)
                .map_err(|e| format!("Failed to write templated output: {}", e))?;
        }
    }
    Ok(())
}

// 16. TO_XML Command
pub enum SpecNode {
    Tag(TagSpec),
    Text(TextSpec),
}

pub struct TagSpec {
    pub indent: usize,
    pub name: String,
    pub group: Vec<usize>,
    pub attribs: Vec<(String, usize)>,
    pub children: Vec<SpecNode>,
}

pub struct TextSpec {
    pub indent: usize,
    pub field: usize,
    pub is_cdata: bool,
}

pub fn parse_xml_spec(content: &str) -> Result<TagSpec, String> {
    let mut root: Option<TagSpec> = None;
    let mut stack: Vec<TagSpec> = Vec::new();
    let mut has_root = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let indent = line.len() - line.trim_start().len();
        let tokens: Vec<String> = trimmed.split_whitespace().map(|s| s.to_string()).collect();
        if tokens.len() < 2 {
            return Err(format!("Invalid spec line: {}", trimmed));
        }

        if tokens[0] == "tag" {
            let name = tokens[1].clone();
            let mut group = Vec::new();
            let mut attribs = Vec::new();
            let mut pos = 2;
            while pos < tokens.len() {
                if tokens[pos] == "group" {
                    pos += 1;
                    if pos >= tokens.len() {
                        return Err("No group fields specified".to_string());
                    }
                    group = tokens[pos].split(',')
                        .filter_map(|s| s.parse::<usize>().ok())
                        .map(|x| if x > 0 { x - 1 } else { 0 })
                        .collect();
                    pos += 1;
                } else if tokens[pos] == "attrib" {
                    if pos + 2 >= tokens.len() {
                        return Err("Invalid attrib spec".to_string());
                    }
                    let attr_name = tokens[pos + 1].clone();
                    let field = tokens[pos + 2].parse::<usize>().map_err(|_| "Invalid field index in attrib".to_string())?;
                    attribs.push((attr_name, if field > 0 { field - 1 } else { 0 }));
                    pos += 3;
                } else {
                    return Err(format!("Invalid token: {}", tokens[pos]));
                }
            }

            let tag = TagSpec {
                indent,
                name,
                group,
                attribs,
                children: Vec::new(),
            };

            if !has_root {
                if indent != 0 {
                    return Err("Root tag must have 0 indentation".to_string());
                }
                has_root = true;
                stack.push(tag);
            } else {
                while !stack.is_empty() && stack.last().unwrap().indent >= indent {
                    let child = stack.pop().unwrap();
                    if !stack.is_empty() {
                        stack.last_mut().unwrap().children.push(SpecNode::Tag(child));
                    } else {
                        root = Some(child);
                    }
                }
                if stack.is_empty() {
                    return Err("Only one root tag allowed".to_string());
                }
                stack.push(tag);
            }
        } else if tokens[0] == "text" || tokens[0] == "cdata" {
            let is_cdata = tokens[0] == "cdata";
            let field = tokens[1].parse::<usize>().map_err(|_| "Field must be integer in text spec".to_string())?;
            let text = TextSpec {
                indent,
                field: if field > 0 { field - 1 } else { 0 },
                is_cdata,
            };

            while !stack.is_empty() && stack.last().unwrap().indent >= indent {
                let child = stack.pop().unwrap();
                if !stack.is_empty() {
                    stack.last_mut().unwrap().children.push(SpecNode::Tag(child));
                } else {
                    root = Some(child);
                }
            }
            if stack.is_empty() {
                return Err("Text spec has no parent tag".to_string());
            }
            stack.last_mut().unwrap().children.push(SpecNode::Text(text));
        } else {
            return Err(format!("Unknown node type: {}", tokens[0]));
        }
    }

    while stack.len() > 1 {
        let child = stack.pop().unwrap();
        stack.last_mut().unwrap().children.push(SpecNode::Tag(child));
    }
    if !stack.is_empty() {
        root = Some(stack.pop().unwrap());
    }

    root.ok_or_else(|| "Empty specification file".to_string())
}

pub fn make_xml(
    writer: &mut dyn std::io::Write,
    tag: &TagSpec,
    rows: &[Vec<String>],
    indent_level: usize,
    indent_chars: &str,
    end_tags: bool,
) -> Result<(), String> {
    if rows.is_empty() {
        return Ok(());
    }

    let slices = make_slices(tag, rows);
    for slice in slices {
        let first_row = &slice[0];
        let indent = indent_chars.repeat(indent_level);
        write!(writer, "{}<{}", indent, tag.name).map_err(|e| e.to_string())?;
        
        for (attr_name, field_idx) in &tag.attribs {
            let val = if *field_idx < first_row.len() {
                html_escape(&first_row[*field_idx])
            } else {
                String::new()
            };
            write!(writer, " {}=\"{}\"", attr_name, val).map_err(|e| e.to_string())?;
        }
        
        if tag.children.is_empty() && !end_tags {
            writeln!(writer, " />").map_err(|e| e.to_string())?;
            continue;
        }
        
        writeln!(writer, ">").map_err(|e| e.to_string())?;
        
        for child in &tag.children {
            match child {
                SpecNode::Tag(child_tag) => {
                    make_xml(writer, child_tag, &slice, indent_level + 1, indent_chars, end_tags)?;
                }
                SpecNode::Text(text_spec) => {
                    let data = if text_spec.field < first_row.len() {
                        if text_spec.is_cdata {
                            first_row[text_spec.field].clone()
                        } else {
                            html_escape(&first_row[text_spec.field])
                        }
                    } else {
                        String::new()
                    };
                    
                    let child_indent = indent_chars.repeat(indent_level + 1);
                    if text_spec.is_cdata {
                        writeln!(writer, "{}<![CDATA[", child_indent).map_err(|e| e.to_string())?;
                        writeln!(writer, "{}{}", indent_chars.repeat(indent_level + 2), data).map_err(|e| e.to_string())?;
                        writeln!(writer, "{}]]>", child_indent).map_err(|e| e.to_string())?;
                    } else {
                        writeln!(writer, "{}{}", child_indent, data).map_err(|e| e.to_string())?;
                    }
                }
            }
        }
        
        writeln!(writer, "{}</{}>", indent, tag.name).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn make_slices(tag: &TagSpec, rows: &[Vec<String>]) -> Vec<Vec<Vec<String>>> {
    if tag.group.is_empty() {
        return vec![rows.to_vec()];
    }
    
    let mut slices = Vec::new();
    let mut current_slice = Vec::new();
    let mut last_key = Vec::new();
    
    for row in rows {
        let key = get_group_key(tag, row);
        if current_slice.is_empty() {
            current_slice.push(row.clone());
            last_key = key;
        } else if key == last_key {
            current_slice.push(row.clone());
        } else {
            slices.push(current_slice);
            current_slice = vec![row.clone()];
            last_key = key;
        }
    }
    if !current_slice.is_empty() {
        slices.push(current_slice);
    }
    slices
}

fn get_group_key(tag: &TagSpec, row: &[String]) -> Vec<String> {
    let mut key = Vec::new();
    for &g in &tag.group {
        if g < row.len() {
            key.push(row[g].clone());
        } else {
            key.push(String::new());
        }
    }
    key
}

fn html_escape(s: &str) -> String {
    let mut escaped = String::new();
    for c in s.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

pub fn run_to_xml(
    mut io: IOManager,
    xml_spec: Option<String>,
    indent: String,
    end_tags: bool,
    skip_expr: Option<Expr>,
) -> Result<(), String> {
    let indent_chars = if indent == "tabs" {
        "\t".to_string()
    } else {
        let count = indent.parse::<usize>().unwrap_or(4);
        " ".repeat(count)
    };

    let mut row = Vec::new();
    let mut rows = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, _) = should_skip_or_pass(&row, io.current_line(), io.current_file_name(), skip_expr.as_ref(), None);
        if !skip {
            rows.push(row.clone());
        }
    }

    if let Some(spec_file) = xml_spec {
        let spec_content = fs::read_to_string(&spec_file)
            .map_err(|e| format!("Cannot open XML specification file: {}: {}", spec_file, e))?;
        let root_spec = parse_xml_spec(&spec_content)?;
        make_xml(&mut io.output_writer, &root_spec, &rows, 0, &indent_chars, end_tags)?;
    } else {
        // Output simple XHTML table
        writeln!(io.output_writer, "<table>").map_err(|e| e.to_string())?;
        for r in rows {
            write!(io.output_writer, "{}<tr>\n{}", indent_chars, &indent_chars.repeat(2)).map_err(|e| e.to_string())?;
            for field in r {
                write!(io.output_writer, "<td>{}</td>", html_escape(&field)).map_err(|e| e.to_string())?;
            }
            writeln!(io.output_writer, "\n{}</tr>", indent_chars).map_err(|e| e.to_string())?;
        }
        writeln!(io.output_writer, "</table>").map_err(|e| e.to_string())?;
    }
    Ok(())
}

// 17. FROM_XML Command
fn matches_path(path: &str, pattern: &str) -> bool {
    if path == pattern {
        return true;
    }
    if path.ends_with(pattern) {
        let prefix_len = path.len() - pattern.len();
        if prefix_len > 0 && path.as_bytes()[prefix_len - 1] == b'@' {
            return true;
        }
    }
    false
}

fn should_exclude_path(path: &str, patterns: &[String]) -> bool {
    for pat in patterns {
        if matches_path(path, pat) {
            return true;
        }
    }
    false
}

fn make_path_to(node: roxmltree::Node) -> String {
    let mut path = String::new();
    let mut curr = Some(node);
    while let Some(n) = curr {
        if n.tag_name().name() != "" {
            if !path.is_empty() {
                path = format!("{}@{}", n.tag_name().name(), path);
            } else {
                path = n.tag_name().name().to_string();
            }
        }
        curr = n.parent();
    }
    path
}

fn output_record_data(
    row: &mut Vec<String>,
    node: roxmltree::Node,
    record_path: &str,
    no_attrib: bool,
    no_child: bool,
    exclude_paths: &[String],
    ml_sep: &str,
    is_parent_recursion: bool,
) {
    let path = make_path_to(node);
    if should_exclude_path(&path, exclude_paths) {
        return;
    }

    // Check if empty leaf
    let is_empty_leaf = {
        let has_attrs = node.attributes().count() > 0;
        let mut has_kids = false;
        for child in node.children() {
            if child.is_element() {
                has_kids = true;
                break;
            }
            if child.is_text() && !child.text().unwrap_or("").trim().is_empty() {
                has_kids = true;
                break;
            }
        }
        !(has_attrs || has_kids)
    };

    if is_empty_leaf {
        row.push(String::new());
        return;
    }

    // Output attributes
    if !no_attrib {
        for attr in node.attributes() {
            row.push(attr.value().to_string());
        }
    }

    let mut text_buf = String::new();
    let mut have_text = false;

    for child in node.children() {
        if child.is_element() {
            if have_text {
                row.push(text_buf.clone());
                have_text = false;
                text_buf.clear();
            }

            let child_path = make_path_to(child);
            if !is_parent_recursion && !no_child {
                output_record_data(row, child, record_path, no_attrib, no_child, exclude_paths, ml_sep, false);
            } else if is_parent_recursion && !record_path.starts_with(&child_path) {
                output_record_data(row, child, record_path, no_attrib, no_child, exclude_paths, ml_sep, true);
            }
        } else if child.is_text() {
            if let Some(t) = child.text() {
                let trimmed = t.trim();
                if !trimmed.is_empty() {
                    if !text_buf.is_empty() {
                        text_buf.push_str(ml_sep);
                    }
                    text_buf.push_str(trimmed);
                    have_text = true;
                }
            }
        }
    }

    if have_text {
        row.push(text_buf);
    }
}

pub fn run_from_xml(
    mut io: IOManager,
    xml_files: Vec<String>,
    re_paths: String,
    ex_paths: Option<String>,
    no_parent: bool,
    no_attrib: bool,
    no_child: bool,
    insert_path: bool,
    ml_sep: String,
) -> Result<(), String> {
    let re_patterns: Vec<String> = re_paths.split(',').map(|s| s.trim().to_string()).collect();
    let ex_patterns: Vec<String> = ex_paths.map_or(Vec::new(), |s| s.split(',').map(|x| x.trim().to_string()).collect());

    let process_file = |content: &str, io: &mut IOManager| -> Result<(), String> {
        let doc = roxmltree::Document::parse(content)
            .map_err(|e| format!("Failed to parse XML: {}", e))?;
        
        let mut parents = Vec::new();

        fn walk<'a, 'b>(
            n: roxmltree::Node<'a, 'b>,
            p: &mut Vec<roxmltree::Node<'a, 'b>>,
            re_patterns: &[String],
            ex_patterns: &[String],
            insert_path: bool,
            no_parent: bool,
            no_attrib: bool,
            no_child: bool,
            ml_sep: &str,
            io: &mut IOManager,
        ) -> Result<(), String> {
            let path = make_path_to(n);
            let mut is_rec = false;
            for pat in re_patterns {
                if matches_path(&path, pat) {
                    is_rec = true;
                    break;
                }
            }

            if is_rec {
                let mut row = Vec::new();
                if insert_path {
                    row.push(path.clone());
                }

                // Output parents
                if !no_parent {
                    for &parent_node in p.iter() {
                        output_record_data(&mut row, parent_node, &path, no_attrib, no_child, ex_patterns, ml_sep, true);
                    }
                }

                // Output current node data
                output_record_data(&mut row, n, &path, no_attrib, no_child, ex_patterns, ml_sep, false);

                io.write_row(&row)?;
            } else {
                p.push(n);
                for child in n.children() {
                    if child.is_element() {
                        walk(child, p, re_patterns, ex_patterns, insert_path, no_parent, no_attrib, no_child, ml_sep, io)?;
                    }
                }
                p.pop();
            }
            Ok(())
        }

        walk(doc.root_element(), &mut parents, &re_patterns, &ex_patterns, insert_path, no_parent, no_attrib, no_child, &ml_sep, io)?;
        Ok(())
    };

    for file_path in xml_files {
        let content = if file_path == "-" {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s).map_err(|e| e.to_string())?;
            s
        } else {
            fs::read_to_string(&file_path).map_err(|e| format!("Cannot open file {}: {}", file_path, e))?
        };
        process_file(&content, &mut io)?;
    }

    Ok(())
}

// 18. ASCII_TABLE Command
pub fn run_ascii_table(
    mut io: IOManager,
    header: Option<String>,
    right_align: Option<String>,
    use_line_sep: bool,
    skip_expr: Option<Expr>,
) -> Result<(), String> {
    let ra_cols = right_align.map(|s| parse_indices(&s)).transpose()?.unwrap_or_default();
    let mut row = Vec::new();
    let mut rows = Vec::new();
    let mut widths = Vec::new();

    let mut has_header_option = false;
    let mut header_is_file = false;
    if let Some(ref h) = header {
        has_header_option = true;
        if h == "@" {
            header_is_file = true;
        } else {
            let h_row: Vec<String> = h.split(',').map(|s| s.to_string()).collect();
            add_ascii_row(&h_row, &mut rows, &mut widths);
        }
    }

    let mut is_first_row = true;
    while io.read_csv(&mut row)? {
        let (skip, _) = should_skip_or_pass(&row, io.current_line(), io.current_file_name(), skip_expr.as_ref(), None);
        if skip {
            continue;
        }

        if is_first_row && has_header_option && header_is_file {
            add_ascii_row(&row, &mut rows, &mut widths);
            is_first_row = false;
        } else {
            add_ascii_row(&row, &mut rows, &mut widths);
            is_first_row = false;
        }
    }

    if rows.is_empty() {
        return Ok(());
    }

    let make_sep = |widths: &[usize]| -> String {
        let mut sep = "+".to_string();
        for &w in widths {
            sep.push_str(&format!("-{}-+", "-".repeat(w)));
        }
        sep
    };

    let sep = make_sep(&widths);
    
    writeln!(io.output_writer, "{} ", sep).map_err(|e| e.to_string())?;
    for (idx, r) in rows.iter().enumerate() {
        if idx == 0 && has_header_option {
            write!(io.output_writer, "|").map_err(|e| e.to_string())?;
            for (i, &w) in widths.iter().enumerate() {
                let val = if i < r.len() { &r[i] } else { "" };
                let padded = centre_align(val, w);
                write!(io.output_writer, " {} |", padded).map_err(|e| e.to_string())?;
            }
            writeln!(io.output_writer).map_err(|e| e.to_string())?;
            writeln!(io.output_writer, "{}", sep).map_err(|e| e.to_string())?;
        } else {
            write!(io.output_writer, "|").map_err(|e| e.to_string())?;
            for (i, &w) in widths.iter().enumerate() {
                let val = if i < r.len() { &r[i] } else { "" };
                let padded = if ra_cols.contains(&i) {
                    left_pad_str(val, w)
                } else {
                    right_pad_str(val, w)
                };
                write!(io.output_writer, " {} |", padded).map_err(|e| e.to_string())?;
            }
            writeln!(io.output_writer).map_err(|e| e.to_string())?;
            if use_line_sep {
                writeln!(io.output_writer, "{}", sep).map_err(|e| e.to_string())?;
            }
        }
    }
    if !use_line_sep {
        writeln!(io.output_writer, "{}", sep).map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn add_ascii_row(row: &[String], rows: &mut Vec<Vec<String>>, widths: &mut Vec<usize>) {
    let n = row.len();
    while widths.len() < n {
        widths.push(0);
    }
    for (i, val) in row.iter().enumerate() {
        if widths[i] < val.len() {
            widths[i] = val.len();
        }
    }
    rows.push(row.to_vec());
}

fn left_pad_str(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{}{}", " ".repeat(width - s.len()), s)
    }
}

fn right_pad_str(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - s.len()))
    }
}

fn centre_align(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        let diff = width - s.len();
        let left = diff / 2;
        let right = diff - left;
        format!("{}{}{}", " ".repeat(left), s, " ".repeat(right))
    }
}

// 19. READ_DSV Command
pub fn run_read_dsv(
    mut io: IOManager,
    fields: Option<String>,
    delim: char,
    csv: bool,
    cm: bool,
    skip_expr: Option<Expr>,
) -> Result<(), String> {
    let cols = fields.map(|s| parse_indices(&s)).transpose()?;
    let mut line = String::new();
    while io.read_line(&mut line)? {
        let parsed_row = parse_dsv_line(&line, delim, cm, csv)?;
        
        let (skip, _) = should_skip_or_pass(&parsed_row, io.current_line(), io.current_file_name(), skip_expr.as_ref(), None);
        if skip {
            continue;
        }

        let mut out_row = Vec::new();
        if let Some(ref cs) = cols {
            for &c in cs {
                if c < parsed_row.len() {
                    out_row.push(parsed_row[c].clone());
                } else {
                    out_row.push(String::new());
                }
            }
        } else {
            out_row = parsed_row;
        }

        io.write_row(&out_row)?;
    }
    Ok(())
}

fn parse_dsv_line(line: &str, delim: char, collapse_sep: bool, is_csv: bool) -> Result<Vec<String>, String> {
    let mut row = Vec::new();
    let mut val = String::new();
    let mut chars = line.chars().peekable();
    
    while let Some(c) = chars.next() {
        if c == delim {
            if collapse_sep {
                while chars.peek() == Some(&delim) {
                    chars.next();
                }
            }
            row.push(unquote_dsv(&val, is_csv));
            val.clear();
        } else if c == '\\' {
            if let Some(esc) = chars.next() {
                val.push(esc);
            } else {
                return Err("Escape at end of line".to_string());
            }
        } else {
            val.push(c);
        }
    }
    row.push(unquote_dsv(&val, is_csv));
    Ok(row)
}

fn unquote_dsv(s: &str, is_csv: bool) -> String {
    if !is_csv {
        return s.to_string();
    }
    let mut t = s;
    if (t.starts_with('"') && t.ends_with('"') && t.len() >= 2) ||
       (t.starts_with('\'') && t.ends_with('\'') && t.len() >= 2) {
        t = &t[1..t.len() - 1];
    }
    let mut res = String::new();
    let mut chars = t.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '"' && chars.peek() == Some(&'"') {
            res.push('"');
            chars.next();
        } else {
            res.push(c);
        }
    }
    res
}

// 20. BLOCK Command
pub fn run_block(
    mut io: IOManager,
    begin_expr: Expr,
    end_expr: Expr,
    action: BlockAction,
    exclusive: bool,
) -> Result<(), String> {
    let mut row = Vec::new();
    let mut in_block = false;
    
    while io.read_csv(&mut row)? {
        let line_no = io.current_line();
        let file_name = io.current_file_name().to_string();
        
        let mut vars = HashMap::new();
        vars.insert("line".to_string(), line_no.to_string());
        vars.insert("file".to_string(), file_name.clone());
        vars.insert("fields".to_string(), row.len().to_string());
        
        let block_state = if !in_block {
            let at_begin = begin_expr.eval(&row, &vars, None)
                .map(|v| expr::to_bool(&v))
                .unwrap_or(false);
            if at_begin {
                in_block = true;
                !exclusive
            } else {
                false
            }
        } else {
            let at_end = end_expr.eval(&row, &vars, None)
                .map(|v| expr::to_bool(&v))
                .unwrap_or(false);
            if at_end {
                in_block = false;
                !exclusive
            } else {
                true
            }
        };
        
        match &action {
            BlockAction::Mark(block_mark, not_mark) => {
                let mut tmp = Vec::new();
                tmp.push(if block_state { block_mark.clone() } else { not_mark.clone() });
                tmp.extend(row.clone());
                io.write_row(&tmp)?;
            }
            BlockAction::Keep => {
                if block_state {
                    io.write_row(&row)?;
                }
            }
            BlockAction::Remove => {
                if !block_state {
                    io.write_row(&row)?;
                }
            }
        }
    }
    Ok(())
}

pub enum BlockAction {
    Keep,
    Remove,
    Mark(String, String),
}

// 21. CHECK Command
pub struct CSVChecker<'a> {
    file_name: String,
    field_sep: char,
    dq_special: bool,
    embed_nl_ok: bool,
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    current_char: char,
    line_no: usize,
    current_line_text: String,
}

impl<'a> CSVChecker<'a> {
    pub fn new(file_name: String, content: &'a str, field_sep: char, dq_special: bool, embed_nl_ok: bool) -> Self {
        let mut chk = CSVChecker {
            file_name,
            field_sep,
            dq_special,
            embed_nl_ok,
            chars: content.chars().peekable(),
            current_char: '\0',
            line_no: 1,
            current_line_text: String::new(),
        };
        chk.next_char();
        chk
    }

    fn next_char(&mut self) {
        loop {
            match self.chars.next() {
                None => {
                    self.current_char = '\0';
                    break;
                }
                Some('\r') => {
                    continue;
                }
                Some('\n') => {
                    self.line_no += 1;
                    self.current_line_text.clear();
                    self.current_char = '\n';
                    break;
                }
                Some(c) => {
                    self.current_line_text.push(c);
                    self.current_char = c;
                    break;
                }
            }
        }
    }

    fn peek(&mut self) -> char {
        loop {
            match self.chars.peek() {
                Some('\r') => {
                    self.chars.next();
                }
                Some(&c) => return c,
                None => return '\0',
            }
        }
    }

    fn error(&self, msg: &str, context: bool) -> String {
        if context {
            format!("{} in {} at line {}\n{}", msg, self.file_name, self.line_no, self.current_line_text)
        } else {
            format!("{} in {}", msg, self.file_name)
        }
    }

    pub fn next_record(&mut self) -> Result<Option<Vec<String>>, String> {
        let mut row = Vec::new();
        while !self.at_end_rec() {
            if self.current_char == '"' && self.dq_special {
                self.read_quoted_field(&mut row)?;
            } else {
                self.read_field(&mut row)?;
            }
        }
        if row.is_empty() {
            Ok(None)
        } else {
            Ok(Some(row))
        }
    }

    fn at_end_rec(&mut self) -> bool {
        if self.current_char == '\0' || self.current_char == '\n' {
            self.next_char();
            true
        } else {
            false
        }
    }

    fn at_end_field(&self) -> bool {
        self.current_char == self.field_sep || self.current_char == '\n' || self.current_char == '\0'
    }

    fn read_field(&mut self, row: &mut Vec<String>) -> Result<(), String> {
        let mut field = String::new();
        while !self.at_end_field() {
            if self.current_char == '"' && self.dq_special {
                return Err(self.error("Unexpected double-quote", true));
            }
            field.push(self.current_char);
            self.next_char();
        }
        row.push(field);
        if self.current_char == self.field_sep {
            self.next_char();
        }
        Ok(())
    }

    fn read_quoted_field(&mut self, row: &mut Vec<String>) -> Result<(), String> {
        let mut field = String::new();
        self.next_char();
        while self.current_char != '\0' {
            if self.current_char == '"' {
                let p = self.peek();
                if p == '"' {
                    field.push('"');
                    self.next_char();
                } else if p == self.field_sep || p == '\n' || p == '\0' {
                    row.push(field);
                    if p == self.field_sep {
                        self.next_char();
                    }
                    self.next_char();
                    return Ok(());
                } else {
                    return Err(self.error("Unexpected double-quote", true));
                }
            } else {
                if self.embed_nl_ok {
                    field.push(self.current_char);
                } else {
                    return Err(self.error("Embedded newline", true));
                }
            }
            self.next_char();
        }
        Err(self.error("Unexpected end of input (probably mis-matched quotes)", false))
    }
}

pub fn run_check(
    mut io: IOManager,
    quiet: bool,
    verbose: bool,
    embed_nl_ok: bool,
    sep: char,
    files: Vec<String>,
) -> Result<(), String> {
    let mut errors = 0;
    let paths = if files.is_empty() {
        vec!["-".to_string()]
    } else {
        files
    };

    for path in paths {
        let content = if path == "-" {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s).map_err(|e| e.to_string())?;
            s
        } else {
            fs::read_to_string(&path).map_err(|e| format!("Cannot open {} for input: {}", path, e))?
        };

        let mut chk = CSVChecker::new(path.clone(), &content, sep, true, embed_nl_ok);
        loop {
            match chk.next_record() {
                Ok(None) => break,
                Ok(_) => {}
                Err(e) => {
                    if quiet {
                        std::process::exit(1);
                    } else {
                        errors += 1;
                        writeln!(io.output_writer, "{}", e).map_err(|err| err.to_string())?;
                        break;
                    }
                }
            }
        }
        if errors == 0 && verbose {
            writeln!(io.output_writer, "{} - OK", path).map_err(|e| e.to_string())?;
        }
    }

    if errors > 0 {
        std::process::exit(1);
    } else {
        Ok(())
    }
}

// 22. DATE Commands
use chrono::{NaiveDate, Datelike};

fn is_leap_year(y: i32) -> bool {
    (y % 4 == 0) && (y % 100 != 0 || y % 400 == 0)
}

fn validate_date(y: i32, m: i32, d: i32) -> bool {
    if y < 1900 || y > 3000 {
        return false;
    }
    if m < 1 || m > 12 {
        return false;
    }
    let mdays = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut limit = mdays[(m - 1) as usize];
    if m == 2 && is_leap_year(y) {
        limit = 29;
    }
    d >= 1 && d <= limit
}

pub struct MaskedDateReader {
    dmy: [char; 3],
    sep: [String; 2],
    year_base: i32,
    month_names: Vec<String>,
}

impl MaskedDateReader {
    pub fn new(mask: &str, months: &str, ybase: i32) -> Result<Self, String> {
        if mask.len() != 5 {
            return Err(format!("Invalid date mask: {}", mask));
        }
        let chars: Vec<char> = mask.chars().collect();
        let c0 = chars[0];
        let c1 = chars[1].to_string();
        let c2 = chars[2];
        let c3 = chars[3].to_string();
        let c4 = chars[4];
        
        let check_mask = |c: char| -> Result<char, String> {
            if c != 'd' && c != 'm' && c != 'y' {
                return Err(format!("Invalid character in date mask: {}", c));
            }
            Ok(c)
        };
        
        let m0 = check_mask(c0)?;
        let m1 = check_mask(c2)?;
        let m2 = check_mask(c4)?;
        
        if chars[1].is_alphanumeric() || chars[3].is_alphanumeric() {
            return Err(format!("Invalid separator in date mask: {}", mask));
        }
        
        if (m0 as u32) + (m1 as u32) + (m2 as u32) != ('d' as u32) + ('m' as u32) + ('y' as u32) {
            return Err(format!("Invalid date mask: {}", mask));
        }
        
        let month_list: Vec<String> = if months.is_empty() {
            "January,February,March,April,May,June,July,August,September,October,November,December"
                .split(',')
                .map(|s| s.to_string())
                .collect()
        } else {
            months.split(',').map(|s| s.to_string()).collect()
        };
        
        if month_list.len() != 12 {
            return Err(format!("Invalid month list: {}", months));
        }
        
        Ok(Self {
            dmy: [m0, m1, m2],
            sep: [c1, c3],
            year_base: ybase,
            month_names: month_list,
        })
    }
    
    pub fn read(&self, ds: &str) -> Option<NaiveDate> {
        let s1 = ds.find(&self.sep[0])?;
        let sep1_char = self.sep[1].chars().next()?;
        let s2 = ds.rfind(sep1_char)?;
        
        if s1 >= s2 {
            return None;
        }
        
        let p0 = &ds[..s1];
        let p1 = &ds[s1 + self.sep[0].len() .. s2];
        let p2 = &ds[s2 + self.sep[1].len()..];
        
        let mut day = -1;
        let mut month = -1;
        let mut year = -1;
        
        let parts = [p0, p1, p2];
        for i in 0..3 {
            match self.dmy[i] {
                'd' => {
                    if let Ok(d) = parts[i].parse::<i32>() {
                        day = d;
                    }
                }
                'm' => {
                    let s = parts[i];
                    if let Ok(m) = s.parse::<i32>() {
                        month = m;
                    } else if s.len() >= 3 {
                        for (idx, name) in self.month_names.iter().enumerate() {
                            if name.len() >= s.len() {
                                let prefix = &name[..s.len()];
                                if prefix.eq_ignore_ascii_case(s) {
                                    month = (idx + 1) as i32;
                                    break;
                                }
                            }
                        }
                    }
                }
                'y' => {
                    let s = parts[i];
                    if s.len() == 2 || s.len() == 4 {
                        if let Ok(mut y) = s.parse::<i32>() {
                            if s.len() == 2 {
                                if y < self.year_base - 1900 {
                                    y += 2000;
                                } else {
                                    y += 1900;
                                }
                            }
                            year = y;
                        }
                    }
                }
                _ => {}
            }
        }
        
        if validate_date(year, month, day) {
            NaiveDate::from_ymd_opt(year, month as u32, day as u32)
        } else {
            None
        }
    }
}

pub enum DateWriteAction {
    WriteAll,
    WriteGood,
    WriteBad,
}

pub fn run_date_iso(
    mut io: IOManager,
    fields: Option<String>,
    mask: String,
    ybase: i32,
    mnames: String,
    write_action: DateWriteAction,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let cols = fields.map(|s| parse_indices(&s)).transpose()?;
    let reader = MaskedDateReader::new(&mask, &mnames, ybase)?;
    let mut row = Vec::new();
    
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }
        if pass {
            io.write_row(&row)?;
            continue;
        }
        
        let mut have_bad = false;
        let mut current = row.clone();
        
        for i in 0..current.len() {
            if cols.is_none() || cols.as_ref().unwrap().contains(&i) {
                if let Some(date) = reader.read(&current[i]) {
                    current[i] = format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day());
                } else {
                    have_bad = true;
                }
            }
        }
        
        let write = match write_action {
            DateWriteAction::WriteAll => true,
            DateWriteAction::WriteGood => !have_bad,
            DateWriteAction::WriteBad => have_bad,
        };
        
        if write {
            io.write_row(&current)?;
        }
    }
    
    Ok(())
}

pub struct FmtEntry {
    text: String,
    is_fmt: bool,
}

fn build_format(fmt: &str) -> Vec<FmtEntry> {
    let mut format_list = Vec::new();
    let mut fs = String::new();
    let mut ls = String::new();
    let mut t = '\0';
    
    let is_fmt_char = |c: char| -> bool {
        "dmyDMYwW".contains(c)
    };
    
    let mut add_fmt = |s: &mut String, format_list: &mut Vec<FmtEntry>| {
        if !s.is_empty() {
            format_list.push(FmtEntry { text: s.clone(), is_fmt: true });
            s.clear();
        }
    };
    
    let mut add_lit = |s: &mut String, format_list: &mut Vec<FmtEntry>| {
        if !s.is_empty() {
            format_list.push(FmtEntry { text: s.clone(), is_fmt: false });
            s.clear();
        }
    };
    
    for c in fmt.chars() {
        if c == t {
            fs.push(c);
        } else if is_fmt_char(c) {
            add_fmt(&mut fs, &mut format_list);
            add_lit(&mut ls, &mut format_list);
            t = c;
            fs.push(c);
        } else {
            add_fmt(&mut fs, &mut format_list);
            t = '\0';
            ls.push(c);
        }
    }
    add_fmt(&mut fs, &mut format_list);
    add_lit(&mut ls, &mut format_list);
    
    format_list
}

fn format_date_entry(fmt: &str, date: &NaiveDate) -> Result<String, String> {
    match fmt {
        "d" => Ok(date.day().to_string()),
        "dd" => Ok(format!("{:02}", date.day())),
        "m" => Ok(date.month().to_string()),
        "mm" => Ok(format!("{:02}", date.month())),
        "mmm" => {
            let short_months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
            Ok(short_months[(date.month() - 1) as usize].to_string())
        }
        "M" => {
            let months = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
            Ok(months[(date.month() - 1) as usize].to_string())
        }
        "y" | "yyyy" => Ok(date.year().to_string()),
        "W" => {
            let days = ["Sunday", "Monday", "Tuesday", "Wedneday", "Thursday", "Friday", "Saturday"];
            let d = date.weekday().num_days_from_sunday() as usize;
            Ok(days[d].to_string())
        }
        "w" => {
            let short_days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
            let d = date.weekday().num_days_from_sunday() as usize;
            Ok(short_days[d].to_string())
        }
        _ => Err(format!("Invalid date format substring: {}", fmt)),
    }
}

fn parse_iso_date(ds: &str) -> Option<NaiveDate> {
    let parts: Vec<&str> = ds.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let y = parts[0].parse::<i32>().ok()?;
    let m = parts[1].parse::<i32>().ok()?;
    let d = parts[2].parse::<i32>().ok()?;
    if validate_date(y, m, d) {
        NaiveDate::from_ymd_opt(y, m as u32, d as u32)
    } else {
        None
    }
}

fn format_date(ds: &str, format_list: &[FmtEntry]) -> String {
    if let Some(date) = parse_iso_date(ds) {
        let mut out = String::new();
        for entry in format_list {
            if entry.is_fmt {
                match format_date_entry(&entry.text, &date) {
                    Ok(s) => out.push_str(&s),
                    Err(_) => return ds.to_string(),
                }
            } else {
                out.push_str(&entry.text);
            }
        }
        out
    } else {
        ds.to_string()
    }
}

pub fn run_date_format(
    mut io: IOManager,
    fields: Option<String>,
    fmt: String,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let cols = fields.map(|s| parse_indices(&s)).transpose()?;
    let format_list = build_format(&fmt);
    let mut row = Vec::new();
    
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }
        if !pass {
            for i in 0..row.len() {
                if cols.is_none() || cols.as_ref().unwrap().contains(&i) {
                    row[i] = format_date(&row[i], &format_list);
                }
            }
        }
        io.write_row(&row)?;
    }
    
    Ok(())
}

// 23. DIFF Command
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditAction {
    NoChange,
    Replace,
    DelSrc,
    AddDest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResultSpan {
    pub action: EditAction,
    pub dest_index: i32,
    pub src_index: i32,
    pub len: i32,
}

impl ResultSpan {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.dest_index.cmp(&other.dest_index)
    }
}

struct State {
    start: i32,
    len: i32,
}

impl State {
    fn new() -> Self {
        Self { start: -1, len: -2 }
    }
    
    fn end(&self) -> i32 {
        self.start + self.len - 1
    }
    
    fn length(&self) -> i32 {
        if self.len > 0 {
            self.len
        } else if self.len == 0 {
            1
        } else {
            0
        }
    }
    
    fn status(&self) -> i32 {
        if self.len > 0 {
            1
        } else if self.len == -1 {
            -1
        } else {
            -2
        }
    }
    
    fn has_valid_length(&mut self, start: i32, end: i32, maxposs: i32) -> bool {
        if self.len > 0 {
            if maxposs < self.len || self.start < start || self.end() > end {
                *self = State::new();
            }
        }
        self.len != -2
    }
}

pub struct Differ {
    fields: Option<Vec<usize>>,
    ignore_case: bool,
    trim: bool,
}

impl Differ {
    pub fn new(fields: Option<Vec<usize>>, ignore_case: bool, trim: bool) -> Self {
        Self { fields, ignore_case, trim }
    }

    fn get_field<'a>(&self, row: &'a [String], idx: usize) -> &'a str {
        if idx < row.len() {
            &row[idx]
        } else {
            ""
        }
    }

    fn not_eq(&self, src: &[String], dest: &[String]) -> bool {
        let cmp_str = |s: &str, d: &str| -> bool {
            let mut s_val = s.to_string();
            let mut d_val = d.to_string();
            if self.ignore_case {
                s_val = s_val.to_uppercase();
                d_val = d_val.to_uppercase();
            }
            if self.trim {
                s_val = s_val.trim().to_string();
                d_val = d_val.trim().to_string();
            }
            s_val != d_val
        };

        if let Some(ref fs) = self.fields {
            for &f in fs {
                let ss = self.get_field(src, f);
                let ds = self.get_field(dest, f);
                if cmp_str(ss, ds) {
                    return true;
                }
            }
            false
        } else {
            let sz = std::cmp::max(src.len(), dest.len());
            for i in 0..sz {
                let ss = self.get_field(src, i);
                let ds = self.get_field(dest, i);
                if cmp_str(ss, ds) {
                    return true;
                }
            }
            false
        }
    }

    fn source_match_len(&self, src: &[Vec<String>], dest: &[Vec<String>], di: usize, si: usize, maxlen: usize) -> usize {
        let mut matchcount = 0;
        while matchcount < maxlen {
            if self.not_eq(&src[si + matchcount], &dest[di + matchcount]) {
                break;
            }
            matchcount += 1;
        }
        matchcount
    }

    fn longest_source_match(&self, src: &[Vec<String>], dest: &[Vec<String>], curitem: &mut State, di: usize, dend: usize, sstart: usize, send: usize) {
        let maxdestlen = (dend - di) + 1;
        let mut bestlen = 0;
        let mut besti = -1;
        
        let mut si = sstart;
        while si <= send {
            let maxlen = std::cmp::min(maxdestlen, (send - si) + 1);
            if maxlen <= bestlen {
                break;
            }
            let curlen = self.source_match_len(src, dest, di, si, maxlen);
            if curlen > bestlen {
                besti = si as i32;
                bestlen = curlen;
            }
            si += bestlen;
            if bestlen == 0 {
                si += 1;
            }
        }
        
        if besti == -1 {
            curitem.start = -1;
            curitem.len = -1;
        } else {
            curitem.start = besti;
            curitem.len = bestlen as i32;
        }
    }

    fn process_range(&self, src: &[Vec<String>], dest: &[Vec<String>], dstart: i32, dend: i32, sstart: i32, send: i32, matches: &mut Vec<ResultSpan>, states: &mut [State]) {
        let mut bestlen = -1;
        let mut bestindex = -1;
        let mut beststate = State::new();
        
        let mut di = dstart;
        while di <= dend {
            let maxposlen = (dend - di) + 1;
            if maxposlen <= bestlen {
                break;
            }
            
            if !states[di as usize].has_valid_length(sstart, send, maxposlen) {
                let mut temp_state = State::new();
                self.longest_source_match(src, dest, &mut temp_state, di as usize, dend as usize, sstart as usize, send as usize);
                states[di as usize] = temp_state;
            }
            
            let curstate = &states[di as usize];
            if curstate.status() == 1 {
                if curstate.length() > bestlen {
                    bestindex = di;
                    bestlen = curstate.length();
                    beststate = State { start: curstate.start, len: curstate.len };
                }
            }
            di += 1;
        }
        
        if bestindex >= 0 {
            let si = beststate.start;
            matches.push(ResultSpan { action: EditAction::NoChange, dest_index: bestindex, src_index: si, len: bestlen });
            if dstart < bestindex {
                if sstart < si {
                    self.process_range(src, dest, dstart, bestindex - 1, sstart, si - 1, matches, states);
                }
            }
            let udstart = bestindex + bestlen;
            let usstart = si + bestlen;
            if dend > udstart {
                if send > usstart {
                    self.process_range(src, dest, udstart, dend, usstart, send, matches, states);
                }
            }
        }
    }

    fn add_changes(&self, report: &mut Vec<ResultSpan>, dest: i32, nextdest: i32, src: i32, nextsrc: i32) -> bool {
        let mut retval = false;
        let diffdest = nextdest - dest;
        let diffsrc = nextsrc - src;
        if diffdest > 0 {
            if diffsrc > 0 {
                let mindiff = std::cmp::min(diffdest, diffsrc);
                report.push(ResultSpan { action: EditAction::Replace, dest_index: dest, src_index: src, len: mindiff });
                if diffdest > diffsrc {
                    let dest_new = dest + mindiff;
                    report.push(ResultSpan { action: EditAction::AddDest, dest_index: dest_new, src_index: -1, len: diffdest - diffsrc });
                } else if diffsrc > diffdest {
                    let src_new = src + mindiff;
                    report.push(ResultSpan { action: EditAction::DelSrc, dest_index: -1, src_index: src_new, len: diffsrc - diffdest });
                }
            } else {
                report.push(ResultSpan { action: EditAction::AddDest, dest_index: dest, src_index: -1, len: diffdest });
            }
            retval = true;
        } else {
            if diffsrc > 0 {
                report.push(ResultSpan { action: EditAction::DelSrc, dest_index: -1, src_index: src, len: diffsrc });
                retval = true;
            }
        }
        retval
    }

    pub fn diff(&self, src: &[Vec<String>], dest: &[Vec<String>]) -> Vec<ResultSpan> {
        let mut matches = Vec::new();
        let dcount = dest.len() as i32;
        let scount = src.len() as i32;
        
        if dcount == 0 {
            let mut res = Vec::new();
            if scount > 0 {
                res.push(ResultSpan { action: EditAction::DelSrc, dest_index: -1, src_index: 0, len: scount });
            }
            return res;
        } else if scount == 0 {
            let mut res = Vec::new();
            res.push(ResultSpan { action: EditAction::AddDest, dest_index: 0, src_index: -1, len: dcount });
            return res;
        }
        
        let mut states: Vec<State> = (0..dcount).map(|_| State::new()).collect();
        self.process_range(src, dest, 0, dcount - 1, 0, scount - 1, &mut matches, &mut states);
        
        matches.sort_by(|a, b| a.cmp(b));
        
        let mut res = Vec::new();
        let mut dest_curr = 0;
        let mut src_curr = 0;
        let mut last_idx: Option<usize> = None;
        
        for drs in matches {
            let added = self.add_changes(&mut res, dest_curr, drs.dest_index, src_curr, drs.src_index);
            if !added {
                if let Some(l_idx) = last_idx {
                    res[l_idx].len += drs.len;
                }
            } else {
                res.push(drs);
                last_idx = Some(res.len() - 1);
            }
            dest_curr = drs.dest_index + drs.len;
            src_curr = drs.src_index + drs.len;
        }
        self.add_changes(&mut res, dest_curr, dcount, src_curr, scount);
        res
    }
}

pub fn run_diff(
    mut io: IOManager,
    file1: String,
    file2: String,
    fields: Option<String>,
    quiet: bool,
    ignore_case: bool,
    trim: bool,
) -> Result<(), String> {
    let mut io1 = IOManager::new(
        vec![file1],
        None,
        io.ignore_blank_lines,
        io.skip_col_names,
        io.input_sep,
        Some(io.output_sep),
        false,
        io.smart_quotes,
        io.quote_fields.clone(),
        None,
    )?;
    let mut src = Vec::new();
    let mut row = Vec::new();
    while io1.read_csv(&mut row)? {
        src.push(row.clone());
    }

    let mut io2 = IOManager::new(
        vec![file2],
        None,
        io.ignore_blank_lines,
        io.skip_col_names,
        io.input_sep,
        Some(io.output_sep),
        false,
        io.smart_quotes,
        io.quote_fields.clone(),
        None,
    )?;
    let mut dest = Vec::new();
    while io2.read_csv(&mut row)? {
        dest.push(row.clone());
    }

    let parsed_fields = fields.map(|s| parse_indices(&s)).transpose()?;
    let differ = Differ::new(parsed_fields, ignore_case, trim);
    let results = differ.diff(&src, &dest);

    let is_same = results.len() == 1 && matches!(results[0].action, EditAction::NoChange);

    if !quiet {
        for rs in &results {
            match rs.action {
                EditAction::NoChange => {}
                EditAction::AddDest => {
                    for i in 0..rs.len {
                        let idx = (rs.dest_index + i) as usize;
                        write!(io.output_writer, "\"{}\",\"{}\",", "+", idx + 1).map_err(|e| e.to_string())?;
                        io.write_row(&dest[idx])?;
                    }
                }
                EditAction::DelSrc => {
                    for i in 0..rs.len {
                        let idx = (rs.src_index + i) as usize;
                        write!(io.output_writer, "\"{}\",\"{}\",", "-", idx + 1).map_err(|e| e.to_string())?;
                        io.write_row(&src[idx])?;
                    }
                }
                EditAction::Replace => {
                    for i in 0..rs.len {
                        let idx_src = (rs.src_index + i) as usize;
                        write!(io.output_writer, "\"{}\",\"{}\",", "-", idx_src + 1).map_err(|e| e.to_string())?;
                        io.write_row(&src[idx_src])?;
                        
                        let idx_dest = (rs.dest_index + i) as usize;
                        write!(io.output_writer, "\"{}\",\"{}\",", "+", idx_dest + 1).map_err(|e| e.to_string())?;
                        io.write_row(&dest[idx_dest])?;
                    }
                }
            }
        }
    }

    if is_same {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

// 24. EDIT Command
pub struct EditSubCmd {
    pub cmd: char,
    pub from: String,
    pub to: String,
    pub opts: String,
}

fn translate_regex(pat: &str) -> String {
    let mut res = String::new();
    let mut chars = pat.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('(') => {
                    res.push('(');
                    chars.next();
                }
                Some(')') => {
                    res.push(')');
                    chars.next();
                }
                Some(other) => {
                    res.push('\\');
                    res.push(*other);
                    chars.next();
                }
                None => {
                    res.push('\\');
                }
            }
        } else if c == '(' {
            res.push_str(r"\(");
        } else if c == ')' {
            res.push_str(r"\)");
        } else {
            res.push(c);
        }
    }
    res
}

fn translate_replacement(repl: &str) -> String {
    let mut res = String::new();
    let mut chars = repl.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some(d) if d.is_ascii_digit() && *d >= '1' && *d <= '9' => {
                    res.push('$');
                    res.push(*d);
                    chars.next();
                }
                Some(other) => {
                    res.push(*other);
                    chars.next();
                }
                None => {
                    res.push('\\');
                }
            }
        } else if c == '$' {
            res.push_str("$$");
        } else {
            res.push(c);
        }
    }
    res
}

fn read_edit_field(s: &str, i: &mut usize, sep: char) -> Result<String, String> {
    let mut f = String::new();
    let mut escaped = false;
    let chars: Vec<char> = s.chars().collect();
    
    loop {
        if *i >= chars.len() {
            return Err(format!("Invalid value for edit command: {}", s));
        }
        let c = chars[*i];
        *i += 1;
        
        if escaped {
            escaped = false;
            f.push(c);
        } else if c == '\\' {
            escaped = true;
            f.push(c);
        } else if c != sep {
            f.push(c);
        } else {
            break;
        }
    }
    Ok(f)
}

fn parse_sub(s: &str) -> Result<EditSubCmd, String> {
    if s.is_empty() {
        return Err("Empty value for edit command".to_string());
    }
    let chars: Vec<char> = s.chars().collect();
    let cmd = chars[0];
    if cmd != 's' {
        return Err(format!("Invalid value for edit command: {}", s));
    }
    if chars.len() < 2 {
        return Err(format!("Invalid value for edit command: {}", s));
    }
    let sep = chars[1];
    if sep == '\\' {
        return Err(format!("Invalid value for edit command: {}", s));
    }
    
    let mut i = 2;
    let from = read_edit_field(s, &mut i, sep)?;
    let to = read_edit_field(s, &mut i, sep)?;
    let opts: String = chars[i..].iter().collect();
    
    Ok(EditSubCmd { cmd, from, to, opts })
}

pub fn run_edit(
    mut io: IOManager,
    fields: Option<String>,
    edit_cmds: Vec<String>,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let cols = fields.map(|s| parse_indices(&s)).transpose()?;
    let mut sub_cmds = Vec::new();
    for cmd_str in edit_cmds {
        sub_cmds.push(parse_sub(&cmd_str)?);
    }
    
    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }
        if !pass {
            for i in 0..row.len() {
                if cols.is_none() || cols.as_ref().unwrap().contains(&i) {
                    for sc in &sub_cmds {
                        if sc.cmd == 's' {
                            let translated_pat = translate_regex(&sc.from);
                            let translated_repl = translate_replacement(&sc.to);
                            
                            let ignore_case = sc.opts.contains('i');
                            let global = sc.opts.contains('g');
                            
                            let mut builder = regex::RegexBuilder::new(&translated_pat);
                            builder.case_insensitive(ignore_case);
                            let re = builder.build().map_err(|e| format!("Invalid regex: {}: {}", sc.from, e))?;
                            
                            if global {
                                row[i] = re.replace_all(&row[i], &translated_repl).to_string();
                            } else {
                                row[i] = re.replace(&row[i], &translated_repl).to_string();
                            }
                        }
                    }
                }
            }
        }
        io.write_row(&row)?;
    }
    Ok(())
}

// 15. Order Command
pub fn run_order(
    mut io: IOManager,
    fields: Option<String>,
    exclf: Option<String>,
    rev_fields: Option<String>,
    fnames: Option<String>,
    nocreat: bool,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let mut flag_count = 0;
    if fields.is_some() { flag_count += 1; }
    if exclf.is_some() { flag_count += 1; }
    if rev_fields.is_some() { flag_count += 1; }
    if fnames.is_some() { flag_count += 1; }
    if flag_count != 1 {
        return Err("Need fields or fnames or rev_fields or exclf flags (but only one)".to_string());
    }

    let mut static_indices = Vec::new();
    let mut is_exclude = false;
    let mut is_rev = false;

    if let Some(ref f) = fields {
        static_indices = parse_indices(f)?;
    } else if let Some(ref xf) = exclf {
        static_indices = parse_indices(xf)?;
        is_exclude = true;
    } else if let Some(ref rf) = rev_fields {
        static_indices = parse_indices(rf)?;
        is_rev = true;
    }

    let fname_list: Option<Vec<String>> = fnames.map(|s| s.split(',').map(|x| x.to_string()).collect());

    let mut last_file = String::new();
    let mut current_order = static_indices.clone();

    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }

        if !pass {
            let file_changed = io.current_file_name() != last_file;
            if file_changed {
                last_file = io.current_file_name().to_string();
            }

            if let Some(ref names) = fname_list {
                if io.current_line() == 1 {
                    let col_map: HashMap<String, usize> = row.iter().enumerate().map(|(idx, val)| (val.clone(), idx)).collect();
                    let mut resolved = Vec::new();
                    for name in names {
                        if let Some(&idx) = col_map.get(name) {
                            resolved.push(idx);
                        } else {
                            return Err(format!("Unknown column name: {}", name));
                        }
                    }
                    current_order = resolved;
                }
            }

            if is_exclude {
                let mut newrow = Vec::new();
                for (i, val) in row.iter().enumerate() {
                    if !current_order.contains(&i) {
                        newrow.push(val.clone());
                    }
                }
                row = newrow;
            } else {
                if is_rev {
                    row.reverse();
                }
                let mut newrow = Vec::new();
                for &ri in &current_order {
                    if ri < row.len() {
                        newrow.push(row[ri].clone());
                    } else if !nocreat {
                        newrow.push(String::new());
                    }
                }
                row = newrow;
            }
        }

        io.write_row(&row)?;
    }
    Ok(())
}

// 16. Join Command
fn make_join_key(row: &[String], indices: &[usize], ignore_case: bool) -> String {
    let mut key = String::new();
    for &col in indices {
        if col < row.len() {
            let val = if ignore_case { row[col].to_uppercase() } else { row[col].clone() };
            key.push_str(&val);
        }
        key.push('\0');
    }
    key
}

pub fn run_join(
    mut io: IOManager,
    fields: String,
    oj: bool,
    inv: bool,
    ic: bool,
    keep: bool,
) -> Result<(), String> {
    if oj && inv {
        return Err("Cannot have both outer join and invert flags".to_string());
    }

    let mut join_specs = Vec::new();
    for spec in fields.split(',') {
        let parts: Vec<&str> = spec.split(':').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid join specification: {}", spec));
        }
        let c1 = parts[0].parse::<usize>().map_err(|_| format!("Invalid column: {}", parts[0]))?;
        let c2 = parts[1].parse::<usize>().map_err(|_| format!("Invalid column: {}", parts[1]))?;
        if c1 < 1 || c2 < 1 {
            return Err(format!("Invalid join specification: {}", spec));
        }
        join_specs.push((c1 - 1, c2 - 1));
    }

    let paths = io.input_paths.clone();
    if paths.len() < 2 {
        return Err("Need at least two input streams".to_string());
    }

    let rhs_path = paths.last().unwrap().clone();
    let lhs_paths = paths[..paths.len() - 1].to_vec();

    // Build RHS row map
    let mut rhs_io = IOManager::new(
        vec![rhs_path],
        None,
        io.ignore_blank_lines,
        io.skip_col_names,
        io.input_sep,
        Some(io.output_sep),
        false,
        io.smart_quotes,
        io.quote_fields.clone(),
        None,
    )?;

    let rhs_join_cols: Vec<usize> = join_specs.iter().map(|&(_, rhs)| rhs).collect();
    let lhs_join_cols: Vec<usize> = join_specs.iter().map(|&(lhs, _)| lhs).collect();

    let mut row_map: HashMap<String, Vec<Vec<String>>> = HashMap::new();
    let mut rhs_row = Vec::new();
    while rhs_io.read_csv(&mut rhs_row)? {
        let key = make_join_key(&rhs_row, &rhs_join_cols, ic);
        let mut jrow = Vec::new();
        for (i, val) in rhs_row.iter().enumerate() {
            if !rhs_join_cols.contains(&i) || keep {
                jrow.push(val.clone());
            }
        }
        row_map.entry(key).or_insert_with(Vec::new).push(jrow);
    }

    io.reset_inputs(lhs_paths)?;

    let mut lhs_row = Vec::new();
    while io.read_csv(&mut lhs_row)? {
        let key = make_join_key(&lhs_row, &lhs_join_cols, ic);
        if let Some(jrows) = row_map.get(&key) {
            if !inv {
                for jr in jrows {
                    let mut newrow = lhs_row.clone();
                    newrow.extend(jr.clone());
                    io.write_row(&newrow)?;
                }
            }
        } else {
            if oj || inv {
                io.write_row(&lhs_row)?;
            }
        }
    }

    Ok(())
}

// ============================================================================
// BATCH 3 SUBCOMMANDS IMPLEMENTATION
// ============================================================================

// Helper for exec escaping
fn escape_unix(val: &str) -> String {
    let mut escaped = String::new();
    for c in val.chars() {
        if c == '\\' || c == '\'' || c == '"' {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

// 17. Eval Command
#[derive(Debug, Clone)]
pub struct FieldEx {
    pub field: i32,
    pub expr: Expr,
}

pub fn run_eval(
    mut io: IOManager,
    discard: bool,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let mut field_exprs: Vec<FieldEx> = Vec::new();
    let mut is_if: Vec<bool> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        if args[i] == "eval" {
            i += 1;
            break;
        }
        i += 1;
    }

    while i < args.len() {
        let arg = &args[i];
        if arg == "-e" || arg == "-if" {
            let is_if_flag = arg == "-if";
            if i + 1 >= args.len() {
                return Err("Missing expression".to_string());
            }
            let expr_str = &args[i + 1];
            i += 2;
            let parsed = expr::parse(expr_str)
                .map_err(|e| format!("{} in {}", e, expr_str))?;
            let ex = parsed.last().cloned().ok_or_else(|| format!("Empty expression in {}", expr_str))?;
            field_exprs.push(FieldEx { field: -1, expr: ex });
            is_if.push(is_if_flag);
        } else if arg == "-r" {
            if i + 1 >= args.len() {
                return Err("Missing field/expression".to_string());
            }
            let field_expr_str = &args[i + 1];
            i += 2;
            let pos = field_expr_str.find(',').ok_or_else(|| format!("Invalid field/index pair: {}", field_expr_str))?;
            let field_str = &field_expr_str[..pos];
            let expr_str = &field_expr_str[pos + 1..];

            let n = field_str.parse::<i32>().map_err(|_| format!("Invalid field (need integer): {}", field_str))?;
            if n <= 0 {
                return Err(format!("Invalid field (must be greater than zero): {}", field_str));
            }
            let parsed = expr::parse(expr_str)
                .map_err(|e| format!("{} in {}", e, expr_str))?;
            let ex = parsed.last().cloned().ok_or_else(|| format!("Empty expression in {}", expr_str))?;
            field_exprs.push(FieldEx { field: n - 1, expr: ex });
            is_if.push(false);
        } else {
            i += 1;
        }
    }

    if field_exprs.is_empty() {
        return Err("Need at least one of -e or -r options".to_string());
    }

    for idx in 0..field_exprs.len() {
        if is_if[idx] {
            if idx < field_exprs.len() - 2 && is_if[idx + 2] {
                return Err("Cannot have consecutive -if options".to_string());
            }
            if idx >= field_exprs.len() - 2 {
                return Err("Need two -e options after -if".to_string());
            }
        }
    }

    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }
        if !pass {
            let mut vars = HashMap::new();
            vars.insert("line".to_string(), io.current_line().to_string());
            vars.insert("file".to_string(), io.current_file_name().to_string());
            vars.insert("fields".to_string(), row.len().to_string());

            let mut eval_row = row.clone();
            if discard {
                eval_row.clear();
            }

            let mut skipelse = false;
            let mut k = 0;
            while k < field_exprs.len() {
                if is_if[k] {
                    vars.insert("fields".to_string(), row.len().to_string());
                    let r = field_exprs[k].expr.eval(&row, &vars, None)?;
                    if expr::to_bool(&r) {
                        skipelse = true;
                    } else {
                        k += 1;
                    }
                    k += 1;
                    continue;
                }

                vars.insert("fields".to_string(), row.len().to_string());
                let r = field_exprs[k].expr.eval(&row, &vars, None)?;
                let field_idx = field_exprs[k].field;
                if field_idx < 0 || field_idx >= eval_row.len() as i32 {
                    eval_row.push(r);
                } else {
                    eval_row[field_idx as usize] = r;
                }

                if skipelse {
                    k += 1;
                    skipelse = false;
                }
                k += 1;
            }
            row = eval_row;
        }
        io.write_row(&row)?;
    }

    Ok(())
}

// 18. Exec Command
fn make_cmd(cmd_template: &str, row: &[String]) -> Result<String, String> {
    let mut cmd = String::new();
    let mut chars = cmd_template.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            if chars.peek() == Some(&'%') {
                chars.next();
                cmd.push('%');
            } else if chars.peek().map_or(false, |ch| ch.is_ascii_digit()) {
                let mut num_str = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch.is_ascii_digit() {
                        num_str.push(ch);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let n: usize = num_str.parse().map_err(|_| format!("Invalid parameter: %{}", num_str))?;
                if n == 0 {
                    return Err(format!("Invalid parameter: %{}", num_str));
                }
                let idx = n - 1;
                if idx < row.len() {
                    #[cfg(windows)]
                    cmd.push_str(&row[idx]);
                    #[cfg(not(windows))]
                    cmd.push_str(&escape_unix(&row[idx]));
                }
            } else {
                return Err("Invalid parameter".to_string());
            }
        } else {
            cmd.push(c);
        }
    }
    Ok(cmd)
}

pub fn run_exec(
    mut io: IOManager,
    cmd_str: String,
    replace: bool,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    if cmd_str.is_empty() {
        return Err("Empty command".to_string());
    }
    let csv = !replace;

    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, _) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            None,
        );
        if skip {
            continue;
        }

        let cmd = make_cmd(&cmd_str, &row)?;
        let mut child = std::process::Command::new("sh");
        child.arg("-c").arg(&cmd);
        let output = child.output().map_err(|e| format!("Command execution error: {}", e))?;
        
        let stdout_str = String::from_utf8_lossy(&output.stdout);
        for line in stdout_str.lines() {
            if csv {
                let mut tmp = row.clone();
                let (_, pass) = should_skip_or_pass(
                    &tmp,
                    io.current_line(),
                    io.current_file_name(),
                    None,
                    pass_expr.as_ref(),
                );
                if !pass {
                    let cmdout = io.parse_csv_line(line)?;
                    tmp.extend(cmdout);
                    io.write_row(&tmp)?;
                }
            } else {
                writeln!(io.output_writer, "{}", line).map_err(|e| e.to_string())?;
            }
        }
    }

    Ok(())
}

// 19. File Info Command
pub fn run_file_info(
    mut io: IOManager,
    basename: bool,
    two_cols: bool,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }
        if pass {
            io.write_row(&row)?;
            continue;
        }

        let fname = if basename {
            std::path::Path::new(io.current_file_name())
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(io.current_file_name())
                .to_string()
        } else {
            io.current_file_name().to_string()
        };

        let mut outrow = Vec::new();
        if two_cols {
            outrow.push(fname);
            outrow.push(io.current_line().to_string());
        } else {
            outrow.push(format!("{} ({})", fname, io.current_line()));
        }
        outrow.extend(row.clone());
        io.write_row(&outrow)?;
    }
    Ok(())
}

// 20. File Merge Command
fn get_field_ref(row: &[String], idx: usize) -> &str {
    if idx < row.len() {
        &row[idx]
    } else {
        ""
    }
}

fn cmp_row(a: &[String], b: &[String], fields: &[usize]) -> std::cmp::Ordering {
    let n = std::cmp::max(a.len(), b.len());
    for i in 0..n {
        if !fields.is_empty() && !fields.contains(&i) {
            continue;
        }
        let fa = get_field_ref(a, i);
        let fb = get_field_ref(b, i);
        if fa != fb {
            return fa.cmp(fb);
        }
    }
    std::cmp::Ordering::Equal
}

struct RowGetter {
    io: IOManager,
    latch: Vec<String>,
    have: bool,
}

impl RowGetter {
    fn new(io: IOManager) -> Self {
        Self {
            io,
            latch: Vec::new(),
            have: false,
        }
    }

    fn get(&mut self) -> Result<Option<&[String]>, String> {
        if self.have {
            Ok(Some(&self.latch))
        } else {
            let mut row = Vec::new();
            if self.io.read_csv(&mut row)? {
                self.latch = row;
                self.have = true;
                Ok(Some(&self.latch))
            } else {
                Ok(None)
            }
        }
    }

    fn clear_latch(&mut self) {
        self.have = false;
    }
}

pub fn run_file_merge(
    mut io: IOManager,
    fields: Option<String>,
) -> Result<(), String> {
    let parsed_fields = fields.map(|s| parse_indices(&s)).transpose()?.unwrap_or_default();
    let paths = io.input_paths.clone();
    let mut getters = Vec::new();

    for path in paths {
        let io_sub = IOManager::new(
            vec![path],
            None,
            io.ignore_blank_lines,
            io.skip_col_names,
            io.input_sep,
            Some(io.output_sep),
            false,
            io.smart_quotes,
            io.quote_fields.clone(),
            None,
        )?;
        getters.push(RowGetter::new(io_sub));
    }

    loop {
        let mut min_idx: Option<usize> = None;
        let mut min_row: Option<Vec<String>> = None;

        for (i, getter) in getters.iter_mut().enumerate() {
            if let Some(row) = getter.get()? {
                if min_row.is_none() || cmp_row(row, min_row.as_ref().unwrap(), &parsed_fields) == std::cmp::Ordering::Less {
                    min_row = Some(row.to_vec());
                    min_idx = Some(i);
                }
            }
        }

        if let Some(idx) = min_idx {
            io.write_row(&min_row.unwrap())?;
            getters[idx].clear_latch();
        } else {
            break;
        }
    }

    Ok(())
}

// 21. Find / Remove Commands
pub fn run_find_remove(
    mut io: IOManager,
    remove: bool,
    fields: Option<String>,
    exprs: Vec<String>,
    strings: Vec<String>,
    exprs_ic: Vec<String>,
    strings_ic: Vec<String>,
    ranges: Vec<String>,
    lengths: Vec<String>,
    fcount: Option<String>,
    if_expr: Option<String>,
    count_only: bool,
) -> Result<(), String> {
    let col_index = fields.map(|s| parse_indices(&s)).transpose()?.unwrap_or_default();

    let mut regexes = Vec::new();
    for e in exprs {
        let re = regex::RegexBuilder::new(&e)
            .case_insensitive(false)
            .build()
            .map_err(|err| format!("Invalid regex: {}: {}", e, err))?;
        regexes.push(re);
    }
    for e in exprs_ic {
        let re = regex::RegexBuilder::new(&e)
            .case_insensitive(true)
            .build()
            .map_err(|err| format!("Invalid regex: {}: {}", e, err))?;
        regexes.push(re);
    }
    for s in strings {
        let escaped = regex::escape(&s);
        let re = regex::RegexBuilder::new(&escaped)
            .case_insensitive(false)
            .build()
            .map_err(|err| format!("Invalid regex: {}: {}", s, err))?;
        regexes.push(re);
    }
    for s in strings_ic {
        let escaped = regex::escape(&s);
        let re = regex::RegexBuilder::new(&escaped)
            .case_insensitive(false)
            .build()
            .map_err(|err| format!("Invalid regex: {}: {}", s, err))?;
        regexes.push(re);
    }

    struct RangeVal {
        low: String,
        high: String,
        is_num: bool,
    }
    let mut parsed_ranges = Vec::new();
    for r in ranges {
        let parts: Vec<&str> = r.split(':').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid range: {}", r));
        }
        let low_str = parts[0].to_string();
        let high_str = parts[1].to_string();
        let low_num = low_str.parse::<f64>();
        let high_num = high_str.parse::<f64>();
        let is_num = low_num.is_ok() && high_num.is_ok();
        if is_num {
            if low_num.unwrap() > high_num.unwrap() {
                return Err(format!("Invalid range: {}:{}", low_str, high_str));
            }
        } else {
            if low_str > high_str {
                return Err(format!("Invalid range: {}:{}", low_str, high_str));
            }
        }
        parsed_ranges.push(RangeVal { low: low_str, high: high_str, is_num });
    }

    let mut parsed_lengths = Vec::new();
    for l in lengths {
        let parts: Vec<&str> = l.split(':').collect();
        let (n1, n2) = if parts.len() == 1 {
            let n = parts[0].parse::<i32>().map_err(|_| format!("Invalid range: {}", l))?;
            (n, n)
        } else if parts.len() == 2 {
            let n1 = parts[0].parse::<i32>().map_err(|_| format!("Invalid range: {}", l))?;
            let n2 = parts[1].parse::<i32>().map_err(|_| format!("Invalid range: {}", l))?;
            (n1, n2)
        } else {
            return Err(format!("Invalid range: {}", l));
        };
        if n1 < 0 || n2 < 0 || n1 > n2 {
            return Err(format!("Invalid range: {}", l));
        }
        parsed_lengths.push((n1 as usize, n2 as usize));
    }

    let mut min_fields = 0;
    let mut max_fields = std::i32::MAX;
    if let Some(ref fc) = fcount {
        let parts: Vec<&str> = fc.split(':').collect();
        let (n1, n2) = if parts.len() == 1 {
            let n = parts[0].parse::<i32>().map_err(|_| format!("Field counts must be integers for -fc"))?;
            (n, n)
        } else if parts.len() == 2 {
            let low = if parts[0].is_empty() { 0 } else { parts[0].parse::<i32>().map_err(|_| format!("Field counts must be integers for -fc"))? };
            let high = if parts[1].is_empty() { std::i32::MAX } else { parts[1].parse::<i32>().map_err(|_| format!("Field counts must be integers for -fc"))? };
            (low, high)
        } else {
            return Err(format!("Invalid field count for -fc flag"));
        };
        if n1 > n2 {
            return Err(format!("invalid field count specified by -fc"));
        }
        min_fields = n1;
        max_fields = n2;
    }

    let eval_expr = if_expr.map(|e| {
        let parsed = expr::parse(&e).map_err(|err| format!("{} {}", err, e))?;
        parsed.last().cloned().ok_or_else(|| format!("Empty expression: {}", e))
    }).transpose()?;

    let have_regex = !regexes.is_empty() || !parsed_ranges.is_empty() || !parsed_lengths.is_empty();

    if !have_regex && fcount.is_none() && eval_expr.is_none() {
        return Err("Need at least one -e, -r, -l, -fc, -if or -ei flag".to_string());
    }

    let mut row = Vec::new();
    let mut count = 0;

    while io.read_csv(&mut row)? {
        let mut vars = HashMap::new();
        vars.insert("line".to_string(), io.current_line().to_string());
        vars.insert("file".to_string(), io.current_file_name().to_string());
        vars.insert("fields".to_string(), row.len().to_string());

        if let Some(ref e) = eval_expr {
            let res_str = e.eval(&row, &vars, None)?;
            let es = expr::to_bool(&res_str);
            if (es && remove) || (!es && !remove) {
                continue;
            }
        }

        if fcount.is_some() {
            let fcok = (row.len() as i32) >= min_fields && (row.len() as i32) <= max_fields;
            if (remove && fcok) || (!remove && !fcok) {
                continue;
            }
        }

        let mut matched = false;
        if have_regex {
            for (i, val) in row.iter().enumerate() {
                if col_index.is_empty() || col_index.contains(&i) {
                    let mut regex_match = false;
                    for re in &regexes {
                        if re.is_match(val) {
                            regex_match = true;
                            break;
                        }
                    }
                    if regex_match {
                        matched = true;
                        break;
                    }

                    let mut range_match = false;
                    for r in &parsed_ranges {
                        if r.is_num {
                            if let (Ok(v), Ok(low), Ok(high)) = (val.parse::<f64>(), r.low.parse::<f64>(), r.high.parse::<f64>()) {
                                if v >= low && v <= high {
                                    range_match = true;
                                    break;
                                }
                            }
                        } else {
                            if val >= &r.low && val <= &r.high {
                                range_match = true;
                                break;
                            }
                        }
                    }
                    if range_match {
                        matched = true;
                        break;
                    }

                    let mut len_match = false;
                    let val_len = val.len();
                    for &(low, high) in &parsed_lengths {
                        if val_len >= low && val_len <= high {
                            len_match = true;
                            break;
                        }
                    }
                    if len_match {
                        matched = true;
                        break;
                    }
                }
            }
        } else {
            matched = !remove;
        }

        if remove ^ matched {
            count += 1;
            if !count_only {
                io.write_row(&row)?;
            }
        }
    }

    if count_only {
        writeln!(io.output_writer, "{}", count).map_err(|e| e.to_string())?;
    }

    Ok(())
}

// 22. Flatten / Unflatten Commands
pub fn run_flatten(
    mut io: IOManager,
    master_expr_str: Option<String>,
    key: Option<String>,
    data_fields_str: Option<String>,
    remove: bool,
    skip_expr: Option<Expr>,
) -> Result<(), String> {
    if let Some(ref m_expr) = master_expr_str {
        if key.is_some() || data_fields_str.is_some() || remove {
            return Err("Cannot use -me with other options".to_string());
        }
        let parsed = expr::parse(m_expr).map_err(|err| format!("{} {}", err, m_expr))?;
        let e = parsed.last().ok_or_else(|| format!("Empty expression: {}", m_expr))?;
        let mut master = Vec::new();
        let mut row = Vec::new();

        while io.read_csv(&mut row)? {
            let (skip, _) = should_skip_or_pass(
                &row,
                io.current_line(),
                io.current_file_name(),
                skip_expr.as_ref(),
                None,
            );
            if skip {
                continue;
            }

            let mut vars = HashMap::new();
            vars.insert("line".to_string(), io.current_line().to_string());
            vars.insert("file".to_string(), io.current_file_name().to_string());
            vars.insert("fields".to_string(), row.len().to_string());

            let res_str = e.eval(&row, &vars, None)?;
            if expr::to_bool(&res_str) {
                master = row.clone();
            } else {
                if master.is_empty() {
                    return Err("No master record identified".to_string());
                }
                let mut output = master.clone();
                output.extend(row.clone());
                io.write_row(&output)?;
            }
        }
    } else {
        let key_fields = if let Some(ref k) = key {
            parse_indices(k)?
        } else {
            vec![0]
        };
        let data_fields = data_fields_str.map(|s| parse_indices(&s)).transpose()?;
        let keep_key = !remove;

        let mut row = Vec::new();
        let mut m_key = String::new();
        let mut m_data = Vec::new();
        let mut read = 0;

        let make_key = |r: &[String], kf: &[usize]| -> String {
            let mut key_str = String::new();
            for &idx in kf {
                if idx < r.len() {
                    key_str.push_str(&r[idx]);
                }
                key_str.push('\0');
            }
            key_str
        };

        let mut new_key = |r: &[String], kf: &[usize], keep: bool, md: &mut Vec<String>| {
            md.clear();
            if keep {
                for &idx in kf {
                    if idx < r.len() {
                        md.push(r[idx].clone());
                    } else {
                        md.push(String::new());
                    }
                }
            }
        };

        let add_data = |r: &[String], kf: &[usize], df: &Option<Vec<usize>>, md: &mut Vec<String>| {
            if let Some(dfs) = df {
                for &idx in dfs {
                    if idx < r.len() {
                        md.push(r[idx].clone());
                    } else {
                        md.push(String::new());
                    }
                }
            } else {
                for (i, val) in r.iter().enumerate() {
                    if !kf.contains(&i) {
                        md.push(val.clone());
                    }
                }
            }
        };

        while io.read_csv(&mut row)? {
            let (skip, _) = should_skip_or_pass(
                &row,
                io.current_line(),
                io.current_file_name(),
                skip_expr.as_ref(),
                None,
            );
            if skip {
                continue;
            }

            let key_str = make_key(&row, &key_fields);
            if read == 0 {
                new_key(&row, &key_fields, keep_key, &mut m_data);
                read += 1;
            } else if key_str != m_key {
                io.write_row(&m_data)?;
                new_key(&row, &key_fields, keep_key, &mut m_data);
            }
            add_data(&row, &key_fields, &data_fields, &mut m_data);
            m_key = key_str;
        }

        if read > 0 {
            io.write_row(&m_data)?;
        }
    }

    Ok(())
}

pub fn run_unflatten(
    mut io: IOManager,
    key: Option<String>,
    num_data_fields: Option<usize>,
    skip_expr: Option<Expr>,
) -> Result<(), String> {
    let key_fields = if let Some(ref k) = key {
        parse_indices(k)?
    } else {
        vec![0]
    };
    let n_fields = num_data_fields.unwrap_or(1);
    if n_fields == 0 {
        return Err("Number of data per output must be greater than zero".to_string());
    }

    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, _) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            None,
        );
        if skip {
            continue;
        }

        let mut key_row = Vec::new();
        for &idx in &key_fields {
            if idx < row.len() {
                key_row.push(row[idx].clone());
            }
        }

        let mut i = 0;
        while i < row.len() {
            let mut out = key_row.clone();
            let mut n = n_fields;
            let mut added = false;
            while n > 0 && i < row.len() {
                if !key_fields.contains(&i) {
                    out.push(row[i].clone());
                    added = true;
                    n -= 1;
                }
                i += 1;
            }
            if added {
                io.write_row(&out)?;
            }
        }
    }
    Ok(())
}

struct FieldSpec {
    src: usize,
    field: usize,
}

// 23. Inter Command
pub fn run_inter(
    mut io: IOManager,
    fields: Option<String>,
) -> Result<(), String> {
    let paths = io.input_paths.clone();
    if paths.len() != 2 {
        return Err("Command requires exactly two input streams".to_string());
    }

    let mut spec_list = Vec::new();
    if let Some(ref f) = fields {
        for item in f.split(',') {
            if item.len() < 2 {
                return Err(format!("Invalid field spec {}", item));
            }
            let src_char = item.chars().next().unwrap().to_ascii_uppercase();
            let src = match src_char {
                'L' => 0,
                'R' => 1,
                _ => return Err(format!("Invalid source spec in field spec {}", item)),
            };
            let fi_str = &item[1..];
            let fi = fi_str.parse::<usize>().map_err(|_| format!("Field index not integer in field {}", item))?;
            if fi == 0 {
                return Err(format!("Field index must be 1 or greater in field {}", item));
            }
            spec_list.push(FieldSpec { src, field: fi - 1 });
        }
    }

    let mut io0 = IOManager::new(
        vec![paths[0].clone()],
        None,
        io.ignore_blank_lines,
        io.skip_col_names,
        io.input_sep,
        Some(io.output_sep),
        false,
        io.smart_quotes,
        io.quote_fields.clone(),
        None,
    )?;

    let mut io1 = IOManager::new(
        vec![paths[1].clone()],
        None,
        io.ignore_blank_lines,
        io.skip_col_names,
        io.input_sep,
        Some(io.output_sep),
        false,
        io.smart_quotes,
        io.quote_fields.clone(),
        None,
    )?;

    let mut row0 = Vec::new();
    let mut row1 = Vec::new();

    while io0.read_csv(&mut row0)? {
        if !io1.read_csv(&mut row1)? {
            row1.clear();
        }

        let mut out = Vec::new();
        if spec_list.is_empty() {
            out = row0.clone();
            out.extend(row1.clone());
        } else {
            for spec in &spec_list {
                let r = if spec.src == 0 { &row0 } else { &row1 };
                let val = if spec.field < r.len() {
                    r[spec.field].clone()
                } else {
                    String::new()
                };
                out.push(val);
            }
        }
        io.write_row(&out)?;
    }

    Ok(())
}

// 24. Map Command
fn parse_comma_list(s: &str) -> Vec<String> {
    if s.is_empty() {
        return vec![String::new()];
    }
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '"' {
            if in_quotes && chars.peek() == Some(&'"') {
                current.push('"');
                chars.next();
            } else {
                in_quotes = !in_quotes;
            }
        } else if c == ',' && !in_quotes {
            result.push(current.clone());
            current.clear();
        } else {
            current.push(c);
        }
    }
    result.push(current);
    result
}

fn expand_map_to(val: &str, row: &[String]) -> Result<String, String> {
    if val.starts_with('$') && val.len() > 1 {
        let field = &val[1..];
        if field.starts_with('$') {
            Ok(field.to_string())
        } else {
            let n = field.parse::<i32>().map_err(|_| format!("Invalid field specifier {}", val))?;
            if n <= 0 {
                return Err(format!("Field numbers must be greater than zero at {}", val));
            }
            let idx = (n - 1) as usize;
            if idx < row.len() {
                Ok(row[idx].clone())
            } else {
                Ok(String::new())
            }
        }
    } else {
        Ok(val.to_string())
    }
}

pub fn run_map(
    mut io: IOManager,
    fields: Option<String>,
    from_val_str: String,
    to_val_str: String,
    ignore_case: bool,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let col_index = fields.map(|s| parse_indices(&s)).transpose()?;
    let from_list = parse_comma_list(&from_val_str);
    let to_list = parse_comma_list(&to_val_str);

    if to_list.len() > from_list.len() {
        return Err("List of 'to values' longer than list of 'from values".to_string());
    }

    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }

        if !pass {
            let mut mapped_row = row.clone();
            let limit = if let Some(ref cols) = col_index {
                cols.len()
            } else {
                row.len()
            };

            for idx in 0..limit {
                let actual_idx = if let Some(ref cols) = col_index {
                    cols[idx]
                } else {
                    idx
                };

                if actual_idx < row.len() {
                    let val = &row[actual_idx];
                    let mut found_idx: Option<usize> = None;
                    for (i, fv) in from_list.iter().enumerate() {
                        let is_eq = if ignore_case {
                            val.eq_ignore_ascii_case(fv)
                        } else {
                            val == fv
                        };
                        if is_eq {
                            found_idx = Some(i);
                            break;
                        }
                    }

                    if let Some(i) = found_idx {
                        let to_val = if to_list.is_empty() {
                            String::new()
                        } else if i < to_list.len() {
                            expand_map_to(&to_list[i], &row)?
                        } else {
                            expand_map_to(to_list.last().unwrap(), &row)?
                        };
                        mapped_row[actual_idx] = to_val;
                    }
                }
            }
            row = mapped_row;
        }
        io.write_row(&row)?;
    }
    Ok(())
}

// 25. Merge Command
fn expand_sep(sep: &str) -> Result<String, String> {
    let mut s = String::new();
    let mut chars = sep.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('t') => s.push('\t'),
                Some('n') => s.push('\n'),
                Some('r') => s.push('\r'),
                Some('\\') => s.push('\\'),
                Some(other) => return Err(format!("Invalid special character: \\{}", other)),
                None => return Err(format!("Invalid escape at end of separator: {}", sep)),
            }
        } else {
            s.push(c);
        }
    }
    Ok(s)
}

pub fn run_merge(
    mut io: IOManager,
    fields: Option<String>,
    sep_str: Option<String>,
    pos_str: Option<String>,
    keep: bool,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    eprintln!("DEBUG MERGE: fields={:?}, sep_str={:?}, pos_str={:?}, keep={}", fields, sep_str, pos_str, keep);
    let cols = if let Some(ref f) = fields {
        let indices = parse_indices(f)?;
        if indices.len() <= 1 {
            return Err("Need to specify two or more fields with -f flag".to_string());
        }
        indices
    } else {
        Vec::new()
    };

    let sep = expand_sep(sep_str.as_deref().unwrap_or(" "))?;
    let default_pos = if !cols.is_empty() {
        (cols[0] + 1).to_string()
    } else {
        "1".to_string()
    };
    let pos_val = pos_str.as_ref().unwrap_or(&default_pos);
    let p = pos_val.parse::<i32>().map_err(|_| format!("Position specified by -p must be integer"))? - 1;
    if p < 0 {
        return Err("Position specified by -p must be greater than zero".to_string());
    }
    let m_pos = p as usize;

    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }

        if !pass {
            let mut merged = String::new();
            if !cols.is_empty() {
                for (i, &ci) in cols.iter().enumerate() {
                    if ci < row.len() {
                        merged.push_str(&row[ci]);
                        if i != cols.len() - 1 {
                            merged.push_str(&sep);
                        }
                    }
                }
            } else {
                for (i, val) in row.iter().enumerate() {
                    if i > 0 {
                        merged.push_str(&sep);
                    }
                    merged.push_str(val);
                }
            }

            let mut newrow = Vec::new();
            for i in 0..row.len() {
                if m_pos == i {
                    newrow.push(merged.clone());
                }
                if keep || (!cols.is_empty() && !cols.contains(&i)) {
                    newrow.push(row[i].clone());
                }
            }
            if m_pos >= row.len() {
                newrow.push(merged);
            }
            row = newrow;
        }
        io.write_row(&row)?;
    }
    Ok(())
}

// 26. Money Command
fn is_numeric(s: &str) -> bool {
    s.trim().parse::<f64>().is_ok()
}

fn format_money_val(
    v: &str,
    cents: bool,
    decimal_point: char,
    thou_sep: Option<char>,
    symbol: &str,
    plus: &str,
    minus: &str,
    width: usize,
) -> String {
    if !is_numeric(v) {
        return v.to_string();
    }

    let mut m = v.trim().parse::<f64>().unwrap_or(0.0);
    if cents {
        m /= 100.0;
    }

    let is_negative = m < 0.0 || (m == 0.0 && v.trim().starts_with('-'));
    let abs_m = m.abs();
    let fs = format!("{:.2}", abs_m);
    let dot_idx = fs.find('.').unwrap();
    let dollars = &fs[..dot_idx];
    let cents_part = &fs[dot_idx + 1..];

    let mut dsep = String::new();
    let mut dcount = 0;
    for (i, c) in dollars.chars().rev().enumerate() {
        if i > 0 && dcount == 3 {
            if let Some(sep) = thou_sep {
                dsep.push(sep);
            }
            dcount = 0;
        }
        dsep.push(c);
        dcount += 1;
    }
    let dsep_rev: String = dsep.chars().rev().collect();
    let formatted_money = format!("{}{}{}", dsep_rev, decimal_point, cents_part);
    let smoney = if width > 0 {
        format!("{:>width$}", formatted_money, width = width)
    } else {
        formatted_money
    };

    let sign = if is_negative { minus } else { plus };
    format!("{}{}{}", sign, symbol, smoney)
}

pub fn run_money(
    mut io: IOManager,
    fields: Option<String>,
    dp_str: Option<String>,
    ts_str: Option<String>,
    symbol: String,
    plus: String,
    minus: String,
    cents: bool,
    replace: bool,
    width_str: Option<String>,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let col_index = fields.map(|s| parse_indices(&s)).transpose()?;

    let dp = if let Some(ref s) = dp_str {
        if s.len() != 1 {
            return Err("Invalid decimal point value".to_string());
        }
        s.chars().next().unwrap()
    } else {
        '.'
    };

    let ts = if let Some(ref s) = ts_str {
        if s.is_empty() {
            None
        } else {
            Some(s.chars().next().unwrap())
        }
    } else {
        Some(',')
    };

    let width = if let Some(ref ws) = width_str {
        let w = ws.parse::<i32>().map_err(|_| format!("Width specified by -w must be integer"))?;
        if w < 0 || w > 50 {
            return Err(format!("Invalid width specified by -w: {}", ws));
        }
        w as usize
    } else {
        0
    };

    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }
        if pass {
            io.write_row(&row)?;
            continue;
        }

        let mut out = row.clone();
        for i in 0..row.len() {
            if col_index.is_none() || col_index.as_ref().unwrap().contains(&i) {
                let formatted = format_money_val(&row[i], cents, dp, ts, &symbol, &plus, &minus, width);
                if replace {
                    out[i] = formatted;
                } else {
                    out.push(formatted);
                }
            }
        }
        io.write_row(&out)?;
    }
    Ok(())
}

// 27. Printf Command
fn parse_printf_specifier(spec: &str) -> Option<PrintfSpec> {
    if !spec.starts_with('%') || spec.is_empty() {
        return None;
    }
    let chars: Vec<char> = spec.chars().collect();
    let mut i = 1;
    let mut flags = String::new();
    while i < chars.len() {
        let c = chars[i];
        if c == '-' || c == '+' || c == ' ' || c == '0' || c == '#' {
            // we ignore or handle below
            flags.push(c);
            i += 1;
        } else {
            break;
        }
    }

    let mut width = None;
    let mut w_str = String::new();
    while i < chars.len() && chars[i].is_ascii_digit() {
        w_str.push(chars[i]);
        i += 1;
    }
    if !w_str.is_empty() {
        width = w_str.parse::<usize>().ok();
    }

    let mut precision = None;
    if i < chars.len() && chars[i] == '.' {
        i += 1;
        let mut p_str = String::new();
        while i < chars.len() && chars[i].is_ascii_digit() {
            p_str.push(chars[i]);
            i += 1;
        }
        if !p_str.is_empty() {
            precision = p_str.parse::<usize>().ok();
        } else {
            precision = Some(0);
        }
    }

    if i < chars.len() {
        let conv = chars[i];
        Some(PrintfSpec { flags, width, precision, conv })
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub struct PrintfSpec {
    pub flags: String,
    pub width: Option<usize>,
    pub precision: Option<usize>,
    pub conv: char,
}

fn format_g(d: f64, precision: usize, upper: bool) -> String {
    let abs_d = d.abs();
    let use_scientific = abs_d > 0.0 && (abs_d < 1e-4 || abs_d >= 1e6);
    if use_scientific {
        if upper {
            format!("{:.precision$E}", d, precision = precision)
        } else {
            format!("{:.precision$e}", d, precision = precision)
        }
    } else {
        let mut s = format!("{:.precision$}", d, precision = precision);
        if s.contains('.') {
            while s.ends_with('0') {
                s.pop();
            }
            if s.ends_with('.') {
                s.pop();
            }
        }
        s
    }
}

fn pad_formatted(mut s: String, width: Option<usize>, left_align: bool, zero_pad: bool, sign: Option<char>) -> String {
    if let Some(c) = sign {
        s.insert(0, c);
    }
    if let Some(w) = width {
        if s.len() < w {
            let diff = w - s.len();
            if left_align {
                s.extend(std::iter::repeat(' ').take(diff));
            } else if zero_pad {
                let insert_pos = if sign.is_some() { 1 } else { 0 };
                s.insert_str(insert_pos, &"0".repeat(diff));
            } else {
                s.insert_str(0, &" ".repeat(diff));
            }
        }
    }
    s
}

fn format_printf_spec(spec: &PrintfSpec, val: &str) -> String {
    let left_align = spec.flags.contains('-');
    let zero_pad = spec.flags.contains('0') && !left_align;
    let plus_sign = spec.flags.contains('+');
    let space_sign = spec.flags.contains(' ');

    match spec.conv {
        's' => {
            let mut s = val.to_string();
            if let Some(prec) = spec.precision {
                s.truncate(prec);
            }
            if let Some(w) = spec.width {
                if s.len() < w {
                    let diff = w - s.len();
                    if left_align {
                        s.push_str(&" ".repeat(diff));
                    } else {
                        s.insert_str(0, &" ".repeat(diff));
                    }
                }
            }
            s
        }
        'f' | 'e' | 'E' | 'g' | 'G' => {
            let d = if val.trim().parse::<f64>().is_ok() {
                val.trim().parse::<f64>().unwrap()
            } else {
                0.0
            };
            let prec = spec.precision.unwrap_or(6);
            let s_raw = match spec.conv {
                'f' => format!("{:.precision$}", d.abs(), precision = prec),
                'e' => format!("{:.precision$e}", d.abs(), precision = prec),
                'E' => format!("{:.precision$E}", d.abs(), precision = prec),
                'g' => format_g(d.abs(), prec, false),
                'G' => format_g(d.abs(), prec, true),
                _ => String::new(),
            };

            let sign = if d < 0.0 || (d == 0.0 && val.trim().starts_with('-')) {
                Some('-')
            } else if plus_sign {
                Some('+')
            } else if space_sign {
                Some(' ')
            } else {
                None
            };
            pad_formatted(s_raw, spec.width, left_align, zero_pad, sign)
        }
        'd' | 'i' | 'o' | 'x' | 'X' | 'u' | 'c' => {
            let n = if val.trim().parse::<f64>().is_ok() {
                val.trim().parse::<f64>().unwrap() as i32
            } else {
                0
            };
            let mut s_raw = match spec.conv {
                'd' | 'i' => format!("{}", n.abs()),
                'o' => format!("{:o}", n.abs()),
                'x' => format!("{:x}", n.abs()),
                'X' => format!("{:X}", n.abs()),
                'u' => format!("{}", n as u32),
                'c' => format!("{}", (n as u8 as char)),
                _ => String::new(),
            };

            if let Some(prec) = spec.precision {
                if s_raw.len() < prec {
                    s_raw.insert_str(0, &"0".repeat(prec - s_raw.len()));
                }
            }

            let sign = if spec.conv != 'u' && (n < 0 || (n == 0 && val.trim().starts_with('-'))) {
                Some('-')
            } else if plus_sign && spec.conv != 'u' {
                Some('+')
            } else if space_sign && spec.conv != 'u' {
                Some(' ')
            } else {
                None
            };
            pad_formatted(s_raw, spec.width, left_align, zero_pad, sign)
        }
        _ => String::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FmtType {
    Literal,
    Formatter,
    Ignore,
}

#[derive(Debug, Clone)]
pub struct PrintfFmt {
    pub type_val: FmtType,
    pub text: String,
    pub spec: Option<PrintfSpec>,
}

pub fn run_printf(
    mut io: IOManager,
    fmt_str: String,
    fields: Option<String>,
    csv_quote: bool,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let order = fields.map(|s| parse_indices(&s)).transpose()?.unwrap_or_default();
    let mut fmt_list = Vec::new();
    let chars: Vec<char> = fmt_str.chars().collect();
    let mut pos = 0;

    while pos < chars.len() {
        let c = chars[pos];
        if c == '%' {
            if pos + 1 < chars.len() && chars[pos + 1] == '%' {
                fmt_list.push(PrintfFmt {
                    type_val: FmtType::Literal,
                    text: "%".to_string(),
                    spec: None,
                });
                pos += 2;
            } else if pos + 1 < chars.len() && chars[pos + 1] == '@' {
                fmt_list.push(PrintfFmt {
                    type_val: FmtType::Ignore,
                    text: String::new(),
                    spec: None,
                });
                pos += 2;
            } else {
                let mut f_str = "%".to_string();
                pos += 1;
                let mut found_conv = false;
                while pos < chars.len() {
                    let ch = chars[pos];
                    pos += 1;
                    f_str.push(ch);
                    if ch.is_ascii_alphabetic() {
                        let ok = "dioxXucsfeEgG";
                        if !ok.contains(ch) {
                            return Err(format!("Invalid conversion type: {}", ch));
                        }
                        found_conv = true;
                        break;
                    }
                }
                if !found_conv {
                    return Err("Unexpected end of format".to_string());
                }

                let spec = parse_printf_specifier(&f_str).ok_or_else(|| format!("Invalid format specifier: {}", f_str))?;
                fmt_list.push(PrintfFmt {
                    type_val: FmtType::Formatter,
                    text: f_str,
                    spec: Some(spec),
                });
            }
        } else {
            let mut lit = String::new();
            while pos < chars.len() && chars[pos] != '%' {
                lit.push(chars[pos]);
                pos += 1;
            }
            fmt_list.push(PrintfFmt {
                type_val: FmtType::Literal,
                text: lit,
                spec: None,
            });
        }
    }

    if fmt_list.is_empty() {
        return Err("Empty format string not allowed".to_string());
    }

    let get_field = |row: &[String], order: &[usize], fieldno: usize| -> String {
        if order.is_empty() {
            if fieldno >= row.len() {
                String::new()
            } else {
                row[fieldno].clone()
            }
        } else {
            if fieldno >= order.len() {
                String::new()
            } else {
                let actual_idx = order[fieldno];
                if actual_idx >= row.len() {
                    String::new()
                } else {
                    row[actual_idx].clone()
                }
            }
        }
    };

    let csv_quote_fn = |s: &str| -> String {
        let mut out = String::new();
        for c in s.chars() {
            if c == '"' {
                out.push('"');
            }
            out.push(c);
        }
        out
    };

    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }
        if pass {
            io.write_row(&row)?;
        } else {
            let mut line = String::new();
            let mut fieldno = 0;

            for fmt in &fmt_list {
                match fmt.type_val {
                    FmtType::Literal => {
                        line.push_str(&fmt.text);
                    }
                    FmtType::Ignore => {
                        fieldno += 1;
                    }
                    FmtType::Formatter => {
                        let field_val = get_field(&row, &order, fieldno);
                        fieldno += 1;
                        let spec = fmt.spec.as_ref().unwrap();
                        let formatted = format_printf_spec(spec, &field_val);
                        if csv_quote && spec.conv == 's' {
                            line.push_str(&csv_quote_fn(&formatted));
                        } else {
                            line.push_str(&formatted);
                        }
                    }
                }
            }
            writeln!(io.output_writer, "{}", line).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// 28. Put Command
pub fn run_put(
    mut io: IOManager,
    pos_str: Option<String>,
    val: Option<String>,
    env: Option<String>,
) -> Result<(), String> {
    if val.is_some() && env.is_some() {
        return Err("Cannot specify both -v and -e options".to_string());
    }
    if val.is_none() && env.is_none() {
        return Err("Need one of -v or -e options".to_string());
    }

    let put_val = if let Some(ref v) = val {
        v.clone()
    } else {
        let ev = env.as_ref().unwrap();
        if ev == "@DATETIME" {
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
        } else if ev == "@DATE" {
            chrono::Local::now().format("%Y-%m-%d").to_string()
        } else if ev == "@COUNT" {
            "@COUNT".to_string()
        } else {
            std::env::var(ev).unwrap_or_default()
        }
    };

    let p = if let Some(ref ps) = pos_str {
        let pos = ps.parse::<i32>().map_err(|_| format!("Position must be non-zero integer"))? - 1;
        if pos < 0 {
            return Err(format!("Invalid position value: {}", ps));
        }
        Some(pos as usize)
    } else {
        None
    };

    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let current_val = if put_val == "@COUNT" {
            row.len().to_string()
        } else {
            put_val.clone()
        };

        if let Some(pos_idx) = p {
            if pos_idx < row.len() {
                row.insert(pos_idx, current_val);
            } else {
                row.push(current_val);
            }
        } else {
            row.push(current_val);
        }
        io.write_row(&row)?;
    }
    Ok(())
}

// 29. Read Fixed / Write Fixed Commands
pub fn run_read_fixed(
    mut io: IOManager,
    fields: String,
    keep: bool,
) -> Result<(), String> {
    let mut field_specs = Vec::new();
    for item in fields.split(',') {
        let parts: Vec<&str> = item.split(':').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid field specification: {}", item));
        }
        let f1 = parts[0].parse::<usize>().map_err(|_| format!("Invalid field specification: {}", item))?;
        let f2 = parts[1].parse::<usize>().map_err(|_| format!("Invalid field specification: {}", item))?;
        if f1 == 0 || f2 == 0 {
            return Err(format!("Invalid field specification: {}", item));
        }
        field_specs.push((f1, f2));
    }

    let trim = !keep;
    let mut line = String::new();
    while io.read_line(&mut line)? {
        let mut row = Vec::new();
        let chars: Vec<char> = line.chars().collect();
        let len = chars.len();
        for &(start_pos, count) in &field_specs {
            if start_pos > len {
                row.push(String::new());
            } else {
                let end_pos = std::cmp::min(len, start_pos - 1 + count);
                let val_str: String = chars[start_pos - 1..end_pos].iter().collect();
                let val = if trim {
                    val_str.trim_end().to_string()
                } else {
                    val_str
                };
                row.push(val);
            }
        }
        io.write_row(&row)?;
    }
    Ok(())
}

pub fn run_write_fixed(
    mut io: IOManager,
    fields: String,
    ruler: bool,
    skip_expr: Option<Expr>,
) -> Result<(), String> {
    let mut field_specs = Vec::new();
    for item in fields.split(',') {
        let parts: Vec<&str> = item.split(':').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid field specification: {}", item));
        }
        let f1 = parts[0].parse::<usize>().map_err(|_| format!("Invalid field specification: {}", item))?;
        let f2 = parts[1].parse::<usize>().map_err(|_| format!("Invalid field specification: {}", item))?;
        if f1 == 0 || f2 == 0 {
            return Err(format!("Invalid field specification: {}", item));
        }
        field_specs.push((f1, f2));
    }

    if ruler {
        let mut r = String::new();
        for _ in 0..8 {
            r.push_str("123456789 ");
        }
        writeln!(io.output_writer, "{}", r).map_err(|e| e.to_string())?;
    }

    let right_pad_fn = |s: &str, width: usize| -> String {
        if s.len() >= width {
            s[..width].to_string()
        } else {
            let mut padded = s.to_string();
            padded.push_str(&" ".repeat(width - s.len()));
            padded
        }
    };

    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, _) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            None,
        );
        if skip {
            continue;
        }

        let mut line = String::new();
        for &(field_idx, width) in &field_specs {
            let val = if field_idx > row.len() {
                ""
            } else {
                &row[field_idx - 1]
            };
            line.push_str(&right_pad_fn(val, width));
        }
        writeln!(io.output_writer, "{}", line).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// 30. Read Multi / Write Multi Commands
pub fn run_read_multi(
    mut io: IOManager,
    num_lines: Option<usize>,
    sep: Option<String>,
) -> Result<(), String> {
    if num_lines.is_some() && sep.is_some() {
        return Err("Cannot specify both -s and -n options".to_string());
    }
    if num_lines.is_none() && sep.is_none() {
        return Err("Need one of -s or -n options".to_string());
    }

    let n_lines = if let Some(n) = num_lines {
        if n < 1 || n > 200 {
            return Err(format!("Invalid number of lines: {}", n));
        }
        n
    } else {
        0
    };

    let separator = sep.unwrap_or_default();
    if n_lines == 0 && separator.is_empty() {
        return Err("Empty separator".to_string());
    }

    let mut row = Vec::new();
    let mut line = String::new();
    let mut nread = 0;

    while io.read_line(&mut line)? {
        nread += 1;
        if n_lines > 0 {
            row.push(line.clone());
            if nread == n_lines {
                io.write_row(&row)?;
                row.clear();
                nread = 0;
            }
        } else {
            if line == separator {
                io.write_row(&row)?;
                row.clear();
            } else {
                row.push(line.clone());
            }
        }
    }

    if !row.is_empty() {
        io.write_row(&row)?;
    }
    Ok(())
}

pub fn run_write_multi(
    mut io: IOManager,
    master_fields: String,
    detail_fields: Option<String>,
    rec_sep: Option<String>,
    skip_expr: Option<Expr>,
) -> Result<(), String> {
    let m_fields = parse_indices(&master_fields)?;
    if m_fields.is_empty() {
        return Err("Must specify at least one master field with -m option".to_string());
    }
    let d_fields = detail_fields.map(|s| parse_indices(&s)).transpose()?;

    let mut row = Vec::new();
    let mut current_master = Vec::new();
    let mut haveout = false;

    let get_master_fields = |r: &[String], mf: &[usize]| -> Vec<String> {
        let mut m = Vec::new();
        for &idx in mf {
            if idx < r.len() {
                m.push(r[idx].clone());
            } else {
                m.push(String::new());
            }
        }
        m
    };

    let get_detail_fields = |r: &[String], mf: &[usize], df: &Option<Vec<usize>>| -> Vec<String> {
        let mut d = Vec::new();
        if let Some(dfs) = df {
            for &idx in dfs {
                if idx < r.len() {
                    d.push(r[idx].clone());
                } else {
                    d.push(String::new());
                }
            }
        } else {
            for (i, val) in r.iter().enumerate() {
                if !mf.contains(&i) {
                    d.push(val.clone());
                }
            }
        }
        d
    };

    while io.read_csv(&mut row)? {
        let (skip, _) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            None,
        );
        if skip {
            continue;
        }

        let next_master = get_master_fields(&row, &m_fields);
        let is_new_master = current_master.is_empty() || next_master != current_master;

        if is_new_master {
            if haveout {
                if let Some(ref sep) = rec_sep {
                    io.write_row(&[sep.clone()])?;
                }
            } else {
                haveout = true;
            }
            current_master = next_master;
            io.write_row(&current_master)?;
        }

        let detail = get_detail_fields(&row, &m_fields, &d_fields);
        io.write_row(&detail)?;
    }

    if haveout {
        if let Some(ref sep) = rec_sep {
            io.write_row(&[sep.clone()])?;
        }
    }
    Ok(())
}

// 31. Remove Newline Command
fn expand_rmnew_sep(sep: &str) -> Result<String, String> {
    let mut s = String::new();
    let mut chars = sep.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next_c) = chars.next() {
                if next_c == 't' {
                    s.push('\t');
                } else {
                    s.push(next_c);
                }
            } else {
                return Err("Invalid escape at end of string".to_string());
            }
        } else {
            s.push(c);
        }
    }
    Ok(s)
}

pub fn run_rmnew(
    mut io: IOManager,
    fields: Option<String>,
    sep_str: Option<String>,
    exclude_after: bool,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let col_index = fields.map(|s| parse_indices(&s)).transpose()?;
    if exclude_after && sep_str.is_some() {
        return Err("Flags -x and -s are mutually exclusive".to_string());
    }

    let sep = if let Some(ref s) = sep_str {
        expand_rmnew_sep(s)?
    } else {
        String::new()
    };

    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }

        if !pass {
            for i in 0..row.len() {
                if col_index.is_none() || col_index.as_ref().unwrap().contains(&i) {
                    if row[i].contains('\n') {
                        let mut s = String::new();
                        let mut excluded = false;
                        for c in row[i].chars() {
                            if c == '\n' {
                                if exclude_after {
                                    excluded = true;
                                    break;
                                } else {
                                    s.push_str(&sep);
                                }
                            } else {
                                s.push(c);
                            }
                        }
                        if excluded || !s.is_empty() || row[i].starts_with('\n') {
                            row[i] = s;
                        }
                    }
                }
            }
        }
        io.write_row(&row)?;
    }
    Ok(())
}

// 32. Split Char / Split Fixed Commands
fn insert_split(row: &mut Vec<String>, field_idx: usize, split_parts: &[String], keep: bool) {
    let mut tmp = Vec::new();
    for j in 0..row.len() {
        if j == field_idx {
            for part in split_parts {
                tmp.push(part.clone());
            }
            if keep {
                tmp.push(row[j].clone());
            }
        } else {
            tmp.push(row[j].clone());
        }
    }
    *row = tmp;
}

pub fn run_split_fixed(
    mut io: IOManager,
    field_str: String,
    pos_list_str: String,
    keep: bool,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let f_idx = field_str.parse::<i32>().map_err(|_| format!("Field specified by -f must be integer"))? - 1;
    if f_idx < 0 {
        return Err(format!("Invalid field index: {}", field_str));
    }
    let field_idx = f_idx as usize;

    let mut positions = Vec::new();
    for item in pos_list_str.split(',') {
        let parts: Vec<&str> = item.split(':').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid position {}", item));
        }
        let pos = parts[0].parse::<i32>().map_err(|_| format!("Invalid position {}", item))?;
        let len = parts[1].parse::<i32>().map_err(|_| format!("Invalid position {}", item))?;
        if pos <= 0 || len <= 0 {
            return Err(format!("Invalid position {}", item));
        }
        positions.push(((pos - 1) as usize, len as usize));
    }

    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }

        if !pass {
            let target = if field_idx < row.len() {
                &row[field_idx]
            } else {
                ""
            };
            let chars: Vec<char> = target.chars().collect();
            let mut split_parts = Vec::new();
            for &(start, len) in &positions {
                if start >= chars.len() {
                    split_parts.push(String::new());
                } else {
                    let end = std::cmp::min(chars.len(), start + len);
                    let part: String = chars[start..end].iter().collect();
                    split_parts.push(part);
                }
            }
            insert_split(&mut row, field_idx, &split_parts, keep);
        }
        io.write_row(&row)?;
    }
    Ok(())
}

fn unescape_str(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('t') => out.push('\t'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn split_char_alib(target: &str, sep: char) -> Vec<String> {
    let mut tok = Vec::new();
    let mut t = String::new();
    for c in target.chars() {
        if c == sep {
            tok.push(t.trim().to_string());
            t.clear();
        } else {
            t.push(c);
        }
    }
    if !(t.is_empty() && tok.is_empty()) {
        tok.push(t.trim().to_string());
    }
    tok
}

fn split_str_alib(target: &str, sep: &str) -> Vec<String> {
    let mut tok = Vec::new();
    let mut t = String::new();
    let mut i = 0;
    let chars: Vec<char> = target.chars().collect();
    let sep_chars: Vec<char> = sep.chars().collect();

    while i < chars.len() {
        let is_match = if i + sep_chars.len() <= chars.len() {
            chars[i..i+sep_chars.len()] == sep_chars[..]
        } else {
            false
        };
        if is_match {
            tok.push(t.clone());
            t.clear();
            i += sep_chars.len();
        } else {
            t.push(chars[i]);
            i += 1;
        }
    }
    if !(t.is_empty() && tok.is_empty()) {
        tok.push(t);
    }
    tok
}

fn trans_split(target: &str, is_alpha_to_num: bool) -> Vec<String> {
    let mut last: Option<char> = None;
    let chars: Vec<char> = target.chars().collect();
    for i in 0..chars.len() {
        let c = chars[i];
        if let Some(l) = last {
            let matches = if is_alpha_to_num {
                c.is_ascii_digit() && l.is_ascii_alphabetic()
            } else {
                c.is_ascii_alphabetic() && l.is_ascii_digit()
            };
            if matches {
                let part1: String = chars[..i].iter().collect();
                let part2: String = chars[i..].iter().collect();
                return vec![part1, part2];
            }
        }
        last = Some(c);
    }
    vec![target.to_string()]
}

pub fn run_split_char(
    mut io: IOManager,
    field_str: String,
    char_str: Option<String>,
    tan: bool,
    tna: bool,
    keep: bool,
    skip_expr: Option<Expr>,
    pass_expr: Option<Expr>,
) -> Result<(), String> {
    let f_idx = field_str.parse::<i32>().map_err(|_| format!("Field specified by -f must be integer"))? - 1;
    if f_idx < 0 {
        return Err(format!("Invalid field index: {}", field_str));
    }
    let field_idx = f_idx as usize;

    if tan && tna {
        return Err("Only one of -tan or -tna allowed".to_string());
    }

    let is_trans = tan || tna;
    let sep_chars = if is_trans {
        if char_str.is_some() {
            return Err("Cannot specify both character and transition".to_string());
        }
        String::new()
    } else {
        let sc = char_str.unwrap_or_else(|| " ".to_string());
        if sc.is_empty() {
            return Err("Need characters specified by -c".to_string());
        }
        unescape_str(&sc)
    };

    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, pass) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            pass_expr.as_ref(),
        );
        if skip {
            continue;
        }

        if !pass {
            let target = if field_idx < row.len() {
                &row[field_idx]
            } else {
                ""
            };
            let split_parts = if is_trans {
                trans_split(target, tan)
            } else {
                if sep_chars.len() == 1 {
                    split_char_alib(target, sep_chars.chars().next().unwrap())
                } else {
                    split_str_alib(target, &sep_chars)
                }
            };
            insert_split(&mut row, field_idx, &split_parts, keep);
        }
        io.write_row(&row)?;
    }
    Ok(())
}

// 33. SQL generation commands (sql_insert, sql_update, sql_delete)
pub struct SQLColSpec {
    pub field: usize,
    pub col_name: String,
}

fn parse_sql_col_specs(s: &str) -> Result<Vec<SQLColSpec>, String> {
    let mut specs = Vec::new();
    let mut havenames = false;
    for item in s.split(',') {
        let parts: Vec<&str> = item.split(':').collect();
        if parts.len() > 2 || parts.is_empty() {
            return Err(format!("Invalid column specification: {}", item));
        }
        let icol = parts[0].parse::<i32>().map_err(|_| format!("Field index must be integer in {}", item))?;
        if icol <= 0 {
            return Err(format!("Field index must be greater than zero in {}", item));
        }
        if parts.len() == 1 && havenames {
            return Err("Must specify all column names".to_string());
        }
        if parts.len() == 2 {
            havenames = true;
        }
        let col_name = if parts.len() == 2 { parts[1].to_string() } else { String::new() };
        specs.push(SQLColSpec { field: (icol - 1) as usize, col_name });
    }
    Ok(specs)
}

fn sql_quote(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c == '\'' {
            out.push('\'');
        }
        out.push(c);
    }
    out
}

fn squote(s: &str) -> String {
    format!("'{}'", s)
}

struct SQLConfig {
    table: String,
    sep: String,
    no_quote: Vec<usize>,
    quote_nulls: bool,
    empty_nulls: bool,
}

impl SQLConfig {
    fn new(
        table: String,
        sep_opt: Option<String>,
        no_quote_str: Option<String>,
        quote_nulls: bool,
        empty_nulls: bool,
    ) -> Result<Self, String> {
        if table.is_empty() {
            return Err("Need table name".to_string());
        }
        let sep = if let Some(s) = sep_opt {
            unescape_str(&s)
        } else {
            "\n;\n".to_string()
        };
        let no_quote = if let Some(ref nq) = no_quote_str {
            if nq.is_empty() {
                Vec::new()
            } else {
                parse_indices(nq)?
            }
        } else {
            Vec::new()
        };
        Ok(Self { table, sep, no_quote, quote_nulls, empty_nulls })
    }

    fn do_sql_quote(&self, i: usize) -> bool {
        !self.no_quote.contains(&i)
    }

    fn no_null_quote(&self, field: &str) -> bool {
        if field.eq_ignore_ascii_case("NULL") {
            !self.quote_nulls
        } else {
            false
        }
    }

    fn empty_to_null(&self, field: &str) -> String {
        if self.empty_nulls && field.is_empty() {
            "NULL".to_string()
        } else {
            field.to_string()
        }
    }

    fn make_value_expr(&self, i: usize, field: &str) -> String {
        let f_val = self.empty_to_null(field);
        if self.do_sql_quote(i) && !self.no_null_quote(&f_val) {
            squote(&sql_quote(&f_val))
        } else {
            f_val
        }
    }

    fn make_where_clause(&self, row: &[String], where_cols: &[SQLColSpec]) -> Result<String, String> {
        let mut wc = String::new();
        for (i, spec) in where_cols.iter().enumerate() {
            let wi = spec.field;
            if wi >= row.len() {
                return Err(format!("Required field {} missing in input", wi + 1));
            }
            if !wc.is_empty() {
                wc.push_str(" AND ");
            }
            let field = self.empty_to_null(&row[wi]);
            let op = if field == "NULL" { " IS " } else { " = " };
            let val_expr = self.make_value_expr(i, &row[wi]);
            wc.push_str(&format!("{}{}{}", spec.col_name, op, val_expr));
        }
        Ok(format!("WHERE {}", wc))
    }
}

pub fn run_sql_insert(
    mut io: IOManager,
    table: String,
    fields: String,
    sep_opt: Option<String>,
    no_quote_str: Option<String>,
    quote_nulls: bool,
    empty_nulls: bool,
    skip_expr: Option<Expr>,
) -> Result<(), String> {
    let cfg = SQLConfig::new(table, sep_opt, no_quote_str, quote_nulls, empty_nulls)?;
    let data_cols = parse_sql_col_specs(&fields)?;
    if data_cols.is_empty() || data_cols[0].col_name.is_empty() {
        return Err("Need column names specified by -f flag".to_string());
    }

    let mut colnames = "( ".to_string();
    for (i, spec) in data_cols.iter().enumerate() {
        if i > 0 {
            colnames.push_str(", ");
        }
        colnames.push_str(&spec.col_name);
    }
    colnames.push_str(" )");

    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, _) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            None,
        );
        if skip {
            continue;
        }

        let mut vals = String::new();
        for (i, spec) in data_cols.iter().enumerate() {
            let fi = spec.field;
            if fi >= row.len() {
                return Err(format!("Required field {} missing from input", fi + 1));
            }
            if i > 0 {
                vals.push_str(", ");
            }
            vals.push_str(&cfg.make_value_expr(i, &row[fi]));
        }

        let sql = format!("INSERT INTO {} {} VALUES( {}){}", cfg.table, colnames, vals, cfg.sep);
        write!(io.output_writer, "{}", sql).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn run_sql_update(
    mut io: IOManager,
    table: String,
    fields: String,
    where_fields: String,
    sep_opt: Option<String>,
    no_quote_str: Option<String>,
    quote_nulls: bool,
    empty_nulls: bool,
    skip_expr: Option<Expr>,
) -> Result<(), String> {
    let cfg = SQLConfig::new(table, sep_opt, no_quote_str, quote_nulls, empty_nulls)?;
    let data_cols = parse_sql_col_specs(&fields)?;
    if data_cols.is_empty() || data_cols[0].col_name.is_empty() {
        return Err("Need column names specified by -f flag".to_string());
    }
    let where_cols = parse_sql_col_specs(&where_fields)?;
    if where_cols.is_empty() || where_cols[0].col_name.is_empty() {
        return Err("Need column names specified by -w flag".to_string());
    }

    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, _) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            None,
        );
        if skip {
            continue;
        }

        let mut set_clause = "SET ".to_string();
        for (i, spec) in data_cols.iter().enumerate() {
            let fi = spec.field;
            if fi >= row.len() {
                return Err(format!("Required field {} missing from input", fi + 1));
            }
            if i > 0 {
                set_clause.push_str(", ");
            }
            let val_expr = cfg.make_value_expr(i, &row[fi]);
            set_clause.push_str(&format!("{} = {}", spec.col_name, val_expr));
        }

        let where_clause = cfg.make_where_clause(&row, &where_cols)?;
        let sql = format!("UPDATE {} {} {}{}", cfg.table, set_clause, where_clause, cfg.sep);
        write!(io.output_writer, "{}", sql).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn run_sql_delete(
    mut io: IOManager,
    table: String,
    where_fields: String,
    sep_opt: Option<String>,
    no_quote_str: Option<String>,
    quote_nulls: bool,
    empty_nulls: bool,
    skip_expr: Option<Expr>,
) -> Result<(), String> {
    let cfg = SQLConfig::new(table, sep_opt, no_quote_str, quote_nulls, empty_nulls)?;
    let where_cols = parse_sql_col_specs(&where_fields)?;
    if where_cols.is_empty() || where_cols[0].col_name.is_empty() {
        return Err("Need column names specified by -w flag".to_string());
    }

    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let (skip, _) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            None,
        );
        if skip {
            continue;
        }

        let where_clause = cfg.make_where_clause(&row, &where_cols)?;
        let sql = format!("DELETE FROM {} {}{}", cfg.table, where_clause, cfg.sep);
        write!(io.output_writer, "{}", sql).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// 34. Stat Command
pub fn run_stat(
    mut io: IOManager,
) -> Result<(), String> {
    let mut filename = String::new();
    let mut lines = 0;
    let mut fmin = std::i32::MAX;
    let mut fmax = 0;

    let mut output_stats = |iom: &mut IOManager, fname: &str, l: i32, minf: i32, maxf: i32| -> Result<(), String> {
        let row = vec![
            fname.to_string(),
            l.to_string(),
            minf.to_string(),
            maxf.to_string(),
        ];
        iom.write_row(&row)?;
        Ok(())
    };

    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let current_file = io.current_file_name().to_string();
        if filename != current_file {
            if !filename.is_empty() {
                output_stats(&mut io, &filename, lines, fmin, fmax)?;
            }
            filename = current_file;
            lines = 0;
            fmin = std::i32::MAX;
            fmax = 0;
        }
        lines += 1;
        fmin = std::cmp::min(fmin, row.len() as i32);
        fmax = std::cmp::max(fmax, row.len() as i32);
    }

    if !filename.is_empty() {
        output_stats(&mut io, &filename, lines, fmin, fmax)?;
    }
    Ok(())
}

// 35. Summary Command
fn nscmp(s1: &str, s2: &str) -> std::cmp::Ordering {
    let n1 = s1.trim().parse::<f64>();
    let n2 = s2.trim().parse::<f64>();
    match (n1, n2) {
        (Ok(v1), Ok(v2)) => v1.partial_cmp(&v2).unwrap_or(std::cmp::Ordering::Equal),
        _ => s1.cmp(s2),
    }
}

fn cmp_summary_rows(r1: &[String], r2: &[String], fields: &[usize]) -> Result<std::cmp::Ordering, String> {
    for &fi in fields {
        if fi >= r1.len() || fi >= r2.len() {
            return Err("Bad field index".to_string());
        }
        let o = nscmp(&r1[fi], &r2[fi]);
        if o != std::cmp::Ordering::Equal {
            return Ok(o);
        }
    }
    Ok(std::cmp::Ordering::Equal)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummaryType {
    Average,
    Min,
    Max,
    Frequency,
    Median,
    Mode,
    Sum,
    Size,
}

struct FreqMapEntry {
    freq: usize,
    indices: Vec<usize>,
}

pub fn run_summary(
    mut io: IOManager,
    avg: Option<String>,
    min: Option<String>,
    max: Option<String>,
    freq: Option<String>,
    median: Option<String>,
    mode: Option<String>,
    sum: Option<String>,
    size: bool,
) -> Result<(), String> {
    let mut flag_count = 0;
    let mut m_type = SummaryType::Size;
    let mut fields_str = String::new();

    if let Some(ref s) = avg { flag_count += 1; m_type = SummaryType::Average; fields_str = s.clone(); }
    if let Some(ref s) = min { flag_count += 1; m_type = SummaryType::Min; fields_str = s.clone(); }
    if let Some(ref s) = max { flag_count += 1; m_type = SummaryType::Max; fields_str = s.clone(); }
    if let Some(ref s) = freq { flag_count += 1; m_type = SummaryType::Frequency; fields_str = s.clone(); }
    if let Some(ref s) = median { flag_count += 1; m_type = SummaryType::Median; fields_str = s.clone(); }
    if let Some(ref s) = mode { flag_count += 1; m_type = SummaryType::Mode; fields_str = s.clone(); }
    if let Some(ref s) = sum { flag_count += 1; m_type = SummaryType::Sum; fields_str = s.clone(); }
    if size { flag_count += 1; m_type = SummaryType::Size; }

    if flag_count == 0 {
        return Err("Need a summary flag".to_string());
    } else if flag_count != 1 {
        return Err("Only one summary flag allowed".to_string());
    }

    let fields = if m_type != SummaryType::Size {
        parse_indices(&fields_str)?
    } else {
        Vec::new()
    };

    let mut row = Vec::new();
    let mut rows = Vec::new();
    let mut size_map: HashMap<usize, (usize, usize)> = HashMap::new();

    while io.read_csv(&mut row)? {
        if m_type == SummaryType::Size {
            for (i, val) in row.iter().enumerate() {
                let sz = val.len();
                let entry = size_map.entry(i).or_insert((std::usize::MAX, 0));
                entry.0 = std::cmp::min(entry.0, sz);
                entry.1 = std::cmp::max(entry.1, sz);
            }
        } else {
            rows.push(row.clone());
        }
    }

    if m_type == SummaryType::Size {
        let mut keys: Vec<&usize> = size_map.keys().collect();
        keys.sort();
        for &k in keys {
            let (min_l, max_l) = size_map.get(&k).unwrap();
            writeln!(io.output_writer, "{}: {},{}", k + 1, min_l, max_l).map_err(|e| e.to_string())?;
        }
    } else {
        if rows.is_empty() {
            return Err("No input".to_string());
        }

        let make_key = |r: &[String], fs: &[usize]| -> String {
            let mut key = String::new();
            for &fi in fs {
                if fi < r.len() {
                    key.push_str(&r[fi]);
                }
                key.push('\0');
            }
            key
        };

        match m_type {
            SummaryType::Min | SummaryType::Max => {
                let mut best_row = rows[0].clone();
                for r in rows.iter().skip(1) {
                    let ord = cmp_summary_rows(r, &best_row, &fields)?;
                    if m_type == SummaryType::Min && ord == std::cmp::Ordering::Less {
                        best_row = r.clone();
                    } else if m_type == SummaryType::Max && ord == std::cmp::Ordering::Greater {
                        best_row = r.clone();
                    }
                }
                for r in &rows {
                    if cmp_summary_rows(&best_row, r, &fields)? == std::cmp::Ordering::Equal {
                        io.write_row(r)?;
                    }
                }
            }
            SummaryType::Sum | SummaryType::Average => {
                let mut sums = vec![0.0; fields.len()];
                for r in &rows {
                    for (i, &fi) in fields.iter().enumerate() {
                        if fi >= r.len() {
                            return Err("Invalid field index".to_string());
                        }
                        sums[i] += r[fi].trim().parse::<f64>().unwrap_or(0.0);
                    }
                }
                let mut out = Vec::new();
                for s in sums {
                    let val = if m_type == SummaryType::Average {
                        s / (rows.len() as f64)
                    } else {
                        s
                    };
                    out.push(val.to_string());
                }
                io.write_row(&out)?;
            }
            SummaryType::Frequency | SummaryType::Mode => {
                let mut freq_map: HashMap<String, FreqMapEntry> = HashMap::new();
                let mut max_freq = 0;
                for (i, r) in rows.iter().enumerate() {
                    let key = make_key(r, &fields);
                    let entry = freq_map.entry(key).or_insert(FreqMapEntry { freq: 0, indices: Vec::new() });
                    entry.freq += 1;
                    entry.indices.push(i);
                    if entry.freq > max_freq {
                        max_freq = entry.freq;
                    }
                }

                if m_type == SummaryType::Frequency {
                    for r in &rows {
                        let key = make_key(r, &fields);
                        let n = freq_map.get(&key).unwrap().freq;
                        let mut out = vec![n.to_string()];
                        out.extend(r.clone());
                        io.write_row(&out)?;
                    }
                } else {
                    for entry in freq_map.values() {
                        if entry.freq == max_freq {
                            for &idx in &entry.indices {
                                let mut out = vec![max_freq.to_string()];
                                out.extend(rows[idx].clone());
                                io.write_row(&out)?;
                            }
                        }
                    }
                }
            }
            SummaryType::Median => {
                let mut out = Vec::new();
                for &col in &fields {
                    let mut col_rows = rows.clone();
                    col_rows.sort_by(|r1, r2| {
                        if col >= r1.len() || col >= r2.len() {
                            std::cmp::Ordering::Equal
                        } else {
                            let d1 = r1[col].trim().parse::<f64>().unwrap_or(0.0);
                            let d2 = r2[col].trim().parse::<f64>().unwrap_or(0.0);
                            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
                        }
                    });

                    for r in &col_rows {
                        if col >= r.len() {
                            return Err(format!("Invalid field index {}", col + 1));
                        }
                    }

                    let sz = col_rows.len();
                    let d = if sz % 2 == 1 {
                        col_rows[sz / 2][col].trim().parse::<f64>().unwrap_or(0.0)
                    } else {
                        let d1 = col_rows[sz / 2 - 1][col].trim().parse::<f64>().unwrap_or(0.0);
                        let d2 = col_rows[sz / 2][col].trim().parse::<f64>().unwrap_or(0.0);
                        (d1 + d2) / 2.0
                    };
                    out.push(d.to_string());
                }
                io.write_row(&out)?;
            }
            SummaryType::Size => {}
        }
    }

    Ok(())
}

// 36. Validate Command
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub ok: bool,
    pub field: usize,
    pub msg: String,
}

impl ValidationResult {
    pub fn ok() -> Self {
        Self { ok: true, field: 0, msg: String::new() }
    }
    pub fn err(field: usize, msg: String) -> Self {
        Self { ok: false, field, msg }
    }
}

pub enum Rule {
    Required { fields: Vec<usize> },
    NotEmpty { fields: Vec<usize> },
    Numeric { fields: Vec<usize>, ranges: Vec<(f64, f64)> },
    Fields { min: usize, max: usize },
    Values { fields: Vec<usize>, not_values: bool, list: Vec<String> },
    Length { fields: Vec<usize>, min: usize, max: usize },
    Lookup { joins: Vec<(usize, usize)>, lookup_file: String, join_vals: HashSet<String> },
    Date { fields: Vec<usize>, mask: String, range: Option<(NaiveDate, NaiveDate)>, reader: MaskedDateReader },
}

impl Rule {
    pub fn apply(&self, row: &[String]) -> Vec<ValidationResult> {
        let mut results = Vec::new();
        match self {
            Rule::Required { fields } => {
                for &idx in fields {
                    if idx >= row.len() {
                        results.push(ValidationResult::err(idx + 1, "required field missing".to_string()));
                    }
                }
            }
            Rule::NotEmpty { fields } => {
                for &idx in fields {
                    if idx < row.len() && row[idx].trim().is_empty() {
                        results.push(ValidationResult::err(idx + 1, "field is empty".to_string()));
                    }
                }
            }
            Rule::Numeric { fields, ranges } => {
                for &idx in fields {
                    if idx < row.len() {
                        let val = &row[idx];
                        if let Ok(n) = val.trim().parse::<f64>() {
                            if !ranges.is_empty() {
                                let mut ok = false;
                                for &(min_val, max_val) in ranges {
                                    if n >= min_val && n <= max_val {
                                        ok = true;
                                        break;
                                    }
                                }
                                if !ok {
                                    results.push(ValidationResult::err(idx + 1, format!("\"{}\" failed range check", val)));
                                }
                            }
                        } else {
                            results.push(ValidationResult::err(idx + 1, format!("\"{}\" is not numeric", val)));
                        }
                    }
                }
            }
            Rule::Fields { min, max } => {
                let n = row.len();
                if n < *min {
                    results.push(ValidationResult::err(1, "Not enough fields".to_string()));
                } else if n > *max {
                    results.push(ValidationResult::err(1, "Too many fields".to_string()));
                }
            }
            Rule::Values { fields, not_values, list } => {
                for &idx in fields {
                    if idx < row.len() {
                        let val = &row[idx];
                        let contains = list.contains(val);
                        let is_err = if *not_values { contains } else { !contains };
                        if is_err {
                            results.push(ValidationResult::err(idx + 1, format!("\"{}\" is invalid value", val)));
                        }
                    }
                }
            }
            Rule::Length { fields, min, max } => {
                for &idx in fields {
                    if idx < row.len() {
                        let val = &row[idx];
                        let len = val.len();
                        if len < *min {
                            results.push(ValidationResult::err(idx + 1, format!("\"{}\" is too short", val)));
                        } else if len > *max {
                            results.push(ValidationResult::err(idx + 1, format!("\"{}\" is too long", val)));
                        }
                    }
                }
            }
            Rule::Lookup { joins, lookup_file, join_vals } => {
                let mut key = String::new();
                for &(fi, _) in joins {
                    if fi < row.len() {
                        key.push_str(&row[fi]);
                    }
                    key.push('\0');
                }
                if !join_vals.contains(&key) {
                    let mut d = String::new();
                    let key_chars: Vec<char> = key.chars().collect();
                    for (i, &c) in key_chars.iter().enumerate() {
                        if c == '\0' {
                            if i != key_chars.len() - 1 {
                                d.push('|');
                            }
                        } else {
                            d.push(c);
                        }
                    }
                    results.push(ValidationResult::err(0, format!("lookup of '{}' in {} failed", d, lookup_file)));
                }
            }
            Rule::Date { fields, mask: _, range, reader } => {
                for &idx in fields {
                    if idx < row.len() {
                        let val = &row[idx];
                        if let Some(dt) = reader.read(val) {
                            if let Some((min_d, max_d)) = range {
                                if dt < *min_d || dt > *max_d {
                                    results.push(ValidationResult::err(idx + 1, format!("Date '{}' is out of range", val)));
                                }
                            }
                        } else {
                            results.push(ValidationResult::err(idx + 1, format!("Invalid date '{}'", val)));
                        }
                    }
                }
            }
        }
        results
    }
}

fn build_lookup_set(lookup_file: &str, joins: &[(usize, usize)]) -> Result<HashSet<String>, String> {
    let mut io = IOManager::new(
        vec![lookup_file.to_string()],
        None,
        false,
        false,
        ',',
        None,
        false,
        false,
        None,
        None,
    )?;

    let mut join_vals = HashSet::new();
    let mut row = Vec::new();
    while io.read_csv(&mut row)? {
        let mut key = String::new();
        for &(_, jf) in joins {
            if jf < row.len() {
                key.push_str(&row[jf]);
            }
            key.push('\0');
        }
        join_vals.insert(key);
    }
    Ok(join_vals)
}

fn parse_validation_line(line: &str) -> Result<Option<(String, Vec<usize>, Vec<String>)>, String> {
    let line_trimmed = line.trim();
    if line_trimmed.is_empty() || line_trimmed.starts_with('#') {
        return Ok(None);
    }

    let chars: Vec<char> = line.chars().collect();
    let mut pos = 0;

    while pos < chars.len() && chars[pos].is_whitespace() {
        pos += 1;
    }
    let mut name = String::new();
    while pos < chars.len() && !chars[pos].is_whitespace() {
        name.push(chars[pos]);
        pos += 1;
    }
    if name.is_empty() {
        return Ok(None);
    }

    while pos < chars.len() && chars[pos].is_whitespace() {
        pos += 1;
    }
    let mut fields_str = String::new();
    while pos < chars.len() && !chars[pos].is_whitespace() {
        fields_str.push(chars[pos]);
        pos += 1;
    }

    let mut fields = Vec::new();
    if !fields_str.is_empty() && fields_str != "*" {
        for f_str in fields_str.split(',') {
            let n = f_str.parse::<i32>().map_err(|_| format!("Invalid field list: {}", fields_str))?;
            if n <= 0 {
                return Err(format!("Invalid field list: {}", fields_str));
            }
            fields.push((n - 1) as usize);
        }
    }

    let mut params = Vec::new();
    while pos < chars.len() {
        while pos < chars.len() && chars[pos].is_whitespace() {
            pos += 1;
        }
        if pos >= chars.len() {
            break;
        }

        let c = chars[pos];
        if c == '\'' || c == '"' {
            let quote = c;
            let start = pos;
            pos += 1;
            let mut s = String::new();
            let mut matched = false;
            while pos < chars.len() {
                if chars[pos] == quote {
                    matched = true;
                    pos += 1;
                    break;
                }
                s.push(chars[pos]);
                pos += 1;
            }
            if !matched {
                let part: String = chars[start..].iter().collect();
                return Err(format!("Unterminated quoted value: {}", part));
            }
            params.push(s);
        } else {
            let mut s = String::new();
            while pos < chars.len() && !chars[pos].is_whitespace() {
                s.push(chars[pos]);
                pos += 1;
            }
            params.push(s);
        }
    }

    Ok(Some((name, fields, params)))
}

fn compile_rule(name: &str, fields: Vec<usize>, params: Vec<String>) -> Result<Rule, String> {
    match name {
        "required" => {
            if fields.is_empty() {
                return Err("required needs field list".to_string());
            }
            Ok(Rule::Required { fields })
        }
        "notempty" => {
            if fields.is_empty() {
                return Err("notempty needs field list".to_string());
            }
            Ok(Rule::NotEmpty { fields })
        }
        "numeric" => {
            if fields.is_empty() {
                return Err("numeric needs field list".to_string());
            }
            let mut ranges = Vec::new();
            for p in &params {
                let parts: Vec<&str> = p.split(':').collect();
                if parts.len() != 2 {
                    return Err(format!("Invalid numeric range: {}", p));
                }
                let min_val = parts[0].parse::<f64>().map_err(|_| format!("Invalid numeric range: {}", p))?;
                let max_val = parts[1].parse::<f64>().map_err(|_| format!("Invalid numeric range: {}", p))?;
                if min_val > max_val {
                    return Err(format!("Invalid numeric range: {}", p));
                }
                ranges.push((min_val, max_val));
            }
            Ok(Rule::Numeric { fields, ranges })
        }
        "fields" => {
            if params.is_empty() {
                return Err("Rule fields needs values".to_string());
            } else if params.len() > 1 {
                return Err("Rule fields needs min,max values only".to_string());
            }
            let parts: Vec<&str> = params[0].split(':').collect();
            if parts.len() != 2 {
                return Err("Rule fields needs min,max values".to_string());
            }
            let min_val = parts[0].parse::<usize>().map_err(|_| "Rule fields needs min,max values as integers".to_string())?;
            let max_val = parts[1].parse::<usize>().map_err(|_| "Rule fields needs min,max values as integers".to_string())?;
            if min_val < 1 || min_val > max_val {
                return Err("Rule fields has invalid min,max values".to_string());
            }
            Ok(Rule::Fields { min: min_val, max: max_val })
        }
        "values" | "notvalues" => {
            if fields.is_empty() {
                return Err(format!("{} needs field list", name));
            }
            if params.is_empty() {
                return Err(format!("Rule {} needs values", name));
            }
            Ok(Rule::Values {
                fields,
                not_values: name == "notvalues",
                list: params,
            })
        }
        "length" => {
            if fields.is_empty() {
                return Err("length needs field list".to_string());
            }
            if params.is_empty() {
                return Err("Rule length needs values".to_string());
            }
            let parts: Vec<&str> = params[0].split(':').collect();
            if parts.len() != 2 {
                return Err(format!("Need range in form n1:n2, not {}", params[0]));
            }
            let min_val = parts[0].parse::<i32>().map_err(|_| format!("Invalid number in range: {}", params[0]))?;
            let max_val = parts[1].parse::<i32>().map_err(|_| format!("Invalid number in range: {}", params[0]))?;
            if min_val < 0 || min_val > max_val {
                return Err(format!("Invalid range {} in rule length", params[0]));
            }
            Ok(Rule::Length { fields, min: min_val as usize, max: max_val as usize })
        }
        "lookup" => {
            if params.len() != 2 {
                return Err("lookup needs field list and filename".to_string());
            }
            let lookup_file = params[1].clone();
            let mut joins = Vec::new();
            for item in params[0].split(',') {
                let parts: Vec<&str> = item.split(':').collect();
                if parts.len() != 2 {
                    return Err(format!("Invalid field list entry: {}", item));
                }
                let f1 = parts[0].parse::<i32>().map_err(|_| format!("Invalid field list entry: {}", item))? - 1;
                let f2 = parts[1].parse::<i32>().map_err(|_| format!("Invalid field list entry: {}", item))? - 1;
                if f1 < 0 || f2 < 0 {
                    return Err(format!("Invalid field list entry: {}", item));
                }
                joins.push((f1 as usize, f2 as usize));
            }
            let join_vals = build_lookup_set(&lookup_file, &joins)?;
            Ok(Rule::Lookup { joins, lookup_file, join_vals })
        }
        "date" => {
            if fields.is_empty() {
                return Err("date needs field list".to_string());
            }
            if params.is_empty() {
                return Err("Rule date needs date mask".to_string());
            }
            let mask = params[0].clone();
            let mut range = None;
            if params.len() == 2 {
                let parts: Vec<&str> = params[1].split(':').collect();
                if parts.len() != 2 {
                    return Err(format!("Rule date has invalid range: {}", params[1]));
                }
                let min_d = parse_iso_date(parts[0]).ok_or_else(|| format!("Rule date has invalid date in range: {}", params[1]))?;
                let max_d = parse_iso_date(parts[1]).ok_or_else(|| format!("Rule date has invalid date in range: {}", params[1]))?;
                if min_d > max_d {
                    return Err(format!("Rule date has invalid range: {}", params[1]));
                }
                range = Some((min_d, max_d));
            } else if params.len() > 2 {
                return Err("Rule date needs mask and range parameters only".to_string());
            }
            let reader = MaskedDateReader::new(&mask, "", 1930)?;
            Ok(Rule::Date { fields, mask, range, reader })
        }
        _ => Err(format!("Unknown rule: {}", name)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidateOutMode {
    Reports,
    Passes,
    Fails,
}

pub fn run_validate(
    mut io: IOManager,
    vfile: String,
    omode: Option<String>,
    errcode: bool,
    skip_expr: Option<Expr>,
) -> Result<(), String> {
    let out_mode = match omode.as_deref().unwrap_or("report") {
        "report" => ValidateOutMode::Reports,
        "pass" => ValidateOutMode::Passes,
        "fail" => ValidateOutMode::Fails,
        _ => return Err(format!("Invalid value for output mode: {}", omode.unwrap())),
    };

    let spec_content = std::fs::read_to_string(&vfile)
        .map_err(|e| format!("Cannot open validation file {} for input: {}", vfile, e))?;

    let mut rules = Vec::new();
    for line in spec_content.lines() {
        if let Some((name, fields, params)) = parse_validation_line(line)? {
            let rule = compile_rule(&name, fields, params)?;
            rules.push(rule);
        }
    }

    let mut row = Vec::new();
    let mut err_total = 0;

    while io.read_csv(&mut row)? {
        let (skip, _) = should_skip_or_pass(
            &row,
            io.current_line(),
            io.current_file_name(),
            skip_expr.as_ref(),
            None,
        );
        if skip {
            continue;
        }

        let mut err_count = 0;
        for rule in &rules {
            let res = rule.apply(&row);
            if !res.is_empty() {
                if out_mode == ValidateOutMode::Reports {
                    if err_count == 0 {
                        let fname = io.current_file_name().to_string();
                        let line_no = io.current_line();
                        let input = io.current_input().to_string();
                        writeln!(io.output_writer, "{} ({}): {}", fname, line_no, input).map_err(|e| e.to_string())?;
                    }
                    for r in res {
                        if r.field > 0 {
                            writeln!(io.output_writer, "    field: {} - {}", r.field, r.msg).map_err(|e| e.to_string())?;
                        } else {
                            writeln!(io.output_writer, "    {}", r.msg).map_err(|e| e.to_string())?;
                        }
                    }
                    err_count += 1;
                    continue;
                }

                err_count += 1;
                if out_mode == ValidateOutMode::Fails {
                    io.write_row(&row)?;
                    break;
                }
            }
        }

        if out_mode == ValidateOutMode::Passes && err_count == 0 {
            io.write_row(&row)?;
        }
        err_total += err_count;
    }

    if err_total > 0 && errcode {
        std::process::exit(2);
    }
    Ok(())
}




