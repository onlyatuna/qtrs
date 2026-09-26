//! Backtracking regular-expression engine used for filtering and matching
//! (the subset of `QRegularExpression`/PCRE syntax item models rely on).
//!
//! Supported syntax: literals, `.`, character classes (`[a-z]`, `[^...]`,
//! POSIX `[:alpha:]` forms), escapes `\d \D \w \W \s \S \b \B \A \z \Z`,
//! `\n \t \r \f \v \0 \e \xHH \x{H..} \uHHHH`, anchors `^ $`, groups `( )`,
//! `(?: )`, inline `(?i)`, alternation `|`, and greedy/lazy quantifiers
//! `* + ? {n} {n,} {n,m}`. Back-references and look-around are rejected.

use std::fmt;

/// Error raised for an invalid pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternError {
    pub message: String,
    /// Character offset in the pattern where the error was detected.
    pub offset: usize,
}

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at offset {}", self.message, self.offset)
    }
}

impl std::error::Error for PatternError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharKind {
    Digit,
    Word,
    Space,
    Alpha,
    Alnum,
    Upper,
    Lower,
    Punct,
    XDigit,
}

impl CharKind {
    fn matches(self, c: char) -> bool {
        match self {
            CharKind::Digit => c.is_ascii_digit(),
            CharKind::Word => c.is_alphanumeric() || c == '_',
            CharKind::Space => c.is_whitespace(),
            CharKind::Alpha => c.is_alphabetic(),
            CharKind::Alnum => c.is_alphanumeric(),
            CharKind::Upper => c.is_uppercase(),
            CharKind::Lower => c.is_lowercase(),
            CharKind::Punct => c.is_ascii_punctuation(),
            CharKind::XDigit => c.is_ascii_hexdigit(),
        }
    }
}

#[derive(Debug, Clone)]
enum ClassItem {
    Range(char, char),
    Kind(CharKind, bool),
}

#[derive(Debug, Clone)]
struct ClassSet {
    negated: bool,
    items: Vec<ClassItem>,
}

#[derive(Debug, Clone)]
enum Node {
    Empty,
    Literal(char),
    Any,
    Class(ClassSet),
    LineStart,
    LineEnd,
    TextStart,
    TextEnd,
    WordBoundary,
    NotWordBoundary,
    Concat(Vec<Node>),
    Alternate(Vec<Node>),
    Repeat {
        node: Box<Node>,
        min: u32,
        max: Option<u32>,
        greedy: bool,
    },
}

impl Node {
    fn is_single_char(&self) -> bool {
        matches!(self, Node::Literal(_) | Node::Any | Node::Class(_))
    }

    fn starts_anchored(&self) -> bool {
        match self {
            Node::LineStart | Node::TextStart => true,
            Node::Concat(nodes) => nodes.first().is_some_and(Node::starts_anchored),
            Node::Alternate(branches) => branches.iter().all(Node::starts_anchored),
            _ => false,
        }
    }
}

/// A compiled regular expression.
#[derive(Debug, Clone)]
pub struct Regex {
    root: Node,
    case_insensitive: bool,
    pattern: String,
}

impl Regex {
    /// Compiles `pattern`.
    pub fn new(pattern: &str, case_insensitive: bool) -> Result<Self, PatternError> {
        let mut parser = Parser {
            chars: pattern.chars().collect(),
            pos: 0,
            case_insensitive,
        };
        let root = parser.parse_alternation()?;
        if parser.pos < parser.chars.len() {
            return Err(parser.error("unmatched ')'"));
        }
        Ok(Self {
            root,
            case_insensitive: parser.case_insensitive,
            pattern: pattern.to_string(),
        })
    }

    /// Compiles a shell wildcard (`*`, `?`, `[...]`). When `anchored`, the whole
    /// text must match; otherwise any substring may match.
    pub fn from_wildcard(
        wildcard: &str,
        anchored: bool,
        case_insensitive: bool,
    ) -> Result<Self, PatternError> {
        Self::new(&wildcard_to_regex(wildcard, anchored), case_insensitive)
    }

    /// Compiles `text` as a literal (every metacharacter escaped).
    pub fn literal(text: &str, case_insensitive: bool) -> Self {
        Self::new(&escape(text), case_insensitive).expect("escaped pattern is always valid")
    }

    /// Source pattern.
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    pub fn is_case_insensitive(&self) -> bool {
        self.case_insensitive
    }

    /// First match as a `(start, end)` range of *character* offsets.
    pub fn find(&self, text: &str) -> Option<(usize, usize)> {
        let chars: Vec<char> = text.chars().collect();
        let matcher = Matcher {
            text: &chars,
            case_insensitive: self.case_insensitive,
        };
        let last_start = if self.root.starts_anchored() {
            0
        } else {
            chars.len()
        };
        for start in 0..=last_start {
            let mut end = None;
            if matcher.match_node(&self.root, start, &mut |e| {
                end = Some(e);
                true
            }) {
                return end.map(|e| (start, e));
            }
        }
        None
    }

    /// `true` if some substring of `text` matches.
    pub fn is_match(&self, text: &str) -> bool {
        self.find(text).is_some()
    }

    /// `true` if all of `text` matches.
    pub fn is_full_match(&self, text: &str) -> bool {
        let chars: Vec<char> = text.chars().collect();
        let matcher = Matcher {
            text: &chars,
            case_insensitive: self.case_insensitive,
        };
        let len = chars.len();
        matcher.match_node(&self.root, 0, &mut |e| e == len)
    }
}

/// Escapes every character except `[A-Za-z0-9_]` (`QRegularExpression::escape`).
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        if c == '\0' {
            out.push_str("\\0");
        } else if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push('\\');
            out.push(c);
        }
    }
    out
}

/// Converts a shell wildcard into regex syntax
/// (`QRegularExpression::wildcardToRegularExpression`, non-path mode).
pub fn wildcard_to_regex(wildcard: &str, anchored: bool) -> String {
    let chars: Vec<char> = wildcard.chars().collect();
    let n = chars.len();
    let mut rx = String::with_capacity(n * 2 + 8);
    let mut i = 0;
    while i < n {
        let c = chars[i];
        match c {
            '*' => rx.push_str(".*"),
            '?' => rx.push('.'),
            '[' => {
                let mut j = i + 1;
                if j < n && (chars[j] == '!' || chars[j] == '^') {
                    j += 1;
                }
                if j < n && chars[j] == ']' {
                    j += 1;
                }
                while j < n && chars[j] != ']' {
                    j += 1;
                }
                if j >= n {
                    rx.push_str("\\[");
                } else {
                    rx.push('[');
                    let mut k = i + 1;
                    if chars[k] == '!' {
                        rx.push('^');
                        k += 1;
                    } else if chars[k] == '^' {
                        rx.push_str("\\^");
                        k += 1;
                    }
                    while k < j {
                        let ch = chars[k];
                        if ch == '\\' || ch == '[' {
                            rx.push('\\');
                        }
                        rx.push(ch);
                        k += 1;
                    }
                    rx.push(']');
                    i = j;
                }
            }
            c if c.is_ascii_alphanumeric() || c == '_' => rx.push(c),
            c => {
                rx.push('\\');
                rx.push(c);
            }
        }
        i += 1;
    }
    if anchored {
        format!("\\A(?:{rx})\\z")
    } else {
        rx
    }
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
    case_insensitive: bool,
}

enum Escaped {
    Char(char),
    Item(ClassItem),
    Node(Node),
}

impl Parser {
    fn error(&self, message: &str) -> PatternError {
        PatternError {
            message: message.to_string(),
            offset: self.pos,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn next(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn parse_alternation(&mut self) -> Result<Node, PatternError> {
        let mut branches = vec![self.parse_concat()?];
        while self.peek() == Some('|') {
            self.pos += 1;
            branches.push(self.parse_concat()?);
        }
        Ok(if branches.len() == 1 {
            branches.pop().expect("one branch")
        } else {
            Node::Alternate(branches)
        })
    }

    fn parse_concat(&mut self) -> Result<Node, PatternError> {
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.parse_atom()?;
            let node = self.parse_quantifier(atom)?;
            items.push(node);
        }
        Ok(match items.len() {
            0 => Node::Empty,
            1 => items.pop().expect("one item"),
            _ => Node::Concat(items),
        })
    }

    fn parse_atom(&mut self) -> Result<Node, PatternError> {
        let c = self
            .next()
            .ok_or_else(|| self.error("unexpected end of pattern"))?;
        match c {
            '(' => self.parse_group(),
            '[' => self.parse_class(),
            '.' => Ok(Node::Any),
            '^' => Ok(Node::LineStart),
            '$' => Ok(Node::LineEnd),
            '\\' => match self.parse_escape(false)? {
                Escaped::Char(ch) => Ok(Node::Literal(ch)),
                Escaped::Item(item) => Ok(Node::Class(ClassSet {
                    negated: false,
                    items: vec![item],
                })),
                Escaped::Node(node) => Ok(node),
            },
            '*' | '+' | '?' => {
                self.pos -= 1;
                Err(self.error("quantifier does not follow a repeatable item"))
            }
            '{' if self.quantifier_bounds_at(self.pos - 1).is_some() => {
                self.pos -= 1;
                Err(self.error("quantifier does not follow a repeatable item"))
            }
            c => Ok(Node::Literal(c)),
        }
    }

    fn parse_group(&mut self) -> Result<Node, PatternError> {
        if self.peek() == Some('?') {
            self.pos += 1;
            match self.peek() {
                Some(':') => {
                    self.pos += 1;
                }
                Some(c) if c.is_ascii_alphabetic() || c == '-' => {
                    let mut enable = true;
                    loop {
                        match self.next() {
                            Some('i') => self.case_insensitive = enable,
                            Some('-') => enable = false,
                            Some('m') | Some('s') | Some('x') | Some('u') => {}
                            Some(')') => return Ok(Node::Empty),
                            Some(':') => break,
                            _ => return Err(self.error("unsupported group flag")),
                        }
                    }
                }
                _ => return Err(self.error("unsupported group construct")),
            }
        }
        let inner = self.parse_alternation()?;
        if self.next() != Some(')') {
            return Err(self.error("missing ')'"));
        }
        Ok(inner)
    }

    /// Parses `{n}`, `{n,}` or `{n,m}` starting at `at` (which holds `{`)
    /// without consuming; returns `(min, max, length)`.
    fn quantifier_bounds_at(&self, at: usize) -> Option<(u32, Option<u32>, usize)> {
        let mut i = at + 1;
        let read_number = |i: &mut usize| -> Option<u32> {
            let start = *i;
            while *i < self.chars.len() && self.chars[*i].is_ascii_digit() {
                *i += 1;
            }
            if *i == start {
                return None;
            }
            self.chars[start..*i]
                .iter()
                .collect::<String>()
                .parse()
                .ok()
        };
        let min = read_number(&mut i)?;
        let max = match self.chars.get(i)? {
            '}' => Some(min),
            ',' => {
                i += 1;
                if self.chars.get(i) == Some(&'}') {
                    None
                } else {
                    let max = read_number(&mut i)?;
                    if self.chars.get(i) != Some(&'}') {
                        return None;
                    }
                    Some(max)
                }
            }
            _ => return None,
        };
        Some((min, max, i + 1 - at))
    }

    fn parse_quantifier(&mut self, atom: Node) -> Result<Node, PatternError> {
        let (min, max) = match self.peek() {
            Some('*') => {
                self.pos += 1;
                (0, None)
            }
            Some('+') => {
                self.pos += 1;
                (1, None)
            }
            Some('?') => {
                self.pos += 1;
                (0, Some(1))
            }
            Some('{') => match self.quantifier_bounds_at(self.pos) {
                Some((min, max, len)) => {
                    if max.is_some_and(|m| m < min) {
                        return Err(self.error("numbers out of order in {} quantifier"));
                    }
                    self.pos += len;
                    (min, max)
                }
                None => return Ok(atom),
            },
            _ => return Ok(atom),
        };
        let greedy = match self.peek() {
            Some('?') => {
                self.pos += 1;
                false
            }
            Some('+') => {
                self.pos += 1;
                true
            }
            _ => true,
        };
        Ok(Node::Repeat {
            node: Box::new(atom),
            min,
            max,
            greedy,
        })
    }

    fn parse_hex(&mut self, digits: usize) -> Result<char, PatternError> {
        let mut value = 0u32;
        for _ in 0..digits {
            let d = self
                .next()
                .and_then(|c| c.to_digit(16))
                .ok_or_else(|| self.error("invalid hexadecimal escape"))?;
            value = value * 16 + d;
        }
        char::from_u32(value).ok_or_else(|| self.error("invalid code point"))
    }

    fn parse_escape(&mut self, in_class: bool) -> Result<Escaped, PatternError> {
        let c = self
            .next()
            .ok_or_else(|| self.error("pattern ends with '\\'"))?;
        Ok(match c {
            'd' => Escaped::Item(ClassItem::Kind(CharKind::Digit, false)),
            'D' => Escaped::Item(ClassItem::Kind(CharKind::Digit, true)),
            'w' => Escaped::Item(ClassItem::Kind(CharKind::Word, false)),
            'W' => Escaped::Item(ClassItem::Kind(CharKind::Word, true)),
            's' => Escaped::Item(ClassItem::Kind(CharKind::Space, false)),
            'S' => Escaped::Item(ClassItem::Kind(CharKind::Space, true)),
            'b' if in_class => Escaped::Char('\u{8}'),
            'b' => Escaped::Node(Node::WordBoundary),
            'B' if !in_class => Escaped::Node(Node::NotWordBoundary),
            'A' if !in_class => Escaped::Node(Node::TextStart),
            'z' if !in_class => Escaped::Node(Node::TextEnd),
            'Z' if !in_class => Escaped::Node(Node::LineEnd),
            'n' => Escaped::Char('\n'),
            't' => Escaped::Char('\t'),
            'r' => Escaped::Char('\r'),
            'f' => Escaped::Char('\u{c}'),
            'v' => Escaped::Char('\u{b}'),
            'e' => Escaped::Char('\u{1b}'),
            'a' => Escaped::Char('\u{7}'),
            '0' => Escaped::Char('\0'),
            'x' => {
                if self.peek() == Some('{') {
                    self.pos += 1;
                    let mut value = 0u32;
                    let mut digits = 0;
                    loop {
                        match self.next() {
                            Some('}') if digits > 0 => break,
                            Some(c) if c.is_ascii_hexdigit() && digits < 8 => {
                                value = value * 16 + c.to_digit(16).expect("hex digit");
                                digits += 1;
                            }
                            _ => return Err(self.error("invalid \\x{...} escape")),
                        }
                    }
                    Escaped::Char(
                        char::from_u32(value).ok_or_else(|| self.error("invalid code point"))?,
                    )
                } else {
                    Escaped::Char(self.parse_hex(2)?)
                }
            }
            'u' => Escaped::Char(self.parse_hex(4)?),
            '1'..='9' => return Err(self.error("back-references are not supported")),
            c if c.is_ascii_alphanumeric() => return Err(self.error("unknown escape sequence")),
            c => Escaped::Char(c),
        })
    }

    fn parse_class(&mut self) -> Result<Node, PatternError> {
        let mut set = ClassSet {
            negated: false,
            items: Vec::new(),
        };
        if self.peek() == Some('^') {
            self.pos += 1;
            set.negated = true;
        }
        let mut first = true;
        loop {
            let c = self
                .next()
                .ok_or_else(|| self.error("missing terminating ']'"))?;
            if c == ']' && !first {
                break;
            }
            first = false;
            let low = match c {
                '\\' => match self.parse_escape(true)? {
                    Escaped::Char(ch) => ch,
                    Escaped::Item(item) => {
                        set.items.push(item);
                        continue;
                    }
                    Escaped::Node(_) => return Err(self.error("invalid escape in class")),
                },
                '[' if self.peek() == Some(':') => {
                    let rest: String = self.chars[self.pos..].iter().collect();
                    let (negated, body) = match rest.strip_prefix(":^") {
                        Some(body) => (true, body),
                        None => (false, &rest[1..]),
                    };
                    let end = body
                        .find(":]")
                        .ok_or_else(|| self.error("unterminated POSIX class"))?;
                    let kind = match &body[..end] {
                        "alpha" => CharKind::Alpha,
                        "digit" => CharKind::Digit,
                        "alnum" => CharKind::Alnum,
                        "space" => CharKind::Space,
                        "upper" => CharKind::Upper,
                        "lower" => CharKind::Lower,
                        "punct" => CharKind::Punct,
                        "xdigit" => CharKind::XDigit,
                        "word" => CharKind::Word,
                        _ => return Err(self.error("unknown POSIX class")),
                    };
                    let prefix_len = if negated { 2 } else { 1 };
                    self.pos += prefix_len + body[..end].chars().count() + 2;
                    set.items.push(ClassItem::Kind(kind, negated));
                    continue;
                }
                c => c,
            };
            if self.peek() == Some('-') && self.peek_at(1).is_some_and(|n| n != ']') {
                self.pos += 1;
                let high = match self.next() {
                    Some('\\') => match self.parse_escape(true)? {
                        Escaped::Char(ch) => ch,
                        _ => return Err(self.error("invalid range in class")),
                    },
                    Some(ch) => ch,
                    None => return Err(self.error("missing terminating ']'")),
                };
                if high < low {
                    return Err(self.error("range out of order in class"));
                }
                set.items.push(ClassItem::Range(low, high));
            } else {
                set.items.push(ClassItem::Range(low, low));
            }
        }
        Ok(Node::Class(set))
    }
}

struct Matcher<'t> {
    text: &'t [char],
    case_insensitive: bool,
}

fn lower(c: char) -> char {
    let mut it = c.to_lowercase();
    match (it.next(), it.next()) {
        (Some(l), None) => l,
        _ => c,
    }
}

fn upper(c: char) -> char {
    let mut it = c.to_uppercase();
    match (it.next(), it.next()) {
        (Some(u), None) => u,
        _ => c,
    }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

impl Matcher<'_> {
    fn chars_equal(&self, a: char, b: char) -> bool {
        a == b || (self.case_insensitive && lower(a) == lower(b))
    }

    fn item_matches(item: &ClassItem, c: char) -> bool {
        match item {
            ClassItem::Range(lo, hi) => *lo <= c && c <= *hi,
            ClassItem::Kind(kind, negated) => kind.matches(c) != *negated,
        }
    }

    fn class_matches(&self, set: &ClassSet, c: char) -> bool {
        let hit = set.items.iter().any(|item| {
            Self::item_matches(item, c)
                || (self.case_insensitive
                    && (Self::item_matches(item, lower(c)) || Self::item_matches(item, upper(c))))
        });
        hit != set.negated
    }

    fn single_matches(&self, node: &Node, c: char) -> bool {
        match node {
            Node::Literal(l) => self.chars_equal(c, *l),
            Node::Any => c != '\n',
            Node::Class(set) => self.class_matches(set, c),
            _ => false,
        }
    }

    fn at_word_boundary(&self, pos: usize) -> bool {
        let before = pos > 0 && is_word(self.text[pos - 1]);
        let after = pos < self.text.len() && is_word(self.text[pos]);
        before != after
    }

    fn match_node(&self, node: &Node, pos: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
        let len = self.text.len();
        match node {
            Node::Empty => k(pos),
            Node::Literal(_) | Node::Any | Node::Class(_) => {
                pos < len && self.single_matches(node, self.text[pos]) && k(pos + 1)
            }
            Node::LineStart | Node::TextStart => pos == 0 && k(pos),
            Node::LineEnd => (pos == len || (pos + 1 == len && self.text[pos] == '\n')) && k(pos),
            Node::TextEnd => pos == len && k(pos),
            Node::WordBoundary => self.at_word_boundary(pos) && k(pos),
            Node::NotWordBoundary => !self.at_word_boundary(pos) && k(pos),
            Node::Concat(nodes) => self.match_sequence(nodes, pos, k),
            Node::Alternate(branches) => {
                for branch in branches {
                    if self.match_node(branch, pos, k) {
                        return true;
                    }
                }
                false
            }
            Node::Repeat {
                node,
                min,
                max,
                greedy,
            } => {
                if node.is_single_char() {
                    self.match_simple_repeat(node, *min, *max, *greedy, pos, k)
                } else {
                    self.match_repeat(node, *min, *max, *greedy, pos, 0, k)
                }
            }
        }
    }

    fn match_sequence(&self, nodes: &[Node], pos: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
        match nodes.split_first() {
            None => k(pos),
            Some((first, rest)) => {
                self.match_node(first, pos, &mut |p| self.match_sequence(rest, p, k))
            }
        }
    }

    fn match_simple_repeat(
        &self,
        node: &Node,
        min: u32,
        max: Option<u32>,
        greedy: bool,
        pos: usize,
        k: &mut dyn FnMut(usize) -> bool,
    ) -> bool {
        let limit = max.map_or(usize::MAX, |m| m as usize);
        let min = min as usize;
        let mut count = 0;
        while count < limit
            && pos + count < self.text.len()
            && self.single_matches(node, self.text[pos + count])
        {
            count += 1;
        }
        if count < min {
            return false;
        }
        if greedy {
            (min..=count).rev().any(|n| k(pos + n))
        } else {
            (min..=count).any(|n| k(pos + n))
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn match_repeat(
        &self,
        node: &Node,
        min: u32,
        max: Option<u32>,
        greedy: bool,
        pos: usize,
        count: u32,
        k: &mut dyn FnMut(usize) -> bool,
    ) -> bool {
        let can_repeat = max.is_none_or(|m| count < m);
        let try_more = |k: &mut dyn FnMut(usize) -> bool| -> bool {
            can_repeat
                && self.match_node(node, pos, &mut |p| {
                    // Zero-width iterations past the minimum cannot make progress.
                    if p == pos && count >= min {
                        return false;
                    }
                    self.match_repeat(node, min, max, greedy, p, count + 1, k)
                })
        };
        if greedy {
            try_more(k) || (count >= min && k(pos))
        } else {
            (count >= min && k(pos)) || try_more(k)
        }
    }
}
