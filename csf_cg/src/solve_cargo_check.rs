// solve cargo check messages of merged output file
/*
use cargo_metadata::{
    diagnostic::{DiagnosticCode, DiagnosticLevel, DiagnosticSpan},
    Message,
};
use std::collections::BTreeMap;
use std::fs;
use std::process::Command;
use std::process::Output;

use super::*;
use crate::configuration::*;

struct CargoCheckItem {
    message: String,
    code: Option<DiagnosticCode>,
    level: DiagnosticLevel,
    span: DiagnosticSpan,
}

enum PatchAction {
    SnipNameSpace(usize),
    AdjustUnusedVariableName(usize, usize),
    SnipNeverConstructedEnumVariant(usize),
}

enum NameSpaceResult {
    Finished,
    FindEndLine,
    FindEndLineMatchArm,
    FindNextSemicolon,
}

struct NameSpace<'a> {
    starts_with: String,
    start_line: usize,
    end_line: usize,
    lines: Vec<&'a str>,
    line_end_chars: String,
    open_bracket_typ: char,
    closing_bracket_typ: char,
}

impl<'a> NameSpace<'a> {
    const NAME_SPACE_PATTERNS: [&'a str; 9] = [
        "fn", "struct", "impl", "use", "mod", "#[", "const", "enum", "type",
    ];
    const POSSIBLE_SINGLE_LINE_PATTERNS: [&'a str; 7] =
        ["fn", "struct", "impl", "use", "mod", "#[", "const"];
    const MUST_END_ON_SEMICOLON: [&'a str; 1] = ["type"];
    fn new(output: &'a str, line_end_chars: String) -> NameSpace {
        NameSpace {
            starts_with: "".into(),
            start_line: 0,
            end_line: output.lines().count() - 1,
            lines: output.lines().collect(),
            line_end_chars,
            open_bracket_typ: '{',
            closing_bracket_typ: '}',
        }
    }
    fn find_start_line(
        &mut self,
        message_line: usize,
        never_constructed_variants: &Vec<String>,
    ) -> BoxResult<NameSpaceResult> {
        // searching known line_start pattern going up line for line starting at message_line (range [0..=message_line])
        for (line, slice) in self.lines[0..message_line].iter().enumerate().rev() {
            for pat in never_constructed_variants
                .iter()
                .map(|ncv| ncv.as_str())
                .chain(NameSpace::NAME_SPACE_PATTERNS.into_iter())
            {
                if slice.trim_start().starts_with(pat) {
                    self.starts_with = pat.to_owned();
                    self.start_line = line;
                    // ToDo: we have to test this better; up until then just continue
                    if never_constructed_variants.contains(&pat.to_owned()) {
                        continue;
                        //return Ok(NameSpaceResult::FindEndLineMatchArm);
                    }
                    // check items, which must end on semicolon
                    if NameSpace::MUST_END_ON_SEMICOLON
                        .iter()
                        .any(|s| *s == self.starts_with)
                    {
                        return Ok(NameSpaceResult::FindNextSemicolon);
                    }
                    // check items, which can end on a single line
                    if NameSpace::POSSIBLE_SINGLE_LINE_PATTERNS
                        .iter()
                        .any(|s| *s == self.starts_with)
                    {
                        if (self.starts_with == "#[" && slice.trim_end().ends_with(']'))
                            || slice.trim_end().ends_with(';')
                        {
                            self.end_line = self.start_line;
                            return Ok(NameSpaceResult::Finished);
                        }
                    }

                    if self.starts_with == "#[" {
                        self.open_bracket_typ = '[';
                        self.closing_bracket_typ = ']';
                    }
                    // return true -> search for end_line of name space
                    return Ok(NameSpaceResult::FindEndLine);
                }
            }
        }
        Err(Box::new(CGError::NoStartLine(message_line)))
    }
    fn find_end_line(&mut self, is_match_arm: bool) -> BoxResult<()> {
        // since every single line statements are found in find_start_line() all
        // name_space must include at least one opening and closing bracket
        //let must_open_bracket = self.starts_with == "fn";
        let mut open_bracket_found = false;
        let mut bracket_count = 0;
        for (line, slice) in self.lines[self.start_line..].iter().enumerate() {
            let open_brackets = slice.matches(self.open_bracket_typ).count() as i32;
            let close_brackets = slice.matches(self.closing_bracket_typ).count() as i32;
            open_bracket_found = open_bracket_found || open_brackets > 0;
            bracket_count += open_brackets - close_brackets;
            match bracket_count {
                1.. => (),
                0 => {
                    if open_bracket_found || (is_match_arm && slice.trim().ends_with(',')) {
                        self.end_line = self.start_line + line;
                        return Ok(());
                    }
                }
                _ => return Err(Box::new(CGError::TooManyClosingBrackets)),
            }
        }
        Err(Box::new(CGError::NoEndLine))
    }
    fn find_next_semicolon(&mut self) -> BoxResult<()> {
        for (line, slice) in self.lines[self.start_line..].iter().enumerate() {
            if slice.contains(';') {
                self.end_line = self.start_line + line;
                return Ok(());
            }
        }
        Err(Box::new(CGError::NoEndLine))
    }
    fn filter_name_space(&self, snip_snap_lines: &mut Vec<usize>) -> (String, String) {
        let pre_lines = &self.lines[0..self.start_line];
        let post_lines = &self.lines[self.end_line + 1..];
        // push filtered lines to snip_snap_lines
        for line in self.start_line..=self.end_line {
            snip_snap_lines.push(line);
        }

        (
            [pre_lines, post_lines]
                .concat()
                .join(self.line_end_chars.as_str()),
            self.lines[self.start_line..=self.end_line].join(self.line_end_chars.as_str()),
        )
    }
}

impl CGData {
    pub fn command_cargo_check(&self) -> BoxResult<Output> {
        let current_dir = fs::canonicalize(self.tmp_dir.as_path())?;
        let bin_name = self.tmp_output_file.file_stem().unwrap().to_str().unwrap();
        Ok(Command::new("cargo")
            .current_dir(current_dir)
            .arg("check")
            .arg("--bin")
            .arg(bin_name)
            .arg("--message-format=json")
            .output()?)
    }
    fn collect_cargo_check_compiler_messages(
        &self,
    ) -> BoxResult<Option<BTreeMap<usize, CargoCheckItem>>> {
        let mut message_collection: BTreeMap<usize, CargoCheckItem> = BTreeMap::new();
        let result = self.command_cargo_check()?;
        let filter_levels = [DiagnosticLevel::Error, DiagnosticLevel::Warning];
        for filter_level in filter_levels {
            for message in cargo_metadata::Message::parse_stream(&result.stdout[..]) {
                if let Message::CompilerMessage(msg) = message? {
                    if msg.message.level == filter_level {
                        // ToDo: have to think more about handling variants
                        // at the moment let's ignore them
                        if msg.message.message.contains("variant") {
                            continue;
                        }
                        for span in msg.message.spans.iter().filter(|s| s.is_primary) {
                            let cargo_check_item = CargoCheckItem {
                                message: msg.message.message.to_owned(),
                                code: msg.message.code.clone(),
                                level: msg.message.level.clone(),
                                span: span.clone(),
                            };
                            // line count in messages start at 1, not 0
                            // therefore we decrement line_start, since we are parsing lines
                            message_collection.insert(span.line_start - 1, cargo_check_item);
                        }
                    }
                }
            }
            // fix errors before warnings
            if !message_collection.is_empty() {
                return Ok(Some(message_collection));
            }
        }
        // no errors or warnings
        Ok(None)
    }
    fn analyze_cargo_check_compiler_message(&self, message: &CargoCheckItem) -> PatchAction {
        let error_code = match message.code {
            Some(ref dc) => dc.code.to_owned(),
            None => "No code provided".into(),
        };
        let patch_action = match error_code.as_str() {
            "unused_variables" => PatchAction::AdjustUnusedVariableName(0, 0),
            _ => {
                if message.message.contains("variant")
                    && message.message.contains("never constructed")
                {
                    PatchAction::SnipNeverConstructedEnumVariant(0)
                } else {
                    PatchAction::SnipNameSpace(0)
                }
            }
        };

        let is_warning = message.level == DiagnosticLevel::Warning;
        let verbose_start = if is_warning { "WARNING" } else { "ERROR" };

        match patch_action {
            PatchAction::AdjustUnusedVariableName(_, _) => {
                if self.options.verbose {
                    println!(
                        "[{} {}] adjusting cargo check message \"{}\" (line_start: {}, byte_start: {})",
                        verbose_start, error_code, message.message, message.span.line_start, message.span.byte_start
                    );
                }
                PatchAction::AdjustUnusedVariableName(
                    message.span.line_start,
                    message.span.byte_start as usize,
                )
            }
            PatchAction::SnipNameSpace(_) => {
                if self.options.verbose {
                    println!(
                        "[{} {}] filtering cargo check message \"{}\" (line_start: {})",
                        verbose_start, error_code, message.message, message.span.line_start
                    );
                }
                PatchAction::SnipNameSpace(message.span.line_start)
            }
            PatchAction::SnipNeverConstructedEnumVariant(_) => {
                if self.options.verbose {
                    println!(
                        "[{} {}] filtering never constructed variant \"{}\" (line_start: {})",
                        verbose_start, error_code, message.message, message.span.line_start
                    );
                }
                PatchAction::SnipNeverConstructedEnumVariant(message.span.line_start)
            }
        }
    }

    fn snip_name_space(
        &self,
        output: &mut String,
        line_start: usize,
        never_constructed_variants: &Vec<String>,
        snip_snap_lines: &mut Vec<usize>,
    ) -> BoxResult<()> {
        let mut name_space = NameSpace::new(output, self.line_end_chars.clone());
        match name_space.find_start_line(line_start, never_constructed_variants)? {
            NameSpaceResult::Finished => (),
            NameSpaceResult::FindEndLine => name_space.find_end_line(false)?,
            NameSpaceResult::FindEndLineMatchArm => name_space.find_end_line(true)?,
            NameSpaceResult::FindNextSemicolon => name_space.find_next_semicolon()?,
        }
        let (new_output, filtered) = name_space.filter_name_space(snip_snap_lines);
        *output = new_output;
        if self.options.verbose {
            println!("SNIP\n{}\nSNAP", filtered);
        }
        Ok(())
    }

    fn snip_never_constructed_enum_variant(
        &self,
        output: &mut String,
        line_start: usize,
        never_constructed_variants: &mut Vec<String>,
        snip_snap_lines: &mut Vec<usize>,
    ) -> BoxResult<()> {
        // collect lines
        let mut lines: Vec<&str> = output.lines().collect();
        // remove variant from message line
        let filtered = lines.remove(line_start);

        let mut enum_variant = filtered.trim().replace(",", "");
        // if variant contains a variable, just take the name
        if enum_variant.contains('(') {
            enum_variant = enum_variant.split_once('(').unwrap().0.to_string();
        }
        // find enum of variant
        for line in lines[0..line_start].iter().rev() {
            if line.trim().starts_with("enum") {
                let mut enum_name = line.trim().split_whitespace().nth(1).unwrap().to_owned();
                if enum_name.contains('<') {
                    enum_name = enum_name.split_once('<').unwrap().0.to_string();
                }
                enum_variant = enum_name + "::" + &enum_variant;
                break;
            }
        }
        // check if enum was found
        if !enum_variant.contains("::") {
            return Err(Box::new(CGError::CouldNotFindEnumName));
        }
        // save enum variant to later remove match arms, which use never constructed variant, if any remain
        never_constructed_variants.push(enum_variant);

        // push filtered line to snip_snap_lines
        snip_snap_lines.push(line_start);

        if self.options.verbose {
            println!("SNIP\n{}\nSNAP", filtered);
        }

        // join lines for new output
        *output = lines.join(self.line_end_chars.as_str());
        Ok(())
    }

    fn adjust_unused_variable_name(
        &self,
        output: &mut String,
        line_start: usize,
        byte_start: usize,
    ) {
        if self.options.verbose {
            println!("OLD: {}", output.lines().nth(line_start - 1).unwrap());
        }
        output.insert(byte_start, '_');
        if self.options.verbose {
            println!("NEW: {}", output.lines().nth(line_start - 1).unwrap());
        }
    }

    pub fn filter_unused_code(&self) -> BoxResult<()> {
        if !self.options.simulate {
            if self.options.verbose {
                println!("starting filtering unused code in output...");
            }
            // use check_counter to prevent endless checking results
            let mut check_counter = 0;
            let max_check_counter = 10_000;
            //let max_check_counter = 1;
            let mut never_constructed_variants: Vec<String> = Vec::new();
            // collect compiler messages in BTreeMap
            // using line_start as key. This results in compiler messages sorted by line_start.
            // By reverse iteration through message_collection the fixes can be applied from bottom to top.
            // When all fixes are applied, the file is saved and a new round is started, until no more
            // messages are collected.
            while let Some(message_collection) = self.collect_cargo_check_compiler_messages()? {
                // initialize output
                let mut output = String::new();
                self.load_output(&mut output)?;
                // initialize snip snap array
                let mut snip_snap_lines: Vec<usize> = Vec::with_capacity(message_collection.len());
                if self.options.verbose {
                    println!(
                        "number of compiler messages in BTreeMap: {}",
                        message_collection.len()
                    );
                }
                // revers iteration of message_collection, which results to work through messages from bottom to top
                for (message_line, message) in message_collection.iter().rev() {
                    // if message_line has already been snip snap, ignore message and continue
                    if snip_snap_lines.contains(message_line) {
                        continue;
                    }
                    // check counter
                    if check_counter >= max_check_counter {
                        break;
                    }
                    check_counter += 1;
                    println!("check_counter: {}", check_counter);
                    // ToDo: Debug stuff. remove later
                    if message.level == DiagnosticLevel::Warning {
                        //break
                    }

                    match self.analyze_cargo_check_compiler_message(message) {
                        PatchAction::AdjustUnusedVariableName(line_start, byte_start) => {
                            self.adjust_unused_variable_name(&mut output, line_start, byte_start)
                        }
                        PatchAction::SnipNameSpace(line_start) => self.snip_name_space(
                            &mut output,
                            line_start,
                            &never_constructed_variants,
                            &mut snip_snap_lines,
                        )?,
                        PatchAction::SnipNeverConstructedEnumVariant(line_start) => self
                            .snip_never_constructed_enum_variant(
                                &mut output,
                                line_start,
                                &mut never_constructed_variants,
                                &mut snip_snap_lines,
                            )?,
                    }
                }

                self.save_output(&output)?;
            }
            let mut output = String::new();
            self.load_output(&mut output)?;
            // removing comments, if option is set
            if self.options.del_comments {
                if self.options.verbose {
                    println!("deleting comments...");
                }
                output = output
                    .lines()
                    .map(|l| {
                        if !l.contains(&['⏬', '⏫'][..]) {
                            match l.split_once("//") {
                                Some((pre_split, _)) => pre_split.trim_end(),
                                None => l,
                            }
                        } else {
                            l
                        }
                    })
                    .collect::<Vec<&str>>()
                    .join(self.line_end_chars.as_str());
            }
            if !self.options.keep_empty_lines {
                // deleting empty lines
                if self.options.verbose {
                    println!("deleting empty lines...");
                }
                output = output
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .collect::<Vec<&str>>()
                    .join(self.line_end_chars.as_str());
                self.save_output(&output)?;
            }
        }
        Ok(())
    }
}
*/
