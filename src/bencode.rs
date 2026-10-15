use std::collections::BTreeMap;
use std::str;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Int(i64),
    Bytes(Vec<u8>),
    List(Vec<Value>),
    Dict(BTreeMap<Vec<u8>, Value>),
}

impl Value {
    pub fn str(s: impl Into<String>) -> Self {
        Value::Bytes(s.into().into_bytes())
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Bytes(bytes) => str::from_utf8(bytes).ok(),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(number) => Some(*number),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Bytes(bytes) => Some(bytes.as_slice()),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(items) => Some(items.as_slice()),
            _ => None,
        }
    }

    pub fn as_dict(&self) -> Option<&BTreeMap<Vec<u8>, Value>> {
        match self {
            Value::Dict(dict) => Some(dict),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_dict().and_then(|dict| dict.get(key.as_bytes()))
    }
}

pub fn dict<I, K>(entries: I) -> Value
where
    I: IntoIterator<Item = (K, Value)>,
    K: Into<String>,
{
    let mut map = BTreeMap::new();
    for (k, v) in entries {
        map.insert(k.into().into_bytes(), v);
    }
    Value::Dict(map)
}

pub fn encode(value: &Value) -> Vec<u8> {
    let mut output = Vec::with_capacity(64);
    encode_into(value, &mut output);
    output
}

pub fn encode_into(value: &Value, output: &mut Vec<u8>) {
    match value {
        Value::Int(number) => {
            output.push(b'i');
            output.extend_from_slice(number.to_string().as_bytes());
            output.push(b'e');
        }
        Value::Bytes(bytes) => {
            output.extend_from_slice(bytes.len().to_string().as_bytes());
            output.push(b':');
            output.extend_from_slice(bytes);
        }
        Value::List(items) => {
            output.push(b'l');
            for item in items {
                encode_into(item, output);
            }
            output.push(b'e');
        }
        Value::Dict(dict) => {
            output.push(b'd');
            for (key, val) in dict {
                output.extend_from_slice(key.len().to_string().as_bytes());
                output.push(b':');
                output.extend_from_slice(key);
                encode_into(val, output);
            }
            output.push(b'e');
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum DecodeOutcome {
    Value { value: Value, consumed: usize },
    Incomplete,
}

pub fn decode_one(input: &[u8]) -> Result<DecodeOutcome, String> {
    let mut parser = Parser { input, pos: 0 };
    match parser.parse_value() {
        Ok(value) => Ok(DecodeOutcome::Value {
            value,
            consumed: parser.pos,
        }),
        Err(ParseError::Incomplete) => Ok(DecodeOutcome::Incomplete),
        Err(ParseError::Malformed(message)) => Err(message),
    }
}

pub fn decode_available(buffer: &mut Vec<u8>) -> Result<Vec<Value>, String> {
    let mut values = Vec::new();
    let mut offset = 0;
    while offset < buffer.len() {
        match decode_one(&buffer[offset..])? {
            DecodeOutcome::Value { value, consumed } => {
                values.push(value);
                offset += consumed;
            }
            DecodeOutcome::Incomplete => break,
        }
    }
    if offset > 0 {
        buffer.drain(0..offset);
    }
    Ok(values)
}

#[derive(Debug)]
enum ParseError {
    Incomplete,
    Malformed(String),
}

struct Parser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Result<u8, ParseError> {
        self.input
            .get(self.pos)
            .copied()
            .ok_or(ParseError::Incomplete)
    }

    fn bump(&mut self) -> Result<u8, ParseError> {
        let byte = self.peek()?;
        self.pos += 1;
        Ok(byte)
    }

    fn expect(&mut self, expected: u8) -> Result<(), ParseError> {
        let actual = self.bump()?;
        if actual != expected {
            return Err(ParseError::Malformed(format!(
                "expected {:?} at position {}, found {:?}",
                expected as char,
                self.pos - 1,
                actual as char
            )));
        }
        Ok(())
    }

    fn parse_value(&mut self) -> Result<Value, ParseError> {
        match self.peek()? {
            b'i' => self.parse_int(),
            b'l' => self.parse_list(),
            b'd' => self.parse_dict(),
            b'0'..=b'9' => self.parse_bytes().map(Value::Bytes),
            other => Err(ParseError::Malformed(format!(
                "unexpected byte {:?} at position {} (expected i/l/d/digit)",
                other as char, self.pos
            ))),
        }
    }

    fn parse_int(&mut self) -> Result<Value, ParseError> {
        self.expect(b'i')?;
        let start = self.pos;
        let mut end = None;
        for (i, byte) in self.input[start..].iter().enumerate() {
            if *byte == b'e' {
                end = Some(start + i);
                break;
            }
        }
        let end = end.ok_or(ParseError::Incomplete)?;
        let digits = &self.input[start..end];
        if digits.is_empty() {
            return Err(ParseError::Malformed(format!("empty integer at {start}")));
        }
        let text = str::from_utf8(digits)
            .map_err(|_| ParseError::Malformed(format!("non-ASCII integer at {start}")))?;
        let bytes = text.as_bytes();
        let invalid = match bytes {
            [b'-'] => true,
            [b'-', b'0'] => true,
            [b'0', _, ..] => true,
            [b'-', b'0', _, ..] => true,
            _ => false,
        };
        if invalid {
            return Err(ParseError::Malformed(format!(
                "non-canonical integer {text:?} at {start}"
            )));
        }
        let number: i64 = text
            .parse()
            .map_err(|error| ParseError::Malformed(format!("integer parse error: {error}")))?;
        self.pos = end + 1;
        Ok(Value::Int(number))
    }

    fn parse_bytes(&mut self) -> Result<Vec<u8>, ParseError> {
        let start = self.pos;
        let mut colon = None;
        for (i, byte) in self.input[start..].iter().enumerate() {
            if *byte == b':' {
                colon = Some(start + i);
                break;
            }
            if !byte.is_ascii_digit() {
                return Err(ParseError::Malformed(format!(
                    "non-digit {:?} in byte-string length at {}",
                    *byte as char,
                    start + i
                )));
            }
        }
        let colon = colon.ok_or(ParseError::Incomplete)?;
        let len_digits = &self.input[start..colon];
        if len_digits.is_empty() {
            return Err(ParseError::Malformed(format!(
                "missing byte-string length at {start}"
            )));
        }
        if len_digits.len() > 1 && len_digits[0] == b'0' {
            return Err(ParseError::Malformed(format!(
                "non-canonical byte-string length at {start}"
            )));
        }
        let len: usize = str::from_utf8(len_digits)
            .ok()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| {
                ParseError::Malformed(format!("invalid byte-string length at {start}"))
            })?;
        let body_start = colon + 1;
        let body_end = body_start
            .checked_add(len)
            .ok_or_else(|| ParseError::Malformed("byte-string length overflow".to_string()))?;
        if body_end > self.input.len() {
            return Err(ParseError::Incomplete);
        }
        let bytes = self.input[body_start..body_end].to_vec();
        self.pos = body_end;
        Ok(bytes)
    }

    fn parse_list(&mut self) -> Result<Value, ParseError> {
        self.expect(b'l')?;
        let mut items = Vec::new();
        loop {
            if self.peek()? == b'e' {
                self.pos += 1;
                return Ok(Value::List(items));
            }
            items.push(self.parse_value()?);
        }
    }

    fn parse_dict(&mut self) -> Result<Value, ParseError> {
        self.expect(b'd')?;
        let mut map = BTreeMap::new();
        loop {
            if self.peek()? == b'e' {
                self.pos += 1;
                return Ok(Value::Dict(map));
            }
            let key = self.parse_bytes()?;
            let value = self.parse_value()?;
            if map.insert(key.clone(), value).is_some() {
                return Err(ParseError::Malformed(format!(
                    "duplicate dict key {:?}",
                    String::from_utf8_lossy(&key)
                )));
            }
        }
    }
}
