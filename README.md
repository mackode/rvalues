# rvalues

`rvalues` is a powerful, stream-oriented command-line tool for editing, filtering, and converting CSV files. It is a complete Rust port of the popular `csvfix` utility, designed for speed, safety, and modern command-line integration.

## Features

- **Stream-Oriented**: Processes data efficiently line-by-line, allowing you to manipulate massive CSV files without consuming excessive memory.
- **Rich Command Set**: Includes a vast array of subcommands for formatting, validating, filtering, sorting, merging, and splitting CSV streams.
- **Flexible Formats**: Reads and writes standard CSV, DSV (delimiter-separated values), fixed-width, multi-line, XML, and SQL insert/update/delete statements.
- **Cross-Platform**: Compiles and runs seamlessly on Linux, macOS, and Windows.

---

## Installation

### From Source

Ensure you have Rust and Cargo installed (edition 2024 or later).

```bash
# Clone the repository
git clone https://github.com/mackode/rvalues.git
cd rvalues/rvalues

# Build the release binary
cargo build --release

# The compiled binary will be available at target/release/rvalues
```

---

## Subcommands Summary

`rvalues` provides subcommands grouped into several core functional areas:

### 1. Basic Manipulation & Inspection
- `echo` - Echo fields and rows from inputs.
- `head` / `tail` - Get the first or last $N$ lines of CSV streams.
- `file_info` - Display basic information about files (e.g., row count).
- `check` - Check CSV files for correctness and field alignment.
- `validate` - Validate files against specific format constraints.
- `stat` / `summary` - Generate summary statistics for fields.

### 2. Formatting & Transformation
- `upper` / `lower` / `mixed` - Change casing of specific fields.
- `trim` / `truncate` / `pad` - Adjust field lengths, whitespaces, and padding.
- `escape` - Escape specific characters in fields.
- `money` - Format fields as currency representation.
- `printf` - Format outputs using custom printf-style formats.
- `put` - Insert new fields or environment variables.
- `edit` - Edit CSV data using basic stream commands.

### 3. Date & Time
- `date_iso` - Convert dates to ISO format.
- `date_format` - Format dates using custom patterns.

### 4. Advanced Filtering & Ordering
- `exclude` / `remove` - Discard specific fields or lines matching filters.
- `unique` - Filter duplicate rows or values.
- `shuffle` - Randomly shuffle rows or fields.
- `sort` - Sort rows based on specific field keys.
- `find` - Search for specific strings/expressions within columns.
- `block` - Extract blocks of data bounded by expressions.

### 5. Structured Data Conversions
- `to_xml` / `from_xml` - Convert CSV streams to XML or extract CSV from XML.
- `read_dsv` / `write_dsv` - Convert between CSV and custom delimiter-separated formats.
- `read_fixed` / `write_fixed` - Convert between CSV and fixed-width files.
- `read_multi` / `write_multi` - Read/write records spread across multiple lines.
- `sql_insert` / `sql_update` / `sql_delete` - Generate SQL queries from CSV data.
- `ascii_table` - Output CSV data as clean, reader-friendly ASCII tables.

### 6. Merging & Splitting
- `join` / `merge` / `fmerge` - Merge or join multiple CSV inputs.
- `split_fixed` / `split_char` - Split fields by position or delimiter.

---

## Usage Examples

### Reordering Columns
Keep and reorder fields (e.g. print columns 2 and 1 of a file):
```bash
rvalues order -f 2,1 input.csv
```

### Filtering Rows
Find all rows where the first column matches "active":
```bash
rvalues find -f 1 -s "active" input.csv
```

### Converting to XML
Format CSV data into structured XML:
```bash
rvalues to_xml --et input.csv
```

### Generating SQL Inserts
Turn CSV rows into SQL statements for a database:
```bash
rvalues sql_insert -t users -f id,name,email input.csv
```

---

## Releases & Versioning

This project uses GitHub Actions to build and release multi-platform binaries. 

To create a new release:
1. Update the version in `Cargo.toml`.
2. Tag your commit with `v*` (e.g. `v0.1.0`):
   ```bash
   git tag -a v0.1.0 -m "Release version 0.1.0"
   git push origin v0.1.0
   ```
3. The Release workflow will compile the code for Linux, macOS, and Windows, then automatically publish a new GitHub Release with the build archives attached.
