use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub uri: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Debug)]
pub enum ParseError {
    Incomplete,
    BadRequest(String),
    BodyTooLarge,
}

impl HttpRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(&name.to_ascii_lowercase()).map(|s| s.as_str())
    }

    pub fn host_name(&self) -> Option<String> {
        self.header("host").map(|h| {
            h.split(':').next().unwrap_or(h).trim().to_ascii_lowercase()
        })
    }

    pub fn cookie(&self, name: &str) -> Option<String> {
        let raw = self.header("cookie")?;
        for part in raw.split(';') {
            let mut kv = part.trim().splitn(2, '=');
            if kv.next()? == name {
                return kv.next().map(|v| v.to_string());
            }
        }
        None
    }
}

pub struct Parser {
    buffer: Vec<u8>,
    max_body: usize,
}

impl Parser {
    pub fn new(max_body: usize) -> Self {
        Self {
            buffer: Vec::new(),
            max_body,
        }
    }

    pub fn feed(&mut self, data: &[u8]) -> Result<Option<HttpRequest>, ParseError> {
        if self.buffer.len() + data.len() > self.max_body.saturating_add(65536) {
            return Err(ParseError::BodyTooLarge);
        }
        self.buffer.extend_from_slice(data);
        self.try_parse()
    }

    fn try_parse(&mut self) -> Result<Option<HttpRequest>, ParseError> {
        let header_end = self
            .buffer
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .map(|i| i + 4);
        let header_end = match header_end {
            Some(e) => e,
            None => return Ok(None),
        };

        let header_bytes = &self.buffer[..header_end];
        let header_text = std::str::from_utf8(header_bytes)
            .map_err(|_| ParseError::BadRequest("invalid utf-8".into()))?;
        let mut lines = header_text.split("\r\n");
        let request_line = lines.next().ok_or_else(|| ParseError::BadRequest("empty".into()))?;
        let mut parts = request_line.split_whitespace();
        let method = parts
            .next()
            .ok_or_else(|| ParseError::BadRequest("no method".into()))?
            .to_string();
        let uri = parts
            .next()
            .ok_or_else(|| ParseError::BadRequest("no uri".into()))?
            .to_string();
        let version = parts
            .next()
            .ok_or_else(|| ParseError::BadRequest("no version".into()))?
            .to_string();

        let mut headers = HashMap::new();
        for line in lines {
            if line.is_empty() {
                continue;
            }
            if let Some((k, v)) = line.split_once(':') {
                headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
            }
        }

        let is_chunked = headers
            .get("transfer-encoding")
            .map(|t| t.to_ascii_lowercase().contains("chunked"))
            .unwrap_or(false);

        let body = if is_chunked {
            match decode_chunked(&self.buffer[header_end..]) {
                Ok(b) => b,
                Err(ParseError::Incomplete) => return Ok(None),
                Err(e) => return Err(e),
            }
        } else {
            let body_len = content_length(&headers)?;
            if body_len > self.max_body {
                return Err(ParseError::BodyTooLarge);
            }
            let total = header_end + body_len;
            if self.buffer.len() < total {
                return Ok(None);
            }
            self.buffer[header_end..total].to_vec()
        };

        if body.len() > self.max_body {
            return Err(ParseError::BodyTooLarge);
        }
        self.buffer.clear();

        Ok(Some(HttpRequest {
            method,
            uri,
            version,
            headers,
            body,
        }))
    }
}

fn content_length(headers: &HashMap<String, String>) -> Result<usize, ParseError> {
    if let Some(cl) = headers.get("content-length") {
        return cl
            .parse()
            .map_err(|_| ParseError::BadRequest("bad content-length".into()));
    }
    Ok(0)
}

/// Decode chunked body from raw bytes after headers (used when Transfer-Encoding: chunked).
pub fn decode_chunked(input: &[u8]) -> Result<Vec<u8>, ParseError> {
    let mut out = Vec::new();
    let mut pos = 0;
    loop {
        let rest = &input[pos..];
        let line_end = rest
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or(ParseError::Incomplete)?;
        let size_line = std::str::from_utf8(&rest[..line_end])
            .map_err(|_| ParseError::BadRequest("chunk size utf8".into()))?;
        let size = usize::from_str_radix(size_line.split(';').next().unwrap_or("").trim(), 16)
            .map_err(|_| ParseError::BadRequest("chunk size".into()))?;
        pos += line_end + 2;
        if size == 0 {
            break;
        }
        if input.len() < pos + size + 2 {
            return Err(ParseError::Incomplete);
        }
        out.extend_from_slice(&input[pos..pos + size]);
        pos += size + 2;
    }
    Ok(out)
}
