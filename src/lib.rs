mod test;

use std::collections::BTreeMap;

// Handles Tag parsing, attributes, text, CDATA, comments, PI.

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Element {
        tag: String,
        attributes: BTreeMap<String, String>, // sorted -> formatt
        children: Vec<Node>,
    },
    Text(String),
    Comment(String),
    CData(String),
    ProcessingInstruction { target: String, content: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    TagOpen(String),
    TagClose(String),
    SelfClosingEnd,
    TagEnd,
    Attribute(String, String),
    Text(String),
    Comment(String),
    CData(String),
    ProcessingInstruction(String, String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, PartialEq)]
pub enum XmlError {
    Syntax { line: usize, col: usize, message: String },
    MismatchedTag { line: usize, col: usize, expected: String, found: String },
}
// Error print
impl std::fmt::Display for XmlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XmlError::Syntax { line, col, message } => {
                write!(f, "Syntax Error at {}:{}: {}", line, col, message)
            }
            XmlError::MismatchedTag { line, col, expected, found } => write!(
                f,
                "Tag Error at {}:{}: Expected </{}>, found </{}>",
                line, col, expected, found
            ),
        }
    }
}

//acts as a cursor for interpreter, sorta like vim ig?
struct Scanner<'a> {
    input: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    line: usize,
    col: usize,
}

impl<'a> Scanner<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, chars: input.char_indices().peekable(), line: 1, col: 1 }
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().map(|&(_, c)| c)
    }

    fn peek_byte_pos(&mut self) -> Option<usize> {
        self.chars.peek().map(|&(i, _)| i)
    }

    // Consumes char, updating line/col, and return char.
    fn advance(&mut self) -> Option<char> {
        let (_, c) = self.chars.next()?;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn skip_to_byte(&mut self, target: usize) {
        while let Some(pos) = self.peek_byte_pos() {
            if pos >= target {
                break;
            }
            self.advance();
        }
    }

    fn starts_with(&self, at: usize, pat: &str) -> bool {
        self.input[at..].starts_with(pat)
    }

    fn find_from(&self, at: usize, pat: &str) -> Option<usize> {
        self.input[at..].find(pat).map(|off| at + off)
    }
}

pub fn tokenize(input: &str) -> Result<Vec<Token>, XmlError> {
    let mut tokens = Vec::new();
    let mut sc = Scanner::new(input);

    while let Some(ch) = sc.peek() {
        let (cur_line, cur_col) = (sc.line, sc.col);
        let i = sc.peek_byte_pos().unwrap();

        if ch != '<' {
            // node = everything up to the next '<'.
            let mut text = String::new();
            while let Some(c) = sc.peek() {
                if c == '<' {
                    break;
                }
                text.push(c);
                sc.advance();
            }
            if !text.is_empty() {
                tokens.push(Token { kind: TokenKind::Text(text), line: cur_line, column: cur_col });
            }
            continue;
        }

        sc.advance(); // consume '<'
        //comment
        if sc.starts_with(i, "<!--") {
            for _ in 0..3 { sc.advance(); } // consume "!--"
            let start = i + 4;
            let end = sc.find_from(start, "-->").ok_or_else(|| XmlError::Syntax {
                line: cur_line, col: cur_col, message: "Unclosed comment".into(),
            })?;
            let content = input[start..end].to_string();
            sc.skip_to_byte(end + 3);
            tokens.push(Token { kind: TokenKind::Comment(content), line: cur_line, column: cur_col });
        //CDATA
        } else if sc.starts_with(i, "<![CDATA[") {
            for _ in 0..8 { sc.advance(); } // consume "![CDATA["
            let start = i + 9;
            let end = sc.find_from(start, "]]>").ok_or_else(|| XmlError::Syntax {
                line: cur_line, col: cur_col, message: "Unclosed CDATA".into(),
            })?;
            let content = input[start..end].to_string();
            sc.skip_to_byte(end + 3);
            tokens.push(Token { kind: TokenKind::CData(content), line: cur_line, column: cur_col });
        //Processing instructions
        } else if sc.starts_with(i, "<?") {
            sc.advance(); // consume '?'
            let start = i + 2;
            let end = sc.find_from(start, "?>").ok_or_else(|| XmlError::Syntax {
                line: cur_line, col: cur_col, message: "Unclosed PI".into(),
            })?;

            // output ProcessingInstruction for scanned <?...?>
            let raw = &input[start..end];
            let mut parts = raw.splitn(2, char::is_whitespace);
            let target = parts.next().unwrap_or("").to_string();
            let content = parts.next().unwrap_or("").trim().to_string();

            sc.skip_to_byte(end + 2);
            tokens.push(Token {
                kind: TokenKind::ProcessingInstruction(target, content),
                line: cur_line,
                column: cur_col,
            });
        //custom elements in the xml
        } else if sc.starts_with(i, "</") {
            sc.advance(); // consume '/'
            let mut tag = String::new();
            while let Some(c) = sc.peek() {
                if c == '>' { break; }
                tag.push(c);
                sc.advance();
            }
            if sc.advance() == Some('>') {
                tokens.push(Token {
                    kind: TokenKind::TagClose(tag.trim().to_string()),
                    line: cur_line,
                    column: cur_col,
                });
            }
        } else {
            // Opening tag + attributes.
            let mut name = String::new();
            while let Some(c) = sc.peek() {
                if c.is_whitespace() || c == '>' || c == '/' { break; }
                name.push(c);
                sc.advance();
            }
            tokens.push(Token { kind: TokenKind::TagOpen(name), line: cur_line, column: cur_col });
            //does syntax
            loop {
                while let Some(c) = sc.peek() {
                    if !c.is_whitespace() { break; }
                    sc.advance();
                }

                match sc.peek() {
                    Some('>') => {
                        sc.advance();
                        tokens.push(Token { kind: TokenKind::TagEnd, line: cur_line, column: cur_col });
                        break;
                    }
                    Some('/') => {
                        sc.advance();
                        if sc.peek() == Some('>') {
                            sc.advance();
                            tokens.push(Token { kind: TokenKind::SelfClosingEnd, line: cur_line, column: cur_col });
                        }
                        break;
                    }
                    Some(_) => {
                        let mut key = String::new();
                        while let Some(k) = sc.peek() {
                            if k == '=' || k.is_whitespace() || k == '>' || k == '/' { break; }
                            key.push(k);
                            sc.advance();
                        }
                        while let Some(w) = sc.peek() {
                            if !w.is_whitespace() { break; }
                            sc.advance();
                        }
                        if sc.peek() == Some('=') {
                            sc.advance();
                            while let Some(w) = sc.peek() {
                                if !w.is_whitespace() { break; }
                                sc.advance();
                            }
                            let quote = sc.advance().ok_or_else(|| XmlError::Syntax {
                                line: cur_line, col: cur_col, message: "Expected quote".into(),
                            })?;
                            let mut val = String::new();
                            while let Some(v) = sc.advance() {
                                if v == quote { break; }
                                val.push(v);
                            }
                            tokens.push(Token { kind: TokenKind::Attribute(key, val), line: cur_line, column: cur_col });
                        }
                    }
                    None => break,
                }
            }
        }
    }

    Ok(tokens)
}

pub fn parse(tokens: &[Token]) -> Result<Vec<Node>, XmlError> {
    let mut stack: Vec<(String, BTreeMap<String, String>, Vec<Node>)> = Vec::new();
    let mut root_nodes = Vec::new();
    let mut iter = tokens.iter().peekable();

    fn push_node(stack: &mut Vec<(String, BTreeMap<String, String>, Vec<Node>)>, root: &mut Vec<Node>, node: Node) {
        if let Some(parent) = stack.last_mut() {
            parent.2.push(node);
        } else {
            root.push(node);
        }
    }

    while let Some(tok) = iter.next() {
        match &tok.kind {
            TokenKind::TagOpen(tag) => {
                let mut attrs = BTreeMap::new();
                let mut self_closing = false;

                while let Some(&next) = iter.peek() {
                    match &next.kind {
                        TokenKind::Attribute(k, v) => {
                            attrs.insert(k.clone(), v.clone());
                            iter.next();
                        }
                        TokenKind::TagEnd => { iter.next(); break; }
                        TokenKind::SelfClosingEnd => { iter.next(); self_closing = true; break; }
                        _ => break,
                    }
                }

                if self_closing {
                    push_node(&mut stack, &mut root_nodes, Node::Element {
                        tag: tag.clone(), attributes: attrs, children: Vec::new(),
                    });
                } else {
                    stack.push((tag.clone(), attrs, Vec::new()));
                }
            }
            TokenKind::TagClose(tag) => {
                let (open_tag, attrs, children) = stack.pop().ok_or_else(|| XmlError::MismatchedTag {
                    line: tok.line, col: tok.column, expected: "None".into(), found: tag.clone(),
                })?;

                if &open_tag != tag {
                    return Err(XmlError::MismatchedTag {
                        line: tok.line, col: tok.column, expected: open_tag, found: tag.clone(),
                    });
                }

                push_node(&mut stack, &mut root_nodes, Node::Element { tag: open_tag, attributes: attrs, children });
            }
            TokenKind::Text(t) => push_node(&mut stack, &mut root_nodes, Node::Text(t.clone())),
            TokenKind::Comment(c) => push_node(&mut stack, &mut root_nodes, Node::Comment(c.clone())),
            TokenKind::CData(c) => push_node(&mut stack, &mut root_nodes, Node::CData(c.clone())),
            TokenKind::ProcessingInstruction(target, content) => push_node(
                &mut stack, &mut root_nodes,
                Node::ProcessingInstruction { target: target.clone(), content: content.clone() },
            ),
            _ => {}
        }
    }

    Ok(root_nodes)
}
// formatter :D
pub fn pretty_print(nodes: &[Node], indent_level: usize) -> String {
    let mut out = String::new();
    let indent = "  ".repeat(indent_level);

    for node in nodes {
        match node {
            Node::Element { tag, attributes, children } => {
                let attr_str: String = attributes
                    .iter()
                    .map(|(k, v)| format!(" {}=\"{}\"", k, escape_attr(v)))
                    .collect();

                match children.as_slice() {
                    [] => out.push_str(&format!("{}<{}{} />\n", indent, tag, attr_str)),
                    [Node::Text(t)] => {
                        out.push_str(&format!("{}<{}{}>{}</{}>\n", indent, tag, attr_str, escape_attr(t.trim()), tag));
                    }
                    _ => {
                        out.push_str(&format!("{}<{}{}>\n", indent, tag, attr_str));
                        out.push_str(&pretty_print(children, indent_level + 1));
                        out.push_str(&format!("{}</{}>\n", indent, tag));
                    }
                }
            }
            Node::Text(t) => {
                let trimmed = t.trim();
                if !trimmed.is_empty() {
                    out.push_str(&format!("{}{}\n", indent, escape_attr(trimmed)));
                }
            }
            Node::Comment(c) => out.push_str(&format!("{}<!--{}-->\n", indent, c)),
            Node::CData(c) => out.push_str(&format!("{}<![CDATA[{}]]>\n", indent, c)),
            Node::ProcessingInstruction { target, content } => {
                out.push_str(&format!("{}<?{} {}?>\n", indent, target, content))
            }
        }
    }
    out
}

// stops xml from picking them up as syntax statements instead of text
fn escape_attr(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

impl Node {
    pub fn find_by_tag<'a>(&'a self, target_tag: &str) -> Vec<&'a Node> {
        let mut results = Vec::new();
        if let Node::Element { tag, children, .. } = self {
            if tag == target_tag {
                results.push(self);
            }
            for child in children {
                results.extend(child.find_by_tag(target_tag));
            }
        }
        results
    }

    pub fn text_content(&self) -> String {
        match self {
            Node::Text(t) => t.clone(),
            Node::CData(c) => c.clone(),
            Node::Element { children, .. } => {
                children.iter().map(|c| c.text_content()).collect::<Vec<_>>().join("")
            }
            _ => String::new(),
        }
    }
}

pub fn stream_tokens(input: &str, mut callback: impl FnMut(Token)) -> Result<(), XmlError> {
    let tokens = tokenize(input)?;
    for token in tokens {
        callback(token);
    }
    Ok(())
}