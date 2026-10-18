//! A small XML reader, just enough for labwc's rc.xml: elements with
//! attributes, text, comments, processing instructions, CDATA and the
//! five predefined entities plus numeric character references. No DTDs,
//! no namespaces (labwc has none). A broken file gives an error with the
//! line number, never a panic.

use std::fmt;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<Element>,
    /// The element's own text (trimmed, children's text not included).
    pub text: String,
}

impl Element {
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// Children with this name (labwc matches element names without case).
    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> {
        self.children
            .iter()
            .filter(move |child| child.name.eq_ignore_ascii_case(name))
    }

    #[cfg(test)]
    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children
            .iter()
            .find(|child| child.name.eq_ignore_ascii_case(name))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Error {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Zeile {}: {}", self.line, self.message)
    }
}

struct Reader<'a> {
    text: &'a str,
    pos: usize,
}

impl<'a> Reader<'a> {
    fn error(&self, message: impl Into<String>) -> Error {
        let line = self.text[..self.pos.min(self.text.len())]
            .matches('\n')
            .count()
            + 1;
        Error {
            line,
            message: message.into(),
        }
    }

    fn rest(&self) -> &'a str {
        &self.text[self.pos..]
    }

    fn skip_until(&mut self, end: &str, what: &str) -> Result<&'a str, Error> {
        match self.rest().find(end) {
            Some(i) => {
                let inner = &self.rest()[..i];
                self.pos += i + end.len();
                Ok(inner)
            }
            None => Err(self.error(format!("{what} nicht abgeschlossen"))),
        }
    }

    fn skip_space(&mut self) {
        let rest = self.rest();
        self.pos += rest.len() - rest.trim_start().len();
    }

    fn name(&mut self) -> Result<String, Error> {
        let rest = self.rest();
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '/' | '>' | '=' | '<'))
            .unwrap_or(rest.len());
        if end == 0 {
            return Err(self.error("Name erwartet"));
        }
        self.pos += end;
        Ok(rest[..end].to_owned())
    }

    /// Skips comments, processing instructions and the doctype.
    fn skip_misc(&mut self) -> Result<bool, Error> {
        let rest = self.rest();
        if rest.starts_with("<!--") {
            self.pos += 4;
            self.skip_until("-->", "Kommentar")?;
        } else if rest.starts_with("<?") {
            self.pos += 2;
            self.skip_until("?>", "Verarbeitungsanweisung")?;
        } else if rest.starts_with("<!DOCTYPE") || rest.starts_with("<!doctype") {
            self.skip_until(">", "DOCTYPE")?;
        } else {
            return Ok(false);
        }
        Ok(true)
    }

    fn element(&mut self) -> Result<Element, Error> {
        debug_assert!(self.rest().starts_with('<'));
        self.pos += 1;
        let mut element = Element {
            name: self.name()?,
            ..Element::default()
        };
        loop {
            self.skip_space();
            let rest = self.rest();
            if rest.starts_with("/>") {
                self.pos += 2;
                return Ok(element);
            }
            if rest.starts_with('>') {
                self.pos += 1;
                break;
            }
            if rest.is_empty() {
                return Err(self.error(format!("<{}> nicht abgeschlossen", element.name)));
            }
            let key = self.name()?;
            self.skip_space();
            if !self.rest().starts_with('=') {
                return Err(self.error(format!("„=“ nach Attribut {key} erwartet")));
            }
            self.pos += 1;
            self.skip_space();
            let quote = self.rest().chars().next();
            let Some(quote @ ('"' | '\'')) = quote else {
                return Err(self.error(format!("Wert von {key} ohne Anführungszeichen")));
            };
            self.pos += 1;
            let raw = self.skip_until(&quote.to_string(), "Attributwert")?;
            let value = self.decode(raw)?;
            element.attributes.push((key, value));
        }
        // content
        let mut text = String::new();
        loop {
            let rest = self.rest();
            if rest.is_empty() {
                return Err(self.error(format!("</{}> fehlt", element.name)));
            }
            if rest.starts_with("</") {
                self.pos += 2;
                let name = self.name()?;
                self.skip_space();
                if !self.rest().starts_with('>') {
                    return Err(self.error("„>“ erwartet"));
                }
                self.pos += 1;
                if name != element.name {
                    return Err(self.error(format!("</{name}> passt nicht zu <{}>", element.name)));
                }
                element.text = text.trim().to_owned();
                return Ok(element);
            }
            if rest.starts_with("<![CDATA[") {
                self.pos += 9;
                text.push_str(self.skip_until("]]>", "CDATA")?);
                continue;
            }
            if self.skip_misc()? {
                continue;
            }
            if rest.starts_with('<') {
                element.children.push(self.element()?);
                continue;
            }
            let end = rest.find('<').unwrap_or(rest.len());
            let raw = &rest[..end];
            self.pos += end;
            text.push_str(&self.decode(raw)?);
        }
    }

    fn decode(&self, raw: &str) -> Result<String, Error> {
        let mut out = String::with_capacity(raw.len());
        let mut rest = raw;
        while let Some(i) = rest.find('&') {
            out.push_str(&rest[..i]);
            rest = &rest[i + 1..];
            let Some(end) = rest.find(';') else {
                return Err(self.error("„&“ ohne „;“"));
            };
            let entity = &rest[..end];
            rest = &rest[end + 1..];
            let c = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                    u32::from_str_radix(&entity[2..], 16)
                        .ok()
                        .and_then(char::from_u32)
                }
                _ if entity.starts_with('#') => {
                    entity[1..].parse::<u32>().ok().and_then(char::from_u32)
                }
                _ => None,
            };
            match c {
                Some(c) => out.push(c),
                None => return Err(self.error(format!("unbekannte Entität &{entity};"))),
            }
        }
        out.push_str(rest);
        Ok(out)
    }
}

/// Reads a whole document; returns its root element.
pub fn parse(text: &str) -> Result<Element, Error> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut reader = Reader { text, pos: 0 };
    loop {
        reader.skip_space();
        if !reader.skip_misc()? {
            break;
        }
    }
    if !reader.rest().starts_with('<') {
        return Err(reader.error("kein Wurzelelement"));
    }
    let root = reader.element()?;
    loop {
        reader.skip_space();
        if !reader.skip_misc()? {
            break;
        }
    }
    if !reader.rest().is_empty() {
        return Err(reader.error("Text nach dem Wurzelelement"));
    }
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elements_attributes_and_text() {
        let root = parse(
            "<?xml version=\"1.0\"?>\n<!-- c <x/> -->\n<a x='1' y=\"z &amp; &#228;\">\n <b>hi &lt;3</b><c/>\n</a>\n",
        )
        .unwrap();
        assert_eq!(root.name, "a");
        assert_eq!(root.attribute("x"), Some("1"));
        assert_eq!(root.attribute("Y"), Some("z & ä"));
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.child("b").unwrap().text, "hi <3");
        assert!(root.child("c").unwrap().children.is_empty());
    }

    #[test]
    fn comments_inside_and_cdata() {
        let root = parse("<a><!-- <b/> --><![CDATA[<x>]]></a>").unwrap();
        assert!(root.children.is_empty());
        assert_eq!(root.text, "<x>");
    }

    #[test]
    fn errors_have_lines() {
        let err = parse("<a>\n<b>\n</a>").unwrap_err();
        assert_eq!(err.line, 3);
        assert!(err.message.contains("passt nicht"));
        assert!(parse("<a x=1/>").is_err());
        assert!(parse("<a>&foo;</a>").is_err());
        assert!(parse("<a/><b/>").is_err());
        assert!(parse("").is_err());
        assert!(parse("<a").is_err());
    }
}
