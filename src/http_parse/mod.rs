pub mod line_parser;

use httparse::{Request, Status, EMPTY_HEADER, Header};
use line_parser::LineParser;
use failure::Error;
use std::iter::repeat;
use hyper::{Request as HyperRequest, Version};

pub struct HttpHeaderParser {
    data: Vec<u8>,
    lines_cnt: usize,
    line_parser:LineParser,
}
impl HttpHeaderParser {
    pub fn new() -> Self {
        Self {
            data: vec![],
            lines_cnt: 0,
            line_parser: LineParser::new(),
        }
    }
    pub fn parse_http<'a>(&'a mut self, data:&[u8]) -> Result<Option<(HyperRequest<()>,usize)>, Error> {
        let mut start = 0;
        loop {
            let r = self.line_parser.parse_line_http(&data[start..]);
            if let Some((l,p)) = r {
                self.data.extend(&data[start..start+p]);
                if l.len()!=0 { 
                    self.lines_cnt += 1;
                    start += p;
                    continue;
                }
                let headers_cnt = if self.lines_cnt>0 { self.lines_cnt-1 } else { 0 };
                let mut headers = vec![EMPTY_HEADER; headers_cnt];
                let mut req = Request::new(&mut headers);
                return match req.parse(&self.data) {
                    Ok(Status::Complete(n)) => {
                        let mut hreq = HyperRequest::builder();
                        hreq.version(
                                match req.version {
                                    None => Version::HTTP_09,
                                    Some(0) => Version::HTTP_10,
                                    Some(1) => Version::HTTP_11,
                                    _ => bail!("Unsupported version")
                                }
                            )
                            .method(req.method.ok_or_else(||format_err!("method is empty"))?)
                            .uri(req.path.ok_or_else(||format_err!("path is empty"))?);
                        for header in req.headers {
                            hreq.header(header.name, header.value);
                        }
                        let r = hreq.body(());
                        Ok(Some((r?, n)))
                    },
                    Ok(Status::Partial) => Ok(None),
                    Err(e) => {
                        info!("err httparse");
                        Err(e.into())
                    }
                }
            } else { return Ok(None) }
        }
    }    
}

#[cfg(test)]
mod test {
    use httparse::Status;
    #[test]
    fn test_httparse() {       
        use httparse::Request;
        let mut req = Request::new(&mut []);
        let v = req.parse(b"GET / HTTP/1.1\r\nContent-Type: text/html\r\n\r\n");
        match v {
            Ok(Status::Complete(n)) => {
                assert_eq!(n, 18);
                assert_eq!(req.method, Some("GET"));
                assert_eq!(req.path, Some("/"));
                assert_eq!(req.version, Some(1));
            },
            Ok(Status::Partial) => {
                assert_eq!(req.method, Some("GET"));
                assert_eq!(req.path, Some("/"));
                assert_eq!(req.version, Some(1));
            },
            Err(e) => {
                error!("{:?}", e);
            }
        }
        info!("{:?}", req);
    }
    #[test]
    fn test_h(){
    }
}