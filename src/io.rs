use std::fs::File;
use std::io::{self, BufRead, BufReader, Write, Read};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParserState {
    OutVal,
    InVal,
    InQVal,
    HaveQ,
}

pub struct IOManager {
    pub input_paths: Vec<String>,
    current_input_idx: usize,
    current_line: usize,
    current_filename: String,
    current_reader: Option<Box<dyn BufRead>>,
    
    pub output_writer: Box<dyn Write>,
    current_input: String,
    
    // Config options
    pub ignore_blank_lines: bool,
    pub skip_col_names: bool,
    pub input_sep: char,
    pub output_sep: char,
    pub smart_quotes: bool,
    pub quote_fields: Option<Vec<usize>>, // None = quote all, Some = quote only these
    pub header: Option<String>,
}

impl IOManager {
    pub fn new(
        input_paths: Vec<String>,
        output_path: Option<&str>,
        ignore_blank_lines: bool,
        skip_col_names: bool,
        input_sep: char,
        output_sep: Option<char>,
        retain_sep: bool,
        smart_quotes: bool,
        quote_fields: Option<Vec<usize>>,
        header: Option<String>,
    ) -> Result<Self, String> {
        let input_paths = if input_paths.is_empty() {
            vec!["-".to_string()]
        } else {
            input_paths
        };

        let output_writer: Box<dyn Write> = match output_path {
            Some(path) if path != "" => {
                let file = File::create(path)
                    .map_err(|e| format!("Could not open {} for output: {}", path, e))?;
                Box::new(file)
            }
            _ => Box::new(io::stdout()),
        };

        let out_sep = if retain_sep {
            input_sep
        } else {
            output_sep.unwrap_or(',')
        };

        let mut iom = Self {
            input_paths,
            current_input_idx: 0,
            current_line: 0,
            current_filename: String::new(),
            current_reader: None,
            output_writer,
            current_input: String::new(),
            ignore_blank_lines,
            skip_col_names,
            input_sep,
            output_sep: out_sep,
            smart_quotes,
            quote_fields,
            header,
        };

        if let Some(h) = &iom.header {
            writeln!(iom.output_writer, "{}", h)
                .map_err(|e| format!("Failed to write header: {}", e))?;
        }

        iom.open_next_stream()?;
        Ok(iom)
    }

    pub fn reset_inputs(&mut self, paths: Vec<String>) -> Result<(), String> {
        self.input_paths = paths;
        self.current_input_idx = 0;
        self.current_line = 0;
        self.open_next_stream()?;
        Ok(())
    }

    fn open_next_stream(&mut self) -> Result<bool, String> {
        if self.current_input_idx >= self.input_paths.len() {
            self.current_reader = None;
            return Ok(false);
        }

        let path = &self.input_paths[self.current_input_idx];
        self.current_filename = path.clone();
        self.current_line = 0;

        let reader: Box<dyn BufRead> = if path == "-" {
            Box::new(BufReader::new(io::stdin()))
        } else {
            let file = File::open(path)
                .map_err(|e| format!("Cannot open {} for input: {}", path, e))?;
            Box::new(BufReader::new(file))
        };

        self.current_reader = Some(reader);
        Ok(true)
    }

    pub fn current_file_name(&self) -> &str {
        &self.current_filename
    }

    pub fn current_line(&self) -> usize {
        self.current_line
    }

    pub fn current_input(&self) -> &str {
        &self.current_input
    }

    pub fn read_line(&mut self, line: &mut String) -> Result<bool, String> {
        line.clear();
        while self.current_reader.is_some() {
            let bytes_read = {
                let reader = self.current_reader.as_mut().unwrap();
                reader.read_line(line)
                    .map_err(|e| format!("Error reading line from {}: {}", self.current_filename, e))?
            };

            if bytes_read == 0 {
                self.current_input_idx += 1;
                self.open_next_stream()?;
                continue;
            }

            self.current_line += 1;
            
            let len = line.trim_end_matches(&['\r', '\n'][..]).len();
            line.truncate(len);
            
            if self.ignore_blank_lines && line.trim().is_empty() {
                line.clear();
                continue;
            }

            if self.skip_col_names && self.current_line == 1 {
                line.clear();
                continue;
            }

            self.current_input = line.clone();
            return Ok(true);
        }
        Ok(false)
    }

    fn read_record_raw(&mut self) -> Result<Option<String>, String> {
        while self.current_reader.is_some() {
            let mut line = String::new();
            let mut state = ParserState::OutVal;
            let mut eof = true;
            self.current_line += 1;

            loop {
                let mut buf = [0u8; 1];
                let bytes_read = {
                    let reader = self.current_reader.as_mut().unwrap();
                    reader.read(&mut buf)
                        .map_err(|e| format!("Error reading from {}: {}", self.current_filename, e))?
                };

                if bytes_read == 0 {
                    break;
                }
                eof = false;
                let c = buf[0] as char;

                if c == '\r' {
                    continue;
                }

                let end_of_line = match state {
                    ParserState::OutVal => {
                        if c == self.input_sep {
                            state = ParserState::OutVal;
                            false
                        } else if c == '"' {
                            state = ParserState::InQVal;
                            false
                        } else if c == '\n' {
                            true
                        } else {
                            state = ParserState::InVal;
                            false
                        }
                    }
                    ParserState::InVal => {
                        if c == self.input_sep {
                            state = ParserState::OutVal;
                            false
                        } else if c == '\n' {
                            true
                        } else {
                            state = ParserState::InVal;
                            false
                        }
                    }
                    ParserState::InQVal => {
                        if c == '"' {
                            state = ParserState::HaveQ;
                            false
                        } else {
                            if c == '\n' {
                                self.current_line += 1;
                            }
                            state = ParserState::InQVal;
                            false
                        }
                    }
                    ParserState::HaveQ => {
                        if c == '"' {
                            state = ParserState::InQVal;
                            false
                        } else if c == '\n' {
                            true
                        } else if c == self.input_sep {
                            state = ParserState::OutVal;
                            false
                        } else {
                            state = ParserState::OutVal;
                            false
                        }
                    }
                };

                if end_of_line {
                    break;
                }

                line.push(c);
            }

            if eof {
                self.current_input_idx += 1;
                self.open_next_stream()?;
                continue;
            }

            return Ok(Some(line));
        }
        Ok(None)
    }

    pub fn read_csv(&mut self, row: &mut Vec<String>) -> Result<bool, String> {
        row.clear();
        
        while let Some(line) = self.read_record_raw()? {
            if self.ignore_blank_lines && line.trim().is_empty() {
                continue;
            }

            if self.skip_col_names && self.current_line == 1 {
                continue;
            }

            *row = self.parse_csv_line(&line)?;
            self.current_input = line;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn write_row(&mut self, row: &[String]) -> Result<(), String> {
        let special_chars = if self.output_sep == '\t' {
            vec!['"', '\t']
        } else {
            vec!['"', self.output_sep]
        };

        for (i, field) in row.iter().enumerate() {
            let should_quote = if self.smart_quotes {
                field.chars().any(|c| special_chars.contains(&c))
            } else if let Some(q_fields) = &self.quote_fields {
                q_fields.contains(&i)
            } else {
                true // default quote all
            };

            if should_quote {
                let mut quoted = String::new();
                quoted.push('"');
                for c in field.chars() {
                    if c == '"' {
                        quoted.push('"');
                    }
                    quoted.push(c);
                }
                quoted.push('"');
                write!(self.output_writer, "{}", quoted)
                    .map_err(|e| format!("Error writing output: {}", e))?;
            } else {
                write!(self.output_writer, "{}", field)
                    .map_err(|e| format!("Error writing output: {}", e))?;
            }

            if i != row.len() - 1 {
                write!(self.output_writer, "{}", self.output_sep)
                    .map_err(|e| format!("Error writing separator: {}", e))?;
            }
        }
        writeln!(self.output_writer)
            .map_err(|e| format!("Error writing newline: {}", e))?;
        Ok(())
    }

    pub fn parse_csv_line(&self, line: &str) -> Result<Vec<String>, String> {
        let mut fields = Vec::new();
        let mut chars = line.chars().peekable();
        
        while chars.peek().is_some() {
            if chars.peek() == Some(&'"') {
                chars.next(); // Consume opening quote
                let mut field = String::new();
                loop {
                    match chars.next() {
                        Some('"') => {
                            if chars.peek() == Some(&'"') {
                                field.push('"');
                                chars.next(); // Consume escaped quote
                            } else {
                                // Closing quote
                                if chars.peek() == Some(&self.input_sep) {
                                    chars.next(); // Consume delimiter
                                }
                                break;
                            }
                        }
                        Some(c) => field.push(c),
                        None => {
                            // Missing closing quote, but we'll accept it as EOF
                            break;
                        }
                    }
                }
                fields.push(field);
            } else {
                let mut field = String::new();
                while let Some(&c) = chars.peek() {
                    if c == self.input_sep {
                        chars.next();
                        break;
                    } else {
                        field.push(c);
                        chars.next();
                    }
                }
                fields.push(field);
            }
        }
        
        // If line ends with a delimiter, we must append an empty field
        if line.ends_with(self.input_sep) {
            fields.push(String::new());
        }

        Ok(fields)
    }
}
