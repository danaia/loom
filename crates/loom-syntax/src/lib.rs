//! Agent-native Loom source parsing and lowering.
//!
//! This crate intentionally starts with a narrow, deterministic grammar. It
//! lowers source directly into `loom_core`'s canonical typed graph so the
//! existing validator remains the execution trust boundary.

use loom_core::{
    DataType, KernelAbiDraft, KernelDraft, Literal, ModuleBuilder, ModuleGraph, PassDraft,
    ResourceAccess, ScalarType, ScheduleDraft, SlotAccess, SlotDraft, StorageClass, StreamDraft,
    Target, Unit, ValueDraft, ViewDraft, metal_implementation, packaged_metal_implementation,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcePosition {
    pub offset: usize,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceDiagnostic {
    pub code: String,
    pub message: String,
    pub span: SourceSpan,
}

impl SourceDiagnostic {
    fn new(code: impl Into<String>, message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            span,
        }
    }
}

/// Parse agent-native Loom source and lower it into the canonical graph.
pub fn parse(source: &str) -> Result<ModuleGraph, Vec<SourceDiagnostic>> {
    let tokens = lex(source)?;
    let mut parser = Parser::new(tokens);
    let module = parser
        .parse_module()
        .map_err(|diagnostic| vec![diagnostic])?;
    lower(module).map_err(|diagnostic| vec![diagnostic])
}

#[derive(Clone, Debug, PartialEq)]
enum TokenKind {
    Word(String),
    Number(String),
    Text(String),
    Symbol(char),
    Arrow,
    Eof,
}

#[derive(Clone, Debug, PartialEq)]
struct Token {
    kind: TokenKind,
    span: SourceSpan,
}

fn lex(source: &str) -> Result<Vec<Token>, Vec<SourceDiagnostic>> {
    let bytes = source.as_bytes();
    let mut index = 0;
    let mut line = 1_u32;
    let mut column = 1_u32;
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();

    while index < bytes.len() {
        let byte = bytes[index];
        if byte.is_ascii_whitespace() {
            advance(byte, &mut index, &mut line, &mut column);
            continue;
        }
        if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            while index < bytes.len() && bytes[index] != b'\n' {
                advance(bytes[index], &mut index, &mut line, &mut column);
            }
            continue;
        }

        let start = position(index, line, column);
        if byte.is_ascii_alphabetic() || byte == b'_' {
            let begin = index;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'_' | b'.'))
            {
                advance(bytes[index], &mut index, &mut line, &mut column);
            }
            tokens.push(Token {
                kind: TokenKind::Word(source[begin..index].to_owned()),
                span: span(start, position(index, line, column)),
            });
            continue;
        }

        if byte.is_ascii_digit() {
            let begin = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                advance(bytes[index], &mut index, &mut line, &mut column);
            }
            if bytes.get(index) == Some(&b'.') {
                advance(bytes[index], &mut index, &mut line, &mut column);
                while index < bytes.len() && bytes[index].is_ascii_digit() {
                    advance(bytes[index], &mut index, &mut line, &mut column);
                }
            }
            if matches!(bytes.get(index), Some(b'e' | b'E')) {
                advance(bytes[index], &mut index, &mut line, &mut column);
                if matches!(bytes.get(index), Some(b'+' | b'-')) {
                    advance(bytes[index], &mut index, &mut line, &mut column);
                }
                while index < bytes.len() && bytes[index].is_ascii_digit() {
                    advance(bytes[index], &mut index, &mut line, &mut column);
                }
            }
            tokens.push(Token {
                kind: TokenKind::Number(source[begin..index].to_owned()),
                span: span(start, position(index, line, column)),
            });
            continue;
        }

        if byte == b'"' {
            advance(byte, &mut index, &mut line, &mut column);
            let mut value = String::new();
            let mut terminated = false;
            while index < bytes.len() {
                let current = bytes[index];
                if current == b'"' {
                    advance(current, &mut index, &mut line, &mut column);
                    terminated = true;
                    break;
                }
                if current == b'\\' {
                    advance(current, &mut index, &mut line, &mut column);
                    let Some(escaped) = bytes.get(index).copied() else {
                        break;
                    };
                    let decoded = match escaped {
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'"' => '"',
                        b'\\' => '\\',
                        _ => {
                            diagnostics.push(SourceDiagnostic::new(
                                "L0002",
                                format!("unsupported escape `\\{}`", escaped as char),
                                span(start, position(index + 1, line, column + 1)),
                            ));
                            escaped as char
                        }
                    };
                    value.push(decoded);
                    advance(escaped, &mut index, &mut line, &mut column);
                    continue;
                }
                value.push(current as char);
                advance(current, &mut index, &mut line, &mut column);
            }
            if !terminated {
                diagnostics.push(SourceDiagnostic::new(
                    "L0001",
                    "unterminated string",
                    span(start, position(index, line, column)),
                ));
            } else {
                tokens.push(Token {
                    kind: TokenKind::Text(value),
                    span: span(start, position(index, line, column)),
                });
            }
            continue;
        }

        if byte == b'-' && bytes.get(index + 1) == Some(&b'>') {
            advance(byte, &mut index, &mut line, &mut column);
            advance(bytes[index], &mut index, &mut line, &mut column);
            tokens.push(Token {
                kind: TokenKind::Arrow,
                span: span(start, position(index, line, column)),
            });
            continue;
        }

        if matches!(
            byte,
            b'{' | b'}'
                | b'('
                | b')'
                | b'['
                | b']'
                | b'<'
                | b'>'
                | b','
                | b':'
                | b'='
                | b'/'
                | b'*'
                | b'^'
                | b'+'
                | b'-'
                | b';'
        ) {
            advance(byte, &mut index, &mut line, &mut column);
            tokens.push(Token {
                kind: TokenKind::Symbol(byte as char),
                span: span(start, position(index, line, column)),
            });
            continue;
        }

        advance(byte, &mut index, &mut line, &mut column);
        diagnostics.push(SourceDiagnostic::new(
            "L0003",
            format!("unexpected character `{}`", byte as char),
            span(start, position(index, line, column)),
        ));
    }

    let eof = position(index, line, column);
    tokens.push(Token {
        kind: TokenKind::Eof,
        span: span(eof, eof),
    });
    if diagnostics.is_empty() {
        Ok(tokens)
    } else {
        Err(diagnostics)
    }
}

fn advance(byte: u8, index: &mut usize, line: &mut u32, column: &mut u32) {
    *index += 1;
    if byte == b'\n' {
        *line += 1;
        *column = 1;
    } else {
        *column += 1;
    }
}

const fn position(offset: usize, line: u32, column: u32) -> SourcePosition {
    SourcePosition {
        offset,
        line,
        column,
    }
}

const fn span(start: SourcePosition, end: SourcePosition) -> SourceSpan {
    SourceSpan { start, end }
}

#[derive(Clone, Debug)]
struct ModuleAst {
    name: String,
    target: String,
    constants: Vec<ConstantAst>,
    streams: Vec<StreamAst>,
    kernels: Vec<KernelAst>,
    passes: Vec<PassAst>,
    views: Vec<ViewAst>,
    flows: Vec<FlowAst>,
    span: SourceSpan,
}

#[derive(Clone, Debug)]
struct TypedAst {
    data_type: DataType,
    unit: Unit,
}

#[derive(Clone, Debug)]
struct ConstantAst {
    name: String,
    typed: TypedAst,
    value: RawLiteral,
    span: SourceSpan,
}

#[derive(Clone, Debug)]
struct StreamAst {
    name: String,
    typed: TypedAst,
    capacity: u32,
    length: u32,
    buffering: u32,
    access: ResourceAccess,
    storage: StorageClass,
    initial: Option<RawLiteral>,
    span: SourceSpan,
}

#[derive(Clone, Debug)]
struct KernelAst {
    name: String,
    parameters: Vec<ParameterAst>,
    implementation: KernelImplementationAst,
    span: SourceSpan,
}

#[derive(Clone, Debug)]
struct ParameterAst {
    name: String,
    access: SlotAccess,
    resource: ResourceKind,
    typed: TypedAst,
}

#[derive(Clone, Debug)]
enum KernelImplementationAst {
    ExternalMetal {
        source: String,
        entry: String,
    },
    Native {
        index: String,
        statements: Vec<StatementAst>,
    },
}

#[derive(Clone, Debug)]
struct StatementAst {
    target: String,
    index: String,
    value: ExprAst,
    span: SourceSpan,
}

#[derive(Clone, Debug)]
struct ExprAst {
    kind: ExprKind,
    span: SourceSpan,
}

#[derive(Clone, Debug)]
enum ExprKind {
    Name(String),
    Index {
        resource: String,
        index: String,
    },
    Number(String),
    Binary {
        operator: BinaryOperator,
        left: Box<ExprAst>,
        right: Box<ExprAst>,
    },
}

#[derive(Clone, Copy, Debug)]
enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
}

impl BinaryOperator {
    const fn precedence(self) -> u8 {
        match self {
            Self::Add | Self::Subtract => 1,
            Self::Multiply | Self::Divide => 2,
        }
    }

    const fn metal(self) -> char {
        match self {
            Self::Add => '+',
            Self::Subtract => '-',
            Self::Multiply => '*',
            Self::Divide => '/',
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum ResourceKind {
    Stream,
    Value,
}

#[derive(Clone, Debug)]
struct PassAst {
    name: String,
    kernel: String,
    bindings: Vec<(String, String)>,
    domain: String,
}

#[derive(Clone, Debug)]
struct ViewAst {
    name: String,
    reads: Vec<(String, String)>,
    source: String,
    entry: String,
}

#[derive(Clone, Debug)]
struct FlowAst {
    name: String,
    rate_hz: u32,
    passes: Vec<String>,
    presentation: Option<(String, String)>,
}

#[derive(Clone, Debug)]
enum RawLiteral {
    Bool(bool),
    Number(String),
    Array(Vec<RawLiteral>),
}

struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, cursor: 0 }
    }

    fn parse_module(&mut self) -> Result<ModuleAst, SourceDiagnostic> {
        let start = self.current().span.start;
        self.expect_word("loom")?;
        let version = self.expect_number()?;
        if version != "0.1" {
            return Err(self.error(
                "P0002",
                format!("unsupported Loom version `{version}`; expected `0.1`"),
            ));
        }
        self.expect_word("module")?;
        let name = self.expect_any_word()?;
        self.expect_word("target")?;
        let target = self.expect_any_word()?;

        let mut module = ModuleAst {
            name,
            target,
            constants: Vec::new(),
            streams: Vec::new(),
            kernels: Vec::new(),
            passes: Vec::new(),
            views: Vec::new(),
            flows: Vec::new(),
            span: span(start, self.current().span.end),
        };

        while !matches!(self.current().kind, TokenKind::Eof) {
            self.eat_symbol(';');
            if matches!(self.current().kind, TokenKind::Eof) {
                break;
            }
            match self.peek_word() {
                Some("const") => module.constants.push(self.parse_constant()?),
                Some("stream") => module.streams.push(self.parse_stream()?),
                Some("kernel") => module.kernels.push(self.parse_kernel()?),
                Some("pass") => module.passes.push(self.parse_pass()?),
                Some("view") => module.views.push(self.parse_view()?),
                Some("flow") => module.flows.push(self.parse_flow()?),
                Some(word) => {
                    return Err(self.error("P0003", format!("unexpected declaration `{word}`")));
                }
                None => return Err(self.error("P0003", "expected a declaration")),
            }
        }
        module.span.end = self.current().span.end;
        if module.flows.is_empty() {
            return Err(SourceDiagnostic::new(
                "P0004",
                "a module must declare at least one flow",
                module.span,
            ));
        }
        Ok(module)
    }

    fn parse_constant(&mut self) -> Result<ConstantAst, SourceDiagnostic> {
        let start = self.current().span.start;
        self.expect_word("const")?;
        let name = self.expect_any_word()?;
        self.expect_symbol(':')?;
        let typed = self.parse_typed()?;
        self.expect_symbol('=')?;
        let value = self.parse_literal()?;
        self.eat_symbol(';');
        Ok(ConstantAst {
            name,
            typed,
            value,
            span: span(start, self.previous().span.end),
        })
    }

    fn parse_stream(&mut self) -> Result<StreamAst, SourceDiagnostic> {
        let start = self.current().span.start;
        self.expect_word("stream")?;
        let name = self.expect_any_word()?;
        self.expect_symbol(':')?;
        let typed = self.parse_typed()?;
        self.expect_symbol('{')?;

        let mut capacity = None;
        let mut length = None;
        let mut buffering = 1;
        let mut access = ResourceAccess::DeviceReadWrite;
        let mut storage = StorageClass::DevicePrivate;
        let mut initial = None;

        while !self.eat_symbol('}') {
            let property = self.expect_any_word()?;
            self.expect_symbol('=')?;
            match property.as_str() {
                "cap" => capacity = Some(self.expect_u32("stream capacity")?),
                "len" => length = Some(self.expect_u32("stream length")?),
                "buffers" => buffering = self.expect_u32("buffer count")?,
                "access" => {
                    access = match self.expect_any_word()?.as_str() {
                        "r" => ResourceAccess::DeviceRead,
                        "rw" => ResourceAccess::DeviceReadWrite,
                        "host_rw" => ResourceAccess::HostReadWrite,
                        other => {
                            return Err(
                                self.error("P0010", format!("unknown stream access `{other}`"))
                            );
                        }
                    }
                }
                "storage" => {
                    storage = match self.expect_any_word()?.as_str() {
                        "device" => StorageClass::DevicePrivate,
                        "shared" => StorageClass::HostShared,
                        other => {
                            return Err(
                                self.error("P0011", format!("unknown storage class `{other}`"))
                            );
                        }
                    }
                }
                "init" => initial = Some(self.parse_literal()?),
                other => {
                    return Err(self.error("P0012", format!("unknown stream property `{other}`")));
                }
            }
            self.eat_symbol(',');
            self.eat_symbol(';');
        }

        let capacity = capacity.unwrap_or(1);
        Ok(StreamAst {
            name,
            typed,
            capacity,
            length: length.unwrap_or(capacity),
            buffering,
            access,
            storage,
            initial,
            span: span(start, self.previous().span.end),
        })
    }

    fn parse_kernel(&mut self) -> Result<KernelAst, SourceDiagnostic> {
        let start = self.current().span.start;
        self.expect_word("kernel")?;
        let name = self.expect_any_word()?;
        self.expect_symbol('(')?;
        let mut parameters = Vec::new();
        while !self.eat_symbol(')') {
            let parameter_name = self.expect_any_word()?;
            self.expect_symbol(':')?;
            let access = match self.expect_any_word()?.as_str() {
                "in" | "r" => SlotAccess::Read,
                "out" | "w" => SlotAccess::Write,
                "rw" => SlotAccess::ReadWrite,
                "atomic" => SlotAccess::Atomic,
                other => {
                    return Err(self.error("P0020", format!("unknown kernel access `{other}`")));
                }
            };
            let resource = match self.expect_any_word()?.as_str() {
                "stream" => ResourceKind::Stream,
                "value" => ResourceKind::Value,
                other => {
                    return Err(
                        self.error("P0021", format!("unknown kernel resource kind `{other}`"))
                    );
                }
            };
            self.expect_symbol('<')?;
            let data_type = self.parse_data_type()?;
            let unit = if self.eat_symbol(',') {
                self.parse_unit_until('>')?
            } else {
                Unit::DIMENSIONLESS
            };
            self.expect_symbol('>')?;
            if matches!(resource, ResourceKind::Value) && access != SlotAccess::Read {
                return Err(self.error("P0022", "value parameters must use `in` access"));
            }
            parameters.push(ParameterAst {
                name: parameter_name,
                access,
                resource,
                typed: TypedAst { data_type, unit },
            });
            if !self.eat_symbol(',') && !self.check_symbol(')') {
                return Err(self.error("P0023", "expected `,` or `)` after kernel parameter"));
            }
        }

        let implementation = if self.peek_word() == Some("extern") {
            self.expect_word("extern")?;
            self.expect_word("metal")?;
            self.expect_symbol('{')?;
            let mut source = None;
            let mut entry = None;
            while !self.eat_symbol('}') {
                let property = self.expect_any_word()?;
                self.expect_symbol('=')?;
                let value = self.expect_text()?;
                match property.as_str() {
                    "source" => source = Some(value),
                    "entry" => entry = Some(value),
                    other => {
                        return Err(self.error(
                            "P0024",
                            format!("unknown Metal implementation property `{other}`"),
                        ));
                    }
                }
                self.eat_symbol(',');
                self.eat_symbol(';');
            }
            KernelImplementationAst::ExternalMetal {
                source: source
                    .ok_or_else(|| self.error("P0025", "Metal implementation requires `source`"))?,
                entry: entry
                    .ok_or_else(|| self.error("P0026", "Metal implementation requires `entry`"))?,
            }
        } else {
            self.expect_word("each")?;
            let index = self.expect_any_word()?;
            self.expect_symbol('{')?;
            let mut statements = Vec::new();
            while !self.eat_symbol('}') {
                statements.push(self.parse_statement()?);
            }
            if statements.is_empty() {
                return Err(self.error("P0027", "native kernel body cannot be empty"));
            }
            KernelImplementationAst::Native { index, statements }
        };

        Ok(KernelAst {
            name,
            parameters,
            implementation,
            span: span(start, self.previous().span.end),
        })
    }

    fn parse_statement(&mut self) -> Result<StatementAst, SourceDiagnostic> {
        let start = self.current().span.start;
        let target = self.expect_any_word()?;
        self.expect_symbol('[')?;
        let index = self.expect_any_word()?;
        self.expect_symbol(']')?;

        let compound = if self.eat_symbol('=') {
            None
        } else {
            let operator = self
                .parse_binary_operator()
                .ok_or_else(|| self.error("P0050", "expected `=`, `+=`, `-=`, `*=`, or `/=`"))?;
            self.expect_symbol('=')?;
            Some(operator)
        };
        let right = self.parse_expression(0)?;
        self.expect_symbol(';')?;
        let target_expression = ExprAst {
            kind: ExprKind::Index {
                resource: target.clone(),
                index: index.clone(),
            },
            span: span(start, self.previous().span.end),
        };
        let value = if let Some(operator) = compound {
            ExprAst {
                span: span(start, right.span.end),
                kind: ExprKind::Binary {
                    operator,
                    left: Box::new(target_expression),
                    right: Box::new(right),
                },
            }
        } else {
            right
        };
        Ok(StatementAst {
            target,
            index,
            value,
            span: span(start, self.previous().span.end),
        })
    }

    fn parse_expression(&mut self, minimum_precedence: u8) -> Result<ExprAst, SourceDiagnostic> {
        let mut left = self.parse_primary_expression()?;
        loop {
            let Some(operator) = self.peek_binary_operator() else {
                break;
            };
            let precedence = operator.precedence();
            if precedence < minimum_precedence {
                break;
            }
            self.cursor += 1;
            let right = self.parse_expression(precedence + 1)?;
            let expression_span = span(left.span.start, right.span.end);
            left = ExprAst {
                kind: ExprKind::Binary {
                    operator,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span: expression_span,
            };
        }
        Ok(left)
    }

    fn parse_primary_expression(&mut self) -> Result<ExprAst, SourceDiagnostic> {
        let start = self.current().span.start;
        if self.eat_symbol('-') {
            let number = self.expect_number()?;
            return Ok(ExprAst {
                kind: ExprKind::Number(format!("-{number}")),
                span: span(start, self.previous().span.end),
            });
        }
        match self.current().kind.clone() {
            TokenKind::Word(name) => {
                self.cursor += 1;
                let kind = if self.eat_symbol('[') {
                    let index = self.expect_any_word()?;
                    self.expect_symbol(']')?;
                    ExprKind::Index {
                        resource: name,
                        index,
                    }
                } else {
                    ExprKind::Name(name)
                };
                Ok(ExprAst {
                    kind,
                    span: span(start, self.previous().span.end),
                })
            }
            TokenKind::Number(number) => {
                self.cursor += 1;
                Ok(ExprAst {
                    kind: ExprKind::Number(number),
                    span: self.previous().span,
                })
            }
            TokenKind::Symbol('(') => {
                self.cursor += 1;
                let mut expression = self.parse_expression(0)?;
                self.expect_symbol(')')?;
                expression.span = span(start, self.previous().span.end);
                Ok(expression)
            }
            _ => Err(self.error("P0051", "expected a kernel expression")),
        }
    }

    fn parse_binary_operator(&mut self) -> Option<BinaryOperator> {
        let operator = self.peek_binary_operator()?;
        self.cursor += 1;
        Some(operator)
    }

    fn peek_binary_operator(&self) -> Option<BinaryOperator> {
        match self.current().kind {
            TokenKind::Symbol('+') => Some(BinaryOperator::Add),
            TokenKind::Symbol('-') => Some(BinaryOperator::Subtract),
            TokenKind::Symbol('*') => Some(BinaryOperator::Multiply),
            TokenKind::Symbol('/') => Some(BinaryOperator::Divide),
            _ => None,
        }
    }

    fn parse_pass(&mut self) -> Result<PassAst, SourceDiagnostic> {
        self.expect_word("pass")?;
        let name = self.expect_any_word()?;
        self.expect_symbol('=')?;
        let kernel = self.expect_any_word()?;
        self.expect_symbol('(')?;
        let mut bindings = Vec::new();
        while !self.eat_symbol(')') {
            let slot = self.expect_any_word()?;
            self.expect_symbol('=')?;
            let resource = self.expect_any_word()?;
            bindings.push((slot, resource));
            self.eat_symbol(',');
        }
        self.expect_word("over")?;
        let domain = self.expect_any_word()?;
        self.eat_symbol(';');
        Ok(PassAst {
            name,
            kernel,
            bindings,
            domain,
        })
    }

    fn parse_flow(&mut self) -> Result<FlowAst, SourceDiagnostic> {
        self.expect_word("flow")?;
        let name = self.expect_any_word()?;
        self.expect_word("rate")?;
        self.expect_symbol('=')?;
        let rate_hz = self.expect_u32("flow rate")?;
        self.expect_word("hz")?;
        self.expect_symbol('{')?;
        let mut passes = vec![self.expect_any_word()?];
        while self.eat_arrow() {
            passes.push(self.expect_any_word()?);
        }
        let presentation = if self.peek_word() == Some("draw") {
            self.expect_word("draw")?;
            let view = self.expect_any_word()?;
            self.expect_word("after")?;
            let producer = self.expect_any_word()?;
            Some((view, producer))
        } else {
            None
        };
        self.expect_symbol('}')?;
        self.eat_symbol(';');
        Ok(FlowAst {
            name,
            rate_hz,
            passes,
            presentation,
        })
    }

    fn parse_view(&mut self) -> Result<ViewAst, SourceDiagnostic> {
        self.expect_word("view")?;
        let name = self.expect_any_word()?;
        self.expect_symbol('(')?;
        let mut reads = Vec::new();
        while !self.eat_symbol(')') {
            let binding = self.expect_any_word()?;
            self.expect_symbol('=')?;
            let stream = self.expect_any_word()?;
            reads.push((binding, stream));
            self.eat_symbol(',');
        }
        self.expect_word("extern")?;
        self.expect_word("metal")?;
        self.expect_symbol('{')?;
        let mut source = None;
        let mut entry = None;
        while !self.eat_symbol('}') {
            let property = self.expect_any_word()?;
            self.expect_symbol('=')?;
            let value = self.expect_text()?;
            match property.as_str() {
                "source" => source = Some(value),
                "entry" => entry = Some(value),
                other => {
                    return Err(
                        self.error("P0060", format!("unknown Metal view property `{other}`"))
                    );
                }
            }
            self.eat_symbol(',');
            self.eat_symbol(';');
        }
        Ok(ViewAst {
            name,
            reads,
            source: source.ok_or_else(|| self.error("P0061", "view requires `source`"))?,
            entry: entry.ok_or_else(|| self.error("P0062", "view requires `entry`"))?,
        })
    }

    fn parse_typed(&mut self) -> Result<TypedAst, SourceDiagnostic> {
        let data_type = self.parse_data_type()?;
        let unit = if self.eat_symbol('<') {
            let unit = self.parse_unit_until('>')?;
            self.expect_symbol('>')?;
            unit
        } else {
            Unit::DIMENSIONLESS
        };
        Ok(TypedAst { data_type, unit })
    }

    fn parse_data_type(&mut self) -> Result<DataType, SourceDiagnostic> {
        let name = self.expect_any_word()?;
        match name.as_str() {
            "bool" => return Ok(DataType::Scalar(ScalarType::Bool)),
            "i32" => return Ok(DataType::Scalar(ScalarType::I32)),
            "u32" => return Ok(DataType::Scalar(ScalarType::U32)),
            "f16" => return Ok(DataType::Scalar(ScalarType::F16)),
            "f32" => return Ok(DataType::Scalar(ScalarType::F32)),
            _ => {}
        }
        for (prefix, scalar) in [
            ("i32x", ScalarType::I32),
            ("u32x", ScalarType::U32),
            ("f16x", ScalarType::F16),
            ("f32x", ScalarType::F32),
        ] {
            if let Some(lanes) = name.strip_prefix(prefix) {
                let lanes = lanes
                    .parse::<u8>()
                    .map_err(|_| self.error("P0030", format!("invalid vector type `{name}`")))?;
                if !(2..=4).contains(&lanes) {
                    return Err(self.error(
                        "P0030",
                        format!("vector `{name}` must have 2, 3, or 4 lanes"),
                    ));
                }
                return Ok(DataType::Vector { scalar, lanes });
            }
        }
        Err(self.error("P0030", format!("unknown data type `{name}`")))
    }

    fn parse_unit_until(&mut self, end: char) -> Result<Unit, SourceDiagnostic> {
        let mut text = String::new();
        while !self.check_symbol(end) {
            match &self.current().kind {
                TokenKind::Word(value) | TokenKind::Number(value) => text.push_str(value),
                TokenKind::Symbol(symbol) if matches!(symbol, '/' | '*' | '^' | '-') => {
                    text.push(*symbol)
                }
                _ => return Err(self.error("P0031", "invalid physical unit")),
            }
            self.cursor += 1;
        }
        parse_unit(&text).map_err(|message| self.error("P0031", message))
    }

    fn parse_literal(&mut self) -> Result<RawLiteral, SourceDiagnostic> {
        if self.eat_symbol('-') {
            let value = self.expect_number()?;
            return Ok(RawLiteral::Number(format!("-{value}")));
        }
        match self.current().kind.clone() {
            TokenKind::Word(value) if value == "true" || value == "false" => {
                self.cursor += 1;
                Ok(RawLiteral::Bool(value == "true"))
            }
            TokenKind::Number(value) => {
                self.cursor += 1;
                Ok(RawLiteral::Number(value))
            }
            TokenKind::Symbol('[') => {
                self.cursor += 1;
                let mut items = Vec::new();
                while !self.eat_symbol(']') {
                    items.push(self.parse_literal()?);
                    if !self.eat_symbol(',') && !self.check_symbol(']') {
                        return Err(self.error("P0040", "expected `,` or `]` in literal"));
                    }
                }
                Ok(RawLiteral::Array(items))
            }
            _ => Err(self.error("P0040", "expected a literal")),
        }
    }

    fn expect_word(&mut self, expected: &str) -> Result<(), SourceDiagnostic> {
        match &self.current().kind {
            TokenKind::Word(actual) if actual == expected => {
                self.cursor += 1;
                Ok(())
            }
            _ => Err(self.error("P0001", format!("expected `{expected}`"))),
        }
    }

    fn expect_any_word(&mut self) -> Result<String, SourceDiagnostic> {
        match self.current().kind.clone() {
            TokenKind::Word(value) => {
                self.cursor += 1;
                Ok(value)
            }
            _ => Err(self.error("P0001", "expected an identifier")),
        }
    }

    fn expect_number(&mut self) -> Result<String, SourceDiagnostic> {
        match self.current().kind.clone() {
            TokenKind::Number(value) => {
                self.cursor += 1;
                Ok(value)
            }
            _ => Err(self.error("P0001", "expected a number")),
        }
    }

    fn expect_u32(&mut self, purpose: &str) -> Result<u32, SourceDiagnostic> {
        let value = self.expect_number()?;
        value.parse::<u32>().map_err(|_| {
            self.error(
                "P0005",
                format!("{purpose} must be an unsigned 32-bit integer"),
            )
        })
    }

    fn expect_text(&mut self) -> Result<String, SourceDiagnostic> {
        match self.current().kind.clone() {
            TokenKind::Text(value) => {
                self.cursor += 1;
                Ok(value)
            }
            _ => Err(self.error("P0001", "expected a quoted string")),
        }
    }

    fn expect_symbol(&mut self, expected: char) -> Result<(), SourceDiagnostic> {
        if self.eat_symbol(expected) {
            Ok(())
        } else {
            Err(self.error("P0001", format!("expected `{expected}`")))
        }
    }

    fn eat_symbol(&mut self, expected: char) -> bool {
        if self.check_symbol(expected) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn check_symbol(&self, expected: char) -> bool {
        matches!(self.current().kind, TokenKind::Symbol(actual) if actual == expected)
    }

    fn eat_arrow(&mut self) -> bool {
        if matches!(self.current().kind, TokenKind::Arrow) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn peek_word(&self) -> Option<&str> {
        match &self.current().kind {
            TokenKind::Word(value) => Some(value),
            _ => None,
        }
    }

    fn current(&self) -> &Token {
        &self.tokens[self.cursor]
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.cursor.saturating_sub(1)]
    }

    fn error(&self, code: impl Into<String>, message: impl Into<String>) -> SourceDiagnostic {
        SourceDiagnostic::new(code, message, self.current().span)
    }
}

fn parse_unit(text: &str) -> Result<Unit, String> {
    if text.is_empty() || text == "1" {
        return Ok(Unit::DIMENSIONLESS);
    }
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut sign = 1_i8;
    let mut result = Unit::DIMENSIONLESS;
    while index < bytes.len() {
        match bytes[index] {
            b'*' => {
                sign = 1;
                index += 1;
                continue;
            }
            b'/' => {
                sign = -1;
                index += 1;
                continue;
            }
            _ => {}
        }
        let begin = index;
        while index < bytes.len() && bytes[index].is_ascii_alphabetic() {
            index += 1;
        }
        if begin == index {
            return Err(format!("invalid unit `{text}`"));
        }
        let base = &text[begin..index];
        let mut exponent = 1_i8;
        if bytes.get(index) == Some(&b'^') {
            index += 1;
            let exponent_begin = index;
            if bytes.get(index) == Some(&b'-') {
                index += 1;
            }
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            exponent = text[exponent_begin..index]
                .parse::<i8>()
                .map_err(|_| format!("invalid exponent in unit `{text}`"))?;
        }
        let exponent = exponent
            .checked_mul(sign)
            .ok_or_else(|| format!("unit exponent overflow in `{text}`"))?;
        match base {
            "m" => result.length = result.length.saturating_add(exponent),
            "kg" => result.mass = result.mass.saturating_add(exponent),
            "s" => result.time = result.time.saturating_add(exponent),
            other => return Err(format!("unknown unit `{other}`")),
        }
    }
    Ok(result)
}

fn lower(module: ModuleAst) -> Result<ModuleGraph, SourceDiagnostic> {
    if module.target != "metal" {
        return Err(SourceDiagnostic::new(
            "S0001",
            format!("unsupported target `{}`; expected `metal`", module.target),
            module.span,
        ));
    }

    let module_name = module.name.clone();
    let mut builder = ModuleBuilder::new(module.name).target(Target::Metal);
    for constant in module.constants {
        let value = lower_value(&constant.value, &constant.typed.data_type)
            .map_err(|message| SourceDiagnostic::new("S0010", message, constant.span))?;
        builder = builder.value(ValueDraft::constant(
            constant.name,
            constant.typed.data_type,
            constant.typed.unit,
            value,
        ));
    }
    for stream in module.streams {
        let mut draft = StreamDraft::new(
            stream.name,
            stream.typed.data_type.clone(),
            stream.typed.unit,
        )
        .capacity(stream.capacity)
        .length(stream.length)
        .buffering(stream.buffering)
        .access(stream.access)
        .storage(stream.storage);
        if let Some(initial) = stream.initial {
            let value = lower_stream_initial(&initial, &stream.typed.data_type)
                .map_err(|message| SourceDiagnostic::new("S0011", message, stream.span))?;
            draft = draft.initial(value);
        }
        builder = builder.stream(draft);
    }
    for kernel in module.kernels {
        let implementation = lower_kernel_implementation(&module_name, &kernel)?;
        let binding_order = kernel
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect::<Vec<_>>();
        let mut draft = KernelDraft::new(kernel.name);
        for parameter in kernel.parameters {
            let slot = match parameter.resource {
                ResourceKind::Stream => SlotDraft::stream(
                    parameter.name,
                    parameter.typed.data_type,
                    parameter.typed.unit,
                    parameter.access,
                ),
                ResourceKind::Value => SlotDraft::value(
                    parameter.name,
                    parameter.typed.data_type,
                    parameter.typed.unit,
                ),
            };
            draft = draft.slot(slot);
        }
        draft = draft
            .abi(KernelAbiDraft::new(binding_order))
            .implementation(implementation);
        builder = builder.kernel(draft);
    }
    for pass in module.passes {
        let mut draft = PassDraft::new(pass.name, pass.kernel).dispatch_over(pass.domain);
        for (slot, resource) in pass.bindings {
            draft = draft.bind(slot, resource);
        }
        builder = builder.pass(draft);
    }
    for view in module.views {
        let mut draft = ViewDraft::render(view.name, metal_implementation(view.source, view.entry));
        for (binding, stream) in view.reads {
            draft = draft.read(binding, stream);
        }
        builder = builder.view(draft);
    }
    for flow in module.flows {
        let mut passes = flow.passes.into_iter();
        let first = passes
            .next()
            .ok_or_else(|| SourceDiagnostic::new("S0020", "flow cannot be empty", module.span))?;
        let mut schedule = ScheduleDraft::fixed(flow.name, flow.rate_hz).run(first.clone());
        let mut previous = first;
        for pass in passes {
            schedule = schedule.run_after(pass.clone(), previous);
            previous = pass;
        }
        if let Some((view, producer)) = flow.presentation {
            schedule = schedule.show_after(view, producer);
        }
        builder = builder.schedule(schedule);
    }

    builder.build().map_err(|diagnostics| {
        let messages = diagnostics
            .into_iter()
            .map(|diagnostic| {
                format!(
                    "{:?} at {}: {}",
                    diagnostic.code,
                    diagnostic.primary.0.join("."),
                    diagnostic.message
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        SourceDiagnostic::new("S0099", messages, module.span)
    })
}

fn lower_kernel_implementation(
    module_name: &str,
    kernel: &KernelAst,
) -> Result<loom_core::BackendImplementation, SourceDiagnostic> {
    match &kernel.implementation {
        KernelImplementationAst::ExternalMetal { source, entry } => {
            Ok(metal_implementation(source, entry))
        }
        KernelImplementationAst::Native { index, statements } => {
            generate_metal(module_name, kernel, index, statements)
        }
    }
}

#[derive(Clone)]
struct CompiledExpr {
    typed: TypedAst,
    metal: String,
}

fn generate_metal(
    module_name: &str,
    kernel: &KernelAst,
    index: &str,
    statements: &[StatementAst],
) -> Result<loom_core::BackendImplementation, SourceDiagnostic> {
    if !is_simple_identifier(module_name)
        || !is_simple_identifier(&kernel.name)
        || !is_simple_identifier(index)
    {
        return Err(SourceDiagnostic::new(
            "M0001",
            "native module, kernel, and index names must be simple identifiers",
            kernel.span,
        ));
    }

    let parameters = kernel
        .parameters
        .iter()
        .map(|parameter| (parameter.name.clone(), parameter))
        .collect::<BTreeMap<_, _>>();
    if parameters.len() != kernel.parameters.len()
        || kernel
            .parameters
            .iter()
            .any(|parameter| !is_simple_identifier(&parameter.name))
    {
        return Err(SourceDiagnostic::new(
            "M0002",
            "native kernel parameter names must be unique simple identifiers",
            kernel.span,
        ));
    }

    let entry = format!("{}_main", kernel.name);
    let mut source = String::from("#include <metal_stdlib>\nusing namespace metal;\n\n");
    source.push_str(&format!("kernel void {entry}(\n"));
    for (buffer, parameter) in kernel.parameters.iter().enumerate() {
        let storage_type = metal_storage_type(&parameter.typed.data_type)
            .map_err(|message| SourceDiagnostic::new("M0003", message, kernel.span))?;
        let declaration = match parameter.resource {
            ResourceKind::Stream => {
                let qualifier = if parameter.access == SlotAccess::Read {
                    "const device"
                } else {
                    "device"
                };
                format!(
                    "    {qualifier} {storage_type} *{} [[buffer({buffer})]],\n",
                    parameter.name
                )
            }
            ResourceKind::Value => format!(
                "    constant {storage_type} &{} [[buffer({buffer})]],\n",
                parameter.name
            ),
        };
        source.push_str(&declaration);
    }
    source.push_str(&format!(
        "    uint {index} [[thread_position_in_grid]])\n{{\n"
    ));

    for statement in statements {
        let Some(parameter) = parameters.get(&statement.target).copied() else {
            return Err(SourceDiagnostic::new(
                "T0001",
                format!("unknown assignment target `{}`", statement.target),
                statement.span,
            ));
        };
        if !matches!(parameter.resource, ResourceKind::Stream) {
            return Err(SourceDiagnostic::new(
                "T0002",
                format!("`{}` is a value and cannot be indexed", statement.target),
                statement.span,
            ));
        }
        if !parameter.access.writes() {
            return Err(SourceDiagnostic::new(
                "T0003",
                format!("`{}` does not declare write access", statement.target),
                statement.span,
            ));
        }
        if statement.index != index {
            return Err(SourceDiagnostic::new(
                "T0004",
                format!(
                    "kernel index is `{index}`, but assignment uses `{}`",
                    statement.index
                ),
                statement.span,
            ));
        }
        let compiled = compile_expression(&statement.value, &parameters, index)?;
        if compiled.typed.data_type != parameter.typed.data_type
            || compiled.typed.unit != parameter.typed.unit
        {
            return Err(SourceDiagnostic::new(
                "T0005",
                format!(
                    "assignment to `{}` has {:?} {:?}, but its expression has {:?} {:?}",
                    statement.target,
                    parameter.typed.data_type,
                    parameter.typed.unit,
                    compiled.typed.data_type,
                    compiled.typed.unit
                ),
                statement.span,
            ));
        }
        let value = metal_store_expression(&parameter.typed.data_type, &compiled.metal)
            .map_err(|message| SourceDiagnostic::new("M0004", message, statement.span))?;
        source.push_str(&format!(
            "    {}[{}] = {};\n",
            statement.target, statement.index, value
        ));
    }
    source.push_str("}\n");

    Ok(packaged_metal_implementation(
        format!("loom://generated/{module_name}/{}.metal", kernel.name),
        entry,
        source,
    ))
}

fn compile_expression(
    expression: &ExprAst,
    parameters: &BTreeMap<String, &ParameterAst>,
    kernel_index: &str,
) -> Result<CompiledExpr, SourceDiagnostic> {
    match &expression.kind {
        ExprKind::Name(name) => {
            if name == kernel_index {
                return Ok(CompiledExpr {
                    typed: TypedAst {
                        data_type: DataType::u32(),
                        unit: Unit::DIMENSIONLESS,
                    },
                    metal: name.clone(),
                });
            }
            let Some(parameter) = parameters.get(name).copied() else {
                return Err(SourceDiagnostic::new(
                    "T0010",
                    format!("unknown name `{name}`"),
                    expression.span,
                ));
            };
            if matches!(parameter.resource, ResourceKind::Stream) {
                return Err(SourceDiagnostic::new(
                    "T0011",
                    format!("stream `{name}` must be indexed"),
                    expression.span,
                ));
            }
            Ok(CompiledExpr {
                typed: parameter.typed.clone(),
                metal: metal_load_expression(&parameter.typed.data_type, name)
                    .map_err(|message| SourceDiagnostic::new("M0010", message, expression.span))?,
            })
        }
        ExprKind::Index { resource, index } => {
            let Some(parameter) = parameters.get(resource).copied() else {
                return Err(SourceDiagnostic::new(
                    "T0010",
                    format!("unknown stream `{resource}`"),
                    expression.span,
                ));
            };
            if !matches!(parameter.resource, ResourceKind::Stream) {
                return Err(SourceDiagnostic::new(
                    "T0012",
                    format!("value `{resource}` cannot be indexed"),
                    expression.span,
                ));
            }
            if !parameter.access.reads() {
                return Err(SourceDiagnostic::new(
                    "T0013",
                    format!("stream `{resource}` does not declare read access"),
                    expression.span,
                ));
            }
            if index != kernel_index {
                return Err(SourceDiagnostic::new(
                    "T0014",
                    format!("kernel index is `{kernel_index}`, but expression uses `{index}`"),
                    expression.span,
                ));
            }
            let value = format!("{resource}[{index}]");
            Ok(CompiledExpr {
                typed: parameter.typed.clone(),
                metal: metal_load_expression(&parameter.typed.data_type, &value)
                    .map_err(|message| SourceDiagnostic::new("M0010", message, expression.span))?,
            })
        }
        ExprKind::Number(number) => {
            number.parse::<f32>().map_err(|_| {
                SourceDiagnostic::new(
                    "T0015",
                    format!("`{number}` is not a valid f32"),
                    expression.span,
                )
            })?;
            Ok(CompiledExpr {
                typed: TypedAst {
                    data_type: DataType::f32(),
                    unit: Unit::DIMENSIONLESS,
                },
                metal: number.clone(),
            })
        }
        ExprKind::Binary {
            operator,
            left,
            right,
        } => {
            let left = compile_expression(left, parameters, kernel_index)?;
            let right = compile_expression(right, parameters, kernel_index)?;
            let typed = check_binary_types(*operator, &left.typed, &right.typed)
                .map_err(|message| SourceDiagnostic::new("T0020", message, expression.span))?;
            Ok(CompiledExpr {
                typed,
                metal: format!("({} {} {})", left.metal, operator.metal(), right.metal),
            })
        }
    }
}

fn check_binary_types(
    operator: BinaryOperator,
    left: &TypedAst,
    right: &TypedAst,
) -> Result<TypedAst, String> {
    match operator {
        BinaryOperator::Add | BinaryOperator::Subtract => {
            if left.data_type != right.data_type || left.unit != right.unit {
                return Err(format!(
                    "`{}` requires matching types and units; found {:?} {:?} and {:?} {:?}",
                    operator.metal(),
                    left.data_type,
                    left.unit,
                    right.data_type,
                    right.unit
                ));
            }
            Ok(left.clone())
        }
        BinaryOperator::Multiply | BinaryOperator::Divide => {
            let data_type = arithmetic_data_type(operator, &left.data_type, &right.data_type)
                .ok_or_else(|| {
                    format!(
                        "`{}` does not support {:?} and {:?}",
                        operator.metal(),
                        left.data_type,
                        right.data_type
                    )
                })?;
            let unit = combine_units(left.unit, right.unit, operator)?;
            Ok(TypedAst { data_type, unit })
        }
    }
}

fn arithmetic_data_type(
    operator: BinaryOperator,
    left: &DataType,
    right: &DataType,
) -> Option<DataType> {
    let f32_scalar = DataType::f32();
    match (left, right) {
        (left, right) if *left == f32_scalar && *right == f32_scalar => Some(f32_scalar),
        (
            DataType::Vector {
                scalar: ScalarType::F32,
                ..
            },
            right,
        ) if *right == f32_scalar => Some(left.clone()),
        (
            left,
            DataType::Vector {
                scalar: ScalarType::F32,
                ..
            },
        ) if *left == f32_scalar && matches!(operator, BinaryOperator::Multiply) => {
            Some(right.clone())
        }
        (
            DataType::Vector {
                scalar: ScalarType::F32,
                lanes: left_lanes,
            },
            DataType::Vector {
                scalar: ScalarType::F32,
                lanes: right_lanes,
            },
        ) if left_lanes == right_lanes => Some(left.clone()),
        _ => None,
    }
}

fn combine_units(left: Unit, right: Unit, operator: BinaryOperator) -> Result<Unit, String> {
    let sign = if matches!(operator, BinaryOperator::Multiply) {
        1
    } else {
        -1
    };
    let combine = |left: i8, right: i8| {
        let right = right
            .checked_mul(sign)
            .ok_or_else(|| "physical unit exponent overflow".to_owned())?;
        left.checked_add(right)
            .ok_or_else(|| "physical unit exponent overflow".to_owned())
    };
    Ok(Unit {
        length: combine(left.length, right.length)?,
        mass: combine(left.mass, right.mass)?,
        time: combine(left.time, right.time)?,
        scale10: combine(left.scale10, right.scale10)?,
    })
}

fn metal_storage_type(data_type: &DataType) -> Result<String, String> {
    match data_type {
        DataType::Scalar(ScalarType::F32) => Ok("float".to_owned()),
        DataType::Vector {
            scalar: ScalarType::F32,
            lanes,
        } => Ok(format!("packed_float{lanes}")),
        _ => Err(format!(
            "native Metal generation currently supports only f32 scalars and vectors, found {data_type:?}"
        )),
    }
}

fn metal_load_expression(data_type: &DataType, value: &str) -> Result<String, String> {
    match data_type {
        DataType::Scalar(ScalarType::F32) => Ok(value.to_owned()),
        DataType::Vector {
            scalar: ScalarType::F32,
            lanes,
        } => Ok(format!("float{lanes}({value})")),
        _ => Err(format!(
            "cannot load native expression of type {data_type:?}"
        )),
    }
}

fn metal_store_expression(data_type: &DataType, value: &str) -> Result<String, String> {
    match data_type {
        DataType::Scalar(ScalarType::F32) => Ok(value.to_owned()),
        DataType::Vector {
            scalar: ScalarType::F32,
            lanes,
        } => Ok(format!("packed_float{lanes}({value})")),
        _ => Err(format!(
            "cannot store native expression of type {data_type:?}"
        )),
    }
}

fn is_simple_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn lower_stream_initial(raw: &RawLiteral, data_type: &DataType) -> Result<Literal, String> {
    let RawLiteral::Array(items) = raw else {
        return Err("stream `init` must be an array of elements".to_owned());
    };
    items
        .iter()
        .map(|item| lower_value(item, data_type))
        .collect::<Result<Vec<_>, _>>()
        .map(Literal::Array)
}

fn lower_value(raw: &RawLiteral, data_type: &DataType) -> Result<Literal, String> {
    match data_type {
        DataType::Scalar(ScalarType::Bool) => match raw {
            RawLiteral::Bool(value) => Ok(Literal::Bool(*value)),
            _ => Err("expected a boolean literal".to_owned()),
        },
        DataType::Scalar(ScalarType::I32) => match raw {
            RawLiteral::Number(value) => value
                .parse::<i32>()
                .map(Literal::I32)
                .map_err(|_| format!("`{value}` is not a valid i32")),
            _ => Err("expected an i32 literal".to_owned()),
        },
        DataType::Scalar(ScalarType::U32) => match raw {
            RawLiteral::Number(value) => value
                .parse::<u32>()
                .map(Literal::U32)
                .map_err(|_| format!("`{value}` is not a valid u32")),
            _ => Err("expected a u32 literal".to_owned()),
        },
        DataType::Scalar(ScalarType::F32) => match raw {
            RawLiteral::Number(value) => value
                .parse::<f32>()
                .map(Literal::f32)
                .map_err(|_| format!("`{value}` is not a valid f32")),
            _ => Err("expected an f32 literal".to_owned()),
        },
        DataType::Vector { scalar, lanes } => {
            let RawLiteral::Array(items) = raw else {
                return Err(format!("expected a {lanes}-element vector literal"));
            };
            if items.len() != *lanes as usize {
                return Err(format!(
                    "expected {lanes} vector elements, found {}",
                    items.len()
                ));
            }
            let element_type = DataType::Scalar(scalar.clone());
            items
                .iter()
                .map(|item| lower_value(item, &element_type))
                .collect::<Result<Vec<_>, _>>()
                .map(Literal::Vector)
        }
        DataType::Scalar(ScalarType::F16) => {
            Err("f16 source literals are not implemented yet".to_owned())
        }
        _ => Err("this source literal type is not implemented yet".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::parse;
    use loom_validator::Validator;

    const SOURCE: &str = include_str!("../../../examples/hello-particle/hello-particle.agent.loom");

    #[test]
    fn agent_source_lowers_to_a_valid_graph() {
        let graph = parse(SOURCE).expect("source should parse");
        let report = Validator::validate(&graph);
        assert!(
            report.is_valid(),
            "unexpected diagnostics: {:#?}",
            report.diagnostics
        );
        assert_eq!(graph.name, "hello_particle_agent");
        assert_eq!(graph.resources.streams.len(), 6);
        assert_eq!(graph.kernels.len(), 2);
        assert_eq!(graph.passes.len(), 2);
        assert_eq!(graph.views.len(), 1);
    }

    #[test]
    fn reports_a_stable_source_location() {
        let diagnostics = parse("loom 0.1\nmodule bad\ntarget metal\nwat nope")
            .expect_err("unknown declaration should fail");
        assert_eq!(diagnostics[0].code, "P0003");
        assert_eq!(diagnostics[0].span.start.line, 4);
        assert_eq!(diagnostics[0].span.start.column, 1);
    }

    #[test]
    fn native_kernel_units_are_checked_before_metal_generation() {
        let invalid = SOURCE.replace("gravity * dt", "gravity + dt");
        let diagnostics = parse(&invalid).expect_err("incompatible units must fail");
        assert_eq!(diagnostics[0].code, "T0020");
        assert!(diagnostics[0].message.contains("matching types and units"));
        assert_eq!(diagnostics[0].span.start.line, 47);
    }

    #[test]
    fn subtraction_is_distinct_from_a_negative_literal() {
        let source = SOURCE.replace("gravity * dt", "velocity[i] - gravity * dt");
        parse(&source).expect("binary subtraction should remain valid");
    }
}
