pub struct LineParser {
    buf: Vec<u8>,
    saw_return: bool
}
impl From<LineParser> for Vec<u8> {
    fn from(lp: LineParser) -> Self {
        lp.buf
    }
}
impl LineParser {
    pub fn new() -> Self {
        LineParser { buf: vec![], saw_return: false }
    }
    pub fn parse_line_http<'a>(&'a mut self, data:&[u8]) -> Option<(Vec<u8>,usize)>
    {
        if self.saw_return && data.len()>1 && data[0]==b'\n' { 
            let mut r = std::mem::replace(&mut self.buf, vec![]);
            r.remove(r.len()-1);
            return Some((r, 1));
        }
        let mut pos=None;
        for i in 0..data.len() {
            if data[i]==b'\r' {
                if i+1<data.len() && data[i+1]==b'\n' {
                    pos = Some(i+2);
                    break;
                } else {
                    self.saw_return=true;
                }            
            } else { self.saw_return=false; }
        }        
        if let Some(pos) = pos {
            let mut r = std::mem::replace(&mut self.buf, vec![]);
            r.extend(&data[0..pos-2]);
            Some((r, pos))
        } else {
            self.buf.extend(data);
            None
        }
    }
}

#[cfg(test)]
mod test {
    use crate::http_parse::line_parser::LineParser;
    #[test]
    fn test_line_parser() {
        env_logger::init(); 
        let mut lp = LineParser::new();
        if let Some(_) = lp.parse_line_http(b"123\r") {
            panic!("expect None");
        }
        if let Some(_) = lp.parse_line_http(b"456\r") {
            panic!("expect None");
        }
        if let Some((line,p)) = lp.parse_line_http(b"\n789"){
            assert_eq!(line, b"123\r456");
            assert_eq!(p, 1);
        } else {
            panic!("expect some");
        }
        if let Some((line,p)) = lp.parse_line_http(b"123456\r\n789") {
            assert_eq!(line, b"123456");
            assert_eq!(p, 8);
        }
    }
}