mod expr;
mod io;
mod commands;

use clap::{Parser, Subcommand, Args};
use commands::{CaseType, should_skip_or_pass};
use io::IOManager;
use std::collections::HashMap;

#[derive(Parser)]
#[command(
    name = "rvalues",
    about = "rvalues - CSV stream editor (Rust port of csvfix)",
    infer_subcommands = true
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Args, Debug, Clone)]
struct CommonArgs {
    #[arg(long = "sep", default_value = ",")]
    sep: String,

    #[arg(long = "rsep")]
    rsep: Option<String>,

    #[arg(long = "osep")]
    osep: Option<String>,

    #[arg(long = "ibl")]
    ibl: bool,

    #[arg(long = "ifn")]
    ifn: bool,

    #[arg(long = "smq")]
    smq: bool,

    #[arg(long = "sqf")]
    sqf: Option<String>,

    #[arg(long = "seed")]
    seed: Option<u64>,

    #[arg(long = "hdr")]
    hdr: Option<String>,

    #[arg(short = 'o', long = "out")]
    out: Option<String>,

    #[arg(long = "skip")]
    skip: Option<String>,

    #[arg(long = "pass")]
    pass: Option<String>,
}

#[derive(Subcommand)]
#[command(rename_all = "snake_case")]
enum Commands {
    Echo {
        #[command(flatten)]
        common: CommonArgs,
        files: Vec<String>,
    },
    Head {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'n', default_value = "10")]
        n: usize,
        files: Vec<String>,
    },
    Tail {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'n', default_value = "10")]
        n: usize,
        files: Vec<String>,
    },
    Upper {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: Option<String>,
        files: Vec<String>,
    },
    Lower {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: Option<String>,
        files: Vec<String>,
    },
    Mixed {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: Option<String>,
        files: Vec<String>,
    },
    Trim {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: Option<String>,
        #[arg(short = 'l')]
        left: bool,
        #[arg(short = 't')]
        right: bool,
        #[arg(short = 'w')]
        widths: Option<String>,
        files: Vec<String>,
    },
    Truncate {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'n')]
        count: usize,
        files: Vec<String>,
    },
    Pad {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'n')]
        count: Option<usize>,
        #[arg(short = 'p')]
        pad_vals: Option<String>,
        files: Vec<String>,
    },
    Exclude {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: Option<String>,
        #[arg(long = "rf")]
        rev_fields: Option<String>,
        #[arg(long = "if")]
        if_expr: Option<String>,
        files: Vec<String>,
    },
    Number {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: Option<String>,
        #[arg(long = "fmt", default_value = "EN")]
        fmt: String,
        #[arg(long = "es")]
        err_str: Option<String>,
        #[arg(long = "ec")]
        err_code: bool,
        files: Vec<String>,
    },
    Sequence {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'n', default_value = "1")]
        start: i32,
        #[arg(short = 'i', default_value = "1")]
        inc: i32,
        #[arg(short = 'd')]
        dec: Option<i32>,
        #[arg(short = 'p', default_value = "0")]
        pad: usize,
        #[arg(short = 'f', default_value = "1")]
        col: usize,
        #[arg(short = 'm')]
        mask: Option<String>,
        files: Vec<String>,
    },
    Unique {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: Option<String>,
        #[arg(short = 'd')]
        show_dupes: bool,
        files: Vec<String>,
    },
    Shuffle {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(long = "rs")]
        seed: Option<u64>,
        #[arg(short = 'f')]
        fields: Option<String>,
        #[arg(short = 'n')]
        count: Option<usize>,
        files: Vec<String>,
    },
    Sort {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: Option<String>,
        #[arg(long = "rh")]
        rh: bool,
        files: Vec<String>,
    },
    Escape {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: Option<String>,
        #[arg(short = 's')]
        chars_val: Option<String>,
        #[arg(short = 'e', default_value = "\\")]
        esc: String,
        #[arg(long = "sql")]
        sql: bool,
        #[arg(long = "noc")]
        escape_off: bool,
        files: Vec<String>,
    },
    Template {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(long = "tf")]
        tpl_file: String,
        #[arg(long = "fn")]
        fn_tpl: Option<String>,
        files: Vec<String>,
    },
    ToXml {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(long = "xf")]
        xml_spec: Option<String>,
        #[arg(long = "in", default_value = "4")]
        indent: String,
        #[arg(long = "et")]
        end_tags: bool,
        files: Vec<String>,
    },
    FromXml {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(long = "re")]
        re_paths: String,
        #[arg(long = "ex")]
        ex_paths: Option<String>,
        #[arg(long = "np")]
        no_parent: bool,
        #[arg(long = "na")]
        no_attrib: bool,
        #[arg(long = "nc")]
        no_child: bool,
        #[arg(long = "ip")]
        insert_path: bool,
        #[arg(long = "ml", default_value = " ")]
        ml_sep: String,
        files: Vec<String>,
    },
    ReadDsv {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: Option<String>,
        #[arg(short = 's', default_value = "|")]
        delim: String,
        #[arg(long = "csv")]
        csv: bool,
        #[arg(long = "cm")]
        cm: bool,
        files: Vec<String>,
    },
    WriteDsv {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: Option<String>,
        #[arg(short = 's', default_value = "|")]
        delim: String,
        files: Vec<String>,
    },
    AsciiTable {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'h')]
        header: Option<String>,
        #[arg(long = "ra")]
        right_align: Option<String>,
        #[arg(short = 's')]
        table_sep: bool,
        files: Vec<String>,
    },
    Block {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(long = "be")]
        begin_expr: String,
        #[arg(long = "ee")]
        end_expr: String,
        #[arg(short = 'r')]
        remove: bool,
        #[arg(short = 'k')]
        keep: bool,
        #[arg(short = 'm')]
        mark: Option<String>,
        #[arg(short = 'x')]
        exclusive: bool,
        files: Vec<String>,
    },
    Check {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(long = "nl")]
        nl: bool,
        #[arg(short = 'q')]
        quiet: bool,
        #[arg(short = 's', default_value = ",")]
        check_sep: String,
        #[arg(short = 'v')]
        verbose: bool,
        files: Vec<String>,
    },
    DateIso {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: Option<String>,
        #[arg(short = 'm')]
        mask: String,
        #[arg(long = "cy")]
        cy: Option<i32>,
        #[arg(long = "mn")]
        mnames: Option<String>,
        #[arg(long = "bdl")]
        bdl: bool,
        #[arg(long = "bdx")]
        bdx: bool,
        files: Vec<String>,
    },
    DateFormat {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: Option<String>,
        #[arg(long = "fmt")]
        fmt: String,
        files: Vec<String>,
    },
    Diff {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: Option<String>,
        #[arg(short = 'q')]
        quiet: bool,
        #[arg(long = "ic")]
        ic: bool,
        #[arg(long = "is")]
        is: bool,
        file1: String,
        file2: String,
    },
    Edit {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: Option<String>,
        #[arg(short = 'e', action = clap::ArgAction::Append)]
        edit_cmds: Vec<String>,
        files: Vec<String>,
    },
    Order {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: Option<String>,
        #[arg(long = "xf")]
        exclf: Option<String>,
        #[arg(long = "rf")]
        rev_fields: Option<String>,
        #[arg(long = "fn")]
        fnames: Option<String>,
        #[arg(long = "nc")]
        nocreat: bool,
        files: Vec<String>,
    },
    Join {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: String,
        #[arg(long = "oj")]
        oj: bool,
        #[arg(long = "inv")]
        inv: bool,
        #[arg(long = "ic")]
        ic: bool,
        #[arg(short = 'k')]
        keep: bool,
        files: Vec<String>,
    },
    Eval {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'd')]
        discard: bool,
        #[arg(short = 'e', action = clap::ArgAction::Append, allow_hyphen_values = true)]
        exprs: Vec<String>,
        #[arg(long = "if", action = clap::ArgAction::Append, allow_hyphen_values = true)]
        if_exprs: Vec<String>,
        #[arg(short = 'r', action = clap::ArgAction::Append, allow_hyphen_values = true)]
        r_exprs: Vec<String>,
        files: Vec<String>,
    },
    Exec {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'c')]
        cmd: String,
        #[arg(short = 'r')]
        replace: bool,
        files: Vec<String>,
    },
    FileInfo {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'b')]
        basename: bool,
        #[arg(long = "tc")]
        two_cols: bool,
        files: Vec<String>,
    },
    #[command(name = "fmerge", alias = "file_merge")]
    FileMerge {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: Option<String>,
        files: Vec<String>,
    },
    Find {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: Option<String>,
        #[arg(short = 'e', action = clap::ArgAction::Append)]
        exprs: Vec<String>,
        #[arg(short = 's', action = clap::ArgAction::Append)]
        strings: Vec<String>,
        #[arg(long = "ei", action = clap::ArgAction::Append)]
        exprs_ic: Vec<String>,
        #[arg(long = "si", action = clap::ArgAction::Append)]
        strings_ic: Vec<String>,
        #[arg(short = 'r', action = clap::ArgAction::Append)]
        ranges: Vec<String>,
        #[arg(short = 'l', action = clap::ArgAction::Append)]
        lengths: Vec<String>,
        #[arg(long = "fc")]
        fcount: Option<String>,
        #[arg(long = "if")]
        if_expr: Option<String>,
        #[arg(short = 'n')]
        count_only: bool,
        files: Vec<String>,
    },
    Remove {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: Option<String>,
        #[arg(short = 'e', action = clap::ArgAction::Append)]
        exprs: Vec<String>,
        #[arg(short = 's', action = clap::ArgAction::Append)]
        strings: Vec<String>,
        #[arg(long = "ei", action = clap::ArgAction::Append)]
        exprs_ic: Vec<String>,
        #[arg(long = "si", action = clap::ArgAction::Append)]
        strings_ic: Vec<String>,
        #[arg(short = 'r', action = clap::ArgAction::Append)]
        ranges: Vec<String>,
        #[arg(short = 'l', action = clap::ArgAction::Append)]
        lengths: Vec<String>,
        #[arg(long = "fc")]
        fcount: Option<String>,
        #[arg(long = "if")]
        if_expr: Option<String>,
        #[arg(short = 'n')]
        count_only: bool,
        files: Vec<String>,
    },
    Flatten {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'k')]
        key: Option<String>,
        #[arg(short = 'r')]
        remove: bool,
        #[arg(short = 'f')]
        fields: Option<String>,
        #[arg(long = "me")]
        master_expr: Option<String>,
        files: Vec<String>,
    },
    Unflatten {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'k')]
        key: Option<String>,
        #[arg(short = 'n')]
        num_data_fields: Option<usize>,
        files: Vec<String>,
    },
    Inter {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: Option<String>,
        files: Vec<String>,
    },
    Map {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: Option<String>,
        #[arg(long = "fv")]
        from_val_str: String,
        #[arg(long = "tv")]
        to_val_str: String,
        #[arg(long = "ic")]
        ignore_case: bool,
        files: Vec<String>,
    },
    Merge {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: Option<String>,
        #[arg(short = 's')]
        sub_sep: Option<String>,
        #[arg(short = 'p')]
        pos: Option<String>,
        #[arg(short = 'k')]
        keep: bool,
        files: Vec<String>,
    },
    Money {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: Option<String>,
        #[arg(long = "dp")]
        dp: Option<String>,
        #[arg(long = "ts")]
        ts: Option<String>,
        #[arg(long = "cs")]
        symbol: Option<String>,
        #[arg(long = "ms")]
        minus: Option<String>,
        #[arg(long = "ps")]
        plus: Option<String>,
        #[arg(long = "cn")]
        cents: bool,
        #[arg(short = 'r')]
        replace: bool,
        #[arg(short = 'w')]
        width: Option<String>,
        files: Vec<String>,
    },
    Printf {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(long = "fmt")]
        fmt: String,
        #[arg(short = 'f')]
        fields: Option<String>,
        #[arg(short = 'q')]
        csv_quote: bool,
        files: Vec<String>,
    },
    Put {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'p')]
        pos: Option<String>,
        #[arg(short = 'v')]
        val: Option<String>,
        #[arg(short = 'e')]
        env: Option<String>,
        files: Vec<String>,
    },
    ReadFixed {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: String,
        #[arg(short = 'k')]
        keep: bool,
        files: Vec<String>,
    },
    WriteFixed {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        fields: String,
        #[arg(long = "ruler")]
        ruler: bool,
        files: Vec<String>,
    },
    ReadMulti {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'n')]
        num_lines: Option<usize>,
        #[arg(short = 's')]
        sub_sep: Option<String>,
        files: Vec<String>,
    },
    WriteMulti {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'm')]
        master_fields: String,
        #[arg(short = 'd')]
        detail_fields: Option<String>,
        #[arg(long = "rs")]
        rec_sep: Option<String>,
        files: Vec<String>,
    },
    Rmnew {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 's')]
        sub_sep: Option<String>,
        #[arg(short = 'x')]
        exclude_after: bool,
        #[arg(short = 'f')]
        fields: Option<String>,
        files: Vec<String>,
    },
    SplitFixed {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        field: String,
        #[arg(short = 'p')]
        pos_list: String,
        #[arg(short = 'k')]
        keep: bool,
        files: Vec<String>,
    },
    SplitChar {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 'f')]
        field: String,
        #[arg(short = 'c')]
        char_val: Option<String>,
        #[arg(long = "tan")]
        tan: bool,
        #[arg(long = "tna")]
        tna: bool,
        #[arg(short = 'k')]
        keep: bool,
        files: Vec<String>,
    },
    SqlInsert {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 't')]
        table: String,
        #[arg(short = 'f')]
        fields: String,
        #[arg(short = 's')]
        sub_sep: Option<String>,
        #[arg(long = "nq")]
        no_quote: Option<String>,
        #[arg(long = "qn")]
        quote_nulls: bool,
        #[arg(long = "en")]
        empty_nulls: bool,
        files: Vec<String>,
    },
    SqlUpdate {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 't')]
        table: String,
        #[arg(short = 'f')]
        fields: String,
        #[arg(short = 'w')]
        where_fields: String,
        #[arg(short = 's')]
        sub_sep: Option<String>,
        #[arg(long = "nq")]
        no_quote: Option<String>,
        #[arg(long = "qn")]
        quote_nulls: bool,
        #[arg(long = "en")]
        empty_nulls: bool,
        files: Vec<String>,
    },
    SqlDelete {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(short = 't')]
        table: String,
        #[arg(short = 'w')]
        where_fields: String,
        #[arg(short = 's')]
        sub_sep: Option<String>,
        #[arg(long = "nq")]
        no_quote: Option<String>,
        #[arg(long = "qn")]
        quote_nulls: bool,
        #[arg(long = "en")]
        empty_nulls: bool,
        files: Vec<String>,
    },
    Stat {
        #[command(flatten)]
        common: CommonArgs,
        files: Vec<String>,
    },
    Summary {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(long = "avg")]
        avg: Option<String>,
        #[arg(long = "frq")]
        freq: Option<String>,
        #[arg(long = "max")]
        max: Option<String>,
        #[arg(long = "min")]
        min: Option<String>,
        #[arg(long = "med")]
        median: Option<String>,
        #[arg(long = "mod")]
        mode: Option<String>,
        #[arg(long = "sum")]
        sum: Option<String>,
        #[arg(long = "siz")]
        size: bool,
        files: Vec<String>,
    },
    Validate {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(long = "vf")]
        vfile: String,
        #[arg(long = "om")]
        omode: Option<String>,
        #[arg(long = "errcode")]
        errcode: bool,
        files: Vec<String>,
    },
}

fn make_iomanager(common: &CommonArgs, files: Vec<String>) -> Result<IOManager, String> {
    let quote_fields = if let Some(ref s) = common.sqf {
        if s == "0" || s == "none" {
            Some(vec![999999])
        } else {
            Some(commands::parse_indices(s)?)
        }
    } else {
        None
    };

    let input_sep_char = if let Some(ref rs) = common.rsep {
        rs.chars().next().unwrap_or(',')
    } else if common.sep.is_empty() {
        ','
    } else {
        common.sep.chars().next().unwrap_or(',')
    };

    let osep = common.osep.as_ref().and_then(|s| {
        if s == "\\t" {
            Some('\t')
        } else if s.chars().count() == 1 {
            s.chars().next()
        } else {
            None
        }
    });

    IOManager::new(
        files,
        common.out.as_deref(),
        common.ibl,
        common.ifn,
        input_sep_char,
        osep,
        common.rsep.is_some(),
        common.smq,
        quote_fields,
        common.hdr.clone(),
    )
}

fn get_expr(expr_str: &Option<String>) -> Option<expr::Expr> {
    expr_str.as_ref().and_then(|s| {
        match expr::parse(s) {
            Ok(v) => v.last().cloned(),
            Err(e) => {
                eprintln!("EXPR PARSE ERROR FOR '{}': {}", s, e);
                None
            }
        }
    })
}

fn run_app() -> Result<(), String> {
    let mut args: Vec<String> = std::env::args().collect();
    for arg in args.iter_mut().skip(1) {
        if arg.starts_with('-') && !arg.starts_with("--") && arg.len() > 2 {
            if let Some(first_char) = arg.chars().nth(1) {
                if first_char.is_alphabetic() || first_char == '_' {
                    *arg = format!("-{}", arg);
                }
            }
        }
    }
    let cli = <Cli as clap::Parser>::try_parse_from(args).map_err(|e| e.to_string())?;
    
    match cli.command {
        Commands::Echo { common, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_echo(io, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Head { common, n, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_head(io, n, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Tail { common, n, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_tail(io, n, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Upper { common, fields, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_case(io, fields, CaseType::Upper, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Lower { common, fields, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_case(io, fields, CaseType::Lower, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Mixed { common, fields, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_case(io, fields, CaseType::Mixed, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Trim { common, fields, left, right, widths, files } => {
            let io = make_iomanager(&common, files)?;
            let l = if !left && !right { true } else { left };
            let r = if !left && !right { true } else { right };
            commands::run_trim(io, fields, l, r, widths, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Truncate { common, count, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_truncate(io, count, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Pad { common, count, pad_vals, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_pad(io, count, pad_vals, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Exclude { common, fields, rev_fields, if_expr, files } => {
            let reverse = rev_fields.is_some();
            let actual_fields = fields.or(rev_fields).ok_or("Must specify either -f or -rf fields")?;
            let io = make_iomanager(&common, files)?;
            commands::run_exclude(io, actual_fields, reverse, get_expr(&if_expr), get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Number { common, fields, fmt, err_str, err_code, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_number(io, fields, fmt, err_str, err_code, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Sequence { common, start, inc, dec, pad, col, mask, files } => {
            let actual_inc = if let Some(d) = dec { -d } else { inc };
            let io = make_iomanager(&common, files)?;
            commands::run_sequence(io, start, actual_inc, pad, if col > 0 { col - 1 } else { 0 }, mask, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Unique { common, fields, show_dupes, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_unique(io, fields, show_dupes, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Shuffle { common, seed, fields, count, files } => {
            let actual_seed = seed.or(common.seed);
            let io = make_iomanager(&common, files)?;
            commands::run_shuffle(io, actual_seed, fields, count, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Sort { common, fields, rh, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_sort(io, fields, rh, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Escape { common, fields, chars_val, esc, sql, escape_off, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_escape(io, fields, chars_val, esc, sql, escape_off, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Template { common, tpl_file, fn_tpl, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_template(io, tpl_file, fn_tpl, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::ToXml { common, xml_spec, indent, end_tags, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_to_xml(io, xml_spec, indent, end_tags, get_expr(&common.skip))
        }
        Commands::FromXml { common, re_paths, ex_paths, no_parent, no_attrib, no_child, insert_path, ml_sep, files } => {
            let io = make_iomanager(&common, files.clone())?;
            commands::run_from_xml(io, files, re_paths, ex_paths, no_parent, no_attrib, no_child, insert_path, ml_sep)
        }
        Commands::ReadDsv { common, fields, delim, csv, cm, files } => {
            let delim_char = if delim == "\\t" { '\t' } else { delim.chars().next().unwrap_or('|') };
            let io = make_iomanager(&common, files)?;
            commands::run_read_dsv(io, fields, delim_char, csv, cm, get_expr(&common.skip))
        }
        Commands::WriteDsv { common, fields, delim, files } => {
            let delim_char = if delim == "\\t" { '\t' } else { delim.chars().next().unwrap_or('|') };
            let io = make_iomanager(&common, files)?;
            let skip_expr = get_expr(&common.skip);
            let cols = fields.map(|s| commands::parse_indices(&s)).transpose()?;
            // Run write_dsv directly
            let mut mut_io = io;
            let mut row = Vec::new();
            while mut_io.read_csv(&mut row)? {
                let (skip, _) = should_skip_or_pass(&row, mut_io.current_line(), mut_io.current_file_name(), skip_expr.as_ref(), None);
                if skip {
                    continue;
                }
                
                let mut out_row = Vec::new();
                if let Some(ref cs) = cols {
                    for &c in cs {
                        if c < row.len() {
                            out_row.push(row[c].clone());
                        } else {
                            out_row.push(String::new());
                        }
                    }
                } else {
                    out_row = row.clone();
                }

                let mut line = String::new();
                for (i, val) in out_row.iter().enumerate() {
                    let field = if !val.contains(delim_char) && !val.contains('\\') {
                        val.clone()
                    } else {
                        let mut t = String::new();
                        for c in val.chars() {
                            if c == delim_char || c == '\\' {
                                t.push('\\');
                            }
                            t.push(c);
                        }
                        t
                    };
                    line.push_str(&field);
                    if i != out_row.len() - 1 {
                        line.push(delim_char);
                    }
                }
                writeln!(mut_io.output_writer, "{}", line).map_err(|e| e.to_string())?;
            }
            Ok(())
        }
        Commands::AsciiTable { common, header, right_align, table_sep, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_ascii_table(io, header, right_align, table_sep, get_expr(&common.skip))
        }
        Commands::Block { common, begin_expr, end_expr, remove, keep, mark, exclusive, files } => {
            let io = make_iomanager(&common, files)?;
            let action = if keep {
                commands::BlockAction::Keep
            } else if remove {
                commands::BlockAction::Remove
            } else if let Some(m) = mark {
                let parts: Vec<&str> = m.split(',').collect();
                let b_mark = parts.first().copied().unwrap_or("").to_string();
                let n_mark = parts.get(1).copied().unwrap_or("").to_string();
                commands::BlockAction::Mark(b_mark, n_mark)
            } else {
                return Err("Must specify one of -k, -r, or -m".to_string());
            };
            let be = get_expr(&Some(begin_expr)).ok_or("Invalid begin expression")?;
            let ee = get_expr(&Some(end_expr)).ok_or("Invalid end expression")?;
            commands::run_block(io, be, ee, action, exclusive)
        }
        Commands::Check { common, nl, quiet, check_sep, verbose, files } => {
            let io = make_iomanager(&common, files.clone())?;
            let sep_char = check_sep.chars().next().unwrap_or(',');
            commands::run_check(io, quiet, verbose, nl, sep_char, files)
        }
        Commands::DateIso { common, fields, mask, cy, mnames, bdl, bdx, files } => {
            let io = make_iomanager(&common, files)?;
            let base_year = cy.unwrap_or(1930);
            let month_names = mnames.unwrap_or_default();
            let action = if bdl {
                commands::DateWriteAction::WriteBad
            } else if bdx {
                commands::DateWriteAction::WriteGood
            } else {
                commands::DateWriteAction::WriteAll
            };
            commands::run_date_iso(io, fields, mask, base_year, month_names, action, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::DateFormat { common, fields, fmt, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_date_format(io, fields, fmt, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Diff { common, fields, quiet, ic, is, file1, file2 } => {
            let io = make_iomanager(&common, Vec::new())?;
            commands::run_diff(io, file1, file2, fields, quiet, ic, is)
        }
        Commands::Edit { common, fields, edit_cmds, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_edit(io, fields, edit_cmds, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Order { common, fields, exclf, rev_fields, fnames, nocreat, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_order(io, fields, exclf, rev_fields, fnames, nocreat, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Join { common, fields, oj, inv, ic, keep, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_join(io, fields, oj, inv, ic, keep)
        }
        Commands::Eval { common, discard, files, .. } => {
            let io = make_iomanager(&common, files)?;
            commands::run_eval(io, discard, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Exec { common, cmd, replace, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_exec(io, cmd, replace, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::FileInfo { common, basename, two_cols, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_file_info(io, basename, two_cols, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::FileMerge { common, fields, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_file_merge(io, fields)
        }
        Commands::Find { common, fields, exprs, strings, exprs_ic, strings_ic, ranges, lengths, fcount, if_expr, count_only, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_find_remove(io, false, fields, exprs, strings, exprs_ic, strings_ic, ranges, lengths, fcount, if_expr, count_only)
        }
        Commands::Remove { common, fields, exprs, strings, exprs_ic, strings_ic, ranges, lengths, fcount, if_expr, count_only, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_find_remove(io, true, fields, exprs, strings, exprs_ic, strings_ic, ranges, lengths, fcount, if_expr, count_only)
        }
        Commands::Flatten { common, key, remove, fields, master_expr, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_flatten(io, master_expr, key, fields, remove, get_expr(&common.skip))
        }
        Commands::Unflatten { common, key, num_data_fields, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_unflatten(io, key, num_data_fields, get_expr(&common.skip))
        }
        Commands::Inter { common, fields, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_inter(io, fields)
        }
        Commands::Map { common, fields, from_val_str, to_val_str, ignore_case, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_map(io, fields, from_val_str, to_val_str, ignore_case, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Merge { common, fields, sub_sep, pos, keep, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_merge(io, fields, sub_sep, pos, keep, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Money { common, fields, dp, ts, symbol, plus, minus, cents, replace, width, files } => {
            let io = make_iomanager(&common, files)?;
            let sym = symbol.unwrap_or_default();
            let pl = plus.unwrap_or_default();
            let mn = minus.unwrap_or_else(|| "-".to_string());
            commands::run_money(io, fields, dp, ts, sym, pl, mn, cents, replace, width, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Printf { common, fmt, fields, csv_quote, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_printf(io, fmt, fields, csv_quote, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::Put { common, pos, val, env, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_put(io, pos, val, env)
        }
        Commands::ReadFixed { common, fields, keep, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_read_fixed(io, fields, keep)
        }
        Commands::WriteFixed { common, fields, ruler, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_write_fixed(io, fields, ruler, get_expr(&common.skip))
        }
        Commands::ReadMulti { common, num_lines, sub_sep, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_read_multi(io, num_lines, sub_sep)
        }
        Commands::WriteMulti { common, master_fields, detail_fields, rec_sep, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_write_multi(io, master_fields, detail_fields, rec_sep, get_expr(&common.skip))
        }
        Commands::Rmnew { common, sub_sep, exclude_after, fields, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_rmnew(io, fields, sub_sep, exclude_after, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::SplitFixed { common, field, pos_list, keep, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_split_fixed(io, field, pos_list, keep, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::SplitChar { common, field, char_val, tan, tna, keep, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_split_char(io, field, char_val, tan, tna, keep, get_expr(&common.skip), get_expr(&common.pass))
        }
        Commands::SqlInsert { common, table, fields, sub_sep, no_quote, quote_nulls, empty_nulls, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_sql_insert(io, table, fields, sub_sep, no_quote, quote_nulls, empty_nulls, get_expr(&common.skip))
        }
        Commands::SqlUpdate { common, table, fields, where_fields, sub_sep, no_quote, quote_nulls, empty_nulls, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_sql_update(io, table, fields, where_fields, sub_sep, no_quote, quote_nulls, empty_nulls, get_expr(&common.skip))
        }
        Commands::SqlDelete { common, table, where_fields, sub_sep, no_quote, quote_nulls, empty_nulls, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_sql_delete(io, table, where_fields, sub_sep, no_quote, quote_nulls, empty_nulls, get_expr(&common.skip))
        }
        Commands::Stat { common, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_stat(io)
        }
        Commands::Summary { common, avg, freq, max, min, median, mode, sum, size, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_summary(io, avg, min, max, freq, median, mode, sum, size)
        }
        Commands::Validate { common, vfile, omode, errcode, files } => {
            let io = make_iomanager(&common, files)?;
            commands::run_validate(io, vfile, omode, errcode, get_expr(&common.skip))
        }
    }
}

fn main() {
    if let Err(e) = run_app() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
