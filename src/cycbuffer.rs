use std::io::{Read,Write,BufRead};
use tokio::io::{AsyncRead,AsyncWrite};
use tokio::prelude::Async::{self,Ready};
use std::sync::{Arc, Mutex};
use std::ops::DerefMut;

struct CycBuffer{
    buf: [u8; 32768], 
    read_pos: usize,
    write_pos: usize
}
#[derive(Clone)]
pub struct SyncCycBuffer(Arc<Mutex<CycBuffer>>);
impl SyncCycBuffer {
    pub fn new() -> Self {
        SyncCycBuffer(Arc::new(Mutex::new(CycBuffer{buf: [0; 32768], read_pos: 0, write_pos: 0})))
    }
    fn get_mut<'a>(&'a mut self) -> Result<impl DerefMut<Target=CycBuffer> + 'a, std::io::Error> {
        self.0.lock()
            .map_err(|e| { error!("{}", e); std::io::ErrorKind::Interrupted.into() } )
    }
}
impl Read for SyncCycBuffer
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize,std::io::Error> {
        let mut ss = self.get_mut()?;
        let s = &mut ss;
        let sbuf = s.buf;
        if s.read_pos<s.write_pos {
            // There are already some data to read,
            // and the data in the beginning are vacant
            let mut max = s.write_pos - s.read_pos;
            if max>buf.len() { max = buf.len() }
            buf[0..max].copy_from_slice(&sbuf[s.read_pos..s.read_pos + max]);
            s.read_pos += max;
            Ok(max)
        } else if s.read_pos==s.write_pos {
            //The buffer is empty and ther is no data to read. 
            // Read next time!
            Err(std::io::ErrorKind::WouldBlock.into())
        } else {
            // There are data to read and 0..write_pos range are good data
            let first = sbuf.len() - s.read_pos;
            if first>buf.len() {
                //The requested data size not exceeding the first part, 
                // only need to read once
                buf.copy_from_slice(&sbuf[s.read_pos..s.read_pos + buf.len()]);
                s.read_pos += buf.len();
                Ok(buf.len())
            } else {
                //The request data size exceeding the first part, need to turn to the
                // begining
                buf[0..first].copy_from_slice(&sbuf[s.read_pos..s.read_pos + first]);
                let mut second = s.write_pos;
                if second>buf.len()-first { second = buf.len()-first }
                buf[first..first+second].copy_from_slice(&sbuf[0..second]);
                s.read_pos = second;
                Ok(first + second)
            }
        }   
    }
}
impl AsyncRead for SyncCycBuffer {}
impl Write for SyncCycBuffer
{
    fn write(&mut self, buf:&[u8]) -> Result<usize, std::io::Error> {
        let mut ss = self.get_mut()?;
        let s = &mut ss;
        let mut sbuf = s.buf;
        if s.read_pos>s.write_pos+1 {
            //The buffer is not full, and the vacant data range is
            // write_pos..read_pos-1
            let mut max = s.write_pos - s.read_pos;
            if buf.len()<max { max=buf.len() }
            sbuf[s.write_pos..s.write_pos+max].copy_from_slice(&buf[0..max]);
            s.write_pos += max;
            Ok(max)
        } else if s.read_pos==s.write_pos+1  {
            //The buffer is full
            Err(std::io::ErrorKind::WouldBlock.into())
        } else if s.read_pos==0 && s.write_pos==sbuf.len()-1 {
            //special case that the buffer is full
            Err(std::io::ErrorKind::WouldBlock.into())
        } else {
            //The buffer is not full, and the vacent data range is
            // write_pos.. and ..read_pos-1
            let first = sbuf.len() - s.write_pos;
            if first>buf.len() {
                //If the first piece is enough, that's it.
                sbuf[s.write_pos..s.write_pos + buf.len()].copy_from_slice(&buf);
                s.write_pos += buf.len();
                Ok(buf.len())
            } else {
                //If not, write two times.
                sbuf[s.write_pos..].copy_from_slice(&buf[0..first]);
                let mut second = buf.len()-first;
                if second > s.read_pos-1 { second = s.read_pos-1 }
                sbuf[..second].copy_from_slice(&buf[first..first+second]);
                s.write_pos = second;
                Ok(first+second)
            }
        }
    }
    fn flush(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }
}
impl AsyncWrite for SyncCycBuffer
{
    fn shutdown(&mut self) -> Result<Async<()>, std::io::Error> {
        Ok(Ready(()))
    }
}


#[cfg(test)]
mod tests {

    use crate::cycbuffer::SyncCycBuffer;
    use std::io::{Read,Write};
    /*#[test]
    fn test_cycbuffer(){
        env_logger::init();

        let mut r = [0; 10];
        let mut buf=SyncCycBuffer::new();
        buf.write(&[1,2,3,4]).unwrap();
        let mut v = [0; 9];
        buf.read(&mut v[0..1]).unwrap();
        assert_eq!(v[0], 1);
        buf.read(&mut v[0..2]).unwrap();
        assert_eq!(v[0], 2);
        assert_eq!(v[1], 3);

        let c = buf.read(&mut v).unwrap();
        assert_eq!(c,1);
        assert_eq!(v[0], 4);
       
        if let Ok(_) = buf.read(&mut v) {
            panic!("expect error");
        }

        let c = buf.write(&[5; 20]).unwrap();
        assert_eq!(c,9);
        if let Ok(_) = buf.write(&[]) {
            panic!("expect error");
        }

        let c = buf.read(&mut v).unwrap();
        assert_eq!(c,9);
        for i in v.iter() {
            assert_eq!(*i, 5);
        }
    }*/
    #[test]
    fn test_httparse(){
        env_logger::init();        
        use httparse::Request;
        let mut req = Request::new(&mut []);
        req.parse(b"GET / HTTP/1.1\r\n");
        info!("{:?}", req);
    }

    use crate::cycbuffer::LineParser;
    #[test]
    fn test_line_parser(){
        //env_logger::init();

        let mut lp = LineParser::new();
        if let Ok(_) = lp.parse_line(b"123") {
            panic!("expect fail");
        }
        let (l, c) = lp.parse_line(b"4\r").unwrap();
        assert_eq!(c, 2);
        assert_eq!(l, b"1234");            

        let v = b"1234\r\n4567\n5678";
        let (l,c) = lp.parse_line(v).unwrap();
        assert_eq!(c, 6);
        assert_eq!(l, b"1234");

        let (l,c) = lp.parse_line(&v[c..]).unwrap();
        assert_eq!(c,5);
        assert_eq!(l, b"4567");

        let (l,c) = lp.parse_line(&v[0..5]).unwrap();
        assert_eq!(c, 5);
        assert_eq!(l, b"1234");

        let (l,d) = lp.parse_line(&v[c..]).unwrap();
        assert_eq!(d,6);
        assert_eq!(l, b"4567");

        if let Err(_) = lp.parse_line(&v[c+d..]) {
            let d:Vec<u8> = lp.into();
            assert_eq!(&d, b"5678");
        } else {
            panic!("expect err")
        }
    }
}